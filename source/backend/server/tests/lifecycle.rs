//! Real server lifecycle tests; native Codex cases are explicitly enabled by the runner.
use futures_util::StreamExt;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use vha_codex_agent::{
    AgentConfig, CodexConfig, LoadedAgentConfig, ModelConfig, ProviderConfig, ProviderWireApi,
};
use vha_server::http_server;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("vha-lifecycle-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn config(root: &Scratch, exe: PathBuf, port: u16) -> LoadedAgentConfig {
    let agent = AgentConfig {
        active_model_id: "lifecycle-fixture".into(),
        providers: vec![ProviderConfig {
            id: "lifecycle-provider".into(),
            base_url: "http://127.0.0.1:1/v1".into(),
            api_key: "NOT_A_REAL_PROVIDER_KEY".into(),
            wire_api: ProviderWireApi::ChatCompletions,
            models: vec![ModelConfig {
                model_id: "lifecycle-fixture".into(),
                model_name: "upstream-lifecycle-fixture".into(),
                effort: "low".into(),
                supports_images: false,
                ..Default::default()
            }],
        }],
        codex: CodexConfig {
            executable: Some(exe),
            workspace: root.0.join("data/workspace"),
            home: root.0.join("data/codex"),
            port,
            ..Default::default()
        },
    };
    LoadedAgentConfig {
        config: agent.resolve().unwrap(),
        created: false,
        migrated: false,
        recovered_from: None,
    }
}
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
async fn status(url: &str) -> Value {
    reqwest::get(format!("{url}/api/agent/status"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn phase(url: &str, expected: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(25), async {
        loop {
            let state = status(url).await;
            if state["phase"] == expected {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn configuration_failure_keeps_http_and_live_websocket_shutdown_is_bounded() {
    let server = http_server::start_with_codex(
        "127.0.0.1:0".parse().unwrap(),
        std::path::Path::new("."),
        Err("fixture setup is missing".into()),
    )
    .unwrap();
    let url = format!("http://{}", server.address());
    assert_eq!(
        phase(&url, "error").await["message"],
        "fixture setup is missing"
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.address()))
        .await
        .unwrap();
    assert!(ws.next().await.unwrap().is_ok());
    let started = std::time::Instant::now();
    tokio::time::timeout(
        Duration::from_secs(6),
        tokio::task::spawn_blocking(move || server.stop()),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert!(started.elapsed() < Duration::from_secs(6));
    assert!(reqwest::get(format!("{url}/api/health")).await.is_err());
}

#[tokio::test]
async fn occupied_codex_port_is_not_connected_to_or_killed() {
    let root = Scratch::new();
    let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = occupied.local_addr().unwrap().port();
    let settings = config(&root, std::env::current_exe().unwrap(), port);
    let server =
        http_server::start_with_codex("127.0.0.1:0".parse().unwrap(), &root.0, Ok(settings))
            .unwrap();
    let state = phase(&format!("http://{}", server.address()), "error").await;
    assert!(state["message"].as_str().unwrap().contains("端口已被占用"));
    assert!(state["pid"].is_null());
    server.stop().unwrap();
    assert!(std::net::TcpStream::connect(occupied.local_addr().unwrap()).is_ok());
}

#[tokio::test]
async fn early_child_exit_is_reported_without_closing_the_application() {
    let root = Scratch::new();
    let settings = config(&root, std::env::current_exe().unwrap(), free_port());
    let server =
        http_server::start_with_codex("127.0.0.1:0".parse().unwrap(), &root.0, Ok(settings))
            .unwrap();
    let url = format!("http://{}", server.address());
    let state = phase(&url, "error").await;
    assert!(state["message"].as_str().unwrap().contains("退出"));
    assert!(state["pid"].is_null());
    assert_eq!(
        reqwest::get(format!("{url}/api/health"))
            .await
            .unwrap()
            .status(),
        200
    );
    server.stop().unwrap();
}

#[tokio::test]
#[ignore = "requires VHA_TEST_CODEX_NATIVE; no external model request is made"]
async fn native_startup_and_normal_shutdown_reap_codex_with_browser_connected() {
    let root = Scratch::new();
    let exe =
        PathBuf::from(std::env::var_os("VHA_TEST_CODEX_NATIVE").expect("native Codex executable"));
    let settings = config(&root, exe, free_port());
    let server =
        http_server::start_with_codex("127.0.0.1:0".parse().unwrap(), &root.0, Ok(settings))
            .unwrap();
    let url = format!("http://{}", server.address());
    let state = phase(&url, "ready").await;
    let pid = state["pid"].as_u64().unwrap();
    assert!(pid > 0);
    let (_ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.address()))
        .await
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(12),
        tokio::task::spawn_blocking(move || server.stop()),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    #[cfg(target_os = "linux")]
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    assert!(reqwest::get(format!("{url}/api/health")).await.is_err());
}

#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "requires VHA_TEST_CODEX_NATIVE; kills only the PID returned by this isolated host"]
async fn unexpected_native_child_death_keeps_http_available_and_reports_failure() {
    let root = Scratch::new();
    let exe =
        PathBuf::from(std::env::var_os("VHA_TEST_CODEX_NATIVE").expect("native Codex executable"));
    let settings = config(&root, exe, free_port());
    let server =
        http_server::start_with_codex("127.0.0.1:0".parse().unwrap(), &root.0, Ok(settings))
            .unwrap();
    let url = format!("http://{}", server.address());
    let before = phase(&url, "ready").await;
    let pid = before["pid"].as_u64().unwrap();
    let metadata = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    assert!(metadata.lines().any(|line| line
        .strip_prefix("PPid:")
        .is_some_and(|n| n.trim() == std::process::id().to_string())));
    assert!(std::process::Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .unwrap()
        .success());
    let after = phase(&url, "error").await;
    assert!(after["pid"].is_null());
    assert_eq!(after["activeTurns"], serde_json::json!({}));
    assert_eq!(
        reqwest::get(format!("{url}/api/health"))
            .await
            .unwrap()
            .status(),
        200
    );
    server.stop().unwrap();
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
}

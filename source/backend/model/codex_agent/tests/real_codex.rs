//! Explicitly-run native protocol smoke test. No remote API or real credentials are used.
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use vha_codex_agent::{CodexAgent, CodexProcess, ConnectionPhase, ProcessConfig, TransportConfig};

struct TempHome(PathBuf);
impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
#[ignore = "requires VHA_TEST_CODEX_NATIVE; run explicitly against installed Codex"]
async fn native_codex_handshake_disconnect_reconnect_and_owned_shutdown() {
    let executable = PathBuf::from(
        std::env::var_os("VHA_TEST_CODEX_NATIVE").expect("set native Codex executable"),
    );
    let path = std::env::temp_dir().join(format!(
        "vha-native-protocol-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    let home = TempHome(path);
    std::fs::write(
        home.0.join("config.toml"),
        r#"
model = "local-fixture-model"
model_provider = "fixture"
[model_providers.fixture]
name = "Local protocol test (no external API)"
base_url = "http://127.0.0.1:1/v1"
wire_api = "responses"
env_key = "VHA_TEST_PROVIDER_TOKEN"
supports_websockets = false
"#,
    )
    .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let endpoint = format!("ws://{addr}");
    let mut child = CodexProcess::spawn(&ProcessConfig {
        executable,
        args: vec![
            "app-server".into(),
            "--listen".into(),
            endpoint.clone().into(),
        ],
        cwd: Some(home.0.clone()),
        env: vec![
            ("CODEX_HOME".into(), home.0.as_os_str().into()),
            (
                "VHA_TEST_PROVIDER_TOKEN".into(),
                "local-fixture-not-a-real-token".into(),
            ),
        ],
    })
    .await
    .unwrap();
    let pid = child.pid().unwrap();
    let agent = CodexAgent::new(TransportConfig {
        websocket_url: endpoint,
        connect_timeout: Duration::from_secs(2),
        reconnect_interval: Duration::from_millis(100),
        max_reconnect_attempts: 40,
        ..Default::default()
    });
    let result = tokio::time::timeout(Duration::from_secs(15), agent.connect()).await;
    assert!(
        matches!(result, Ok(Ok(()))),
        "native Codex connection failed; diagnostics={:?}",
        child.diagnostics()
    );
    assert_eq!(agent.status().phase, ConnectionPhase::Ready);
    let first = agent.status().connection_id;
    let _models = agent
        .list_models(None)
        .await
        .expect("native model/list schema must decode");
    agent.disconnect().await.unwrap();
    assert!(
        child.try_wait().unwrap().is_none(),
        "transport disconnect must not terminate Codex"
    );
    assert_eq!(child.pid(), Some(pid));
    agent.connect().await.unwrap();
    assert!(agent.status().connection_id > first);
    agent.disconnect().await.unwrap();
    child.shutdown(Duration::from_secs(2)).await.unwrap();
    assert!(child.try_wait().unwrap().is_some());
    assert!(
        tokio::net::TcpStream::connect(addr).await.is_err(),
        "listener must close when its process ends"
    );
}

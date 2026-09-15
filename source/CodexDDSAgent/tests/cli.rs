use std::{
    net::TcpListener,
    path::Path,
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use vha_codex_dds_agent::registry::{RegistryStore, ServiceRegistryState, StoragePaths};
use zenoh::Config;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_manages_service_configuration_and_agents_end_to_end() {
    let home = tempfile::tempdir().unwrap();
    let paths = StoragePaths::new(home.path());
    install_mock_codex(&paths);
    let service_name = "cli-e2e";
    let first_port = free_port();
    let second_port = free_port();

    let mut service = spawn_cli(
        home.path(),
        [
            "service",
            "create",
            "--service-name",
            service_name,
            "--agent-name",
            "initial-agent",
            "--port-mode",
            "fixed",
            "--port",
            &first_port.to_string(),
            "--disable-discovery",
        ],
    );
    wait_for_service_state(home.path(), service_name, ServiceRegistryState::Running);
    let client = open_client(first_port).await;

    let status = query_json(
        client.clone(),
        &format!("codex-dds/v1/{service_name}/status/get"),
        json!({"version": 1}),
    )
    .await;
    assert_eq!(status["state"], "running");
    assert_eq!(status["port"], first_port);

    let config = cli_json(
        home.path(),
        ["config", "get", "--service-name", service_name],
    );
    assert_eq!(config["port_mode"], "fixed");
    assert_eq!(config["port"], first_port);
    assert_eq!(config["discovery_enabled"], false);

    let config = cli_json(
        home.path(),
        [
            "config",
            "set",
            "--service-name",
            service_name,
            &format!("port={second_port}"),
        ],
    );
    assert_eq!(config["port"], second_port);

    let services = cli_json(home.path(), ["service", "list"]);
    assert_eq!(services[0]["service_name"], service_name);
    assert_eq!(services[0]["state"], "running");

    let created = query_json(
        client.clone(),
        &format!("codex-dds/v1/{service_name}/agent/desktop-a/create"),
        model_payload(),
    )
    .await;
    assert_eq!(created["state"], "running", "{created}");

    let agents = cli_json(
        home.path(),
        ["agent", "list", "--service-name", service_name],
    );
    assert!(agents
        .as_array()
        .unwrap()
        .iter()
        .any(|agent| agent["agent_name"] == "desktop-a"));

    let blocked = cli_output(
        home.path(),
        [
            "agent",
            "delete",
            "--service-name",
            service_name,
            "--agent-name",
            "desktop-a",
        ],
    );
    assert!(!blocked.status.success());
    assert!(
        String::from_utf8_lossy(&blocked.stderr).contains("agent is active"),
        "blocked delete output: {}{}",
        String::from_utf8_lossy(&blocked.stdout),
        String::from_utf8_lossy(&blocked.stderr)
    );

    let deleted = cli_json(
        home.path(),
        [
            "agent",
            "delete",
            "--force",
            "--service-name",
            service_name,
            "--agent-name",
            "desktop-a",
        ],
    );
    assert_eq!(deleted["deleted"], true);
    assert!(!paths.codex_home("desktop-a").exists());
    assert!(!paths.agent_log("desktop-a").exists());

    let deleted = cli_json(
        home.path(),
        [
            "agent",
            "delete",
            "--service-name",
            service_name,
            "--agent-name",
            "initial-agent",
        ],
    );
    assert_eq!(deleted["deleted"], true);
    assert!(!paths.codex_home("initial-agent").exists());

    client.close().await.unwrap();
    let shutdown = cli_json(
        home.path(),
        ["service", "shutdown", "--service-name", service_name],
    );
    assert_eq!(shutdown["state"], "stopped");
    wait_for_service_exit(&mut service);
    wait_for_service_state(home.path(), service_name, ServiceRegistryState::Stopped);

    let mut service = spawn_cli(
        home.path(),
        ["service", "start", "--service-name", service_name],
    );
    wait_for_service_state(home.path(), service_name, ServiceRegistryState::Running);
    let client = open_client(second_port).await;
    let status = query_json(
        client.clone(),
        &format!("codex-dds/v1/{service_name}/status/get"),
        json!({"version": 1}),
    )
    .await;
    assert_eq!(status["port"], second_port);
    client.close().await.unwrap();

    let shutdown = cli_json(
        home.path(),
        ["service", "shutdown", "--service-name", service_name],
    );
    assert_eq!(shutdown["state"], "stopped");
    wait_for_service_exit(&mut service);
}

fn install_mock_codex(paths: &StoragePaths) {
    paths.prepare().unwrap();
    let mock = env!("CARGO_BIN_EXE_mock-codex-server");
    std::fs::write(
        &paths.agent_binary,
        format!("#!/bin/sh\nexec '{}' \"$@\"\n", mock.replace('\'', "'\\''")),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&paths.agent_binary, std::fs::Permissions::from_mode(0o755))
            .unwrap();
    }
}

fn spawn_cli<I, S>(home: &Path, args: I) -> Child
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    Command::new(env!("CARGO_BIN_EXE_vha-codex-dds-agent"))
        .args(args.into_iter().map(Into::into).collect::<Vec<_>>())
        .env("CODEX_DDS_AGENT_HOME", home)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn CLI service process")
}

fn cli_json<I, S>(home: &Path, args: I) -> Value
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let output = cli_output(home, args);
    assert!(
        output.status.success(),
        "CLI failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI output is JSON")
}

fn cli_output<I, S>(home: &Path, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    Command::new(env!("CARGO_BIN_EXE_vha-codex-dds-agent"))
        .args(args.into_iter().map(Into::into).collect::<Vec<_>>())
        .env("CODEX_DDS_AGENT_HOME", home)
        .output()
        .expect("run CLI command")
}

fn wait_for_service_state(home: &Path, service_name: &str, expected: ServiceRegistryState) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let store = RegistryStore::open(StoragePaths::new(home)).unwrap();
        if let Some(service) = store.get_service(service_name).unwrap() {
            if service.state == expected {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "service did not become {expected:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_service_exit(service: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = service.try_wait().unwrap() {
            assert!(status.success(), "service process failed: {status}");
            return;
        }
        assert!(Instant::now() < deadline, "service process did not exit");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn open_client(port: u16) -> zenoh::Session {
    let mut config = Config::default();
    config.insert_json5("mode", "\"client\"").unwrap();
    config
        .insert_json5("connect/endpoints", &format!("[\"tcp/127.0.0.1:{port}\"]"))
        .unwrap();
    config
        .insert_json5("scouting/multicast/enabled", "false")
        .unwrap();
    zenoh::open(config).await.unwrap()
}

async fn query_json(session: zenoh::Session, key: &str, payload: Value) -> Value {
    let replies = session
        .get(key)
        .timeout(Duration::from_secs(5))
        .payload(payload.to_string())
        .await
        .unwrap();
    while let Ok(reply) = replies.recv_async().await {
        let payload = match reply.result() {
            Ok(sample) => sample.payload().try_to_string().unwrap(),
            Err(error) => error.payload().try_to_string().unwrap(),
        };
        if let Ok(value) = serde_json::from_str::<Value>(&payload) {
            return value;
        }
    }
    panic!("query {key} did not return JSON")
}

fn model_payload() -> Value {
    json!({
        "version": 1,
        "model": {
            "default_provider": "main",
            "providers": [{
                "id": "main",
                "base_url": "https://api.example.com/v1",
                "api_key": "test-key",
                "default_model": "model-a",
                "models": ["model-a"]
            }]
        }
    })
}

use std::{
    net::TcpListener,
    sync::Arc,
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use vha_codex_dds_agent::{
    registry::{
        AgentRegistryState, PortMode, RegistryStore, ServiceConfig, ServiceRegistryState,
        StoragePaths,
    },
    service::ServiceRuntime,
};
use zenoh::Config;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn local_shutdown_file_stops_service_and_agents() {
    let mut fixture = ControlFixture::new("service-control-shutdown").await;
    let client = open_client(fixture.port).await;
    let created = query_json(
        client.clone(),
        "codex-dds/v1/service-control-shutdown/agent/desktop-a/create",
        model_payload(),
    )
    .await;
    assert_eq!(created["state"], "running");

    let service = fixture.take();
    let runtime = tokio::spawn(service.run_until_shutdown());
    write_atomic(
        fixture
            .paths
            .service_shutdown_file("service-control-shutdown"),
        b"",
    );

    let result = tokio::time::timeout(Duration::from_secs(5), runtime)
        .await
        .unwrap()
        .unwrap();
    result.unwrap();

    let store = RegistryStore::open(StoragePaths::new(fixture.root.path())).unwrap();
    let service = store
        .get_service("service-control-shutdown")
        .unwrap()
        .unwrap();
    let agent = store
        .get_agent("service-control-shutdown", "desktop-a")
        .unwrap()
        .unwrap();
    assert_eq!(service.state, ServiceRegistryState::Stopped);
    assert_eq!(agent.state, AgentRegistryState::Stopped);
    assert!(!fixture
        .paths
        .service_shutdown_file("service-control-shutdown")
        .exists());
    client.close().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn local_agent_delete_file_stops_runtime_and_removes_data() {
    let mut fixture = ControlFixture::new("service-control-delete").await;
    let client = open_client(fixture.port).await;
    let created = query_json(
        client.clone(),
        "codex-dds/v1/service-control-delete/agent/desktop-a/create",
        model_payload(),
    )
    .await;
    assert_eq!(created["state"], "running");

    let commands = fixture
        .paths
        .service_agent_commands_dir("service-control-delete");
    std::fs::create_dir_all(&commands).unwrap();
    write_atomic(
        commands.join("delete-desktop-a.json"),
        json!({"action": "delete", "agent_name": "desktop-a"}).to_string(),
    );

    let store = RegistryStore::open(StoragePaths::new(fixture.root.path())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while store
        .get_agent("service-control-delete", "desktop-a")
        .unwrap()
        .is_some()
        && Instant::now() < deadline
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(store
        .get_agent("service-control-delete", "desktop-a")
        .unwrap()
        .is_none());
    assert!(!fixture.paths.codex_home("desktop-a").exists());
    assert!(!fixture.paths.agent_log("desktop-a").exists());

    let service = fixture.take();
    let runtime = tokio::spawn(service.run_until_shutdown());
    write_atomic(
        fixture
            .paths
            .service_shutdown_file("service-control-delete"),
        b"",
    );
    tokio::time::timeout(Duration::from_secs(5), runtime)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    client.close().await.unwrap();
}

struct ControlFixture {
    root: tempfile::TempDir,
    paths: StoragePaths,
    port: u16,
    service: Option<ServiceRuntime>,
}

impl ControlFixture {
    async fn new(service_name: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(root.path());
        install_mock_binary(&paths);
        let store = RegistryStore::open(paths.clone()).unwrap();
        let port = free_port();
        let mut config = ServiceConfig::default();
        config.port_mode = PortMode::Fixed;
        config.port = Some(port);
        config.discovery_enabled = false;
        store
            .create_service(service_name, "initial-agent", &config)
            .unwrap();
        let service = ServiceRuntime::start(Arc::new(std::sync::Mutex::new(store)), service_name)
            .await
            .unwrap();
        Self {
            root,
            paths,
            port,
            service: Some(service),
        }
    }

    fn take(&mut self) -> ServiceRuntime {
        self.service.take().unwrap()
    }
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

fn install_mock_binary(paths: &StoragePaths) {
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

fn write_atomic(path: std::path::PathBuf, contents: impl AsRef<[u8]>) {
    let temporary = path.with_extension("tmp");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&temporary, contents).unwrap();
    std::fs::rename(&temporary, &path).unwrap();
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
    panic!("query {key} did not return JSON");
}

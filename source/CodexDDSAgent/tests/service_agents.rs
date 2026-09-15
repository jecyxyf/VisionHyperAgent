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
async fn multiple_agents_have_isolated_lifecycles_and_parallel_queues() {
    let fixture = ServiceFixture::new("service-agents-multi", &["--delay-ms", "100"]).await;
    let client = open_client(fixture.port).await;

    let model = model_payload();
    query_json(
        client.clone(),
        "codex-dds/v1/service-agents-multi/agent/desktop-a/create",
        model.clone(),
    )
    .await;
    query_json(
        client.clone(),
        "codex-dds/v1/service-agents-multi/agent/desktop-b/create",
        model,
    )
    .await;

    for agent in ["desktop-a", "desktop-b"] {
        query_json(
            client.clone(),
            &format!("codex-dds/v1/service-agents-multi/agent/{agent}/heartbeat"),
            json!({"version": 1}),
        )
        .await;
        let status = query_json(
            client.clone(),
            &format!("codex-dds/v1/service-agents-multi/{agent}/status/get"),
            json!({"version": 1}),
        )
        .await;
        assert_eq!(status["state"], "running");
        assert_eq!(status["websocket_state"], "connected");
        assert_eq!(status["codex_process_state"], "running");
        assert_eq!(status["heartbeat_active"], true);
    }

    let first_a = tokio::spawn(query_json(
        client.clone(),
        "codex-dds/v1/service-agents-multi/desktop-a/rpc",
        rpc_payload("a-first", "account/usage/read"),
    ));
    let second_a = tokio::spawn(query_json(
        client.clone(),
        "codex-dds/v1/service-agents-multi/desktop-a/rpc",
        rpc_payload("a-second", "account/rateLimits/read"),
    ));
    tokio::time::sleep(Duration::from_millis(30)).await;
    let only_b = tokio::spawn(query_json(
        client.clone(),
        "codex-dds/v1/service-agents-multi/desktop-b/rpc",
        rpc_payload("b-first", "account/usage/read"),
    ));

    let first_a = tokio::time::timeout(Duration::from_secs(4), first_a)
        .await
        .unwrap()
        .unwrap();
    let only_b = tokio::time::timeout(Duration::from_secs(4), only_b)
        .await
        .unwrap()
        .unwrap();
    let second_a = tokio::time::timeout(Duration::from_secs(4), second_a)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(first_a["request_id"], "a-first");
    assert_eq!(second_a["request_id"], "a-second");
    assert_eq!(only_b["request_id"], "b-first");
    assert_eq!(
        only_b["result"]["method"], "account/usage/read",
        "different agents must not cross-deliver requests"
    );

    let detached = query_json(
        client.clone(),
        "codex-dds/v1/service-agents-multi/agent/desktop-a/detach",
        json!({"version": 1}),
    )
    .await;
    assert_eq!(detached["state"], "stopped");

    let stopped_status = query_json(
        client.clone(),
        "codex-dds/v1/service-agents-multi/desktop-a/status/get",
        json!({"version": 1}),
    )
    .await;
    assert_eq!(stopped_status["state"], "stopped");
    assert_eq!(stopped_status["heartbeat_active"], false);
    assert_eq!(stopped_status["codex_process_state"], "stopped");

    let attached = query_json(
        client.clone(),
        "codex-dds/v1/service-agents-multi/agent/desktop-a/attach",
        json!({"version": 1}),
    )
    .await;
    assert_eq!(attached["state"], "running");
    assert_eq!(attached["model_config_locked"], true);

    client.close().await.unwrap();
    fixture.service.shutdown().await.unwrap();

    let store = RegistryStore::open(StoragePaths::new(fixture.root.path())).unwrap();
    for agent in ["desktop-a", "desktop-b"] {
        let record = store
            .get_agent("service-agents-multi", agent)
            .unwrap()
            .unwrap();
        assert_eq!(record.state, AgentRegistryState::Stopped);
        let config = StoragePaths::new(fixture.root.path())
            .codex_home(agent)
            .join("config.toml");
        assert!(config.is_file());
    }
    let service = store.get_service("service-agents-multi").unwrap().unwrap();
    assert_eq!(service.state, ServiceRegistryState::Stopped);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn heartbeat_timeout_stops_runtime_but_keeps_registration_and_config() {
    let fixture = ServiceFixture::new("service-agents-heartbeat", &[]).await;
    let client = open_client(fixture.port).await;

    query_json(
        client.clone(),
        "codex-dds/v1/service-agents-heartbeat/agent/desktop-a/create",
        model_payload(),
    )
    .await;

    let deadline = Instant::now() + Duration::from_secs(8);
    let status = loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let status = query_json(
            client.clone(),
            "codex-dds/v1/service-agents-heartbeat/desktop-a/status/get",
            json!({"version": 1}),
        )
        .await;
        if (status["heartbeat_active"] == false && status["state"] == "stopped")
            || Instant::now() > deadline
        {
            break status;
        }
    };
    assert_eq!(status["state"], "stopped", "timeout status: {status}");
    assert_eq!(status["heartbeat_active"], false);
    assert_eq!(status["codex_process_state"], "stopped");

    let attached = query_json(
        client.clone(),
        "codex-dds/v1/service-agents-heartbeat/agent/desktop-a/attach",
        json!({"version": 1}),
    )
    .await;
    assert_eq!(attached["state"], "running");

    client.close().await.unwrap();
    fixture.service.shutdown().await.unwrap();

    let paths = StoragePaths::new(fixture.root.path());
    let store = RegistryStore::open(paths.clone()).unwrap();
    let agent = store
        .get_agent("service-agents-heartbeat", "desktop-a")
        .unwrap()
        .unwrap();
    assert_eq!(agent.state, AgentRegistryState::Stopped);
    assert!(paths.codex_home("desktop-a").join("config.toml").is_file());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn notifications_and_reverse_requests_are_forwarded_per_agent() {
    let fixture = ServiceFixture::new(
        "service-agents-events",
        &["--notify-after-handshake", "--reverse-count", "1"],
    )
    .await;
    let client = open_client(fixture.port).await;

    let mut events = subscribe(
        &client,
        "codex-dds/v1/service-agents-events/desktop-a/event",
    )
    .await;
    let mut reverse = subscribe(
        &client,
        "codex-dds/v1/service-agents-events/desktop-a/reverse/request",
    )
    .await;

    query_json(
        client.clone(),
        "codex-dds/v1/service-agents-events/agent/desktop-a/create",
        model_payload(),
    )
    .await;

    let event = tokio::time::timeout(Duration::from_secs(4), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event["method"], "turn/started");
    assert_eq!(event["params"]["reason"], "mock");

    let reverse_request = tokio::time::timeout(Duration::from_secs(4), reverse.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reverse_request["method"], "currentTime/read");
    let reverse_id = reverse_request["reverse_id"].as_str().unwrap().to_string();

    let response = query_json(
        client.clone(),
        "codex-dds/v1/service-agents-events/desktop-a/reverse/response",
        json!({
            "version": 1,
            "reverse_id": reverse_id,
            "result": {"now": "2026-09-16T00:00:00Z"}
        }),
    )
    .await;
    assert_eq!(response["ok"], true);

    client.close().await.unwrap();
    fixture.service.shutdown().await.unwrap();
}

struct ServiceFixture {
    root: tempfile::TempDir,
    port: u16,
    service: ServiceRuntime,
}

impl ServiceFixture {
    async fn new(service_name: &str, mock_arguments: &[&str]) -> Self {
        let root = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(root.path());
        install_mock_binary(&paths, mock_arguments);
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
            port,
            service,
        }
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
                "models": ["model-a", "model-b"]
            }]
        }
    })
}

fn rpc_payload(request_id: &str, method: &str) -> Value {
    json!({
        "version": 1,
        "request_id": request_id,
        "method": method,
        "params": {"request_id": request_id}
    })
}

fn install_mock_binary(paths: &StoragePaths, arguments: &[&str]) {
    paths.prepare().unwrap();
    let mock = env!("CARGO_BIN_EXE_mock-codex-server");
    let mut script = format!("#!/bin/sh\nexec '{}'", mock.replace('\'', "'\\''"));
    for argument in arguments {
        script.push_str(&format!(" '{}'", argument.replace('\'', "'\\''")));
    }
    script.push_str(" \"$@\"\n");
    std::fs::write(&paths.agent_binary, script).unwrap();
    make_executable(&paths.agent_binary);
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) {}

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

async fn subscribe(session: &zenoh::Session, key: &str) -> tokio::sync::mpsc::Receiver<Value> {
    let subscriber = session.declare_subscriber(key).await.unwrap();
    let (sender, receiver) = tokio::sync::mpsc::channel(16);
    tokio::spawn(async move {
        while let Ok(sample) = subscriber.recv_async().await {
            let Some(payload) = sample.payload().try_to_string().ok() else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&payload) else {
                continue;
            };
            if sender.send(value).await.is_err() {
                break;
            }
        }
    });
    receiver
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

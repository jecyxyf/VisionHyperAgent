use std::{net::TcpListener, sync::Arc, time::Duration};

use serde_json::Value;
use vha_codex_dds_agent::{
    registry::{PortMode, RegistryStore, ServiceConfig, ServiceRegistryState, StoragePaths},
    service::{ServiceRuntime, INFO_KEY},
};
use zenoh::Config;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn service_discovery_and_status_snapshot_are_available() {
    let root = tempfile::tempdir().unwrap();
    let paths = StoragePaths::new(root.path());
    let store = RegistryStore::open(paths).unwrap();
    let port = free_port();
    let mut config = ServiceConfig::default();
    config.port_mode = PortMode::Fixed;
    config.port = Some(port);
    config.discovery_enabled = false;
    store
        .create_service("service-discovery-test", "desktop-a", &config)
        .unwrap();

    let service = ServiceRuntime::start(
        Arc::new(std::sync::Mutex::new(store)),
        "service-discovery-test",
    )
    .await
    .unwrap();

    let client = open_client(port).await;
    let info = query_json(&client, INFO_KEY).await;
    assert_eq!(info["service_name"], "service-discovery-test");
    assert_eq!(info["port"], port);
    assert_eq!(info["state"], "running");
    assert!(info.get("created_at").is_none());
    assert!(info.get("conflict_id").is_none());

    let status_key = "codex-dds/v1/service-discovery-test/status/get";
    let status = query_json(&client, status_key).await;
    assert_eq!(status["service_name"], "service-discovery-test");
    assert_eq!(status["state"], "running");

    service.shutdown().await.unwrap();
    client.close().await.unwrap();

    let root_store = RegistryStore::open(StoragePaths::new(root.path())).unwrap();
    let record = root_store
        .get_service("service-discovery-test")
        .unwrap()
        .unwrap();
    assert_eq!(record.state, ServiceRegistryState::Stopped);
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
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

async fn query_json(session: &zenoh::Session, key: &str) -> Value {
    let replies = session
        .get(key)
        .timeout(Duration::from_secs(3))
        .await
        .unwrap();
    while let Ok(reply) = replies.recv_async().await {
        if let Ok(sample) = reply.result() {
            if let Some(payload) = sample.payload().try_to_string().ok() {
                if let Ok(value) = serde_json::from_str::<Value>(&payload) {
                    return value;
                }
            }
        }
    }
    panic!("query {key} did not return a JSON payload");
}

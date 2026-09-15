use std::{
    net::TcpListener,
    sync::{Arc, Mutex},
    time::Duration,
};

use vha_codex_dds_agent::{
    registry::{PortMode, RegistryStore, ServiceConfig, ServiceRegistryState, StoragePaths},
    service::ServiceRuntime,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn newer_online_service_loses_name_conflict() {
    let service_name = "service-conflict-runtime";
    let old = ConflictFixture::new(service_name).await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    let new = ConflictFixture::new(service_name).await;

    let newer_runtime: Option<ServiceRuntime> =
        match ServiceRuntime::start(new.store.clone(), service_name).await {
            Ok(runtime) => Some(runtime),
            Err(error) => {
                assert!(error.to_string().contains("name conflict"), "{error}");
                None
            }
        };
    if let Some(runtime) = newer_runtime {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let state = new
                .store
                .lock()
                .unwrap()
                .get_service(service_name)
                .unwrap()
                .unwrap()
                .state;
            if state == ServiceRegistryState::NameConflict
                || tokio::time::Instant::now() >= deadline
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        runtime.shutdown().await.unwrap();
    }

    let new_state = new
        .store
        .lock()
        .unwrap()
        .get_service(service_name)
        .unwrap()
        .unwrap()
        .state;
    let old_state = old
        .store
        .lock()
        .unwrap()
        .get_service(service_name)
        .unwrap()
        .unwrap()
        .state;
    assert_eq!(new_state, ServiceRegistryState::NameConflict);
    assert_eq!(old_state, ServiceRegistryState::Running);

    old.service.unwrap().shutdown().await.unwrap();
}

struct ConflictFixture {
    _root: tempfile::TempDir,
    store: Arc<Mutex<RegistryStore>>,
    service: Option<ServiceRuntime>,
}

impl ConflictFixture {
    async fn new(service_name: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(root.path());
        let store = Arc::new(Mutex::new(RegistryStore::open(paths).unwrap()));
        let mut config = ServiceConfig::default();
        config.port_mode = PortMode::Fixed;
        config.port = Some(free_port());
        config.discovery_enabled = true;
        store
            .lock()
            .unwrap()
            .create_service(service_name, "initial-agent", &config)
            .unwrap();
        let service = match ServiceRuntime::start(store.clone(), service_name).await {
            Ok(service) => Some(service),
            Err(error) => {
                assert!(error.to_string().contains("name conflict"), "{error}");
                None
            }
        };
        Self {
            _root: root,
            store,
            service,
        }
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

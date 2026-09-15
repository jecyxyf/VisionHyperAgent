use std::{
    fs,
    net::{TcpListener, UdpSocket},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::{json, Value};
use tokio::{sync::watch, task::JoinSet};
use zenoh::{Config, Session};

use crate::{
    error::{Error, RpcError},
    instance::{
        delete_agent, handle_agent_query, stop_all_agents, stop_expired_agents, AgentManager,
        AgentQueryKind,
    },
    registry::{
        PortMode, RegistryStore, ServiceConfig, ServiceRecord, ServiceRegistryState, StoragePaths,
    },
};

pub const INFO_KEY: &str = "codex-dds/v1/service/info";
pub const PROTOCOL_VERSION: i64 = 1;

#[derive(Debug, Clone)]
struct RuntimeState {
    service_name: String,
    created_at: String,
    conflict_id: String,
    host: String,
    port: u16,
    store: Arc<Mutex<RegistryStore>>,
}

pub struct ServiceRuntime {
    session: Session,
    state: RuntimeState,
    agents: Arc<Mutex<AgentManager>>,
    tasks: JoinSet<()>,
    shutdown_tx: watch::Sender<bool>,
    requested_state: Arc<Mutex<Option<ServiceRegistryState>>>,
}

impl ServiceRuntime {
    pub async fn start(
        store: Arc<Mutex<RegistryStore>>,
        service_name: &str,
    ) -> Result<Self, Error> {
        let (service, config) = {
            let store = store
                .lock()
                .map_err(|_| Error::internal("registry lock poisoned"))?;
            let service = store.get_service(service_name)?.ok_or_else(|| {
                Error::Rpc(RpcError::new("service_not_found", "service does not exist"))
            })?;
            let config = crate::registry::service_config(&service)?;
            (service, config)
        };

        let (session, port) = open_zenoh_session(&config).await?;
        let state = RuntimeState {
            service_name: service_name.to_string(),
            created_at: service.created_at,
            conflict_id: service.conflict_id,
            host: advertised_host(&config.listen_host),
            port,
            store,
        };

        if let Some(competitor) = conflicting_service(&session, &state)
            .await?
            .filter(|competitor| service_loses(&state, competitor))
        {
            session
                .close()
                .await
                .map_err(|error| Error::internal(error.to_string()))?;
            state.update_registry(ServiceRegistryState::NameConflict, Some(port))?;
            return Err(Error::Rpc(RpcError::new(
                "name_conflict",
                format!(
                    "newer service loses the name conflict against {}",
                    competitor.conflict_id
                ),
            )));
        }

        let mut tasks = JoinSet::new();
        declare_info_queryable(&session, state.clone(), &mut tasks).await?;
        declare_status_queryable(&session, state.clone(), &mut tasks).await?;
        let agents = Arc::new(Mutex::new(AgentManager::new(state.store.clone())));
        declare_agent_queryables(&session, state.clone(), agents.clone(), &mut tasks).await?;
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        tasks.spawn(heartbeat_monitor(
            session.clone(),
            agents.clone(),
            service_name.to_string(),
        ));
        let requested_state = Arc::new(Mutex::new(None));
        tasks.spawn(name_conflict_monitor(
            session.clone(),
            state.clone(),
            requested_state.clone(),
            shutdown_tx.clone(),
        ));
        tasks.spawn(local_control_monitor(
            session.clone(),
            state.clone(),
            agents.clone(),
            shutdown_tx.clone(),
        ));

        state.update_registry(ServiceRegistryState::Running, Some(port))?;
        publish_status(&session, &state, "running").await?;
        tokio::spawn(wait_for_shutdown(shutdown_rx));

        Ok(Self {
            session,
            state,
            agents,
            tasks,
            shutdown_tx,
            requested_state,
        })
    }

    pub fn port(&self) -> u16 {
        self.state.port
    }

    pub async fn run_until_shutdown(self) -> Result<(), Error> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        loop {
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    if changed.is_err() || *shutdown_rx.borrow() {
                        break;
                    }
                }
                _ = tokio::signal::ctrl_c() => break,
            }
        }
        self.shutdown().await
    }

    pub async fn shutdown(mut self) -> Result<(), Error> {
        let _ = self.shutdown_tx.send(true);
        publish_status(&self.session, &self.state, "stopped").await?;
        self.tasks.abort_all();
        while self.tasks.join_next().await.is_some() {}
        let stop_agents_result =
            stop_all_agents(&self.session, &self.agents, &self.state.service_name).await;
        self.session
            .close()
            .await
            .map_err(|error| Error::internal(error.to_string()))?;
        let final_state = self
            .requested_state
            .lock()
            .map_err(|_| Error::internal("shutdown state lock poisoned"))?
            .unwrap_or(ServiceRegistryState::Stopped);
        self.state
            .update_registry(final_state, Some(self.state.port))?;
        stop_agents_result?;
        Ok(())
    }
}

async fn name_conflict_monitor(
    session: Session,
    state: RuntimeState,
    requested_state: Arc<Mutex<Option<ServiceRegistryState>>>,
    shutdown_tx: watch::Sender<bool>,
) {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let Ok(competitor) = conflicting_service(&session, &state).await else {
            continue;
        };
        let Some(competitor) = competitor.filter(|competitor| service_loses(&state, competitor))
        else {
            continue;
        };
        let _ = competitor;
        if let Ok(mut requested) = requested_state.lock() {
            *requested = Some(ServiceRegistryState::NameConflict);
        }
        let _ = shutdown_tx.send(true);
        break;
    }
}

async fn local_control_monitor(
    session: Session,
    state: RuntimeState,
    agents: Arc<Mutex<AgentManager>>,
    shutdown_tx: watch::Sender<bool>,
) {
    loop {
        let paths = {
            let Ok(store) = state.store.lock() else {
                break;
            };
            store.paths().clone()
        };
        if handle_shutdown_file(&paths, &state.service_name) {
            let _ = shutdown_tx.send(true);
            break;
        }
        if let Some(command_file) = latest_command_file(&paths, &state.service_name) {
            if let Some(agent_name) = read_agent_delete_command(&command_file) {
                let _ = delete_agent(&session, &agents, &state.service_name, &agent_name).await;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

fn handle_shutdown_file(paths: &StoragePaths, service_name: &str) -> bool {
    let path = paths.service_shutdown_file(service_name);
    if !path.is_file() {
        return false;
    }
    fs::remove_file(&path).is_ok() || !path.exists()
}

fn latest_command_file(paths: &StoragePaths, service_name: &str) -> Option<PathBuf> {
    let directory = paths.service_agent_commands_dir(service_name);
    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(&directory).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let modified = entry.metadata().ok()?.modified().ok()?;
        if latest.as_ref().is_none_or(|(old, _)| modified > *old) {
            latest = Some((modified, path));
        }
    }
    latest.map(|(_, path)| path)
}

fn read_agent_delete_command(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let removed = fs::remove_file(path).is_ok();
    if !removed && path.exists() {
        return None;
    }
    let value = serde_json::from_str::<Value>(&contents).ok()?;
    let agent_name = value.get("agent_name")?.as_str()?.to_string();
    (!agent_name.is_empty()).then_some(agent_name)
}

async fn heartbeat_monitor(
    session: Session,
    agents: Arc<Mutex<AgentManager>>,
    service_name: String,
) {
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let _ = stop_expired_agents(&session, &agents, &service_name).await;
    }
}

impl RuntimeState {
    fn update_registry(&self, state: ServiceRegistryState, port: Option<u16>) -> Result<(), Error> {
        let store = self
            .store
            .lock()
            .map_err(|_| Error::internal("registry lock poisoned"))?;
        store.update_service_runtime(&self.service_name, state, port)
    }

    fn info_response(&self) -> Value {
        json!({
            "version": PROTOCOL_VERSION,
            "protocol_version": PROTOCOL_VERSION,
            "service_name": self.service_name,
            "created_at": self.created_at,
            "conflict_id": self.conflict_id,
            "host": self.host,
            "port": self.port,
            "state": "running",
        })
    }

    fn status_response(&self) -> Result<Value, Error> {
        let service = {
            let store = self
                .store
                .lock()
                .map_err(|_| Error::internal("registry lock poisoned"))?;
            store.get_service(&self.service_name)?.ok_or_else(|| {
                Error::Rpc(RpcError::new("service_not_found", "service does not exist"))
            })?
        };
        Ok(json!({
            "version": PROTOCOL_VERSION,
            "service_name": self.service_name,
            "state": service.state,
            "listen_host": self.host,
            "port": self.port,
            "changed_at": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

async fn wait_for_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    loop {
        if shutdown_rx.changed().await.is_err() || *shutdown_rx.borrow() {
            break;
        }
    }
}

async fn declare_info_queryable(
    session: &Session,
    state: RuntimeState,
    tasks: &mut JoinSet<()>,
) -> Result<(), Error> {
    let queryable = session
        .declare_queryable(INFO_KEY)
        .await
        .map_err(|error| Error::internal(error.to_string()))?;
    tasks.spawn(async move {
        loop {
            let Ok(query) = queryable.recv_async().await else {
                break;
            };
            let payload =
                serde_json::to_string(&state.info_response()).unwrap_or_else(|_| "{}".to_string());
            let _ = query.reply(INFO_KEY, payload).await;
        }
    });
    Ok(())
}

async fn declare_status_queryable(
    session: &Session,
    state: RuntimeState,
    tasks: &mut JoinSet<()>,
) -> Result<(), Error> {
    let key = format!("codex-dds/v1/{}/status/get", state.service_name);
    let queryable = session
        .declare_queryable(key.as_str())
        .await
        .map_err(|error| Error::internal(error.to_string()))?;
    tasks.spawn(async move {
        loop {
            let Ok(query) = queryable.recv_async().await else {
                break;
            };
            let payload = state
                .status_response()
                .and_then(|value| Ok(serde_json::to_string(&value)?))
                .unwrap_or_else(|error| {
                    serde_json::to_string(&json!({
                        "version": PROTOCOL_VERSION,
                        "ok": false,
                        "error": {"code": "internal_error", "message": error.to_string()}
                    }))
                    .unwrap_or_else(|_| "{}".to_string())
                });
            let _ = query.reply(key.as_str(), payload).await;
        }
    });
    Ok(())
}

async fn declare_agent_queryables(
    session: &Session,
    state: RuntimeState,
    agents: Arc<Mutex<AgentManager>>,
    tasks: &mut JoinSet<()>,
) -> Result<(), Error> {
    let definitions = [
        (
            format!("codex-dds/v1/{}/agent/*/create", state.service_name),
            AgentQueryKind::Create,
        ),
        (
            format!("codex-dds/v1/{}/agent/*/attach", state.service_name),
            AgentQueryKind::Attach,
        ),
        (
            format!("codex-dds/v1/{}/agent/*/detach", state.service_name),
            AgentQueryKind::Detach,
        ),
        (
            format!("codex-dds/v1/{}/agent/*/heartbeat", state.service_name),
            AgentQueryKind::Heartbeat,
        ),
        (
            format!("codex-dds/v1/{}/*/rpc", state.service_name),
            AgentQueryKind::Rpc,
        ),
        (
            format!("codex-dds/v1/{}/*/reverse/response", state.service_name),
            AgentQueryKind::ReverseResponse,
        ),
        (
            format!("codex-dds/v1/{}/*/status/get", state.service_name),
            AgentQueryKind::StatusGet,
        ),
    ];

    for (key, kind) in definitions {
        let queryable = session
            .declare_queryable(key.as_str())
            .await
            .map_err(|error| Error::internal(error.to_string()))?;
        let session = session.clone();
        let agents = agents.clone();
        let service_name = state.service_name.clone();
        let lifecycle_prefix = format!("codex-dds/v1/{service_name}/agent/");
        let rpc_prefix = format!("codex-dds/v1/{service_name}/");
        let status_prefix = rpc_prefix.clone();
        tasks.spawn(async move {
            loop {
                let Ok(query) = queryable.recv_async().await else {
                    break;
                };
                let query_key = query.key_expr().as_str().to_string();
                let Some(agent_name) = extract_agent_name(
                    &query_key,
                    kind,
                    &lifecycle_prefix,
                    &rpc_prefix,
                    &status_prefix,
                ) else {
                    reply_error_for_invalid_key(&query).await;
                    continue;
                };
                tokio::spawn({
                    let session = session.clone();
                    let agents = agents.clone();
                    let service_name = service_name.clone();
                    let agent_name = agent_name.clone();
                    async move {
                        handle_agent_query(
                            &session,
                            &agents,
                            &service_name,
                            &agent_name,
                            kind,
                            query,
                        )
                        .await;
                    }
                });
            }
        });
    }
    Ok(())
}

fn extract_agent_name(
    query_key: &str,
    kind: AgentQueryKind,
    lifecycle_prefix: &str,
    rpc_prefix: &str,
    status_prefix: &str,
) -> Option<String> {
    match kind {
        AgentQueryKind::Create
        | AgentQueryKind::Attach
        | AgentQueryKind::Detach
        | AgentQueryKind::Heartbeat => {
            let suffix = match kind {
                AgentQueryKind::Create => "/create",
                AgentQueryKind::Attach => "/attach",
                AgentQueryKind::Detach => "/detach",
                AgentQueryKind::Heartbeat => "/heartbeat",
                _ => unreachable!(),
            };
            query_key
                .strip_prefix(lifecycle_prefix)?
                .strip_suffix(suffix)
        }
        AgentQueryKind::Rpc => query_key.strip_prefix(rpc_prefix)?.strip_suffix("/rpc"),
        AgentQueryKind::ReverseResponse => query_key
            .strip_prefix(rpc_prefix)?
            .strip_suffix("/reverse/response"),
        AgentQueryKind::StatusGet => query_key
            .strip_prefix(status_prefix)?
            .strip_suffix("/status/get"),
    }
    .filter(|name| !name.is_empty())
    .map(ToString::to_string)
}

async fn reply_error_for_invalid_key(query: &zenoh::query::Query) {
    let payload = json!({
        "version": PROTOCOL_VERSION,
        "ok": false,
        "error": {"code": "invalid_request", "message": "invalid agent key"},
    });
    let _ = query.reply_err(payload.to_string()).await;
}

async fn publish_status(
    session: &Session,
    state: &RuntimeState,
    state_name: &str,
) -> Result<(), Error> {
    let key = format!("codex-dds/v1/{}/status", state.service_name);
    let payload = json!({
        "version": PROTOCOL_VERSION,
        "service_name": state.service_name,
        "state": state_name,
        "changed_at": chrono::Utc::now().to_rfc3339(),
    });
    session
        .put(key.as_str(), payload.to_string())
        .await
        .map_err(|error| Error::internal(error.to_string()))?;
    Ok(())
}

async fn open_zenoh_session(config: &ServiceConfig) -> Result<(Session, u16), Error> {
    let candidates = port_candidates(config)?;
    let mut last_error = None;
    for port in candidates {
        let endpoint = listen_endpoint(&config.listen_host, port);
        let mut zenoh_config = Config::default();
        zenoh_config
            .insert_json5("mode", "\"peer\"")
            .map_err(|error| Error::internal(error.to_string()))?;
        zenoh_config
            .insert_json5("listen/endpoints", &format!("[\"{endpoint}\"]"))
            .map_err(|error| Error::internal(error.to_string()))?;
        if !config.discovery_enabled {
            zenoh_config
                .insert_json5("scouting/multicast/enabled", "false")
                .map_err(|error| Error::internal(error.to_string()))?;
        }

        match zenoh::open(zenoh_config).await {
            Ok(session) => return Ok((session, port)),
            Err(error) => {
                last_error = Some(error.to_string());
                if config.port_mode == PortMode::Fixed {
                    break;
                }
            }
        }
    }
    Err(Error::Rpc(RpcError::new(
        "server_address_invalid",
        format!(
            "failed to listen on Zenoh endpoint: {}",
            last_error.unwrap_or_else(|| "no available port".to_string())
        ),
    )))
}

async fn conflicting_service(
    session: &Session,
    state: &RuntimeState,
) -> Result<Option<ServiceRecord>, Error> {
    let replies = session
        .get(INFO_KEY)
        .timeout(Duration::from_millis(500))
        .await
        .map_err(|error| Error::internal(error.to_string()))?;
    while let Ok(reply) = replies.recv_async().await {
        let Ok(sample) = reply.result() else {
            continue;
        };
        let Some(payload) = sample.payload().try_to_string().ok() else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&payload) else {
            continue;
        };
        let service_name = value.get("service_name").and_then(Value::as_str);
        let conflict_id = value.get("conflict_id").and_then(Value::as_str);
        let created_at = value.get("created_at").and_then(Value::as_str);
        if service_name == Some(state.service_name.as_str())
            && conflict_id != Some(state.conflict_id.as_str())
        {
            return Ok(Some(ServiceRecord {
                service_name: service_name.unwrap_or_default().to_string(),
                created_at: created_at.unwrap_or_default().to_string(),
                config_json: String::new(),
                last_started_at: None,
                last_listen_port: None,
                conflict_id: conflict_id.unwrap_or_default().to_string(),
                state: ServiceRegistryState::Running,
            }));
        }
    }
    Ok(None)
}

fn service_loses(state: &RuntimeState, competitor: &ServiceRecord) -> bool {
    match state.created_at.cmp(&competitor.created_at) {
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Equal => state.conflict_id > competitor.conflict_id,
    }
}

fn port_candidates(config: &ServiceConfig) -> Result<Vec<u16>, Error> {
    if config.port_mode == PortMode::Fixed {
        return Ok(vec![config.port.ok_or_else(|| {
            Error::Rpc(RpcError::new(
                "server_address_invalid",
                "fixed port mode requires port",
            ))
        })?]);
    }
    if let Some((start, end)) = config.port_range {
        if end < start || start == 0 {
            return Err(Error::Rpc(RpcError::new(
                "server_address_invalid",
                "invalid port range",
            )));
        }
        return Ok((start..=end).collect());
    }
    let listener = TcpListener::bind((config.listen_host.as_str(), 0)).map_err(Error::Io)?;
    Ok(vec![listener.local_addr().map_err(Error::Io)?.port()])
}

fn listen_endpoint(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("tcp/[{host}]:{port}")
    } else {
        format!("tcp/{host}:{port}")
    }
}

fn advertised_host(listen_host: &str) -> String {
    if listen_host != "0.0.0.0" && listen_host != "::" {
        return listen_host.to_string();
    }
    for target in ["8.8.8.8:80", "1.1.1.1:80"] {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if socket.connect(target).is_ok() {
                if let Ok(address) = socket.local_addr() {
                    return address.ip().to_string();
                }
            }
        }
    }
    "127.0.0.1".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_ipv4_and_ipv6_endpoints() {
        assert_eq!(listen_endpoint("0.0.0.0", 17600), "tcp/0.0.0.0:17600");
        assert_eq!(listen_endpoint("::", 17600), "tcp/[::]:17600");
    }
}

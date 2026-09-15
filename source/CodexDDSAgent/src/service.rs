use std::{
    net::{TcpListener, UdpSocket},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::{json, Value};
use tokio::{sync::watch, task::JoinSet};
use zenoh::{Config, Session};

use crate::{
    error::{Error, RpcError},
    instance::{
        handle_agent_query, stop_all_agents, stop_expired_agents, AgentManager, AgentQueryKind,
    },
    registry::{PortMode, RegistryStore, ServiceConfig, ServiceRegistryState},
};

pub const INFO_KEY: &str = "codex-dds/v1/service/info";
pub const PROTOCOL_VERSION: i64 = 1;

#[derive(Debug, Clone)]
struct RuntimeState {
    service_name: String,
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
}

impl ServiceRuntime {
    pub async fn start(
        store: Arc<Mutex<RegistryStore>>,
        service_name: &str,
    ) -> Result<Self, Error> {
        let config = {
            let store = store
                .lock()
                .map_err(|_| Error::internal("registry lock poisoned"))?;
            let service = store.get_service(service_name)?.ok_or_else(|| {
                Error::Rpc(RpcError::new("service_not_found", "service does not exist"))
            })?;
            crate::registry::service_config(&service)?
        };

        let (session, port) = open_zenoh_session(&config).await?;
        let state = RuntimeState {
            service_name: service_name.to_string(),
            host: advertised_host(&config.listen_host),
            port,
            store,
        };

        if service_name_conflict(&session, service_name).await? {
            session
                .close()
                .await
                .map_err(|error| Error::internal(error.to_string()))?;
            state.update_registry(ServiceRegistryState::NameConflict, Some(port))?;
            return Err(Error::Rpc(RpcError::new(
                "name_conflict",
                "another online service uses the same service_name",
            )));
        }

        let mut tasks = JoinSet::new();
        declare_info_queryable(&session, state.clone(), &mut tasks).await?;
        declare_status_queryable(&session, state.clone(), &mut tasks).await?;
        let agents = Arc::new(Mutex::new(AgentManager::new(state.store.clone())));
        declare_agent_queryables(&session, state.clone(), agents.clone(), &mut tasks).await?;
        tasks.spawn(heartbeat_monitor(
            session.clone(),
            agents.clone(),
            service_name.to_string(),
        ));

        state.update_registry(ServiceRegistryState::Running, Some(port))?;
        publish_status(&session, &state, "running").await?;
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        tokio::spawn(wait_for_shutdown(shutdown_rx));

        Ok(Self {
            session,
            state,
            agents,
            tasks,
            shutdown_tx,
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
        self.state
            .update_registry(ServiceRegistryState::Stopped, Some(self.state.port))?;
        stop_agents_result?;
        Ok(())
    }
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
                handle_agent_query(&session, &agents, &service_name, &agent_name, kind, query)
                    .await;
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

async fn service_name_conflict(session: &Session, service_name: &str) -> Result<bool, Error> {
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
        if value.get("service_name").and_then(Value::as_str) == Some(service_name) {
            return Ok(true);
        }
    }
    Ok(false)
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

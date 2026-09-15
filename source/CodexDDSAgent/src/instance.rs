use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::oneshot;
use zenoh::{query::Query, Session};

use crate::{
    error::{Error, RpcError},
    model_config::ModelConfig,
    protocol::RpcRequest,
    registry::{AgentRegistryState, RegistryStore},
    runtime::{AgentRuntime, RuntimeCommand, RuntimeEvent, RuntimeSnapshot},
    service::PROTOCOL_VERSION,
};

pub const HEARTBEAT_INTERVAL_MS: u64 = 1000;
pub const HEARTBEAT_TIMEOUT_MS: u64 = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentQueryKind {
    Create,
    Attach,
    Detach,
    Heartbeat,
    Rpc,
    ReverseResponse,
    StatusGet,
}

pub struct AgentManager {
    store: Arc<Mutex<RegistryStore>>,
    agents: HashMap<String, ManagedAgent>,
    initializing: HashMap<String, Arc<tokio::sync::Mutex<()>>>,
}

#[derive(Clone)]
struct ManagedAgent {
    runtime: AgentRuntime,
    last_heartbeat: Instant,
}

impl AgentManager {
    pub fn new(store: Arc<Mutex<RegistryStore>>) -> Self {
        Self {
            store,
            agents: HashMap::new(),
            initializing: HashMap::new(),
        }
    }

    pub fn active_agent_names(&self) -> Vec<String> {
        self.agents.keys().cloned().collect()
    }

    pub fn is_active(&self, agent_name: &str) -> bool {
        self.agents
            .get(agent_name)
            .is_some_and(|agent| agent.last_heartbeat.elapsed() < heartbeat_timeout())
    }

    fn initialization_lock(&mut self, agent_name: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.initializing
            .entry(agent_name.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    fn take_all_agents(&mut self) -> Vec<(String, ManagedAgent)> {
        self.agents.drain().collect()
    }

    fn command_sender(
        &self,
        agent_name: &str,
    ) -> Option<tokio::sync::mpsc::Sender<RuntimeCommand>> {
        self.agents
            .get(agent_name)
            .filter(|agent| agent.last_heartbeat.elapsed() < heartbeat_timeout())
            .map(|agent| agent.runtime.command_sender())
    }

    fn touch_heartbeat(&mut self, agent_name: &str) -> bool {
        let Some(agent) = self.agents.get_mut(agent_name) else {
            return false;
        };
        if agent.last_heartbeat.elapsed() >= heartbeat_timeout() {
            return false;
        }
        agent.last_heartbeat = Instant::now();
        true
    }
}

pub async fn handle_agent_query(
    session: &Session,
    manager: &Arc<Mutex<AgentManager>>,
    service_name: &str,
    agent_name: &str,
    kind: AgentQueryKind,
    query: Query,
) {
    let result = match kind {
        AgentQueryKind::Create => {
            let request = lifecycle_payload(&query, true);
            create_or_attach(session, manager, service_name, agent_name, request, true).await
        }
        AgentQueryKind::Attach => {
            let request = lifecycle_payload(&query, false);
            create_or_attach(session, manager, service_name, agent_name, request, false).await
        }
        AgentQueryKind::Detach => stop_agent(
            session,
            manager,
            service_name,
            agent_name,
            AgentRegistryState::Stopped,
        )
        .await
        .map(|_| detach_response()),
        AgentQueryKind::Heartbeat => heartbeat(manager, agent_name).map(|_| heartbeat_response()),
        AgentQueryKind::Rpc => rpc(manager, agent_name, &query).await,
        AgentQueryKind::ReverseResponse => reverse_response(manager, agent_name, &query).await,
        AgentQueryKind::StatusGet => agent_status(manager, service_name, agent_name).await,
    };

    match result {
        Ok(value) => reply_json(&query, value).await,
        Err(error) => reply_error(&query, error).await,
    }
}

async fn create_or_attach(
    session: &Session,
    manager: &Arc<Mutex<AgentManager>>,
    service_name: &str,
    agent_name: &str,
    request: Result<LifecycleRequest, Error>,
    create: bool,
) -> Result<Value, Error> {
    let request = request?;
    if request.version != PROTOCOL_VERSION {
        return Err(invalid_request("unsupported protocol version"));
    }

    let initialization = {
        let mut agents = lock_manager(manager)?;
        agents.initialization_lock(agent_name)
    };
    let _initialization_guard = initialization.lock().await;

    if create {
        let store_handle = registry_store(manager)?;
        let store = lock_store(&store_handle)?;
        store.create_agent(service_name, agent_name)?;
    } else {
        let store_handle = registry_store(manager)?;
        let store = lock_store(&store_handle)?;
        let agent = store
            .get_agent(service_name, agent_name)?
            .ok_or_else(agent_not_found)?;
        if !agent.model_config_initialized && request.model.is_none() {
            return Err(invalid_model_config("model configuration is required"));
        }
    }

    {
        let agents = lock_manager(manager)?;
        if agents.is_active(agent_name) {
            return Err(Error::Rpc(RpcError::new(
                "agent_busy",
                "agent heartbeat is active",
            )));
        }
    }

    let stale_runtime = {
        let mut agents = lock_manager(manager)?;
        agents.agents.remove(agent_name)
    };
    if let Some(stale) = stale_runtime {
        let _ = stale.runtime.stop().await;
    }

    if let Some(model) = &request.model {
        let store_handle = registry_store(manager)?;
        let store = lock_store(&store_handle)?;
        let codex_home = store.paths().codex_home(agent_name);
        model.initialize_or_lock(&codex_home)?;
        store.mark_agent_model_initialized(service_name, agent_name)?;
    } else {
        let store_handle = registry_store(manager)?;
        let store = lock_store(&store_handle)?;
        let agent = store
            .get_agent(service_name, agent_name)?
            .ok_or_else(agent_not_found)?;
        if !agent.model_config_initialized {
            return Err(invalid_model_config("model configuration is required"));
        }
    }

    start_runtime(session, manager, service_name, agent_name).await?;
    publish_agent_state(session, service_name, agent_name, "running").await?;
    Ok(json!({
        "version": PROTOCOL_VERSION,
        "heartbeat_interval_ms": HEARTBEAT_INTERVAL_MS,
        "heartbeat_timeout_ms": HEARTBEAT_TIMEOUT_MS,
        "model_config_locked": true,
        "state": "running",
    }))
}

async fn start_runtime(
    session: &Session,
    manager: &Arc<Mutex<AgentManager>>,
    service_name: &str,
    agent_name: &str,
) -> Result<(), Error> {
    let (binary, codex_home, log_path) = {
        let store_handle = registry_store(manager)?;
        let store = lock_store(&store_handle)?;
        let paths = store.paths();
        (
            paths.agent_binary.clone(),
            paths.codex_home(agent_name),
            paths.agent_log(agent_name).join("codex-app-server.log"),
        )
    };

    let (runtime, mut events) = AgentRuntime::start(&binary, &codex_home, &log_path).await?;
    let store_handle = registry_store(manager)?;
    lock_store(&store_handle)?.mark_agent_running(service_name, agent_name)?;
    let event_session = session.clone();
    let event_service = service_name.to_string();
    let event_agent = agent_name.to_string();
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            let (key, payload) = match event {
                RuntimeEvent::Notification {
                    event_id,
                    method,
                    params,
                    received_at,
                } => (
                    format!("codex-dds/v1/{event_service}/{event_agent}/event"),
                    json!({
                        "version": PROTOCOL_VERSION,
                        "event_id": event_id,
                        "method": method,
                        "params": params,
                        "received_at": received_at,
                    }),
                ),
                RuntimeEvent::ReverseRequest {
                    reverse_id,
                    method,
                    params,
                } => (
                    format!("codex-dds/v1/{event_service}/{event_agent}/reverse/request"),
                    json!({
                        "version": PROTOCOL_VERSION,
                        "reverse_id": reverse_id,
                        "method": method,
                        "params": params,
                    }),
                ),
                RuntimeEvent::State {
                    state,
                    websocket_state,
                    codex_process_state,
                    processing_request,
                    failure_count,
                    error,
                } => (
                    format!("codex-dds/v1/{event_service}/{event_agent}/status"),
                    json!({
                        "version": PROTOCOL_VERSION,
                        "service_name": event_service,
                        "agent_name": event_agent,
                        "state": state,
                        "websocket_state": websocket_state,
                        "codex_process_state": codex_process_state,
                        "heartbeat_active": true,
                        "model_config_initialized": true,
                        "processing_request": processing_request,
                        "failure_count": failure_count,
                        "error": error,
                        "changed_at": chrono::Utc::now().to_rfc3339(),
                    }),
                ),
            };
            let _ = event_session.put(key.as_str(), payload.to_string()).await;
        }
    });

    let mut agents = lock_manager(manager)?;
    agents.agents.insert(
        agent_name.to_string(),
        ManagedAgent {
            runtime,
            last_heartbeat: Instant::now(),
        },
    );
    Ok(())
}

async fn stop_agent(
    session: &Session,
    manager: &Arc<Mutex<AgentManager>>,
    service_name: &str,
    agent_name: &str,
    final_state: AgentRegistryState,
) -> Result<(), Error> {
    let initialization = {
        let mut agents = lock_manager(manager)?;
        agents.initialization_lock(agent_name)
    };
    let _initialization_guard = initialization.lock().await;
    let runtime = {
        let mut agents = lock_manager(manager)?;
        agents.agents.remove(agent_name)
    };
    if let Some(runtime) = runtime {
        runtime.runtime.stop().await?;
    }
    let store_handle = registry_store(manager)?;
    lock_store(&store_handle)?.mark_agent_stopped(service_name, agent_name, final_state)?;
    publish_agent_state(
        session,
        service_name,
        agent_name,
        agent_registry_state_name(final_state),
    )
    .await
}

fn heartbeat(manager: &Arc<Mutex<AgentManager>>, agent_name: &str) -> Result<(), Error> {
    let mut agents = lock_manager(manager)?;
    if agents.touch_heartbeat(agent_name) {
        Ok(())
    } else {
        Err(agent_stopped())
    }
}

async fn rpc(
    manager: &Arc<Mutex<AgentManager>>,
    agent_name: &str,
    query: &Query,
) -> Result<Value, Error> {
    let payload = query_payload(query)?;
    let request: RpcRequest =
        serde_json::from_value(payload).map_err(|_| invalid_request("invalid rpc request"))?;
    request.validate()?;
    let sender = {
        let agents = lock_manager(manager)?;
        agents
            .command_sender(agent_name)
            .ok_or_else(agent_stopped)?
    };
    let (reply_tx, reply_rx) = oneshot::channel();
    sender
        .send(RuntimeCommand::Rpc {
            request_id: request.request_id,
            method: request.method,
            params: request.params,
            reply: reply_tx,
        })
        .await
        .map_err(|_| agent_stopped())?;
    let response = reply_rx.await.map_err(|_| agent_stopped())?;
    serde_json::to_value(response).map_err(Error::Serialization)
}

async fn reverse_response(
    manager: &Arc<Mutex<AgentManager>>,
    agent_name: &str,
    query: &Query,
) -> Result<Value, Error> {
    let payload = query_payload(query)?;
    let reverse_id = payload
        .get("reverse_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_request("reverse_id is required"))?
        .to_string();
    if payload.get("version").and_then(Value::as_i64) != Some(PROTOCOL_VERSION) {
        return Err(invalid_request("unsupported protocol version"));
    }
    let result = payload.get("result").cloned();
    let error = payload.get("error").cloned();
    if result.is_some() == error.is_some() {
        return Err(invalid_request(
            "exactly one of result or error is required",
        ));
    }
    let sender = {
        let agents = lock_manager(manager)?;
        agents
            .command_sender(agent_name)
            .ok_or_else(agent_stopped)?
    };
    let (reply_tx, reply_rx) = oneshot::channel();
    sender
        .send(RuntimeCommand::ReverseResponse {
            reverse_id,
            result,
            error,
            reply: reply_tx,
        })
        .await
        .map_err(|_| agent_stopped())?;
    reply_rx.await.map_err(|_| agent_stopped())??;
    Ok(json!({"version": PROTOCOL_VERSION, "ok": true}))
}

async fn agent_status(
    manager: &Arc<Mutex<AgentManager>>,
    service_name: &str,
    agent_name: &str,
) -> Result<Value, Error> {
    let (active, runtime) = {
        let agents = lock_manager(manager)?;
        let active = agents.is_active(agent_name);
        (active, agents.agents.get(agent_name).cloned())
    };
    let store_handle = registry_store(manager)?;
    let agent = {
        let store = lock_store(&store_handle)?;
        store
            .get_agent(service_name, agent_name)?
            .ok_or_else(agent_not_found)?
            .clone()
    };
    let snapshot = if active {
        runtime
            .as_ref()
            .ok_or_else(agent_stopped)?
            .runtime
            .snapshot()
            .await?
    } else {
        RuntimeSnapshot {
            state: "stopped",
            websocket_state: "disconnected",
            codex_process_state: "stopped",
            processing_request: false,
            failure_count: 0,
            error: None,
        }
    };
    let state = if active {
        snapshot.state
    } else {
        agent_registry_state_name(agent.state)
    };
    Ok(json!({
        "version": PROTOCOL_VERSION,
        "service_name": service_name,
        "agent_name": agent_name,
        "state": state,
        "websocket_state": if active { snapshot.websocket_state } else { "disconnected" },
        "codex_process_state": if active { snapshot.codex_process_state } else { "stopped" },
        "heartbeat_active": active,
        "model_config_initialized": agent.model_config_initialized,
        "processing_request": snapshot.processing_request,
        "failure_count": snapshot.failure_count,
        "error": snapshot.error,
        "changed_at": chrono::Utc::now().to_rfc3339(),
    }))
}

pub async fn stop_expired_agents(
    session: &Session,
    manager: &Arc<Mutex<AgentManager>>,
    service_name: &str,
) -> Vec<String> {
    let expired: Vec<String> = {
        let agents = match manager.lock() {
            Ok(agents) => agents,
            Err(_) => return Vec::new(),
        };
        let expired: Vec<String> = agents
            .agents
            .iter()
            .filter(|(_, agent)| agent.last_heartbeat.elapsed() >= heartbeat_timeout())
            .map(|(name, _)| name.clone())
            .collect();
        expired
    };

    let store_handle = match registry_store(manager) {
        Ok(store_handle) => store_handle,
        Err(_) => return expired,
    };
    let mut stopped = Vec::new();
    for name in expired {
        let initialization = {
            let mut agents = match manager.lock() {
                Ok(agents) => agents,
                Err(_) => break,
            };
            agents.initialization_lock(&name)
        };
        let _initialization_guard = initialization.lock().await;
        let runtime = match manager.lock() {
            Ok(mut agents) => {
                let expired = agents
                    .agents
                    .get(&name)
                    .is_some_and(|agent| agent.last_heartbeat.elapsed() >= heartbeat_timeout());
                if expired {
                    agents.agents.remove(&name)
                } else {
                    None
                }
            }
            Err(_) => None,
        };
        if let Some(runtime) = runtime {
            let _ = runtime.runtime.stop().await;
            stopped.push(name.clone());
        }
        if let Ok(store) = lock_store(&store_handle) {
            let _ = store.mark_agent_stopped(service_name, &name, AgentRegistryState::Stopped);
        }
        let _ = publish_agent_state(session, service_name, &name, "stopped").await;
    }
    stopped
}

pub async fn stop_all_agents(
    session: &Session,
    manager: &Arc<Mutex<AgentManager>>,
    service_name: &str,
) -> Result<(), Error> {
    let agents = {
        let mut manager = lock_manager(manager)?;
        manager.take_all_agents()
    };
    let store_handle = registry_store(manager)?;
    for (agent_name, managed) in agents {
        let stop_result = managed.runtime.stop().await;
        if let Ok(store) = lock_store(&store_handle) {
            let _ =
                store.mark_agent_stopped(service_name, &agent_name, AgentRegistryState::Stopped);
        }
        let _ = publish_agent_state(session, service_name, &agent_name, "stopped").await;
        stop_result?;
    }
    Ok(())
}

pub async fn delete_agent(
    session: &Session,
    manager: &Arc<Mutex<AgentManager>>,
    service_name: &str,
    agent_name: &str,
) -> Result<(), Error> {
    let store_handle = registry_store(manager)?;
    {
        let store = lock_store(&store_handle)?;
        if store.get_agent(service_name, agent_name)?.is_none() {
            return Ok(());
        }
    }

    stop_agent(
        session,
        manager,
        service_name,
        agent_name,
        AgentRegistryState::Stopped,
    )
    .await?;
    let store_handle = registry_store(manager)?;
    let result = lock_store(&store_handle)?.delete_agent(service_name, agent_name);
    result
}

async fn publish_agent_state(
    session: &Session,
    service_name: &str,
    agent_name: &str,
    state: &str,
) -> Result<(), Error> {
    let key = format!("codex-dds/v1/{service_name}/{agent_name}/status");
    session
        .put(
            key.as_str(),
            json!({
                "version": PROTOCOL_VERSION,
                "service_name": service_name,
                "agent_name": agent_name,
                "state": state,
                "changed_at": chrono::Utc::now().to_rfc3339(),
            })
            .to_string(),
        )
        .await
        .map_err(|error| Error::internal(error.to_string()))?;
    Ok(())
}

fn lifecycle_payload(query: &Query, model_required: bool) -> Result<LifecycleRequest, Error> {
    let payload = query_payload(query)?;
    let request: LifecycleRequest = serde_json::from_value(payload)
        .map_err(|_| invalid_request("invalid lifecycle request"))?;
    if model_required && request.model.is_none() {
        return Err(invalid_model_config("model configuration is required"));
    }
    Ok(request)
}

fn query_payload(query: &Query) -> Result<Value, Error> {
    let Some(payload) = query.payload() else {
        return Ok(json!({}));
    };
    let text = payload
        .try_to_string()
        .map_err(|_| invalid_request("query payload is not UTF-8 text"))?;
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&text).map_err(|_| invalid_request("query payload is not valid JSON"))
}

async fn reply_json(query: &Query, value: Value) {
    let payload = serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string());
    let _ = query.reply(query.key_expr().clone(), payload).await;
}

async fn reply_error(query: &Query, error: Error) {
    let rpc_error = match error {
        Error::Rpc(error) => error,
        error => RpcError::internal(error.to_string()),
    };
    let payload = json!({
        "version": PROTOCOL_VERSION,
        "ok": false,
        "error": rpc_error,
    });
    let _ = query.reply_err(payload.to_string()).await;
}

fn registry_store(manager: &Arc<Mutex<AgentManager>>) -> Result<Arc<Mutex<RegistryStore>>, Error> {
    manager
        .lock()
        .map(|manager| manager.store.clone())
        .map_err(|_| Error::internal("agent manager lock poisoned"))
}

fn lock_store(
    store: &Arc<Mutex<RegistryStore>>,
) -> Result<std::sync::MutexGuard<'_, RegistryStore>, Error> {
    store
        .lock()
        .map_err(|_| Error::internal("registry lock poisoned"))
}

fn lock_manager(
    manager: &Arc<Mutex<AgentManager>>,
) -> Result<std::sync::MutexGuard<'_, AgentManager>, Error> {
    manager
        .lock()
        .map_err(|_| Error::internal("agent manager lock poisoned"))
}

fn heartbeat_timeout() -> Duration {
    Duration::from_millis(HEARTBEAT_TIMEOUT_MS)
}

fn heartbeat_response() -> Value {
    json!({"version": PROTOCOL_VERSION, "ok": true})
}

fn detach_response() -> Value {
    json!({"version": PROTOCOL_VERSION, "state": "stopped"})
}

fn agent_registry_state_name(state: AgentRegistryState) -> &'static str {
    match state {
        AgentRegistryState::Registered => "registered",
        AgentRegistryState::Running => "running",
        AgentRegistryState::Stopped => "stopped",
        AgentRegistryState::Error => "error",
    }
}

fn agent_not_found() -> Error {
    Error::Rpc(RpcError::new("agent_not_found", "agent does not exist"))
}

fn agent_stopped() -> Error {
    Error::Rpc(RpcError::new("agent_stopped", "agent runtime is stopped"))
}

fn invalid_request(message: &str) -> Error {
    Error::Rpc(RpcError::new("invalid_request", message))
}

fn invalid_model_config(message: &str) -> Error {
    Error::Rpc(RpcError::new("invalid_model_config", message))
}

#[derive(Debug, Deserialize)]
struct LifecycleRequest {
    version: i64,
    model: Option<ModelConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_timeout_is_five_seconds() {
        assert_eq!(heartbeat_timeout(), Duration::from_secs(5));
    }
}

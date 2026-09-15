use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{
    sync::{mpsc, oneshot},
    time::{sleep_until, Instant},
};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::{
    error::{Error, RpcError},
    process::{ManagedProcess, ProcessSupervisor},
    protocol::{self, RpcResponse},
    websocket::{connect_and_handshake, parse_text_message, CodexWebSocket},
};

const MAX_RECOVERY_FAILURES: usize = 5;
const RECOVERY_DELAY: Duration = Duration::from_millis(200);
const RECOVERY_STABLE_AFTER: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub enum RuntimeCommand {
    Rpc {
        request_id: String,
        method: String,
        params: Value,
        reply: oneshot::Sender<RpcResponse>,
    },
    ReverseResponse {
        reverse_id: String,
        result: Option<Value>,
        error: Option<Value>,
        reply: oneshot::Sender<Result<(), RpcError>>,
    },
    Status {
        reply: oneshot::Sender<RuntimeSnapshot>,
    },
    Stop {
        reply: oneshot::Sender<()>,
    },
}

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    Notification {
        event_id: String,
        method: String,
        params: Value,
        received_at: String,
    },
    ReverseRequest {
        reverse_id: String,
        method: String,
        params: Value,
    },
    State {
        state: &'static str,
        websocket_state: &'static str,
        codex_process_state: &'static str,
        processing_request: bool,
        failure_count: usize,
        error: Option<String>,
    },
    Stopped {
        runtime_id: String,
        error: Option<String>,
        failure_count: usize,
    },
}

#[derive(Debug, Clone)]
pub struct RuntimeSnapshot {
    pub state: &'static str,
    pub websocket_state: &'static str,
    pub codex_process_state: &'static str,
    pub processing_request: bool,
    pub failure_count: usize,
    pub error: Option<String>,
}

struct PendingRpc {
    request_id: String,
    method: String,
    params: Value,
    reply: oneshot::Sender<RpcResponse>,
}

struct PendingReverse {
    reverse_id: String,
    original_id: Value,
    method: String,
    params: Value,
}

#[derive(Clone)]
pub struct AgentRuntime {
    id: String,
    command_tx: mpsc::UnboundedSender<RuntimeCommand>,
    log_path: PathBuf,
}

impl AgentRuntime {
    pub async fn start(
        binary: &Path,
        codex_home: &Path,
        log_path: &Path,
    ) -> Result<(Self, mpsc::Receiver<RuntimeEvent>), Error> {
        let (process, websocket_url) =
            ProcessSupervisor::start(binary, codex_home, log_path).await?;
        let websocket = connect_and_handshake(&websocket_url).await?;
        let restart = Some(RestartPaths {
            binary: binary.to_path_buf(),
            codex_home: codex_home.to_path_buf(),
            log_path: log_path.to_path_buf(),
        });
        let runtime_id = Uuid::new_v4().to_string();
        Self::spawn_worker(
            Some(process),
            Some(websocket),
            runtime_id,
            binary,
            codex_home,
            log_path,
            restart,
        )
    }

    pub async fn connect(
        websocket_url: &str,
        log_path: &Path,
    ) -> Result<(Self, mpsc::Receiver<RuntimeEvent>), Error> {
        let websocket = connect_and_handshake(websocket_url).await?;
        let binary = PathBuf::from("/nonexistent/codex-app-server");
        let codex_home = PathBuf::from("/nonexistent/codex-home");
        Self::spawn_worker(
            None,
            Some(websocket),
            Uuid::new_v4().to_string(),
            &binary,
            &codex_home,
            log_path,
            None,
        )
    }

    fn spawn_worker(
        process: Option<ManagedProcess>,
        websocket: Option<CodexWebSocket>,
        runtime_id: String,
        _binary: &Path,
        _codex_home: &Path,
        log_path: &Path,
        restart: Option<RestartPaths>,
    ) -> Result<(Self, mpsc::Receiver<RuntimeEvent>), Error> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::channel(4096);
        tokio::spawn(runtime_worker(
            process,
            websocket,
            runtime_id.clone(),
            command_rx,
            event_tx,
            restart,
        ));
        Ok((
            Self {
                id: runtime_id,
                command_tx,
                log_path: log_path.to_path_buf(),
            },
            event_rx,
        ))
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn stop(self) -> Result<(), Error> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self
            .command_tx
            .send(RuntimeCommand::Stop { reply: reply_tx });
        let _ = reply_rx.await;
        Ok(())
    }

    pub fn command_sender(&self) -> mpsc::UnboundedSender<RuntimeCommand> {
        self.command_tx.clone()
    }

    pub async fn snapshot(&self) -> Result<RuntimeSnapshot, Error> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send(RuntimeCommand::Status { reply: reply_tx })
            .map_err(|_| agent_stopped())?;
        reply_rx.await.map_err(|_| Error::Rpc(agent_stopped()))
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }
}

struct RestartPaths {
    binary: PathBuf,
    codex_home: PathBuf,
    log_path: PathBuf,
}

async fn runtime_worker(
    mut process: Option<ManagedProcess>,
    mut websocket: Option<CodexWebSocket>,
    runtime_id: String,
    mut commands: mpsc::UnboundedReceiver<RuntimeCommand>,
    events: mpsc::Sender<RuntimeEvent>,
    restart: Option<RestartPaths>,
) {
    let mut rpc_queue: VecDeque<PendingRpc> = VecDeque::new();
    let mut active_rpc: Option<PendingRpc> = None;
    let mut reverse_queue: VecDeque<PendingReverse> = VecDeque::new();
    let mut reverse_current: Option<PendingReverse> = None;
    let mut failure_count = 0usize;
    let mut fatal_error: Option<String> = None;
    let mut next_recovery: Option<Instant> = None;
    let mut stop_ack: Option<oneshot::Sender<()>> = None;
    let mut connected_at = Instant::now();

    publish_state(
        &events,
        "running",
        websocket_state(&websocket),
        process_state(&process),
        false,
        failure_count,
        None,
    )
    .await;

    loop {
        if websocket.is_some()
            && failure_count > 0
            && connected_at.elapsed() >= RECOVERY_STABLE_AFTER
        {
            failure_count = 0;
        }

        tokio::select! {
            biased;
            command = commands.recv() => {
                let Some(command) = command else { break };
                match command {
                    RuntimeCommand::Stop { reply } => {
                        fail_pending_rpcs(&mut active_rpc, &mut rpc_queue, "agent_stopped", "agent runtime stopped");
                        fail_reverse(&mut reverse_current, &mut reverse_queue, "agent_stopped", "agent runtime stopped", &mut websocket).await;
                        stop_ack = Some(reply);
                        break;
                    }
                    RuntimeCommand::Rpc { request_id, method, params, reply } => {
                        rpc_queue.push_back(PendingRpc { request_id, method, params, reply });
                    }
                    RuntimeCommand::ReverseResponse { reverse_id, result, error, reply } => {
                        let outcome = reverse_response(
                            reverse_id,
                            result,
                            error,
                            &mut reverse_current,
                            &mut reverse_queue,
                            &mut websocket,
                            &mut process,
                            restart.as_ref(),
                            &mut next_recovery,
                            &mut failure_count,
                        ).await;
                        let _ = reply.send(outcome);
                    }
                    RuntimeCommand::Status { reply } => {
                        let snapshot = RuntimeSnapshot {
                            state: if fatal_error.is_some() { "error" } else { "running" },
                            websocket_state: websocket_state(&websocket),
                            codex_process_state: process_state(&process),
                            processing_request: active_rpc.is_some(),
                            failure_count,
                            error: fatal_error.clone(),
                        };
                        let _ = reply.send(snapshot);
                    }
                }
            }
            message = async {
                match websocket.as_mut() {
                    Some(stream) => stream.next().await,
                    None => std::future::pending().await,
                }
            }, if websocket.is_some() => {
                let received = message
                    .transpose()
                    .ok()
                    .flatten()
                    .and_then(|message| parse_text_message(message).ok());
                let Some(value) = received else {
                    websocket = None;
                    failure_count += 1;
                    if let Some(active) = active_rpc.take() {
                        rpc_queue.push_front(active);
                    }
                    process = stop_process(process).await;
                    if restart.is_some() {
                        next_recovery = Some(Instant::now() + RECOVERY_DELAY);
                    } else {
                        fatal_error = Some("codex websocket closed".to_string());
                        break;
                    }
                    continue;
                };

                handle_server_message(
                    value,
                    &events,
                    &mut websocket,
                    &mut active_rpc,
                    &mut rpc_queue,
                    &mut reverse_queue,
                )
                .await;
            }
            _ = async {
                match next_recovery {
                    Some(deadline) => sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            }, if websocket.is_none() && restart.is_some() && next_recovery.is_some() => {
                next_recovery = None;
                if let Some(restart) = restart.as_ref() {
                    match recover(restart).await {
                        Ok((new_process, new_websocket)) => {
                            process = Some(new_process);
                            websocket = Some(new_websocket);
                            connected_at = Instant::now();
                        }
                        Err(error) => {
                            failure_count += 1;
                            if failure_count >= MAX_RECOVERY_FAILURES {
                                fatal_error = Some(error);
                                break;
                            }
                            next_recovery = Some(Instant::now() + RECOVERY_DELAY);
                        }
                    }
                }
            }
        }

        dispatch_rpc(
            &mut active_rpc,
            &mut rpc_queue,
            &mut websocket,
            &mut process,
            restart.as_ref(),
            &mut next_recovery,
            &mut failure_count,
        )
        .await;
        dispatch_reverse(
            &mut reverse_queue,
            &mut reverse_current,
            &events,
            &mut websocket,
        )
        .await;
        if failure_count >= MAX_RECOVERY_FAILURES && websocket.is_none() && restart.is_some() {
            fatal_error = Some("codex-app-server could not be recovered".to_string());
            break;
        }
        publish_state(
            &events,
            if fatal_error.is_some() {
                "error"
            } else {
                "running"
            },
            websocket_state(&websocket),
            process_state(&process),
            active_rpc.is_some(),
            failure_count,
            fatal_error.clone(),
        )
        .await;
    }

    if fatal_error.is_some() {
        fail_pending_rpcs(
            &mut active_rpc,
            &mut rpc_queue,
            "process_start_failed",
            "codex-app-server could not be recovered",
        );
        fail_reverse(
            &mut reverse_current,
            &mut reverse_queue,
            "process_start_failed",
            "codex-app-server could not be recovered",
            &mut websocket,
        )
        .await;
    }
    if let Some(mut stream) = websocket.take() {
        let _ = stream.close(None).await;
    }
    stop_process(process).await;
    let _ = events
        .send(RuntimeEvent::Stopped {
            runtime_id: runtime_id.clone(),
            error: fatal_error.clone(),
            failure_count,
        })
        .await;
    publish_state(
        &events,
        "stopped",
        "disconnected",
        "stopped",
        false,
        failure_count,
        fatal_error,
    )
    .await;
    if let Some(reply) = stop_ack {
        let _ = reply.send(());
    }
}

async fn handle_server_message(
    value: Value,
    events: &mpsc::Sender<RuntimeEvent>,
    websocket: &mut Option<CodexWebSocket>,
    active_rpc: &mut Option<PendingRpc>,
    rpc_queue: &mut VecDeque<PendingRpc>,
    reverse_queue: &mut VecDeque<PendingReverse>,
) {
    if value.get("method").is_some() {
        let method = value
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = value.get("params").cloned().unwrap_or(Value::Null);
        if value.get("id").is_some() {
            if !protocol::is_server_request_method(method) {
                send_reverse_error(
                    value["id"].clone(),
                    "unknown_method",
                    "method is not part of the fixed Codex protocol",
                    websocket,
                )
                .await;
                return;
            }
            if protocol::disabled_login_method(method) {
                send_reverse_error(
                    value["id"].clone(),
                    "disabled_by_policy",
                    "account login is disabled",
                    websocket,
                )
                .await;
                return;
            }
            let pending = PendingReverse {
                reverse_id: Uuid::new_v4().to_string(),
                original_id: value["id"].clone(),
                method: method.to_string(),
                params: params.clone(),
            };
            reverse_queue.push_back(pending);
        } else if protocol::is_server_notification_method(method) {
            let event = RuntimeEvent::Notification {
                event_id: Uuid::new_v4().to_string(),
                method: method.to_string(),
                params,
                received_at: chrono::Utc::now().to_rfc3339(),
            };
            if events.send(event).await.is_err() {
                fail_pending_rpcs(
                    active_rpc,
                    rpc_queue,
                    "internal_error",
                    "runtime event receiver closed",
                );
            }
        }
        return;
    }

    let response_id = value.get("id").cloned().unwrap_or(Value::Null);
    let matches_active = active_rpc
        .as_ref()
        .is_some_and(|pending| Value::String(pending.request_id.clone()) == response_id);
    if !matches_active {
        return;
    }
    if let Some(pending) = active_rpc.take() {
        let _ = pending
            .reply
            .send(response_from_codex(pending.request_id, value));
    }
}

async fn dispatch_rpc(
    active_rpc: &mut Option<PendingRpc>,
    rpc_queue: &mut VecDeque<PendingRpc>,
    websocket: &mut Option<CodexWebSocket>,
    process: &mut Option<ManagedProcess>,
    restart: Option<&RestartPaths>,
    next_recovery: &mut Option<Instant>,
    failure_count: &mut usize,
) {
    if active_rpc.is_some() || websocket.is_none() {
        return;
    }
    let Some(pending) = rpc_queue.pop_front() else {
        return;
    };
    let request = json!({
        "id": pending.request_id,
        "method": pending.method,
        "params": pending.params,
    });
    let Some(stream) = websocket.as_mut() else {
        rpc_queue.push_front(pending);
        return;
    };
    if stream
        .send(Message::Text(request.to_string().into()))
        .await
        .is_ok()
    {
        *active_rpc = Some(pending);
    } else {
        rpc_queue.push_front(pending);
        *websocket = None;
        *failure_count += 1;
        *process = stop_process(process.take()).await;
        if restart.is_some() {
            *next_recovery = Some(Instant::now() + RECOVERY_DELAY);
        }
    }
}

async fn dispatch_reverse(
    reverse_queue: &mut VecDeque<PendingReverse>,
    reverse_current: &mut Option<PendingReverse>,
    events: &mpsc::Sender<RuntimeEvent>,
    websocket: &mut Option<CodexWebSocket>,
) {
    if reverse_current.is_some() || websocket.is_none() {
        return;
    }
    let Some(pending) = reverse_queue.pop_front() else {
        return;
    };
    let event = RuntimeEvent::ReverseRequest {
        reverse_id: pending.reverse_id.clone(),
        method: pending.method.clone(),
        params: pending.params.clone(),
    };
    *reverse_current = Some(pending);
    if events.send(event).await.is_err() {
        let pending = reverse_current.take().unwrap();
        send_reverse_error(
            pending.original_id,
            "internal_error",
            "runtime event receiver closed",
            websocket,
        )
        .await;
    }
}

async fn reverse_response(
    reverse_id: String,
    result: Option<Value>,
    error: Option<Value>,
    reverse_current: &mut Option<PendingReverse>,
    _reverse_queue: &mut VecDeque<PendingReverse>,
    websocket: &mut Option<CodexWebSocket>,
    process: &mut Option<ManagedProcess>,
    restart: Option<&RestartPaths>,
    next_recovery: &mut Option<Instant>,
    failure_count: &mut usize,
) -> Result<(), RpcError> {
    let invalid_state = || RpcError::new("invalid_state", "no matching reverse request is active");
    let Some(current) = reverse_current.as_ref() else {
        return Err(invalid_state());
    };
    if current.reverse_id != reverse_id {
        return Err(RpcError::new(
            "not_found",
            "reverse_id does not match the active request",
        ));
    }
    let Some(pending) = reverse_current.take() else {
        return Err(invalid_state());
    };
    let response = if let Some(error) = error {
        json!({"id": pending.original_id, "error": error})
    } else if let Some(result) = result {
        json!({"id": pending.original_id, "result": result})
    } else {
        json!({
            "id": pending.original_id,
            "error": {"code": "invalid_request", "message": "result or error is required"}
        })
    };
    let sent = match websocket.as_mut() {
        Some(stream) => stream
            .send(Message::Text(response.to_string().into()))
            .await
            .is_ok(),
        None => false,
    };
    if !sent {
        *websocket = None;
        *failure_count += 1;
        *process = stop_process(process.take()).await;
        if restart.is_some() {
            *next_recovery = Some(Instant::now() + RECOVERY_DELAY);
        }
        return Err(RpcError::new("connection_closed", "codex websocket closed"));
    }
    Ok(())
}

async fn recover(restart: &RestartPaths) -> Result<(ManagedProcess, CodexWebSocket), String> {
    let (process, url) =
        ProcessSupervisor::start(&restart.binary, &restart.codex_home, &restart.log_path)
            .await
            .map_err(|error| error.to_string())?;
    let websocket = connect_and_handshake(&url)
        .await
        .map_err(|error| error.to_string())?;
    Ok((process, websocket))
}

async fn stop_process(process: Option<ManagedProcess>) -> Option<ManagedProcess> {
    if let Some(mut child) = process {
        let _ = child.stop().await;
    }
    None
}

async fn send_reverse_error(
    id: Value,
    code: &str,
    message: &str,
    websocket: &mut Option<CodexWebSocket>,
) {
    let response = json!({
        "id": id,
        "error": {"code": code, "message": message}
    });
    if let Some(stream) = websocket.as_mut() {
        let _ = stream
            .send(Message::Text(response.to_string().into()))
            .await;
    }
}

async fn fail_reverse(
    reverse_current: &mut Option<PendingReverse>,
    reverse_queue: &mut VecDeque<PendingReverse>,
    code: &str,
    message: &str,
    websocket: &mut Option<CodexWebSocket>,
) {
    if let Some(current) = reverse_current.take() {
        send_reverse_error(current.original_id, code, message, websocket).await;
    }
    for pending in reverse_queue.drain(..) {
        send_reverse_error(pending.original_id, code, message, websocket).await;
    }
}

fn fail_pending_rpcs(
    active_rpc: &mut Option<PendingRpc>,
    rpc_queue: &mut VecDeque<PendingRpc>,
    code: &'static str,
    message: &str,
) {
    if let Some(pending) = active_rpc.take() {
        let _ = pending.reply.send(RpcResponse::failure(
            pending.request_id.clone(),
            RpcError::new(code, message),
        ));
    }
    for pending in rpc_queue.drain(..) {
        let _ = pending.reply.send(RpcResponse::failure(
            pending.request_id.clone(),
            RpcError::new(code, message),
        ));
    }
}

fn response_from_codex(request_id: String, value: Value) -> RpcResponse {
    if let Some(error) = value.get("error") {
        return RpcResponse::failure(
            request_id,
            RpcError {
                code: error
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("codex_error")
                    .to_owned(),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("codex error")
                    .to_string(),
                data: error.get("data").cloned(),
            },
        );
    }
    RpcResponse::success(
        request_id,
        value.get("result").cloned().unwrap_or(Value::Null),
    )
}

fn websocket_state(websocket: &Option<CodexWebSocket>) -> &'static str {
    if websocket.is_some() {
        "connected"
    } else {
        "disconnected"
    }
}

fn process_state(process: &Option<ManagedProcess>) -> &'static str {
    if process.is_some() {
        "running"
    } else {
        "stopped"
    }
}

fn agent_stopped() -> RpcError {
    RpcError::new("agent_stopped", "agent runtime is stopped")
}

async fn publish_state(
    events: &mpsc::Sender<RuntimeEvent>,
    state: &'static str,
    websocket_state: &'static str,
    codex_process_state: &'static str,
    processing_request: bool,
    failure_count: usize,
    error: Option<String>,
) {
    let _ = events
        .send(RuntimeEvent::State {
            state,
            websocket_state,
            codex_process_state,
            processing_request,
            failure_count,
            error,
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_error_code_message_and_data_are_preserved() {
        let response = response_from_codex(
            "request-1".to_string(),
            json!({
                "error": {
                    "code": "mock_custom_error",
                    "message": "mock Codex error",
                    "data": {"source": "mock"}
                }
            }),
        );

        assert!(!response.ok);
        let error = response.error.unwrap();
        assert_eq!(error.code, "mock_custom_error");
        assert_eq!(error.message, "mock Codex error");
        assert_eq!(error.data, Some(json!({"source": "mock"})));
    }
}

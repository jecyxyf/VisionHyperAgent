//! A bounded single-owner transport actor, with no process management or replay queue.
use std::collections::HashMap;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::{timeout, Instant};
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    http::{header::AUTHORIZATION, HeaderValue},
};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::{
    AgentError, AgentEvent, AgentState, ConnectionPhase, Result, RpcId, ServerRequest,
    TransportConfig,
};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub(crate) type Reply = oneshot::Sender<Result<Value>>;

pub(crate) struct Request {
    pub id: i64,
    pub connection_id: u64,
    pub method: &'static str,
    pub params: Value,
    pub deadline: Instant,
    pub reply: Reply,
}

pub(crate) enum Command {
    Request(Request),
    Respond {
        request: ServerRequest,
        result: Result<Value>,
        reply: Reply,
    },
}

impl Command {
    fn reject(self) {
        let reply = match self {
            Self::Request(req) => req.reply,
            Self::Respond { reply, .. } => reply,
        };
        let _ = reply.send(Err(AgentError::Disconnected));
    }
}

pub(crate) struct Actor {
    pub config: TransportConfig,
    pub commands: mpsc::Receiver<Command>,
    pub stop: watch::Receiver<bool>,
    pub state: watch::Sender<AgentState>,
    pub events: broadcast::Sender<AgentEvent>,
}

impl Actor {
    fn phase(&self, phase: ConnectionPhase) {
        let state = AgentState {
            phase,
            connection_id: self.state.borrow().connection_id,
        };
        self.state.send_replace(state.clone());
        let _ = self.events.send(AgentEvent::Status { state });
        log::debug!("Codex connection phase={phase:?}");
    }

    pub async fn run(mut self) {
        let mut attempts = 0;
        loop {
            if *self.stop.borrow() {
                break;
            }
            let config = WebSocketConfig::default()
                .max_message_size(Some(8 * 1024 * 1024))
                .max_frame_size(Some(8 * 1024 * 1024));
            let Ok(mut request) = self.config.websocket_url.as_str().into_client_request() else {
                self.phase(ConnectionPhase::Error);
                return;
            };
            if let Some(token) = &self.config.auth_token {
                let Ok(mut value) = HeaderValue::from_str(&format!("Bearer {token}")) else {
                    self.phase(ConnectionPhase::Error);
                    return;
                };
                value.set_sensitive(true);
                request.headers_mut().insert(AUTHORIZATION, value);
            }
            let connection = tokio::select! {
                biased;
                _ = self.stop.changed() => break,
                result = timeout(self.config.connect_timeout, connect_async_with_config(request, Some(config), false)) => result,
            };
            if let Ok(Ok((mut socket, _))) = connection {
                self.phase(ConnectionPhase::Initializing);
                let initialized = tokio::select! {
                    biased;
                    _ = self.stop.changed() => false,
                    result = timeout(self.config.connect_timeout, initialize(&mut socket, &self.events, self.config.experimental_api)) => matches!(result, Ok(Ok(()))),
                };
                if initialized && !*self.stop.borrow() {
                    let id = self.state.borrow().connection_id + 1;
                    self.state.send_modify(|state| state.connection_id = id);
                    self.phase(ConnectionPhase::Ready);
                    attempts = 0;
                    let result = self.session(&mut socket, id).await;
                    log::debug!("Codex connection ended; clean={}", result.is_ok());
                }
                if !*self.stop.borrow() {
                    self.phase(ConnectionPhase::Reconnecting);
                }
                let _ = timeout(Duration::from_millis(300), socket.close(None)).await;
            }
            while let Ok(command) = self.commands.try_recv() {
                command.reject();
            }
            if *self.stop.borrow() {
                break;
            }
            attempts += 1;
            if attempts > self.config.max_reconnect_attempts {
                self.phase(ConnectionPhase::Error);
                return;
            }
            self.phase(ConnectionPhase::Reconnecting);
            tokio::select! {
                _ = self.stop.changed() => break,
                _ = tokio::time::sleep(self.config.reconnect_interval) => {},
            }
        }
        self.phase(ConnectionPhase::Disconnected);
    }

    async fn session(&mut self, socket: &mut Socket, connection_id: u64) -> Result<()> {
        let mut requests: HashMap<i64, Request> = HashMap::new();
        let mut approvals: HashMap<RpcId, Instant> = HashMap::new();
        let mut tick = tokio::time::interval(Duration::from_millis(50));
        let result = async {
            loop {
                tokio::select! {
                    _ = self.stop.changed() => return Ok(()),
                    _ = tick.tick() => {
                        let now = Instant::now();
                        let expired: Vec<_> = requests.iter().filter(|(_, req)| req.deadline <= now || req.reply.is_closed()).map(|(id, _)| *id).collect();
                        for id in expired {
                            if let Some(req) = requests.remove(&id) { let _ = req.reply.send(Err(AgentError::Timeout)); }
                        }
                        let expired: Vec<_> = approvals.iter().filter(|(_, deadline)| **deadline <= now).map(|(id, _)| id.clone()).collect();
                        for id in expired {
                            approvals.remove(&id);
                            write(socket, json!({"id": id, "error": {"code": -32000, "message": "User response timed out; request denied"}})).await?;
                            let _ = self.events.send(AgentEvent::ServerRequestExpired { id, connection_id });
                        }
                    }
                    command = self.commands.recv() => {
                        let Some(command) = command else { return Ok(()); };
                        match command {
                            Command::Request(req) => {
                                if req.connection_id != connection_id { let _ = req.reply.send(Err(AgentError::Disconnected)); continue; }
                                if req.reply.is_closed() || req.deadline <= Instant::now() { let _ = req.reply.send(Err(AgentError::Timeout)); continue; }
                                if requests.len() >= 128 { let _ = req.reply.send(Err(AgentError::Busy)); continue; }
                                let payload = json!({"id": req.id, "method": req.method, "params": req.params});
                                log::debug!("Codex request method={}, id={}", req.method, req.id);
                                requests.insert(req.id, req);
                                write(socket, payload).await?;
                            }
                            Command::Respond { request, result, reply } => {
                                if request.connection_id != connection_id || approvals.remove(&request.id).is_none() {
                                    let _ = reply.send(Err(AgentError::StaleRequest)); continue;
                                }
                                let payload = match result {
                                    Ok(value) => json!({"id": request.id, "result": value}),
                                    Err(_) => json!({"id": request.id, "error": {"code": -32602, "message": "Request denied or invalid response"}}),
                                };
                                let result = write(socket, payload).await;
                                let _ = reply.send(result.clone().map(|()| Value::Null));
                                result?;
                            }
                        }
                    }
                    frame = socket.next() => {
                        match frame {
                            Some(Ok(Message::Text(text))) => {
                                let value: Value = serde_json::from_str(&text).map_err(|_| AgentError::Protocol)?;
                                if let Some(method) = value.get("method").and_then(Value::as_str) {
                                    let params = value.get("params").cloned().unwrap_or(Value::Null);
                                    if let Some(id) = value.get("id") {
                                        let id: RpcId = serde_json::from_value(id.clone()).map_err(|_| AgentError::Protocol)?;
                                        if !is_server_method(method) || approvals.len() >= 64 || approvals.contains_key(&id) {
                                            write(socket, json!({"id": id, "error": {"code": -32601, "message": "Unsupported or duplicate client interaction"}})).await?;
                                            continue;
                                        }
                                        let request = ServerRequest { id: id.clone(), connection_id, method: method.to_owned(), params };
                                        if self.events.send(AgentEvent::ServerRequest { request }).is_err() {
                                            write(socket, json!({"id": id, "error": {"code": -32000, "message": "No interaction handler available"}})).await?;
                                        } else { approvals.insert(id, Instant::now() + self.config.approval_timeout); }
                                    } else {
                                        let _ = self.events.send(AgentEvent::Notification { method: method.to_owned(), params });
                                    }
                                } else if let Some(id) = value.get("id").and_then(Value::as_i64) {
                                    if let Some(req) = requests.remove(&id) { let _ = req.reply.send(response(value)); }
                                } else { return Err(AgentError::Protocol); }
                            }
                            Some(Ok(Message::Ping(bytes))) => {
                                timeout(Duration::from_secs(2), socket.send(Message::Pong(bytes))).await.map_err(|_| AgentError::Timeout)?.map_err(|_| AgentError::Transport)?;
                            }
                            Some(Ok(Message::Pong(_))) => {},
                            Some(Ok(Message::Close(_))) | None => return Err(AgentError::Disconnected),
                            _ => return Err(AgentError::Protocol),
                        }
                    }
                }
            }
        }.await;
        for (_, req) in requests {
            let _ = req.reply.send(Err(AgentError::Disconnected));
        }
        for (id, _) in approvals {
            let _ = self
                .events
                .send(AgentEvent::ServerRequestExpired { id, connection_id });
        }
        result
    }
}

fn is_server_method(method: &str) -> bool {
    matches!(
        method,
        "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/tool/requestUserInput"
    )
}

async fn write(socket: &mut Socket, value: Value) -> Result<()> {
    timeout(
        Duration::from_secs(2),
        socket.send(Message::Text(value.to_string().into())),
    )
    .await
    .map_err(|_| AgentError::Timeout)?
    .map_err(|_| AgentError::Transport)
}

fn response(value: Value) -> Result<Value> {
    if let Some(error) = value.get("error") {
        return Err(AgentError::Rpc {
            code: error
                .get("code")
                .and_then(Value::as_i64)
                .ok_or(AgentError::Protocol)?,
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Codex request failed")
                .to_owned(),
        });
    }
    value.get("result").cloned().ok_or(AgentError::Protocol)
}

async fn initialize(
    socket: &mut Socket,
    events: &broadcast::Sender<AgentEvent>,
    experimental: bool,
) -> Result<()> {
    write(socket, json!({"id": 0, "method": "initialize", "params": {
        "clientInfo": {"name": "vision_hyper_agent", "title": "VisionHyperAgent", "version": env!("CARGO_PKG_VERSION")},
        "capabilities": {"experimentalApi": experimental}
    }})).await?;
    while let Some(frame) = socket.next().await {
        match frame.map_err(|_| AgentError::Transport)? {
            Message::Text(text) => {
                let value: Value = serde_json::from_str(&text).map_err(|_| AgentError::Protocol)?;
                if value.get("id").and_then(Value::as_i64) == Some(0)
                    && value.get("method").is_none()
                {
                    response(value)?;
                    return write(socket, json!({"method": "initialized"})).await;
                }
                if let Some(method) = value.get("method").and_then(Value::as_str) {
                    if let Some(id) = value.get("id") {
                        write(socket, json!({"id": id, "error": {"code": -32601, "message": "Client is initializing"}})).await?;
                    } else {
                        let _ = events.send(AgentEvent::Notification {
                            method: method.to_owned(),
                            params: value.get("params").cloned().unwrap_or(Value::Null),
                        });
                    }
                } else {
                    return Err(AgentError::Protocol);
                }
            }
            Message::Ping(bytes) => {
                socket
                    .send(Message::Pong(bytes))
                    .await
                    .map_err(|_| AgentError::Transport)?;
            }
            Message::Pong(_) => {}
            _ => return Err(AgentError::Protocol),
        }
    }
    Err(AgentError::Disconnected)
}

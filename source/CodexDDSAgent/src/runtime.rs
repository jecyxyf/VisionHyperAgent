use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::{
    error::{Error, RpcError},
    process::{ManagedProcess, ProcessSupervisor},
    protocol::RpcResponse,
    websocket::{connect_and_handshake, parse_text_message},
};

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
        result: Value,
    },
    Stop,
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
}

pub struct AgentRuntime {
    command_tx: mpsc::Sender<RuntimeCommand>,
    process: ManagedProcess,
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
        let (command_tx, command_rx) = mpsc::channel(1024);
        let (event_tx, event_rx) = mpsc::channel(4096);
        tokio::spawn(runtime_worker(websocket, command_rx, event_tx));
        Ok((
            Self {
                command_tx,
                process,
                log_path: log_path.to_path_buf(),
            },
            event_rx,
        ))
    }

    pub async fn stop(mut self) -> Result<(), Error> {
        let _ = self.command_tx.send(RuntimeCommand::Stop).await;
        self.process.stop().await
    }

    pub fn command_sender(&self) -> mpsc::Sender<RuntimeCommand> {
        self.command_tx.clone()
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }
}

async fn runtime_worker(
    mut websocket: crate::websocket::CodexWebSocket,
    mut commands: mpsc::Receiver<RuntimeCommand>,
    events: mpsc::Sender<RuntimeEvent>,
) {
    let mut reverse_ids: HashMap<String, Value> = HashMap::new();
    while let Some(command) = commands.recv().await {
        match command {
            RuntimeCommand::Stop => break,
            RuntimeCommand::ReverseResponse { reverse_id, result } => {
                let Some(original_id) = reverse_ids.remove(&reverse_id) else {
                    continue;
                };
                let response = json!({"id": original_id, "result": result});
                if websocket
                    .send(Message::Text(response.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            RuntimeCommand::Rpc {
                request_id,
                method,
                params,
                reply,
            } => {
                let request = json!({
                    "id": request_id,
                    "method": method,
                    "params": params,
                });
                if websocket
                    .send(Message::Text(request.to_string().into()))
                    .await
                    .is_err()
                {
                    let _ = reply.send(RpcResponse::failure(
                        request_id,
                        RpcError::new("connection_closed", "codex websocket closed"),
                    ));
                    break;
                }

                let response = loop {
                    let Some(Ok(message)) = websocket.next().await else {
                        break RpcResponse::failure(
                            request_id.clone(),
                            RpcError::new("connection_closed", "codex websocket closed"),
                        );
                    };
                    let Ok(value) = parse_text_message(message) else {
                        continue;
                    };
                    if value.get("method").is_some() {
                        if let Err(error) =
                            forward_server_message(value, &events, &mut reverse_ids).await
                        {
                            let error = match error {
                                Error::Rpc(error) => error,
                                error => RpcError::internal(error.to_string()),
                            };
                            break RpcResponse::failure(request_id.clone(), error);
                        }
                        continue;
                    }
                    if value.get("id").and_then(Value::as_str) == Some(request_id.as_str()) {
                        break response_from_codex(request_id.clone(), value);
                    }
                };
                let _ = reply.send(response);
            }
        }
    }
    let _ = websocket.close(None).await;
}

async fn forward_server_message(
    value: Value,
    events: &mpsc::Sender<RuntimeEvent>,
    reverse_ids: &mut HashMap<String, Value>,
) -> Result<(), Error> {
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let params = value.get("params").cloned().unwrap_or(Value::Null);
    if value.get("id").is_some() {
        let reverse_id = Uuid::new_v4().to_string();
        reverse_ids.insert(reverse_id.clone(), value["id"].clone());
        events
            .send(RuntimeEvent::ReverseRequest {
                reverse_id,
                method,
                params,
            })
            .await
            .map_err(|_| Error::internal("runtime event receiver closed"))
    } else {
        events
            .send(RuntimeEvent::Notification {
                event_id: Uuid::new_v4().to_string(),
                method,
                params,
                received_at: chrono::Utc::now().to_rfc3339(),
            })
            .await
            .map_err(|_| Error::internal("runtime event receiver closed"))
    }
}

fn response_from_codex(request_id: String, value: Value) -> RpcResponse {
    if let Some(error) = value.get("error") {
        return RpcResponse::failure(
            request_id,
            RpcError {
                code: "codex_error",
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

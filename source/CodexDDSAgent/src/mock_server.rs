use std::{env, io::Write, path::PathBuf, process::exit, time::Duration};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

type MockWebSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    writeln!(std::io::stderr(), "mock listening ws://{address}")?;
    std::io::stderr().flush()?;

    let (stream, _) = listener.accept().await?;
    let mut websocket = tokio_tungstenite::accept_async(stream).await?;
    let initialize = read_json(&mut websocket).await?;
    send_json(
        &mut websocket,
        json!({
            "id": initialize.get("id").cloned().unwrap_or_else(|| json!("initialize")),
            "result": {"serverInfo": {"name": "mock-codex-server"}}
        }),
    )
    .await?;

    let initialized = read_json(&mut websocket).await?;
    if initialized.get("method").and_then(Value::as_str) != Some("initialized") {
        return Err("expected initialized notification".into());
    }

    if flag("--crash-always") {
        tokio::time::sleep(Duration::from_millis(50)).await;
        exit(2);
    }

    if should_crash_once() {
        tokio::time::sleep(Duration::from_millis(100)).await;
        exit(2);
    }

    if flag("--notify-after-handshake") {
        send_json(
            &mut websocket,
            json!({"method": "turn/started", "params": {"reason": "mock"}}),
        )
        .await?;
    }

    let mut reverse_ids = std::collections::HashSet::new();
    for index in 0..value("--reverse-count")?.unwrap_or(0) {
        let id = format!("mock-reverse-{index}");
        reverse_ids.insert(id.clone());
        send_json(
            &mut websocket,
            json!({
                "id": id,
                "method": "currentTime/read",
                "params": {}
            }),
        )
        .await?;
    }

    let delay_ms = value("--delay-ms")?.unwrap_or(0);
    let rpc_error_code = value_string("--rpc-error-code");
    while let Some(message) = websocket.next().await {
        let request = match message? {
            Message::Text(text) => serde_json::from_str::<Value>(&text)?,
            Message::Close(_) => break,
            _ => continue,
        };
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
        let response_id = request.get("id").cloned().unwrap_or_else(|| json!(null));
        if request.get("method").is_none() && reverse_ids.remove(response_id.as_str().unwrap_or(""))
        {
            let acknowledged = if request.get("error").is_some() {
                json!({
                    "method": "turn/started",
                    "params": {
                        "reason": "reverse-error",
                        "reverse_id": response_id,
                        "error": request.get("error").cloned().unwrap_or(Value::Null)
                    }
                })
            } else {
                json!({
                    "method": "turn/started",
                    "params": {
                        "reason": "reverse-response",
                        "reverse_id": response_id,
                        "result": request.get("result").cloned().unwrap_or(Value::Null)
                    }
                })
            };
            send_json(&mut websocket, acknowledged).await?;
            continue;
        }

        let response = if let Some(code) = rpc_error_code.as_deref() {
            json!({
                "id": response_id,
                "error": {
                    "code": code,
                    "message": "mock Codex error",
                    "data": {"source": "mock"}
                }
            })
        } else {
            json!({
                "id": request.get("id").cloned().unwrap_or_else(|| json!(null)),
                "result": {
                    "method": request.get("method").cloned().unwrap_or(Value::Null),
                    "params": request.get("params").cloned().unwrap_or(Value::Null)
                }
            })
        };
        send_json(&mut websocket, response).await?;
    }
    Ok(())
}

async fn read_json(websocket: &mut MockWebSocket) -> Result<Value, Box<dyn std::error::Error>> {
    let message = websocket.next().await.ok_or("websocket closed")??;
    match message {
        Message::Text(text) => Ok(serde_json::from_str(&text)?),
        _ => Err("expected text websocket frame".into()),
    }
}

async fn send_json(
    websocket: &mut MockWebSocket,
    value: Value,
) -> Result<(), Box<dyn std::error::Error>> {
    websocket
        .send(Message::Text(value.to_string().into()))
        .await?;
    Ok(())
}

fn flag(name: &str) -> bool {
    env::args().any(|argument| argument == name)
}

fn value(name: &str) -> Result<Option<u64>, Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument != name {
            continue;
        }
        let raw = arguments
            .next()
            .ok_or_else(|| format!("{name} requires a value"))?;
        return Ok(Some(raw.parse()?));
    }
    Ok(None)
}

fn value_string(name: &str) -> Option<String> {
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments.next();
        }
    }
    None
}

fn should_crash_once() -> bool {
    let Some(path) = value_path("--crash-once-file") else {
        return false;
    };
    if path.exists() {
        return false;
    }
    std::fs::write(&path, b"crashed").is_ok()
}

fn value_path(name: &str) -> Option<PathBuf> {
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments.next().map(PathBuf::from);
        }
    }
    None
}

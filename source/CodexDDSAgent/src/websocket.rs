use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::error::Error;

pub type CodexWebSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub async fn connect_and_handshake(url: &str) -> Result<CodexWebSocket, Error> {
    let (mut websocket, _) = connect_async(url).await.map_err(|err| {
        Error::Rpc(crate::error::RpcError::new(
            "connection_closed",
            format!("failed to connect codex websocket: {err}"),
        ))
    })?;

    let initialize = json!({
        "id": "codex-dds-initialize",
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "CodexDDSAgent",
                "title": "VisionHyperAgent Codex DDS Agent",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "experimentalApi": true,
                "requestAttestation": true,
                "mcpServerOpenaiFormElicitation": true,
                "optOutNotificationMethods": null,
                "extensions": null
            }
        }
    });
    websocket
        .send(Message::Text(initialize.to_string().into()))
        .await
        .map_err(|err| Error::Rpc(websocket_error(err)))?;

    loop {
        let message = websocket
            .next()
            .await
            .ok_or_else(|| Error::Rpc(websocket_closed()))?
            .map_err(|err| Error::Rpc(websocket_error(err)))?;
        let Value::Object(fields) = parse_text_message(message)? else {
            continue;
        };
        if fields.get("id").and_then(Value::as_str) == Some("codex-dds-initialize") {
            if fields.contains_key("error") {
                return Err(Error::Rpc(crate::error::RpcError::new(
                    "codex_error",
                    "initialize failed",
                )));
            }
            break;
        }
    }

    websocket
        .send(Message::Text(
            json!({"method": "initialized"}).to_string().into(),
        ))
        .await
        .map_err(|err| Error::Rpc(websocket_error(err)))?;
    Ok(websocket)
}

pub fn parse_text_message(message: Message) -> Result<Value, Error> {
    match message {
        Message::Text(text) => Ok(serde_json::from_str(&text)?),
        Message::Close(_) => Err(Error::Rpc(websocket_closed())),
        _ => Err(Error::internal("unexpected websocket frame")),
    }
}

fn websocket_closed() -> crate::error::RpcError {
    crate::error::RpcError::new("connection_closed", "codex websocket closed")
}

fn websocket_error(err: tokio_tungstenite::tungstenite::Error) -> crate::error::RpcError {
    crate::error::RpcError::new("connection_closed", err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn performs_initialize_and_initialized_handshake() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let initialize = ws.next().await.unwrap().unwrap();
            let initialize = match initialize {
                Message::Text(text) => serde_json::from_str::<Value>(&text).unwrap(),
                _ => panic!("expected initialize text frame"),
            };
            ws.send(Message::Text(
                json!({"id": initialize["id"], "result": {"serverInfo": {}}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            let initialized = ws.next().await.unwrap().unwrap();
            assert!(
                matches!(initialized, Message::Text(ref text) if text.as_str() == "{\"method\":\"initialized\"}")
            );
        });

        let mut client = connect_and_handshake(&format!("ws://{address}"))
            .await
            .unwrap();
        client.close(None).await.unwrap();
        server.await.unwrap();
    }
}

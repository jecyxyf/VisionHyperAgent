#![allow(dead_code)]
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use vha_codex_agent::{AgentConfig, AgentEvent, CodexAgent, ConnectionPhase, ServerRequest};

pub type Socket = WebSocketStream<TcpStream>;
pub const DEADLINE: Duration = Duration::from_secs(3);

pub async fn fixture() -> (TcpListener, CodexAgent) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let agent = CodexAgent::new(config(&listener));
    (listener, agent)
}

pub fn config(listener: &TcpListener) -> AgentConfig {
    AgentConfig {
        websocket_url: format!("ws://{}", listener.local_addr().unwrap()),
        connect_timeout: Duration::from_millis(500),
        request_timeout: Duration::from_millis(600),
        reconnect_interval: Duration::from_millis(20),
        max_reconnect_attempts: 0,
        approval_timeout: Duration::from_millis(200),
        ..Default::default()
    }
}

pub async fn accept(listener: &TcpListener) -> Socket {
    let (tcp, _) = timeout(DEADLINE, listener.accept()).await.unwrap().unwrap();
    timeout(DEADLINE, accept_async(tcp)).await.unwrap().unwrap()
}

pub async fn send(socket: &mut Socket, value: Value) {
    timeout(
        DEADLINE,
        socket.send(Message::Text(value.to_string().into())),
    )
    .await
    .unwrap()
    .unwrap();
}

pub async fn receive(socket: &mut Socket) -> Value {
    loop {
        let frame = timeout(DEADLINE, socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        match frame {
            Message::Text(text) => return serde_json::from_str(&text).unwrap(),
            Message::Ping(data) => {
                socket.send(Message::Pong(data)).await.unwrap();
            }
            Message::Pong(_) => {}
            other => panic!("unexpected frame: {other:?}"),
        }
    }
}

pub async fn handshake(socket: &mut Socket) {
    let init = receive(socket).await;
    assert_eq!(init["method"], "initialize");
    assert_eq!(init["params"]["clientInfo"]["name"], "vision_hyper_agent");
    assert_eq!(init["params"]["capabilities"]["experimentalApi"], false);
    send(
        socket,
        json!({"id":init["id"], "result":{"userAgent":"fixture"}}),
    )
    .await;
    assert_eq!(receive(socket).await, json!({"method":"initialized"}));
}

pub async fn interaction(
    events: &mut tokio::sync::broadcast::Receiver<AgentEvent>,
) -> ServerRequest {
    timeout(DEADLINE, async {
        loop {
            if let AgentEvent::ServerRequest { request } = events.recv().await.unwrap() {
                return request;
            }
        }
    })
    .await
    .unwrap()
}

pub async fn phase(agent: &CodexAgent, phase: ConnectionPhase) {
    timeout(DEADLINE, async {
        while agent.status().phase != phase {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

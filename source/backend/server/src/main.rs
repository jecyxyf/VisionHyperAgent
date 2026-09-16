//! VisionHyperAgent 本地服务器入口。
//!
//! 启动后监听 127.0.0.1:8420，提供：
//! - 静态文件服务（打包后的前端产物）
//! - WebSocket 端点（实时通信）
//! - REST API（项目/模型/配置）
//!
//! 当所有 WebSocket 连接断开且无训练任务时，5 秒后自动退出。

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tokio::sync::watch;

#[derive(Clone)]
struct AppState {
    active_connections: Arc<AtomicUsize>,
    shutdown_tx: watch::Sender<bool>,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        active_connections: Arc::new(AtomicUsize::new(0)),
        shutdown_tx: watch::channel(false).0,
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/health", get(health))
        .fallback_service(get(static_handler))
        .with_state(state.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], 8420));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("VisionHyperAgent running at http://{addr}");

    let shutdown_rx = state.shutdown_tx.subscribe();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_rx))
        .await
        .unwrap();
}

async fn health() -> &'static str {
    "ok"
}

async fn static_handler() -> impl IntoResponse {
    // TODO: serve frontend/dist
    "VisionHyperAgent"
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    state.active_connections.fetch_add(1, Ordering::SeqCst);
    println!("client connected, total: {}", state.active_connections.load(Ordering::SeqCst));

    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            if text.as_str() == r#"{"type":"ping"}"# {
                let _ = socket.send(Message::Text(r#"{"type":"pong"}"#.into())).await;
            }
        }
    }

    let count = state.active_connections.fetch_sub(1, Ordering::SeqCst) - 1;
    println!("client disconnected, remaining: {count}");

    if count == 0 {
        // 所有连接断开，启动 5 秒宽限期后退出
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if state.active_connections.load(Ordering::SeqCst) == 0 {
                println!("all clients disconnected, shutting down");
                let _ = state.shutdown_tx.send(true);
            }
        });
    }
}

async fn shutdown_signal(mut shutdown_rx: watch::Receiver<bool>) {
    loop {
        if *shutdown_rx.borrow_and_update() {
            return;
        }
        if shutdown_rx.changed().await.is_err() {
            return;
        }
    }
}

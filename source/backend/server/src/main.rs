//! VisionHyperAgent entry point.
//!
//! Process model:
//! - Main thread: system tray event loop
//! - Background thread: tokio async runtime (HTTP + WebSocket server)
//! - Child process: Codex App Server (auto-killed when parent exits)
//!
//! Exit strategy:
//! - Tray icon right-click "退出" is the ONLY exit trigger
//! - Closing the browser tab does NOT exit (browser is just a view)
//! - On exit: kill Codex child, stop server, drop tray, process exits

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::RustEmbed;
use tokio::sync::watch;

#[derive(RustEmbed)]
#[folder = "../../frontend/dist/"]
struct FrontendAssets;

#[derive(Clone)]
struct AppState {
    active_connections: Arc<AtomicUsize>,
    shutdown_tx: watch::Sender<bool>,
}

fn main() {
    let url = "http://127.0.0.1:8420";

    // Create tray icon
    let (_tray_handle, tray_rx) = match tray::create_tray(url) {
        Ok((h, rx)) => (Some(h), rx),
        Err(e) => {
            eprintln!("Warning: tray icon unavailable: {}", e);
            (None, mpsc_channel_fallback())
        }
    };

    // Auto-open browser on first launch
    let _ = webbrowser::open(url);

    // Start server in background thread
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server_shutdown = shutdown_rx.clone();
    let connections = Arc::new(AtomicUsize::new(0));

    let server_state = AppState {
        active_connections: connections.clone(),
        shutdown_tx: shutdown_tx.clone(),
    };

    let server_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime");

        rt.block_on(async move {
            if let Err(e) = run_server(server_state, server_shutdown).await {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }
        });
    });

    // Main thread: wait for tray Exit event
    println!("VisionHyperAgent running at {}", url);
    println!("Close via tray icon -> 退出");

    for event in tray_rx.iter() {
        match event {
            tray::TrayEvent::OpenBrowser => {
                // Already handled in tray callback
            }
            tray::TrayEvent::Exit => {
                println!("Shutting down...");
                let _ = shutdown_tx.send(true);
                break;
            }
        }
    }

    // Wait for server to finish
    let _ = server_thread.join();

    // TODO: kill Codex child process here
    // codex_process.shutdown(3).await;

    println!("Goodbye");
}

async fn run_server(state: AppState, mut shutdown_rx: watch::Receiver<bool>) -> Result<(), String> {
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/health", get(health))
        .fallback(static_handler)
        .with_state(state.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], 8420));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("port {} in use: {}", addr, e))?;

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            loop {
                if *shutdown_rx.borrow_and_update() {
                    return;
                }
                if shutdown_rx.changed().await.is_err() {
                    return;
                }
            }
        })
        .await
        .map_err(|e| e.to_string())
}

async fn health() -> &'static str {
    "ok"
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match FrontendAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404").into_response(),
    }
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

    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            if text.as_str() == r#"{"type":"ping"}"# {
                let _ = socket.send(Message::Text(r#"{"type":"pong"}"#.into())).await;
            }
        }
    }

    state.active_connections.fetch_sub(1, Ordering::SeqCst);
}

fn mpsc_channel_fallback() -> std::sync::mpsc::Receiver<tray::TrayEvent> {
    let (_, rx) = std::sync::mpsc::channel();
    rx
}

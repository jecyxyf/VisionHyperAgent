use std::net::SocketAddr;
use std::sync::mpsc;
use std::thread::JoinHandle;

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::RustEmbed;

use crate::shutdown::{ShutdownController, ShutdownSignal};

#[derive(RustEmbed)]
#[folder = "../../frontend/dist/"]
struct FrontendAssets;

/// A running local HTTP server owned by the host process.
pub struct ServerHandle {
    shutdown: ShutdownController,
    thread: Option<JoinHandle<()>>,
}

impl ServerHandle {
    /// Requests graceful shutdown and waits until the server thread finishes.
    pub fn stop(mut self) -> Result<(), String> {
        self.shutdown
            .request_shutdown()
            .then_some(())
            .ok_or_else(|| "shutdown sender is closed".to_string())?;

        let thread = self
            .thread
            .take()
            .ok_or_else(|| "server thread is missing".to_string())?;

        thread
            .join()
            .map_err(|error| format!("server thread panicked: {error:?}"))
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.shutdown.request_shutdown();

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Starts the server on a background thread and returns after the port is bound.
pub fn start(addr: SocketAddr) -> Result<ServerHandle, String> {
    let shutdown = ShutdownController::new();
    let thread_shutdown = shutdown.clone();
    let (startup_tx, startup_rx) = mpsc::channel();

    let thread = std::thread::Builder::new()
        .name("vha-http-server".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = startup_tx.send(Err(format!("failed to create runtime: {error}")));
                    return;
                }
            };

            let listener = match runtime.block_on(tokio::net::TcpListener::bind(addr)) {
                Ok(listener) => listener,
                Err(error) => {
                    let _ = startup_tx.send(Err(format!("failed to bind {addr}: {error}")));
                    return;
                }
            };

            if startup_tx.send(Ok(())).is_err() {
                return;
            }

            let signal = ShutdownSignal::from_controller(&thread_shutdown);
            if let Err(error) = runtime.block_on(run_server(listener, signal)) {
                eprintln!("local server stopped with an error: {error}");
            }
        })
        .map_err(|error| format!("failed to spawn server thread: {error}"))?;

    match startup_rx.recv() {
        Ok(Ok(())) => Ok(ServerHandle {
            shutdown,
            thread: Some(thread),
        }),
        Ok(Err(error)) => {
            let _ = thread.join();
            Err(error)
        }
        Err(_) => {
            let _ = thread.join();
            Err("server exited before reporting startup status".to_string())
        }
    }
}

async fn run_server(
    listener: tokio::net::TcpListener,
    mut shutdown: ShutdownSignal,
) -> Result<(), String> {
    let app = Router::new()
        .route("/api/health", get(health))
        .fallback(static_handler);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.wait().await;
        })
        .await
        .map_err(|error| error.to_string())
}

async fn health() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], "ok")
}

async fn static_handler(uri: Uri) -> Response {
    serve_static_path(uri.path())
}

fn serve_static_path(path: &str) -> Response {
    let path = path.trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = FrontendAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response();
    }

    // Unknown extension-less paths are treated as SPA routes.
    if std::path::Path::new(path).extension().is_none() {
        if let Some(content) = FrontendAssets::get("index.html") {
            return (
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                content.data,
            )
                .into_response();
        }
    }

    (StatusCode::NOT_FOUND, "Not Found").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, TcpListener, TcpStream};

    #[test]
    fn root_serves_embedded_index_html() {
        let response = serve_static_path("/");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn spa_route_falls_back_to_index_html() {
        let response = serve_static_path("/agent");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn missing_asset_returns_not_found() {
        let response = serve_static_path("/missing/definitely-missing.js");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn server_serves_health_and_stops_cleanly() {
        let probe = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);

        let server = start(addr).unwrap();
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .write_all(b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("ok"));

        server.stop().unwrap();
        assert!(TcpStream::connect(addr).is_err());
    }
}

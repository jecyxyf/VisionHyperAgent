//! HTTP/WebSocket host and its owned Agent supervisor.
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::RustEmbed;

use crate::shutdown::{ShutdownController, ShutdownSignal};
use crate::{agent_api, agent_runtime::AgentRuntime};
use vha_codex_agent::LoadedAgentConfig;

#[derive(RustEmbed)]
#[folder = "../../frontend/dist/"]
struct FrontendAssets;

pub struct ServerHandle {
    shutdown: ShutdownController,
    thread: Option<JoinHandle<()>>,
    addr: SocketAddr,
}

impl ServerHandle {
    pub fn address(&self) -> SocketAddr {
        self.addr
    }

    pub fn stop(mut self) -> Result<(), String> {
        log::info!("requesting application backend shutdown");
        self.shutdown.request_shutdown();
        let thread = self.thread.take().ok_or("server thread is missing")?;
        thread
            .join()
            .map_err(|_| "backend thread panicked during shutdown".to_string())
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

/// A host without provider configuration, useful for serving setup/error UI and isolated tests.
pub fn local_addr() -> SocketAddr {
    SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 8420))
}

pub fn start(addr: SocketAddr) -> Result<ServerHandle, String> {
    let app_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    start_with_codex(addr, &app_dir, Err("尚未配置 Agent 模型服务".into()))
}

/// Starts HTTP promptly. Codex initialization runs in the owned background supervisor.
pub fn start_with_codex(
    addr: SocketAddr,
    app_dir: &std::path::Path,
    loaded: Result<LoadedAgentConfig, String>,
) -> Result<ServerHandle, String> {
    if !addr.ip().is_loopback() {
        return Err("backend must listen on a loopback address".into());
    }
    let app_dir = app_dir.to_path_buf();
    let shutdown = ShutdownController::new();
    let thread_shutdown = shutdown.clone();
    let (startup_tx, startup_rx) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("vha-http-server".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
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
                    log::error!("failed to bind HTTP server on {addr}: {error}");
                    let _ = startup_tx.send(Err(format!("failed to bind {addr}: {error}")));
                    return;
                }
            };
            let bound = match listener.local_addr() {
                Ok(addr) => addr,
                Err(error) => {
                    let _ = startup_tx.send(Err(error.to_string()));
                    return;
                }
            };
            if startup_tx.send(Ok(bound)).is_err() {
                return;
            }
            if let Err(error) = runtime.block_on(run_server(
                listener,
                thread_shutdown,
                app_dir.to_path_buf(),
                loaded,
            )) {
                log::error!("application backend stopped with error: {error}");
            }
            runtime.shutdown_timeout(Duration::from_secs(1));
        })
        .map_err(|error| format!("failed to spawn backend thread: {error}"))?;
    match startup_rx.recv() {
        Ok(Ok(addr)) => {
            log::info!("local HTTP server started on {addr}");
            Ok(ServerHandle {
                shutdown,
                thread: Some(thread),
                addr,
            })
        }
        Ok(Err(error)) => {
            let _ = thread.join();
            Err(error)
        }
        Err(_) => {
            let _ = thread.join();
            Err("backend exited before reporting startup status".into())
        }
    }
}

async fn run_server(
    listener: tokio::net::TcpListener,
    shutdown: ShutdownController,
    app_dir: std::path::PathBuf,
    loaded: Result<LoadedAgentConfig, String>,
) -> Result<(), String> {
    let addr = listener.local_addr().map_err(|error| error.to_string())?;
    let runtime = AgentRuntime::prepare(loaded, &app_dir, addr).await;
    let service = runtime.service.clone();
    let mut supervisor = tokio::spawn(runtime.run(ShutdownSignal::from_controller(&shutdown)));
    let gateway = crate::model_gateway::routes(service.agent.clone())?;
    let app = Router::new()
        .route("/api/health", get(health))
        .merge(agent_api::routes(
            service.clone(),
            ShutdownSignal::from_controller(&shutdown),
            addr,
        ))
        .merge(gateway)
        .fallback(static_handler);
    let mut signal = ShutdownSignal::from_controller(&shutdown);
    let mut http_signal = signal.clone();
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            http_signal.wait().await;
        })
        .into_future();
    tokio::pin!(server);
    let mut http_finished = false;
    let mut result = Ok(());
    tokio::select! {
        _=signal.wait()=>{},
        outcome=&mut server=>{http_finished=true;result=outcome.map_err(|error|error.to_string());shutdown.request_shutdown();}
    }
    service.begin_shutdown().await;
    if tokio::time::timeout(Duration::from_secs(9), &mut supervisor)
        .await
        .is_err()
    {
        log::error!("Agent supervisor exceeded shutdown deadline; dropping owned process guard");
        supervisor.abort();
        let _ = supervisor.await;
    }
    if !http_finished
        && tokio::time::timeout(Duration::from_secs(3), &mut server)
            .await
            .is_err()
    {
        log::warn!("HTTP connections exceeded graceful shutdown deadline");
    }
    log::info!("application backend stopped; owned Codex supervisor finished");
    result
}

async fn health() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], "ok")
}
async fn static_handler(uri: Uri) -> Response {
    serve_static_path(uri.path())
}

fn serve_static_path(path: &str) -> Response {
    let path = path.trim_start_matches('/');
    if path.starts_with("api/") || path.starts_with("internal/") || path == "ws" {
        return (StatusCode::NOT_FOUND, "Not Found").into_response();
    }
    let path = if path.is_empty() { "index.html" } else { path };
    if let Some(content) = FrontendAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response();
    }
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

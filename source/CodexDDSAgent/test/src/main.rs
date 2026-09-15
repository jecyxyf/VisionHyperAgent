use std::{
    collections::HashMap, io::Error as IoError, path::PathBuf, process::exit, sync::Arc,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use uuid::Uuid;
use vha_codex_dds_agent::{
    registry::{PortMode, RegistryStore, ServiceConfig, StoragePaths},
    service::ServiceRuntime,
};
use zenoh::{Config, Session};

const INDEX_HTML: &str = include_str!("index.html");
const MAX_HTTP_BODY: usize = 1024 * 1024;

struct TestUiState {
    home: PathBuf,
    inner: Mutex<TestUiInner>,
}

struct TestUiInner {
    session: Option<Session>,
    endpoint: Option<String>,
    services: HashMap<String, LocalService>,
    feeds: HashMap<String, AgentFeed>,
}

struct LocalService {
    runtime: ServiceRuntime,
    port: u16,
}

struct AgentFeed {
    records: Arc<Mutex<Vec<Value>>>,
    reverse: Arc<Mutex<Vec<Value>>>,
}

#[derive(Debug, Deserialize)]
struct OpenSessionRequest {
    endpoint: String,
}

#[derive(Debug, Deserialize)]
struct StartServiceRequest {
    service_name: String,
    agent_name: String,
    fixed_port: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct ServiceNameRequest {
    service_name: String,
}

#[derive(Debug, Deserialize)]
struct AgentRequest {
    endpoint: Option<String>,
    service_name: String,
    agent_name: String,
    model: Option<ModelPayload>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ModelPayload {
    default_provider: String,
    providers: Vec<ProviderPayload>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ProviderPayload {
    id: String,
    base_url: String,
    api_key: String,
    default_model: String,
    models: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    endpoint: Option<String>,
    service_name: String,
    agent_name: String,
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct ReverseResponseRequest {
    endpoint: Option<String>,
    service_name: String,
    agent_name: String,
    reverse_id: String,
    result: Option<Value>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct LogsRequest {
    endpoint: Option<String>,
    service_name: String,
    agent_name: String,
}

#[tokio::main]
async fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments
        .iter()
        .any(|argument| argument == "--mock-codex-server")
    {
        if let Err(error) = vha_codex_dds_agent::mock_server::run().await {
            eprintln!("mock-codex-server error: {error}");
            exit(1);
        }
        return;
    }

    let mut http_port = 17700u16;
    let mut home = std::env::temp_dir().join("CodexDDSAgentTestUI");
    let mut open_browser = false;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--http-port" => {
                index += 1;
                http_port = arguments
                    .get(index)
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_else(|| {
                        eprintln!("--http-port requires a valid port");
                        exit(2);
                    });
            }
            "--home" => {
                index += 1;
                home = arguments.get(index).map(PathBuf::from).unwrap_or_else(|| {
                    eprintln!("--home requires a directory");
                    exit(2);
                });
            }
            "--open" => open_browser = true,
            _ => {}
        }
        index += 1;
    }

    let state = Arc::new(TestUiState {
        home,
        inner: Mutex::new(TestUiInner {
            session: Some(open_discovery_session().await),
            endpoint: None,
            services: HashMap::new(),
            feeds: HashMap::new(),
        }),
    });
    let listener = TcpListener::bind(("127.0.0.1", http_port))
        .await
        .unwrap_or_else(|error| {
            eprintln!("failed to bind HTTP UI on 127.0.0.1:{http_port}: {error}");
            exit(2);
        });
    println!("CodexDDSAgent Test UI: http://127.0.0.1:{http_port}");
    println!("Test data home: {}", state.home.display());

    if open_browser {
        let _ = std::process::Command::new("xdg-open")
            .arg(format!("http://127.0.0.1:{http_port}"))
            .spawn();
    }

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, state).await {
                eprintln!("HTTP connection error: {error}");
            }
        });
    }
}

async fn handle_connection(mut stream: TcpStream, state: Arc<TestUiState>) -> Result<(), IoError> {
    let request = read_http_request(&mut stream).await?;
    let (status, content_type, body) = route(request, state).await;
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.shutdown().await?;
    Ok(())
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

async fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, IoError> {
    let mut buffer = Vec::with_capacity(16 * 1024);
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        if let Some(position) = find_header_end(&buffer) {
            break position;
        }
        if buffer.len() > MAX_HTTP_BODY {
            return Err(IoError::new(
                std::io::ErrorKind::InvalidData,
                "HTTP request is too large",
            ));
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(IoError::new(
                std::io::ErrorKind::UnexpectedEof,
                "HTTP request closed",
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
    };

    let header = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = header.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let raw_path = parts.next().unwrap_or("/");
    let path = raw_path.split('?').next().unwrap_or("/").to_string();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(IoError::new(
                std::io::ErrorKind::UnexpectedEof,
                "HTTP body closed",
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body = buffer[body_start..body_start + content_length].to_vec();
    Ok(HttpRequest { method, path, body })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn route(
    request: HttpRequest,
    state: Arc<TestUiState>,
) -> (&'static str, &'static str, Vec<u8>) {
    if request.method == "GET" && (request.path == "/" || request.path == "/index.html") {
        return (
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML.as_bytes().to_vec(),
        );
    }
    if request.method == "OPTIONS" {
        return ("204 No Content", "text/plain", Vec::new());
    }

    let body: Value = if request.body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&request.body) {
            Ok(value) => value,
            Err(error) => return api_error(400, format!("invalid JSON body: {error}")),
        }
    };

    let result = match (request.method.as_str(), request.path.as_str()) {
        ("POST", "/api/session/open") => open_session(&state, &body).await,
        ("POST", "/api/discover") => discover_services(&state).await,
        ("POST", "/api/service/start") => start_local_service(&state, &body).await,
        ("POST", "/api/service/list") => list_local_services(&state).await,
        ("POST", "/api/service/shutdown") => shutdown_local_service(&state, &body).await,
        ("POST", "/api/agent/create") => lifecycle(&state, &body, "create").await,
        ("POST", "/api/agent/attach") => lifecycle(&state, &body, "attach").await,
        ("POST", "/api/agent/detach") => lifecycle(&state, &body, "detach").await,
        ("POST", "/api/agent/heartbeat") => lifecycle(&state, &body, "heartbeat").await,
        ("POST", "/api/agent/status") => agent_status(&state, &body).await,
        ("POST", "/api/rpc") => send_rpc(&state, &body).await,
        ("POST", "/api/reverse/response") => send_reverse_response(&state, &body).await,
        ("POST", "/api/logs") => read_logs(&state, &body).await,
        ("POST", "/api/logs/clear") => clear_logs(&state, &body).await,
        _ => Err(ApiError::new(404, "unknown API route")),
    };

    match result {
        Ok(value) => ("200 OK", "application/json", value.to_string().into_bytes()),
        Err(error) => api_error(error.status, error.message),
    }
}

fn api_error(status: u16, message: String) -> (&'static str, &'static str, Vec<u8>) {
    let reason = match status {
        400 => "400 Bad Request",
        404 => "404 Not Found",
        409 => "409 Conflict",
        _ => "500 Internal Server Error",
    };
    (
        reason,
        "application/json",
        json!({"ok": false, "error": {"code": "testui_error", "message": message}})
            .to_string()
            .into_bytes(),
    )
}

#[derive(Debug)]
struct ApiError {
    status: u16,
    message: String,
}

impl ApiError {
    fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl From<String> for ApiError {
    fn from(value: String) -> Self {
        Self::new(500, value)
    }
}

impl From<&str> for ApiError {
    fn from(value: &str) -> Self {
        Self::new(500, value)
    }
}

fn parse_body<T: for<'de> Deserialize<'de>>(body: &Value) -> Result<T, ApiError> {
    serde_json::from_value(body.clone()).map_err(|error| ApiError::new(400, error.to_string()))
}

async fn open_session(state: &Arc<TestUiState>, body: &Value) -> Result<Value, ApiError> {
    let request: OpenSessionRequest = parse_body(body)?;
    if request.endpoint.trim().is_empty() {
        return Err(ApiError::new(400, "endpoint cannot be empty"));
    }
    let endpoint = normalize_endpoint(&request.endpoint)?;
    let session = open_direct_session(&endpoint).await?;
    let mut inner = state.inner.lock().await;
    inner.session = Some(session);
    inner.endpoint = Some(endpoint.clone());
    Ok(json!({"ok": true, "endpoint": endpoint}))
}

async fn discover_services(state: &Arc<TestUiState>) -> Result<Value, ApiError> {
    let session = {
        let inner = state.inner.lock().await;
        inner
            .session
            .clone()
            .ok_or_else(|| ApiError::new(400, "Zenoh session is not open"))?
    };
    let mut found = Vec::new();
    let replies = session
        .get("codex-dds/v1/service/info")
        .timeout(Duration::from_secs(2))
        .await
        .map_err(|error| ApiError::new(500, error.to_string()))?;
    while let Ok(reply) = replies.recv_async().await {
        let payload = match reply.result() {
            Ok(sample) => sample
                .payload()
                .try_to_string()
                .map_err(|_| "payload is not UTF-8")?,
            Err(error) => error
                .payload()
                .try_to_string()
                .map_err(|_| "error payload is not UTF-8")?,
        };
        if let Ok(value) = serde_json::from_str::<Value>(&payload) {
            found.push(value);
        }
    }
    found.sort_by_key(|value| {
        value
            .get("service_name")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    });
    found.dedup_by_key(|value| {
        value
            .get("service_name")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    });
    Ok(json!({"ok": true, "services": found}))
}

async fn start_local_service(state: &Arc<TestUiState>, body: &Value) -> Result<Value, ApiError> {
    let request: StartServiceRequest = parse_body(body)?;
    let paths = StoragePaths::new(&state.home);
    install_mock_binary(&paths)?;
    let mut config = ServiceConfig::default();
    config.discovery_enabled = false;
    if let Some(port) = request.fixed_port {
        config.port_mode = PortMode::Fixed;
        config.port = Some(port);
    }
    let store = Arc::new(std::sync::Mutex::new(
        RegistryStore::open(paths).map_err(|error| error.to_string())?,
    ));
    store
        .lock()
        .map_err(|_| "registry lock poisoned")?
        .create_service(&request.service_name, &request.agent_name, &config)
        .map_err(|error| error.to_string())?;
    let runtime = ServiceRuntime::start(store, &request.service_name)
        .await
        .map_err(|error| error.to_string())?;
    let port = runtime.port();
    let endpoint = format!("127.0.0.1:{port}");
    let session = open_direct_session(&endpoint).await?;
    let mut inner = state.inner.lock().await;
    inner
        .services
        .insert(request.service_name.clone(), LocalService { runtime, port });
    inner.session = Some(session);
    inner.endpoint = Some(endpoint.clone());
    Ok(json!({
        "ok": true,
        "service_name": request.service_name,
        "initial_agent_name": request.agent_name,
        "endpoint": endpoint,
        "port": port
    }))
}

async fn list_local_services(state: &Arc<TestUiState>) -> Result<Value, ApiError> {
    let inner = state.inner.lock().await;
    let services: Vec<Value> = inner
        .services
        .iter()
        .map(|(name, service)| json!({"service_name": name, "port": service.port}))
        .collect();
    Ok(json!({"ok": true, "services": services}))
}

async fn shutdown_local_service(state: &Arc<TestUiState>, body: &Value) -> Result<Value, ApiError> {
    let request: ServiceNameRequest = parse_body(body)?;
    let service = {
        let mut inner = state.inner.lock().await;
        inner.services.remove(&request.service_name)
    };
    let Some(service) = service else {
        return Err(ApiError::new(
            404,
            format!("local service {} is not running", request.service_name),
        ));
    };
    service
        .runtime
        .shutdown()
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({"ok": true, "service_name": request.service_name, "state": "stopped"}))
}

async fn lifecycle(
    state: &Arc<TestUiState>,
    body: &Value,
    action: &str,
) -> Result<Value, ApiError> {
    let request: AgentRequest = parse_body(body)?;
    let session = ensure_session(state, request.endpoint.as_deref()).await?;
    let key = format!(
        "codex-dds/v1/{}/agent/{}/{}",
        request.service_name, request.agent_name, action
    );
    let payload = match action {
        "create" => json!({
            "version": 1,
            "model": request.model.ok_or_else(|| ApiError::new(400, "model is required"))?
        }),
        "attach" => json!({"version": 1, "model": request.model}),
        _ => json!({"version": 1}),
    };
    let result = query_json(&session, &key, payload).await?;
    if action == "create" || action == "attach" {
        declare_feed(state, &session, &request.service_name, &request.agent_name).await?;
    }
    Ok(result)
}

async fn agent_status(state: &Arc<TestUiState>, body: &Value) -> Result<Value, ApiError> {
    let request: AgentRequest = parse_body(body)?;
    let session = ensure_session(state, request.endpoint.as_deref()).await?;
    let key = format!(
        "codex-dds/v1/{}/agent/{}/status/get",
        request.service_name, request.agent_name
    );
    query_json(&session, &key, json!({"version": 1})).await
}

async fn send_rpc(state: &Arc<TestUiState>, body: &Value) -> Result<Value, ApiError> {
    let request: RpcRequest = parse_body(body)?;
    let session = ensure_session(state, request.endpoint.as_deref()).await?;
    let key = format!(
        "codex-dds/v1/{}/{}/rpc",
        request.service_name, request.agent_name
    );
    let payload = json!({
        "version": 1,
        "request_id": Uuid::new_v4().to_string(),
        "method": request.method,
        "params": request.params
    });
    query_json(&session, &key, payload).await
}

async fn send_reverse_response(state: &Arc<TestUiState>, body: &Value) -> Result<Value, ApiError> {
    let request: ReverseResponseRequest = parse_body(body)?;
    let session = ensure_session(state, request.endpoint.as_deref()).await?;
    let key = feed_key(&request.service_name, &request.agent_name);
    {
        let inner = state.inner.lock().await;
        if let Some(feed) = inner.feeds.get(&key) {
            let mut reverse = feed.reverse.lock().await;
            if let Some(item) = reverse.iter_mut().find(|item| {
                item.get("reverse_id").and_then(Value::as_str) == Some(&request.reverse_id)
            }) {
                item["answered"] = json!(true);
            }
        }
    }
    let key = format!(
        "codex-dds/v1/{}/{}/reverse/response",
        request.service_name, request.agent_name
    );
    let payload = json!({
        "version": 1,
        "reverse_id": request.reverse_id,
        "result": request.result,
        "error": request.error
    });
    query_json(&session, &key, payload).await
}

async fn read_logs(state: &Arc<TestUiState>, body: &Value) -> Result<Value, ApiError> {
    let request: LogsRequest = parse_body(body)?;
    let key = feed_key(&request.service_name, &request.agent_name);
    let inner = state.inner.lock().await;
    let Some(feed) = inner.feeds.get(&key) else {
        return Ok(json!({"ok": true, "records": [], "reverse": []}));
    };
    let records = feed.records.lock().await.clone();
    let reverse = feed.reverse.lock().await.clone();
    Ok(json!({"ok": true, "records": records, "reverse": reverse}))
}

async fn clear_logs(state: &Arc<TestUiState>, body: &Value) -> Result<Value, ApiError> {
    let request: LogsRequest = parse_body(body)?;
    let key = feed_key(&request.service_name, &request.agent_name);
    let inner = state.inner.lock().await;
    if let Some(feed) = inner.feeds.get(&key) {
        feed.records.lock().await.clear();
        feed.reverse.lock().await.clear();
    }
    drop(inner);
    Ok(json!({"ok": true}))
}

async fn ensure_session(
    state: &Arc<TestUiState>,
    endpoint: Option<&str>,
) -> Result<Session, ApiError> {
    if let Some(endpoint) = endpoint.filter(|value| !value.trim().is_empty()) {
        let endpoint = normalize_endpoint(endpoint)?;
        let mut inner = state.inner.lock().await;
        if inner.endpoint.as_deref() == Some(endpoint.as_str()) {
            return inner
                .session
                .clone()
                .ok_or_else(|| ApiError::new(500, "session is missing"));
        }
        let session = open_direct_session(&endpoint).await?;
        inner.session = Some(session.clone());
        inner.endpoint = Some(endpoint);
        return Ok(session);
    }
    state
        .inner
        .lock()
        .await
        .session
        .clone()
        .ok_or_else(|| ApiError::new(400, "Zenoh session is not open"))
}

async fn query_json(session: &Session, key: &str, payload: Value) -> Result<Value, ApiError> {
    let replies = session
        .get(key)
        .timeout(Duration::from_secs(15))
        .payload(payload.to_string())
        .await
        .map_err(|error| ApiError::new(500, error.to_string()))?;
    while let Ok(reply) = replies.recv_async().await {
        let payload = match reply.result() {
            Ok(sample) => sample
                .payload()
                .try_to_string()
                .map_err(|_| "payload is not UTF-8")?,
            Err(error) => error
                .payload()
                .try_to_string()
                .map_err(|_| "error payload is not UTF-8")?,
        };
        if let Ok(value) = serde_json::from_str::<Value>(&payload) {
            return Ok(value);
        }
    }
    Err(ApiError::new(
        500,
        format!("query {key} did not return JSON"),
    ))
}

async fn declare_feed(
    state: &Arc<TestUiState>,
    session: &Session,
    service_name: &str,
    agent_name: &str,
) -> Result<(), ApiError> {
    let key = feed_key(service_name, agent_name);
    {
        let inner = state.inner.lock().await;
        if inner.feeds.contains_key(&key) {
            return Ok(());
        }
    }

    let records = Arc::new(Mutex::new(Vec::new()));
    let reverse = Arc::new(Mutex::new(Vec::new()));
    let definitions = [
        (
            "event",
            format!("codex-dds/v1/{service_name}/{agent_name}/event"),
        ),
        (
            "status",
            format!("codex-dds/v1/{service_name}/{agent_name}/status"),
        ),
        (
            "reverse",
            format!("codex-dds/v1/{service_name}/{agent_name}/reverse/request"),
        ),
    ];
    for (kind, expression) in definitions {
        let subscriber = session
            .declare_subscriber(expression.as_str())
            .await
            .map_err(|error| ApiError::new(500, error.to_string()))?;
        let records = records.clone();
        let reverse = reverse.clone();
        let kind = kind.to_string();
        tokio::spawn(async move {
            while let Ok(sample) = subscriber.recv_async().await {
                let Ok(payload) = sample.payload().try_to_string() else {
                    continue;
                };
                let Ok(mut value) = serde_json::from_str::<Value>(&payload) else {
                    continue;
                };
                value["kind"] = json!(kind);
                value["received_at"] = json!(chrono_like_now());
                if kind == "reverse" {
                    value["answered"] = json!(false);
                    let mut items = reverse.lock().await;
                    items.insert(0, value);
                    items.truncate(100);
                } else {
                    let mut items = records.lock().await;
                    items.insert(0, value);
                    items.truncate(500);
                }
            }
        });
    }

    let mut inner = state.inner.lock().await;
    inner.feeds.insert(key, AgentFeed { records, reverse });
    Ok(())
}

fn feed_key(service_name: &str, agent_name: &str) -> String {
    format!("{service_name}\u{1f}{agent_name}")
}

fn normalize_endpoint(endpoint: &str) -> Result<String, ApiError> {
    let endpoint = endpoint
        .trim()
        .trim_start_matches("tcp://")
        .trim_start_matches("tcp/");
    if endpoint.matches(':').count() == 1 && !endpoint.ends_with(':') {
        return Ok(format!("tcp/{endpoint}"));
    }
    Err(ApiError::new(400, "endpoint must be HOST:PORT"))
}

async fn open_discovery_session() -> Session {
    let mut config = Config::default();
    let _ = config.insert_json5("mode", "\"client\"");
    let _ = config.insert_json5("scouting/multicast/enabled", "true");
    zenoh::open(config)
        .await
        .expect("open Zenoh discovery session")
}

async fn open_direct_session(endpoint: &str) -> Result<Session, ApiError> {
    let mut config = Config::default();
    config
        .insert_json5("mode", "\"client\"")
        .map_err(|error| error.to_string())?;
    config
        .insert_json5("connect/endpoints", &format!("[\"{endpoint}\"]"))
        .map_err(|error| error.to_string())?;
    config
        .insert_json5("scouting/multicast/enabled", "false")
        .map_err(|error| error.to_string())?;
    zenoh::open(config)
        .await
        .map_err(|error| ApiError::new(500, error.to_string()))
}

fn install_mock_binary(paths: &StoragePaths) -> Result<(), ApiError> {
    paths.prepare().map_err(|error| error.to_string())?;
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let executable = shell_quote(&executable.to_string_lossy());
    let script = format!("#!/bin/sh\nexec {executable} --mock-codex-server \"$@\"\n");
    std::fs::write(&paths.agent_binary, script).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&paths.agent_binary, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn chrono_like_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis().to_string())
        .unwrap_or_default()
}

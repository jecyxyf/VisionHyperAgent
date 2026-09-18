//! Same-origin configuration editor. Provider credentials never travel back to the browser.
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header::ORIGIN, HeaderName, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use vha_codex_agent::{
    config::{self, ConfigManager},
    AgentConfig, CodexConfig, ModelConfig, ProviderConfig, ProviderWireApi,
};

use crate::agent_service::AgentService;

const SETTINGS_HEADER: HeaderName = HeaderName::from_static("x-vha-settings");

#[derive(Clone)]
struct SettingsState {
    service: Arc<AgentService>,
    origins: Arc<HashSet<String>>,
    manager: Option<&'static ConfigManager>,
}

pub fn routes(service: Arc<AgentService>, addr: SocketAddr) -> Router {
    routes_with_manager(service, addr, config::global())
}

fn routes_with_manager(
    service: Arc<AgentService>,
    addr: SocketAddr,
    manager: Option<&'static ConfigManager>,
) -> Router {
    let state = SettingsState {
        service,
        origins: allowed_origins(addr),
        manager,
    };
    Router::new()
        .route("/api/settings/agent", get(read).post(write))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            protect_settings_request,
        ))
        .with_state(state)
}

fn allowed_origins(addr: SocketAddr) -> Arc<HashSet<String>> {
    Arc::new(HashSet::from([
        format!("http://127.0.0.1:{}", addr.port()),
        format!("http://localhost:{}", addr.port()),
        "http://127.0.0.1:5173".into(),
        "http://localhost:5173".into(),
    ]))
}

async fn protect_settings_request(
    State(state): State<SettingsState>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(origin) = request.headers().get(ORIGIN) {
        if origin
            .to_str()
            .ok()
            .is_none_or(|origin| !state.origins.contains(origin))
        {
            return error(
                StatusCode::FORBIDDEN,
                "origin",
                "不允许来自该网页的设置请求",
            );
        }
    }
    if request.method() == "POST"
        && request
            .headers()
            .get(&SETTINGS_HEADER)
            .and_then(|value| value.to_str().ok())
            != Some("1")
    {
        return error(
            StatusCode::FORBIDDEN,
            "settings_header",
            "设置请求缺少本应用标识",
        );
    }
    next.run(request).await
}

async fn read(State(state): State<SettingsState>) -> Response {
    let Some(manager) = state.manager else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "configuration",
            "配置服务尚未初始化",
        );
    };
    let app = manager.app_config();
    Json(json!({
        "revision": manager.revision(),
        "configPath": manager.path(),
        "agent": AgentView::new(&app.agent),
    }))
    .into_response()
}

async fn write(
    State(state): State<SettingsState>,
    Json(request): Json<SettingsRequest>,
) -> Response {
    if state
        .service
        .closing
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return error(StatusCode::SERVICE_UNAVAILABLE, "closing", "程序正在退出");
    }
    let Some(manager) = state.manager else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "configuration",
            "配置服务尚未初始化",
        );
    };
    let agent = request.agent.into_agent();
    let update = match manager.update(agent, Some(&request.revision)) {
        Ok(update) => update,
        Err(message) if message.starts_with("CONFIG_CONFLICT:") => {
            return error(
                StatusCode::CONFLICT,
                "config_conflict",
                message.trim_start_matches("CONFIG_CONFLICT: "),
            )
        }
        Err(message) => return error(StatusCode::BAD_REQUEST, "invalid_config", message),
    };
    let restart_required = update.codex_changed || state.service.agent.is_none();
    state
        .service
        .apply_configuration_change(restart_required)
        .await;
    Json(json!({
        "ok": true,
        "revision": update.revision,
        "restartRequired": restart_required,
    }))
    .into_response()
}

fn error(status: StatusCode, code: &'static str, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({"error":{"code":code,"message":message.into()}})),
    )
        .into_response()
}

#[derive(Deserialize)]
struct SettingsRequest {
    revision: String,
    agent: AgentInput,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentInput {
    active_model_id: String,
    providers: Vec<ProviderInput>,
    codex: CodexConfig,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderInput {
    id: String,
    base_url: String,
    #[serde(default)]
    api_key: Option<String>,
    wire_api: ProviderWireApi,
    models: Vec<ModelConfig>,
}

impl AgentInput {
    fn into_agent(self) -> AgentConfig {
        AgentConfig {
            active_model_id: self.active_model_id,
            providers: self
                .providers
                .into_iter()
                .map(|provider| ProviderConfig {
                    id: provider.id,
                    base_url: provider.base_url,
                    api_key: provider.api_key.unwrap_or_default(),
                    wire_api: provider.wire_api,
                    models: provider.models,
                })
                .collect(),
            codex: self.codex,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentView {
    active_model_id: String,
    providers: Vec<ProviderView>,
    codex: CodexConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderView {
    id: String,
    base_url: String,
    api_key: String,
    api_key_configured: bool,
    wire_api: ProviderWireApi,
    models: Vec<ModelConfig>,
}

impl AgentView {
    fn new(agent: &AgentConfig) -> Self {
        Self {
            active_model_id: agent.active_model_id.clone(),
            providers: agent
                .providers
                .iter()
                .map(|provider| ProviderView {
                    id: provider.id.clone(),
                    base_url: provider.base_url.clone(),
                    api_key: String::new(),
                    api_key_configured: !provider.api_key.trim().is_empty(),
                    wire_api: provider.wire_api,
                    models: provider.models.clone(),
                })
                .collect(),
            codex: agent.codex.clone(),
        }
    }
}

#[cfg(test)]
#[path = "settings_api_tests.rs"]
mod tests;

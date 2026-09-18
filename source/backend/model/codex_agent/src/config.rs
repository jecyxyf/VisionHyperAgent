//! The complete Agent configuration model and runtime preparation.
//!
//! Provider credentials stay backend-only. Structures containing them deliberately do not
//! implement `Debug` after being resolved.
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use url::Url;

use crate::executable::{install_process_alias, resolve_codex};
use crate::process::ProcessConfig;
use crate::types::{Model, ReasoningEffortOption};

pub const SUPPORTED_EFFORTS: [&str; 6] = ["low", "medium", "high", "xhigh", "max", "ultra"];

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderWireApi {
    Responses,
    #[default]
    ChatCompletions,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppConfig {
    pub version: u32,
    pub agent: AgentConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            agent: AgentConfig::default(),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct AgentConfig {
    pub active_model_id: String,
    pub providers: Vec<ProviderConfig>,
    pub codex: CodexConfig,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: String,
    pub base_url: String,
    pub api_key: String,
    pub wire_api: ProviderWireApi,
    pub models: Vec<ModelConfig>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct ModelConfig {
    pub model_id: String,
    pub model_name: String,
    pub supported_efforts: Vec<String>,
    pub effort: String,
    pub supports_images: bool,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model_id: String::new(),
            model_name: String::new(),
            supported_efforts: SUPPORTED_EFFORTS
                .iter()
                .map(|effort| (*effort).to_string())
                .collect(),
            effort: "medium".to_string(),
            supports_images: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct CodexConfig {
    pub executable: Option<PathBuf>,
    pub workspace: PathBuf,
    pub home: PathBuf,
    /// Old releases exposed a fixed Codex port. It is accepted only so existing
    /// app_config.json files keep loading; the runtime always asks the OS for a
    /// fresh local port and never serializes this field back.
    #[serde(default, rename = "port", skip_serializing)]
    pub legacy_port: Option<u16>,
    pub connect_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub reconnect_interval_ms: u64,
    pub max_reconnect_attempts: u32,
    pub approval_timeout_ms: u64,
    pub event_capacity: usize,
    pub experimental_api: bool,
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            executable: None,
            workspace: PathBuf::from("data/workspace"),
            home: PathBuf::from("data/codex"),
            legacy_port: None,
            connect_timeout_ms: 5_000,
            request_timeout_ms: 30_000,
            reconnect_interval_ms: 500,
            max_reconnect_attempts: 20,
            approval_timeout_ms: 300_000,
            event_capacity: 1_024,
            experimental_api: true,
        }
    }
}

pub struct ResolvedModel {
    pub model_id: String,
    pub model_name: String,
    pub provider_id: String,
    pub base_url: String,
    pub api_key: String,
    pub wire_api: ProviderWireApi,
    pub supported_efforts: Vec<String>,
    pub effort: String,
    pub supports_images: bool,
}

#[derive(Clone)]
pub struct ResolvedAgentConfig {
    pub active_model_id: String,
    pub active_model: Arc<ResolvedModel>,
    pub models: Arc<HashMap<String, Arc<ResolvedModel>>>,
    pub codex: CodexConfig,
}

impl ResolvedAgentConfig {
    pub fn model(&self, model_id: &str) -> Option<Arc<ResolvedModel>> {
        self.models.get(model_id).cloned()
    }

    pub fn browser_models(&self) -> Vec<Model> {
        let mut models: Vec<_> = self
            .models
            .values()
            .map(|model| Model {
                id: model.model_id.clone(),
                model: model.model_id.clone(),
                display_name: model.model_id.clone(),
                is_default: model.model_id == self.active_model_id,
                hidden: false,
                supported_reasoning_efforts: model
                    .supported_efforts
                    .iter()
                    .map(|effort| ReasoningEffortOption {
                        reasoning_effort: effort.clone(),
                        description: String::new(),
                    })
                    .collect(),
                default_reasoning_effort: Some(model.effort.clone()),
                input_modalities: if model.supports_images {
                    vec!["text".into(), "image".into()]
                } else {
                    vec!["text".into()]
                },
            })
            .collect();
        models.sort_by(|left, right| left.id.cmp(&right.id));
        models
    }
}

impl AgentConfig {
    pub fn resolve(&self) -> Result<ResolvedAgentConfig, String> {
        let mut models = HashMap::new();
        let mut provider_ids = HashSet::new();
        if !self.providers.is_empty() && self.active_model_id.trim().is_empty() {
            return Err("请配置 agent.activeModelId".into());
        }
        for provider in &self.providers {
            validate_token(&provider.id, "id").map_err(|message| format!("Provider {message}"))?;
            if !provider_ids.insert(provider.id.clone()) {
                return Err(format!("Provider id 重复：{}", provider.id));
            }
            if provider.api_key.trim().is_empty() {
                return Err(format!("Provider {} 缺少 api_key", provider.id));
            }
            let base = validate_base_url(&provider.base_url)
                .map_err(|message| format!("Provider {} {}", provider.id, message))?;
            if provider.models.is_empty() {
                return Err(format!("Provider {} 至少需要配置一个模型", provider.id));
            }
            for model in &provider.models {
                validate_token(&model.model_id, "id")
                    .map_err(|message| format!("模型 {} {}", model.model_id, message))?;
                validate_token(&model.model_name, "名称")
                    .map_err(|message| format!("模型 {} {}", model.model_id, message))?;
                if model.supported_efforts.is_empty() {
                    return Err(format!(
                        "模型 {} 必须配置至少一个 supportedEfforts",
                        model.model_id
                    ));
                }
                if model.supported_efforts.len()
                    != model.supported_efforts.iter().collect::<HashSet<_>>().len()
                {
                    return Err(format!("模型 {} 的 supportedEfforts 重复", model.model_id));
                }
                if model
                    .supported_efforts
                    .iter()
                    .any(|effort| !SUPPORTED_EFFORTS.contains(&effort.as_str()))
                {
                    return Err(format!("模型 {} 包含无效 Effort", model.model_id));
                }
                if !model.supported_efforts.contains(&model.effort) {
                    return Err(format!(
                        "模型 {} 的 effort 必须属于 supportedEfforts",
                        model.model_id
                    ));
                }
                let resolved = Arc::new(ResolvedModel {
                    model_id: model.model_id.clone(),
                    model_name: model.model_name.clone(),
                    provider_id: provider.id.clone(),
                    base_url: base.clone(),
                    api_key: provider.api_key.clone(),
                    wire_api: provider.wire_api,
                    supported_efforts: model.supported_efforts.clone(),
                    effort: model.effort.clone(),
                    supports_images: model.supports_images,
                });
                if models.insert(model.model_id.clone(), resolved).is_some() {
                    return Err(format!("modelId 重复：{}", model.model_id));
                }
            }
        }
        let active_model = if self.providers.is_empty() {
            Arc::new(ResolvedModel {
                model_id: String::new(),
                model_name: String::new(),
                provider_id: String::new(),
                base_url: String::new(),
                api_key: String::new(),
                wire_api: ProviderWireApi::Responses,
                supported_efforts: Vec::new(),
                effort: String::new(),
                supports_images: false,
            })
        } else {
            models
                .get(&self.active_model_id)
                .cloned()
                .ok_or_else(|| format!("activeModelId 不存在：{}", self.active_model_id))?
        };
        self.codex.validate()?;
        Ok(ResolvedAgentConfig {
            active_model_id: self.active_model_id.clone(),
            active_model,
            models: Arc::new(models),
            codex: self.codex.clone(),
        })
    }
}

impl CodexConfig {
    fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("connectTimeoutMs", self.connect_timeout_ms),
            ("requestTimeoutMs", self.request_timeout_ms),
            ("reconnectIntervalMs", self.reconnect_interval_ms),
            ("approvalTimeoutMs", self.approval_timeout_ms),
        ] {
            if value == 0 || value > 3_600_000 {
                return Err(format!("Codex {name} 必须在 1 到 3600000 毫秒之间"));
            }
        }
        if self.max_reconnect_attempts > 100 {
            return Err("Codex maxReconnectAttempts 不能超过 100".into());
        }
        if self.event_capacity == 0 || self.event_capacity > 65_536 {
            return Err("Codex eventCapacity 必须在 1 到 65536 之间".into());
        }
        Ok(())
    }
}

pub struct LoadedAgentConfig {
    pub config: ResolvedAgentConfig,
    pub created: bool,
    pub migrated: bool,
    pub recovered_from: Option<PathBuf>,
}

struct LoadedAppConfig {
    value: AppConfig,
    created: bool,
    migrated: bool,
    recovered_from: Option<PathBuf>,
}

fn load_app(app_dir: &Path) -> Result<LoadedAppConfig, String> {
    let path = app_dir.join("app_config.json");
    if path.exists() {
        let loaded = vha_common::config::load_or_create::<AppConfig>(&path)
            .map_err(|_| "无法读取 app_config.json，请检查文件权限".to_string())?;
        return Ok(LoadedAppConfig {
            value: loaded.value,
            created: loaded.created,
            migrated: false,
            recovered_from: loaded.recovered_from,
        });
    }

    let legacy_path = app_dir.join("config.local.toml");
    let legacy = if legacy_path.exists() {
        std::fs::read_to_string(&legacy_path)
            .map_err(|_| "无法读取旧 config.local.toml，请检查文件权限".to_string())?
    } else {
        String::new()
    };
    if let Some(legacy) = parse_legacy(&legacy)? {
        let app = AppConfig {
            version: 1,
            agent: AgentConfig {
                active_model_id: legacy.model_id.clone(),
                providers: vec![ProviderConfig {
                    id: "imported".into(),
                    base_url: legacy.base_url,
                    api_key: legacy.api_key,
                    wire_api: legacy.wire_api,
                    models: vec![ModelConfig {
                        model_id: legacy.model_id,
                        model_name: legacy.model_name,
                        ..Default::default()
                    }],
                }],
                codex: CodexConfig {
                    executable: legacy.executable,
                    workspace: legacy.workspace.unwrap_or_else(|| "data/workspace".into()),
                    ..Default::default()
                },
            },
        };
        vha_common::config::save(&path, &app)
            .map_err(|_| "无法保存迁移后的 app_config.json".to_string())?;
        return Ok(LoadedAppConfig {
            value: app,
            created: true,
            migrated: true,
            recovered_from: None,
        });
    }

    let loaded = vha_common::config::load_or_create::<AppConfig>(&path)
        .map_err(|_| "无法创建默认 app_config.json".to_string())?;
    Ok(LoadedAppConfig {
        value: loaded.value,
        created: loaded.created,
        migrated: false,
        recovered_from: loaded.recovered_from,
    })
}

pub fn load(app_dir: &Path) -> Result<LoadedAgentConfig, String> {
    let loaded = load_app(app_dir)?;
    let mut app = loaded.value;
    apply_environment(&mut app)?;
    Ok(LoadedAgentConfig {
        config: app.agent.resolve()?,
        created: loaded.created,
        migrated: loaded.migrated,
        recovered_from: loaded.recovered_from,
    })
}

/// The process-wide Agent configuration store.
///
/// Like the logger, one instance is installed at process startup and all later readers and
/// writers go through it. Tests construct the same manager directly against temporary
/// directories instead of replacing the process-wide instance.
pub struct ConfigManager {
    path: PathBuf,
    state: Mutex<ConfigManagerState>,
}

struct ConfigManagerState {
    app: AppConfig,
    resolved: Arc<ResolvedAgentConfig>,
    revision: String,
    created: bool,
    migrated: bool,
    recovered_from: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigUpdate {
    pub revision: String,
    pub codex_changed: bool,
}

static CONFIG_MANAGER: OnceLock<ConfigManager> = OnceLock::new();
static CONFIG_INIT: Mutex<()> = Mutex::new(());

/// Installs and loads the singleton configuration manager.
///
/// A second call returns the already-installed manager's configuration, just as repeated
/// logger initialization returns the existing logger.
pub fn initialize(app_dir: &Path) -> Result<LoadedAgentConfig, String> {
    let guard = CONFIG_INIT
        .lock()
        .expect("config initialization mutex poisoned");
    if let Some(manager) = CONFIG_MANAGER.get() {
        return manager.loaded();
    }

    let loaded = load_app(app_dir)?;
    let mut runtime_app = loaded.value.clone();
    apply_environment(&mut runtime_app)?;
    let resolved = Arc::new(runtime_app.agent.resolve()?);
    let manager = ConfigManager {
        path: app_dir.join("app_config.json"),
        state: Mutex::new(ConfigManagerState {
            revision: revision(&loaded.value),
            app: loaded.value,
            resolved,
            created: loaded.created,
            migrated: loaded.migrated,
            recovered_from: loaded.recovered_from,
        }),
    };
    let manager = CONFIG_MANAGER.get_or_init(|| manager);
    let result = manager.loaded();
    drop(guard);
    result
}

pub fn global() -> Option<&'static ConfigManager> {
    CONFIG_MANAGER.get()
}

/// Returns the singleton's current runtime configuration.
pub fn current() -> Option<Arc<ResolvedAgentConfig>> {
    global().map(|manager| manager.current_config())
}

impl ConfigManager {
    /// Creates an independent manager. Production code uses initialize instead.
    pub fn new(app_dir: &Path) -> Result<Self, String> {
        let loaded = load_app(app_dir)?;
        let mut runtime_app = loaded.value.clone();
        apply_environment(&mut runtime_app)?;
        Ok(Self {
            path: app_dir.join("app_config.json"),
            state: Mutex::new(ConfigManagerState {
                revision: revision(&loaded.value),
                app: loaded.value,
                resolved: Arc::new(runtime_app.agent.resolve()?),
                created: loaded.created,
                migrated: loaded.migrated,
                recovered_from: loaded.recovered_from,
            }),
        })
    }

    pub fn loaded(&self) -> Result<LoadedAgentConfig, String> {
        let state = self.lock();
        Ok(LoadedAgentConfig {
            config: (*state.resolved).clone(),
            created: state.created,
            migrated: state.migrated,
            recovered_from: state.recovered_from.clone(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn app_config(&self) -> AppConfig {
        self.lock().app.clone()
    }

    pub fn revision(&self) -> String {
        self.lock().revision.clone()
    }

    pub fn current_config(&self) -> Arc<ResolvedAgentConfig> {
        self.lock().resolved.clone()
    }

    /// Validates, merges retained credentials, atomically saves, and publishes a new snapshot.
    pub fn update(
        &self,
        mut agent: AgentConfig,
        expected_revision: Option<&str>,
    ) -> Result<ConfigUpdate, String> {
        let mut state = self.lock_poisoned_ok();
        if let Some(expected) = expected_revision {
            if expected != state.revision {
                return Err("CONFIG_CONFLICT: app_config.json 已被外部修改，请重新加载设置".into());
            }
        }

        let old_app = state.app.clone();
        let codex_changed = old_app.agent.codex != agent.codex;
        merge_retained_api_keys(&old_app.agent, &mut agent);
        let app = AppConfig { version: 1, agent };
        let mut runtime_app = app.clone();
        apply_environment(&mut runtime_app)?;
        let resolved = Arc::new(runtime_app.agent.resolve()?);
        vha_common::config::save(&self.path, &app)
            .map_err(|_| "无法保存 app_config.json，请检查文件权限".to_string())?;

        let next_revision = revision(&app);
        state.app = app;
        state.resolved = resolved;
        state.revision = next_revision.clone();
        Ok(ConfigUpdate {
            revision: next_revision,
            codex_changed,
        })
    }

    pub fn redact(&self, value: &mut Value) {
        let secrets: Vec<String> = {
            let state = self.lock();
            state
                .resolved
                .models
                .values()
                .map(|model| model.api_key.clone())
                .collect()
        };
        let secrets: Vec<&str> = secrets.iter().map(String::as_str).collect();
        redact_value(value, &secrets);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ConfigManagerState> {
        self.state.lock().expect("config manager mutex poisoned")
    }

    fn lock_poisoned_ok(&self) -> std::sync::MutexGuard<'_, ConfigManagerState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn merge_retained_api_keys(old: &AgentConfig, next: &mut AgentConfig) {
    let old_keys: HashMap<_, _> = old
        .providers
        .iter()
        .map(|provider| (provider.id.as_str(), provider.api_key.as_str()))
        .collect();
    for provider in &mut next.providers {
        if provider.api_key.trim().is_empty() {
            if let Some(key) = old_keys.get(provider.id.as_str()) {
                provider.api_key = (*key).to_string();
            }
        }
    }
}

fn revision(app: &AppConfig) -> String {
    let bytes = serde_json::to_vec(app).unwrap_or_default();
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

struct LegacyConfig {
    base_url: String,
    api_key: String,
    model_name: String,
    model_id: String,
    wire_api: ProviderWireApi,
    executable: Option<PathBuf>,
    workspace: Option<PathBuf>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyFile {
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    codex_path: Option<PathBuf>,
    workspace: Option<PathBuf>,
    wire_api: ProviderWireApi,
}

fn parse_legacy(text: &str) -> Result<Option<LegacyConfig>, String> {
    let file: LegacyFile = toml::from_str(text)
        .map_err(|_| "config.local.toml 格式错误或包含未知配置项".to_string())?;
    let non_empty = |value: &Option<String>| {
        value
            .as_ref()
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    };
    let Some(base_url) = non_empty(&file.base_url) else {
        return Ok(None);
    };
    let model_name = non_empty(&file.model).ok_or_else(|| "旧配置缺少 model".to_string())?;
    let api_key = non_empty(&file.api_key).ok_or_else(|| "旧配置缺少 api_key".to_string())?;
    Ok(Some(LegacyConfig {
        model_id: model_alias(&model_name),
        base_url,
        api_key,
        model_name,
        wire_api: file.wire_api,
        executable: file.codex_path,
        workspace: file.workspace,
    }))
}

fn model_alias(model_name: &str) -> String {
    let mut alias = String::new();
    let mut previous_separator = false;
    for character in model_name.chars().take(64) {
        if character.is_ascii_alphanumeric() {
            alias.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator {
            alias.push('-');
            previous_separator = true;
        }
    }
    let alias = alias.trim_matches('-').to_string();
    if alias.is_empty() {
        "imported-model".into()
    } else {
        alias
    }
}

fn apply_environment(app: &mut AppConfig) -> Result<(), String> {
    let value = |name: &str| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    };
    let Some(base_url) = value("VHA_CODEX_BASE_URL") else {
        return Ok(());
    };
    let Some(model_name) = value("VHA_CODEX_MODEL") else {
        return Ok(());
    };
    let Some(api_key) = value("VHA_CODEX_API_KEY") else {
        return Ok(());
    };
    let model_id = model_alias(&model_name);
    app.agent
        .providers
        .retain(|provider| provider.id != "imported");
    app.agent.providers.push(ProviderConfig {
        id: "imported".into(),
        base_url,
        api_key,
        wire_api: match value("VHA_CODEX_WIRE_API").as_deref() {
            Some("responses") => ProviderWireApi::Responses,
            Some("chat_completions") | None => ProviderWireApi::ChatCompletions,
            Some(_) => return Err("VHA_CODEX_WIRE_API 只支持 responses 或 chat_completions".into()),
        },
        models: vec![ModelConfig {
            model_id: model_id.clone(),
            model_name,
            ..Default::default()
        }],
    });
    app.agent.active_model_id = model_id;
    if let Some(path) = value("VHA_CODEX_PATH") {
        app.agent.codex.executable = Some(PathBuf::from(path));
    }
    if let Some(workspace) = value("VHA_CODEX_WORKSPACE") {
        app.agent.codex.workspace = PathBuf::from(workspace);
    }
    Ok(())
}

pub struct PreparedAgent {
    pub config: Arc<ResolvedAgentConfig>,
    pub workspace: PathBuf,
    pub home: PathBuf,
    pub executable: PathBuf,
    websocket_token: String,
    gateway_token: String,
    listen_port: AtomicU16,
    pub process: ProcessConfig,
}

impl PreparedAgent {
    pub fn websocket_url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.listen_port())
    }

    pub fn listen_port(&self) -> u16 {
        self.listen_port.load(Ordering::Acquire)
    }

    /// Selects a runtime-only listen port and returns the matching process args.
    ///
    /// The host binds port 0 before calling this method, drops that probe listener,
    /// then lets Codex bind the selected port. A failed bind is handled by selecting
    /// another port rather than persisting a fixed value in app_config.json.
    pub fn rebind_port(&self, port: u16) -> Result<ProcessConfig, String> {
        if port == 0 {
            return Err("无法分配 Codex 本机端口".into());
        }
        self.listen_port.store(port, Ordering::Release);
        Ok(self.process_config(port))
    }

    pub fn websocket_token(&self) -> String {
        self.websocket_token.clone()
    }

    pub fn gateway_token(&self) -> &str {
        &self.gateway_token
    }

    pub fn transport_config(&self) -> TransportConfig {
        let codex = &self.config.codex;
        TransportConfig {
            websocket_url: self.websocket_url(),
            auth_token: Some(self.websocket_token.clone()),
            experimental_api: codex.experimental_api,
            connect_timeout: Duration::from_millis(codex.connect_timeout_ms),
            request_timeout: Duration::from_millis(codex.request_timeout_ms),
            reconnect_interval: Duration::from_millis(codex.reconnect_interval_ms),
            max_reconnect_attempts: codex.max_reconnect_attempts,
            approval_timeout: Duration::from_millis(codex.approval_timeout_ms),
            event_capacity: codex.event_capacity,
        }
    }

    fn process_config(&self, port: u16) -> ProcessConfig {
        ProcessConfig {
            executable: self.executable.clone(),
            args: process_arguments(&self.websocket_token, port),
            cwd: self.process.cwd.clone(),
            env: self.process.env.clone(),
        }
    }

    pub fn redact(&self, value: &mut Value) {
        let mut secrets: Vec<&str> = self
            .config
            .models
            .values()
            .map(|model| model.api_key.as_str())
            .collect();
        secrets.push(&self.websocket_token);
        secrets.push(&self.gateway_token);
        redact_value(value, &secrets);
    }
}

fn process_arguments(websocket_token: &str, port: u16) -> Vec<std::ffi::OsString> {
    let digest = Sha256::digest(websocket_token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    vec![
        "app-server".into(),
        "--listen".into(),
        format!("ws://127.0.0.1:{port}").into(),
        "--ws-auth".into(),
        "capability-token".into(),
        "--ws-token-sha256".into(),
        digest.into(),
    ]
}

pub fn prepare(
    config: Arc<ResolvedAgentConfig>,
    app_dir: &Path,
    address: SocketAddr,
) -> Result<PreparedAgent, String> {
    if config.models.is_empty() {
        return Err("app_config.json 尚未配置任何 Agent 模型 Provider".into());
    }
    let codex = &config.codex;
    let configured_executable = codex.executable.as_deref();
    let resolved_executable = resolve_codex(configured_executable, app_dir)?;
    let executable = install_process_alias(&resolved_executable, app_dir)?;
    let workspace = absolute_path(app_dir, &codex.workspace);
    let home = absolute_path(app_dir, &codex.home);
    std::fs::create_dir_all(&workspace).map_err(|_| "无法创建 Agent 工作目录".to_string())?;
    std::fs::create_dir_all(&home).map_err(|_| "无法创建独立 Codex 配置目录".to_string())?;
    let workspace = workspace
        .canonicalize()
        .map_err(|_| "无法访问 Agent 工作目录".to_string())?;
    let home = home
        .canonicalize()
        .map_err(|_| "无法访问 Codex 配置目录".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "无法保护 Codex 配置目录权限".to_string())?;
    }

    let websocket_token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let gateway_token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let gateway_url = format!("http://{address}/internal/model/v1");
    let mut projects = Map::new();
    projects.insert(
        workspace.to_string_lossy().into_owned(),
        json!({"trust_level":"trusted"}),
    );
    let codex_config = json!({
        "model": config.active_model_id,
        "model_provider":"vha_provider",
        "approval_policy":"on-request",
        "approvals_reviewer":"user",
        "sandbox_mode":"workspace-write",
        "web_search":"disabled",
        "model_providers":{"vha_provider":{
            "name":"VisionHyperAgent internal model gateway",
            "base_url":gateway_url,
            "wire_api":"responses",
            "env_key":"VHA_INTERNAL_MODEL_TOKEN",
            "requires_openai_auth":false,
            "supports_websockets":false
        }},
        "shell_environment_policy":{"exclude":["VHA_INTERNAL_MODEL_TOKEN"]},
        "projects":projects
    });
    let encoded = toml::to_string(&codex_config).map_err(|_| "无法生成 Codex 配置".to_string())?;
    let config_path = home.join("config.toml");
    std::fs::write(&config_path, encoded).map_err(|_| "无法写入独立 Codex 配置".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| "无法保护 Codex 配置权限".to_string())?;
    }
    let process = ProcessConfig {
        executable: executable.clone(),
        args: process_arguments(&websocket_token, 0),
        cwd: Some(workspace.clone()),
        env: vec![
            ("CODEX_HOME".into(), home.as_os_str().into()),
            ("RUST_LOG".into(), "warn".into()),
            (
                "VHA_INTERNAL_MODEL_TOKEN".into(),
                gateway_token.clone().into(),
            ),
        ],
    };
    Ok(PreparedAgent {
        config,
        workspace,
        home,
        executable,
        websocket_token,
        gateway_token,
        listen_port: AtomicU16::new(0),
        process,
    })
}

#[derive(Clone)]
pub struct TransportConfig {
    pub websocket_url: String,
    pub auth_token: Option<String>,
    pub experimental_api: bool,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub reconnect_interval: Duration,
    pub max_reconnect_attempts: u32,
    pub approval_timeout: Duration,
    pub event_capacity: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            // No fixed endpoint is configured for unprepared services. Prepared agents
            // replace this with the OS-selected runtime-only port before connecting.
            websocket_url: "ws://127.0.0.1:0".into(),
            auth_token: None,
            experimental_api: false,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            reconnect_interval: Duration::from_millis(500),
            max_reconnect_attempts: 20,
            approval_timeout: Duration::from_secs(300),
            event_capacity: 1_024,
        }
    }
}

fn absolute_path(app_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        app_dir.join(path)
    }
}

fn validate_token(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label}不能为空"));
    }
    if value.len() > 256 || value.chars().any(|c| c.is_control()) {
        return Err(format!("{label}过长或包含控制字符"));
    }
    Ok(())
}

fn validate_base_url(value: &str) -> Result<String, String> {
    let parsed = Url::parse(value).map_err(|_| "baseUrl 格式错误".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("baseUrl 必须是无凭据、无查询参数的 HTTP/HTTPS 地址".into());
    }
    Ok(parsed.to_string())
}

pub fn redact_value(value: &mut Value, secrets: &[&str]) {
    match value {
        Value::String(text) => {
            for secret in secrets {
                *text = text.replace(secret, "[REDACTED]");
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| redact_value(item, secrets)),
        Value::Object(fields) => {
            let original = std::mem::take(fields);
            for (mut key, mut item) in original {
                for secret in secrets {
                    key = key.replace(secret, "[REDACTED]");
                }
                redact_value(&mut item, secrets);
                fields.insert(key, item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> AppConfig {
        AppConfig {
            version: 1,
            agent: AgentConfig {
                active_model_id: "mini".into(),
                providers: vec![ProviderConfig {
                    id: "provider".into(),
                    base_url: "http://127.0.0.1:1/v1".into(),
                    api_key: "PRIVATE".into(),
                    wire_api: ProviderWireApi::ChatCompletions,
                    models: vec![ModelConfig {
                        model_id: "mini".into(),
                        model_name: "Provider Mini".into(),
                        ..Default::default()
                    }],
                }],
                codex: CodexConfig::default(),
            },
        }
    }

    fn prepared_app(root: &Path) -> AppConfig {
        let mut app = app();
        app.agent.codex.executable = Some(std::env::current_exe().unwrap());
        app.agent.codex.workspace = root.join("data/workspace");
        app.agent.codex.home = root.join("data/codex");
        app
    }

    #[test]
    fn resolves_models_into_a_constant_time_index() {
        let resolved = app().agent.resolve().unwrap();
        assert_eq!(resolved.model("mini").unwrap().model_name, "Provider Mini");
        assert!(resolved.model("missing").is_none());
        assert_eq!(resolved.browser_models().len(), 1);
        assert_eq!(
            resolved.browser_models()[0].supported_reasoning_efforts[0].reasoning_effort,
            "low"
        );
    }

    #[test]
    fn rejects_duplicate_models_and_invalid_effort() {
        let mut config = app();
        config.agent.providers[0].models.push(ModelConfig {
            model_id: "mini".into(),
            model_name: "duplicate".into(),
            ..Default::default()
        });
        assert!(config.agent.resolve().is_err());

        let mut config = app();
        config.agent.providers[0].models[0].effort = "unknown".into();
        assert!(config.agent.resolve().is_err());
    }

    #[test]
    fn resolves_multiple_providers_and_rejects_invalid_references() {
        let mut config = app();
        config.agent.providers.push(ProviderConfig {
            id: "second".into(),
            base_url: "https://models.example.test/v1".into(),
            api_key: "SECOND_PRIVATE_KEY".into(),
            wire_api: ProviderWireApi::Responses,
            models: vec![ModelConfig {
                model_id: "text-only".into(),
                model_name: "upstream-text".into(),
                supported_efforts: vec!["low".into(), "medium".into()],
                effort: "low".into(),
                supports_images: false,
            }],
        });
        config.agent.active_model_id = "text-only".into();
        let resolved = config.agent.resolve().unwrap();
        assert_eq!(resolved.model("mini").unwrap().provider_id, "provider");
        assert_eq!(
            resolved.model("text-only").unwrap().base_url,
            "https://models.example.test/v1"
        );
        assert_eq!(resolved.browser_models().len(), 2);

        config.agent.active_model_id = "missing".into();
        let Err(error) = config.agent.resolve() else {
            panic!("missing activeModelId was accepted");
        };
        assert_eq!(error, "activeModelId 不存在：missing");
    }

    #[test]
    fn rejects_provider_duplicates_and_unsafe_base_urls() {
        let mut config = app();
        config
            .agent
            .providers
            .push(config.agent.providers[0].clone());
        let Err(error) = config.agent.resolve() else {
            panic!("duplicate provider was accepted");
        };
        assert_eq!(error, "Provider id 重复：provider");

        let mut config = app();
        config.agent.providers[0].base_url = "https://user@example.test/v1".into();
        let Err(error) = config.agent.resolve() else {
            panic!("URL credentials were accepted");
        };
        assert!(error.contains("无凭据、无查询参数"));

        let mut config = app();
        config.agent.providers[0].base_url = "https://example.test/v1?key=1".into();
        let Err(error) = config.agent.resolve() else {
            panic!("URL query was accepted");
        };
        assert!(error.contains("无凭据、无查询参数"));
    }

    #[test]
    fn empty_default_is_valid_until_runtime_preparation() {
        assert!(AppConfig::default().agent.resolve().is_ok());
    }

    #[test]
    fn migrates_legacy_toml_once_and_reports_corrupt_json_recovery() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("config.local.toml"),
            r#"
                base_url = "http://127.0.0.1:9/v1"
                api_key = "LEGACY_FIXTURE_KEY"
                model = "MiniMax-M3"
                workspace = "legacy-workspace"
            "#,
        )
        .unwrap();
        let loaded = load(root.path()).unwrap();
        assert!(loaded.migrated);
        assert_eq!(loaded.config.active_model_id, "minimax-m3");
        let migrated: AppConfig = serde_json::from_str(
            &std::fs::read_to_string(root.path().join("app_config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(migrated.agent.providers.len(), 1);
        assert!(serde_json::to_value(&migrated).unwrap()["agent"]["codex"]
            .as_object()
            .unwrap()
            .get("port")
            .is_none());

        let corrupt_root = tempfile::tempdir().unwrap();
        let path = corrupt_root.path().join("app_config.json");
        std::fs::write(&path, b"{ invalid").unwrap();
        let loaded = load(corrupt_root.path()).unwrap();
        assert!(loaded.recovered_from.is_some());
        assert!(loaded.config.models.is_empty());
        assert!(path.is_file());
    }

    #[test]
    fn preparation_keeps_runtime_secrets_out_of_files_and_arguments() {
        let root = tempfile::tempdir().unwrap();
        let app = prepared_app(root.path());
        let prepared = prepare(
            Arc::new(app.agent.resolve().unwrap()),
            root.path(),
            "127.0.0.1:8420".parse().unwrap(),
        )
        .unwrap();
        assert_eq!(
            prepared.executable.file_name().unwrap().to_string_lossy(),
            crate::executable::CODEX_PROCESS_FILE_NAME
        );
        assert_eq!(prepared.listen_port(), 0);
        let codex_config = std::fs::read_to_string(prepared.home.join("config.toml")).unwrap();
        assert!(codex_config.contains("vha_provider"));
        assert!(!codex_config.contains("PRIVATE"));
        assert!(!prepared
            .process
            .args
            .iter()
            .any(|argument| argument.to_string_lossy().contains("PRIVATE")));
        assert_eq!(prepared.process.env.len(), 3);
        assert_eq!(
            prepared
                .process
                .env
                .iter()
                .find(|(key, _)| key == "VHA_INTERNAL_MODEL_TOKEN")
                .unwrap()
                .1,
            prepared.gateway_token()
        );

        let mut response = json!({"message":"provider key PRIVATE and gateway token", "token": prepared.gateway_token()});
        prepared.redact(&mut response);
        assert_eq!(
            response["message"],
            "provider key [REDACTED] and gateway token"
        );
        assert_eq!(response["token"], "[REDACTED]");
    }

    #[test]
    fn fixed_port_is_legacy_only_and_runtime_ports_are_rebindable() {
        let root = tempfile::tempdir().unwrap();
        let app = prepared_app(root.path());
        let value = serde_json::to_value(&app).unwrap();
        assert!(!value.to_string().contains("\"port\""));

        let mut object = value;
        object["agent"]["codex"]["port"] = json!(9421);
        let app: AppConfig = serde_json::from_value(object).unwrap();
        assert!(app.agent.resolve().is_ok());
        assert!(!serde_json::to_string(&app).unwrap().contains("\"port\""));

        let prepared = prepare(
            Arc::new(app.agent.resolve().unwrap()),
            root.path(),
            "127.0.0.1:8420".parse().unwrap(),
        )
        .unwrap();
        let process = prepared.rebind_port(53211).unwrap();
        assert_eq!(prepared.listen_port(), 53_211);
        assert_eq!(prepared.websocket_url(), "ws://127.0.0.1:53211");
        assert!(process.args.contains(&"ws://127.0.0.1:53211".into()));
    }

    #[test]
    fn legacy_model_names_become_safe_model_ids() {
        assert_eq!(model_alias("MiniMax-M3 / Large"), "minimax-m3-large");
        assert_eq!(model_alias("///"), "imported-model");
    }

    #[test]
    fn config_manager_creates_defaults_updates_atomically_and_retains_keys() {
        let root = tempfile::tempdir().unwrap();
        let manager = ConfigManager::new(root.path()).unwrap();
        assert!(root.path().join("app_config.json").is_file());
        assert!(manager.current_config().models.is_empty());
        assert_eq!(manager.revision().len(), 64);

        let mut next = app().agent;
        let first_revision = manager.revision();
        let update = manager.update(next.clone(), Some(&first_revision)).unwrap();
        assert!(!update.codex_changed);
        assert_ne!(update.revision, first_revision);
        assert_eq!(
            manager.current_config().model("mini").unwrap().api_key,
            "PRIVATE"
        );

        next.providers[0].api_key = String::new();
        next.providers[0].models[0].model_name = "Provider Mini 2".into();
        next.codex.workspace = "data/updated-workspace".into();
        let update = manager
            .update(next.clone(), Some(&manager.revision()))
            .unwrap();
        assert!(update.codex_changed);
        assert_eq!(
            manager.current_config().codex.workspace,
            PathBuf::from("data/updated-workspace")
        );
        assert_eq!(
            manager.current_config().model("mini").unwrap().model_name,
            "Provider Mini 2"
        );

        let saved: AppConfig = serde_json::from_str(
            &std::fs::read_to_string(root.path().join("app_config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved.agent.providers[0].api_key, "PRIVATE");
        assert_eq!(
            saved.agent.codex.workspace,
            PathBuf::from("data/updated-workspace")
        );

        let mut response = json!({"message":"key PRIVATE"});
        manager.redact(&mut response);
        assert_eq!(response["message"], "key [REDACTED]");
    }

    #[test]
    fn initialize_installs_one_process_wide_manager() {
        let first = tempfile::tempdir().unwrap();
        initialize(first.path()).unwrap();
        let manager = global().expect("configuration singleton");
        assert_eq!(manager.path(), first.path().join("app_config.json"));
        assert!(current().is_some());

        let second = tempfile::tempdir().unwrap();
        initialize(second.path()).unwrap();
        assert_eq!(manager.path(), first.path().join("app_config.json"));
        assert_ne!(manager.path(), second.path().join("app_config.json"));
    }

    #[test]
    fn config_manager_rejects_stale_revisions_and_invalid_updates() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path()).unwrap();
        vha_common::config::save(&root.path().join("app_config.json"), &app()).unwrap();
        let manager = ConfigManager::new(root.path()).unwrap();

        let mut stale = app().agent;
        stale.providers[0].models[0].model_name = "stale".into();
        let error = manager.update(stale, Some("wrong-revision")).unwrap_err();
        assert!(error.starts_with("CONFIG_CONFLICT"));

        let mut invalid = app().agent;
        invalid.providers[0].models.push(ModelConfig {
            model_id: "mini".into(),
            model_name: "duplicate".into(),
            ..Default::default()
        });
        assert!(manager.update(invalid, Some(&manager.revision())).is_err());
        assert_eq!(
            manager.current_config().model("mini").unwrap().model_name,
            "Provider Mini"
        );
    }
}

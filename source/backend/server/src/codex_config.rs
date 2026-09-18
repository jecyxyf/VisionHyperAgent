//! Host-owned configuration. Nothing in this module is sent to the browser.
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use vha_codex_agent::ProcessConfig;

use crate::executable::resolve_codex;

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LocalConfig {
    codex_path: Option<PathBuf>,
    base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    workspace: Option<PathBuf>,
    port: Option<u16>,
    supports_images: bool,
    wire_api: ProviderProtocol,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocol {
    #[default]
    Responses,
    ChatCompletions,
}

/// No Debug/Serialize: a backend-only value contains the provider secret.
pub struct CodexSettings {
    pub executable: PathBuf,
    pub base_url: String,
    pub model: String,
    pub workspace: PathBuf,
    pub home: PathBuf,
    pub port: u16,
    pub supports_images: bool,
    pub wire_api: ProviderProtocol,
    api_key: String,
    websocket_token: String,
    gateway_token: String,
    gateway_url: Option<String>,
}

impl CodexSettings {
    pub fn from_toml(app_dir: &Path, text: &str) -> Result<Self, String> {
        Self::parse(app_dir, text, &HashMap::new())
    }
    pub fn load(app_dir: &Path) -> Result<Self, String> {
        let config_path = app_dir.join("config.local.toml");
        let text = match std::fs::read_to_string(&config_path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(_) => return Err("无法读取应用目录中的 config.local.toml".into()),
        };
        Self::parse(app_dir, &text, &std::env::vars().collect())
    }

    fn parse(app_dir: &Path, text: &str, env: &HashMap<String, String>) -> Result<Self, String> {
        // Do not include TOML parser diagnostics: they may echo a line containing the API key.
        let local: LocalConfig = toml::from_str(text)
            .map_err(|_| "config.local.toml 格式错误或包含未知配置项".to_string())?;
        let value = |name: &str, file: Option<String>| {
            env.get(name)
                .cloned()
                .or(file)
                .filter(|s| !s.trim().is_empty())
        };
        let base_url = value("VHA_CODEX_BASE_URL", local.base_url)
            .ok_or("请配置模型服务 base_url 或 VHA_CODEX_BASE_URL")?;
        let parsed = url::Url::parse(&base_url).map_err(|_| "模型服务地址格式错误")?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err("模型地址必须是无凭据、无查询参数的 HTTP/HTTPS 地址".into());
        }
        let model =
            value("VHA_CODEX_MODEL", local.model).ok_or("请配置 model 或 VHA_CODEX_MODEL")?;
        if model.len() > 256 {
            return Err("模型名称过长".into());
        }
        let api_key = value("VHA_CODEX_API_KEY", local.api_key)
            .ok_or("请配置 VHA_CODEX_API_KEY 或私有配置中的 api_key")?;
        let configured_path = env
            .get("VHA_CODEX_PATH")
            .map(PathBuf::from)
            .or(local.codex_path);
        let executable = resolve_codex(configured_path.as_deref(), app_dir)?;
        let workspace = env
            .get("VHA_CODEX_WORKSPACE")
            .map(PathBuf::from)
            .or(local.workspace)
            .unwrap_or_else(|| app_dir.join("data/workspace"));
        let workspace = if workspace.is_absolute() {
            workspace
        } else {
            app_dir.join(workspace)
        };
        let port = match env.get("VHA_CODEX_PORT") {
            Some(port) => port
                .parse::<u16>()
                .map_err(|_| "VHA_CODEX_PORT 必须是有效端口")?,
            None => local.port.unwrap_or(8421),
        };
        if port == 0 || port == 8420 {
            return Err("Codex 端口必须非零且不同于网页服务端口".into());
        }
        let wire_api = match env.get("VHA_CODEX_WIRE_API").map(String::as_str) {
            Some("responses") => ProviderProtocol::Responses,
            Some("chat_completions") => ProviderProtocol::ChatCompletions,
            Some(_) => return Err("VHA_CODEX_WIRE_API 只支持 responses 或 chat_completions".into()),
            None => local.wire_api,
        };
        Ok(Self {
            executable,
            base_url,
            model,
            workspace,
            home: app_dir.join("data/codex"),
            port,
            supports_images: local.supports_images,
            wire_api,
            api_key,
            websocket_token: format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            ),
            gateway_token: format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            ),
            gateway_url: None,
        })
    }

    pub fn websocket_url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }

    pub(crate) fn websocket_token(&self) -> String {
        self.websocket_token.clone()
    }

    pub(crate) fn gateway_token(&self) -> &str {
        &self.gateway_token
    }
    pub(crate) fn provider_key(&self) -> &str {
        &self.api_key
    }
    pub(crate) fn configure_gateway(&mut self, addr: std::net::SocketAddr) {
        if self.wire_api == ProviderProtocol::ChatCompletions {
            self.gateway_url = Some(format!("http://{addr}/internal/model/v1"));
        }
    }

    /// Remove the actual secret if an upstream JSON value unexpectedly includes it.
    pub fn redact(&self, value: &mut serde_json::Value) {
        match value {
            serde_json::Value::String(s) => {
                for secret in [&self.api_key, &self.websocket_token, &self.gateway_token] {
                    *s = s.replace(secret, "[REDACTED]");
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|v| self.redact(v)),
            serde_json::Value::Object(fields) => {
                let original = std::mem::take(fields);
                for (mut key, mut value) in original {
                    for secret in [&self.api_key, &self.websocket_token, &self.gateway_token] {
                        key = key.replace(secret, "[REDACTED]");
                    }
                    self.redact(&mut value);
                    fields.insert(key, value);
                }
            }
            _ => {}
        }
    }

    pub fn prepare(&mut self) -> Result<ProcessConfig, String> {
        std::fs::create_dir_all(&self.workspace).map_err(|_| "无法创建 Agent 工作目录")?;
        std::fs::create_dir_all(&self.home).map_err(|_| "无法创建独立 Codex 配置目录")?;
        self.workspace = self
            .workspace
            .canonicalize()
            .map_err(|_| "无法访问 Agent 工作目录")?;
        self.home = self
            .home
            .canonicalize()
            .map_err(|_| "无法访问 Codex 配置目录")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.home, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "无法保护 Codex 配置目录权限")?;
        }
        if self.wire_api == ProviderProtocol::ChatCompletions && self.gateway_url.is_none() {
            return Err("Chat 协议兼容层尚未配置本地监听地址".into());
        }
        let mut config = json!({
            "model":self.model,
            "model_provider":"vha_provider",
            "approval_policy":"on-request",
            "approvals_reviewer":"user",
            "sandbox_mode":"workspace-write",
            "model_providers":{"vha_provider":{
                "name":"VisionHyperAgent configured provider",
                "base_url":self.gateway_url.as_deref().unwrap_or(&self.base_url),
                "wire_api":"responses",
                "env_key":"VHA_CODEX_API_KEY",
                "requires_openai_auth":false,
                "supports_websockets":false
            }},
            "shell_environment_policy":{"exclude":["VHA_CODEX_API_KEY"]},
            "projects":{self.workspace.to_string_lossy().as_ref():{"trust_level":"trusted"}}
        });
        if self.wire_api == ProviderProtocol::ChatCompletions {
            // Chat Completions has no provider-hosted Responses web-search tool. Local
            // Codex tools, sandboxing, skills and approvals remain enabled and unchanged.
            config["web_search"] = json!("disabled");
        }
        let encoded = toml::to_string(&config).map_err(|_| "无法生成 Codex 配置")?;
        let path = self.home.join("config.toml");
        std::fs::write(&path, encoded).map_err(|_| "无法写入独立 Codex 配置")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| "无法保护 Codex 配置权限")?;
        }
        Ok(ProcessConfig {
            executable: self.executable.clone(),
            args: vec![
                "app-server".into(),
                "--listen".into(),
                self.websocket_url().into(),
                "--ws-auth".into(),
                "capability-token".into(),
                "--ws-token-sha256".into(),
                Sha256::digest(self.websocket_token.as_bytes())
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    .into(),
            ],
            cwd: Some(self.workspace.clone()),
            env: vec![
                ("CODEX_HOME".into(), self.home.as_os_str().into()),
                ("RUST_LOG".into(), "warn".into()),
                (
                    "VHA_CODEX_API_KEY".into(),
                    if self.wire_api == ProviderProtocol::ChatCompletions {
                        self.gateway_token.clone().into()
                    } else {
                        self.api_key.clone().into()
                    },
                ),
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("vha-config-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn environment() -> HashMap<String, String> {
        HashMap::from([
            ("VHA_CODEX_BASE_URL".into(), "http://127.0.0.1:1/v1".into()),
            ("VHA_CODEX_MODEL".into(), "fixture-model".into()),
            ("VHA_CODEX_API_KEY".into(), "ENV_SECRET_MARKER".into()),
            (
                "VHA_CODEX_PATH".into(),
                std::env::current_exe()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            ),
        ])
    }

    #[test]
    fn prepares_isolated_home_without_key_in_file_or_command_line() {
        let scratch = Scratch::new();
        let mut settings = CodexSettings::parse(&scratch.0, "", &environment()).unwrap();
        let child = settings.prepare().unwrap();
        let text = std::fs::read_to_string(settings.home.join("config.toml")).unwrap();
        assert!(!text.contains("ENV_SECRET_MARKER"));
        assert!(child
            .args
            .iter()
            .all(|arg| !arg.to_string_lossy().contains("ENV_SECRET_MARKER")));
        let config: toml::Value = toml::from_str(&text).unwrap();
        assert_eq!(config["model"].as_str(), Some("fixture-model"));
        assert_eq!(
            config["model_providers"]["vha_provider"]["env_key"].as_str(),
            Some("VHA_CODEX_API_KEY")
        );
        assert_eq!(config["sandbox_mode"].as_str(), Some("workspace-write"));
        assert_eq!(config["approval_policy"].as_str(), Some("on-request"));
        assert_eq!(child.cwd, Some(settings.workspace.clone()));
        assert_eq!(child.env[0].0, "CODEX_HOME");
        assert_eq!(child.env[0].1, settings.home.as_os_str());
        assert!(child
            .env
            .iter()
            .any(|(key, value)| key == "VHA_CODEX_API_KEY" && value == "ENV_SECRET_MARKER"));
        assert!(settings.home.starts_with(scratch.0.canonicalize().unwrap()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&settings.home)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(settings.home.join("config.toml"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn environment_overrides_file_and_nested_payloads_are_redacted() {
        let scratch = Scratch::new();
        let settings = CodexSettings::parse(
            &scratch.0,
            "model = 'file-model'\napi_key = 'FILE_SECRET_MARKER'",
            &environment(),
        )
        .unwrap();
        assert_eq!(settings.model, "fixture-model");
        let mut payload =
            json!({"nested":["prefix ENV_SECRET_MARKER suffix",{"error":"ENV_SECRET_MARKER"}]});
        settings.redact(&mut payload);
        assert_eq!(
            payload,
            json!({"nested":["prefix [REDACTED] suffix",{"error":"[REDACTED]"}]})
        );
    }

    #[test]
    fn rejects_zero_http_conflicting_and_invalid_ports() {
        let scratch = Scratch::new();
        for port in ["0", "8420", "not-a-port", "65536"] {
            let mut env = environment();
            env.insert("VHA_CODEX_PORT".into(), port.into());
            assert!(CodexSettings::parse(&scratch.0, "", &env).is_err());
        }
    }

    #[test]
    fn invalid_private_config_does_not_echo_credentials() {
        let error = CodexSettings::parse(
            Path::new("/unused"),
            "api_key = \"SECRET_MARKER",
            &HashMap::new(),
        )
        .err()
        .unwrap();
        assert!(!error.contains("SECRET_MARKER"));
    }

    #[test]
    fn rejects_urls_with_embedded_credentials_or_non_http_schemes() {
        for base in [
            "file:///tmp/foo",
            "http://user:secret@localhost/v1",
            "http://localhost/v1?api_key=secret",
        ] {
            let env = HashMap::from([("VHA_CODEX_BASE_URL".into(), base.into())]);
            assert!(CodexSettings::parse(Path::new("/unused"), "", &env).is_err());
        }
    }
}

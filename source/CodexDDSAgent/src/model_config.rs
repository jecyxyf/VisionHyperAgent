use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, RpcError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    pub default_provider: String,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub base_url: String,
    pub api_key: String,
    pub default_model: String,
    pub models: Vec<String>,
}

#[derive(Debug, Serialize)]
struct StoredConfig<'a> {
    model_provider: &'a str,
    model: &'a str,
    model_providers: BTreeMap<&'a str, StoredProvider<'a>>,
}

#[derive(Debug, Serialize)]
struct StoredProvider<'a> {
    name: &'a str,
    base_url: &'a str,
    experimental_bearer_token: &'a str,
    requires_openai_auth: bool,
    wire_api: &'static str,
}

#[derive(Debug, Deserialize, PartialEq)]
struct StoredConfigReader {
    model_provider: String,
    model: String,
    #[allow(dead_code)]
    model_providers: BTreeMap<String, StoredProviderReader>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct StoredProviderReader {
    name: String,
    base_url: String,
    experimental_bearer_token: String,
    requires_openai_auth: bool,
    wire_api: String,
}

impl ModelConfig {
    pub fn validate(&self) -> Result<(), Error> {
        if self.providers.is_empty() {
            return Err(invalid("providers cannot be empty"));
        }
        if !self.providers.iter().any(|p| p.id == self.default_provider) {
            return Err(invalid("default_provider does not exist"));
        }

        let mut ids = std::collections::BTreeSet::new();
        for provider in &self.providers {
            crate::names::validate_name(&provider.id)
                .map_err(|_| invalid("provider id contains invalid characters"))?;
            if !ids.insert(provider.id.as_str()) {
                return Err(invalid("provider ids must be unique"));
            }
            let valid_url = Url::parse(&provider.base_url)
                .ok()
                .map(|url| url.scheme() == "http" || url.scheme() == "https")
                .unwrap_or(false);
            if !valid_url {
                return Err(invalid("provider base_url must be a valid http(s) URL"));
            }
            if provider.api_key.is_empty() {
                return Err(invalid("provider api_key cannot be empty"));
            }
            if provider.models.is_empty() {
                return Err(invalid("provider models cannot be empty"));
            }
            if !provider.models.iter().any(|m| *m == provider.default_model) {
                return Err(invalid("provider default_model must exist in models"));
            }
        }
        Ok(())
    }

    pub fn initialize_or_lock(&self, codex_home: &Path) -> Result<(), Error> {
        self.validate()?;
        let config_path = codex_home.join("config.toml");
        fs::create_dir_all(codex_home)?;

        if config_path.exists() {
            let existing = fs::read_to_string(&config_path)?;
            if equivalent(&existing, self)? {
                return Ok(());
            }
            return Err(Error::Rpc(RpcError::model_config_locked()));
        }

        let content = toml::to_string(&self.to_stored_config()?)?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&config_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        crate::platform::set_owner_only_permissions(&config_path)?;
        Ok(())
    }

    fn to_stored_config(&self) -> Result<StoredConfig<'_>, Error> {
        let default = self
            .providers
            .iter()
            .find(|provider| provider.id == self.default_provider)
            .ok_or_else(|| invalid("default_provider does not exist"))?;
        let mut providers = BTreeMap::new();
        for provider in &self.providers {
            providers.insert(
                provider.id.as_str(),
                StoredProvider {
                    name: provider.id.as_str(),
                    base_url: provider.base_url.as_str(),
                    experimental_bearer_token: provider.api_key.as_str(),
                    requires_openai_auth: false,
                    wire_api: "responses",
                },
            );
        }
        Ok(StoredConfig {
            model_provider: default.id.as_str(),
            model: default.default_model.as_str(),
            model_providers: providers,
        })
    }
}

fn equivalent(existing: &str, config: &ModelConfig) -> Result<bool, Error> {
    let existing: StoredConfigReader = toml::from_str(existing)?;
    let expected = config.to_stored_config()?;
    let provider_equal = existing.model_providers.iter().all(|(id, provider)| {
        expected
            .model_providers
            .get(id.as_str())
            .is_some_and(|want| {
                provider.name == want.name
                    && provider.base_url == want.base_url
                    && provider.experimental_bearer_token == want.experimental_bearer_token
                    && provider.requires_openai_auth == want.requires_openai_auth
                    && provider.wire_api == want.wire_api
            })
    }) && existing.model_providers.len() == expected.model_providers.len();
    Ok(existing.model_provider == expected.model_provider
        && existing.model == expected.model
        && provider_equal)
}

fn invalid(message: &'static str) -> Error {
    Error::invalid_model_config(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ModelConfig {
        ModelConfig {
            default_provider: "main".to_string(),
            providers: vec![ProviderConfig {
                id: "main".to_string(),
                base_url: "https://api.example.com/v1".to_string(),
                api_key: "secret-key".to_string(),
                default_model: "model-a".to_string(),
                models: vec!["model-a".to_string(), "model-b".to_string()],
            }],
        }
    }

    #[test]
    fn writes_required_codex_config_with_owner_only_permissions() {
        let root = tempfile::tempdir().unwrap();
        config().initialize_or_lock(root.path()).unwrap();
        let path = root.path().join("config.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("experimental_bearer_token = \"secret-key\""));
        assert!(text.contains("wire_api = \"responses\""));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn rejects_changed_model_config_after_lock() {
        let root = tempfile::tempdir().unwrap();
        config().initialize_or_lock(root.path()).unwrap();
        config().initialize_or_lock(root.path()).unwrap();
        let mut changed = config();
        changed.providers[0].api_key = "other-key".to_string();
        let error = changed.initialize_or_lock(root.path()).unwrap_err();
        assert!(matches!(
            error,
            Error::Rpc(RpcError {
                code: "model_config_locked",
                ..
            })
        ));
    }

    #[test]
    fn rejects_invalid_model_config() {
        let root = tempfile::tempdir().unwrap();
        let mut invalid = config();
        invalid.providers[0].default_model = "missing".to_string();
        let error = invalid.initialize_or_lock(root.path()).unwrap_err();
        assert!(matches!(
            error,
            Error::Rpc(RpcError {
                code: "invalid_model_config",
                ..
            })
        ));
    }
}

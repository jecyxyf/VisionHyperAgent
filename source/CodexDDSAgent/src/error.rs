use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct RpcError {
    #[serde(skip_serializing)]
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn invalid_name(message: impl Into<String>) -> Self {
        Self::new("invalid_name", message)
    }

    pub fn invalid_model_config(message: impl Into<String>) -> Self {
        Self::new("invalid_model_config", message)
    }

    pub fn model_config_locked() -> Self {
        Self::new(
            "model_config_locked",
            "model configuration is initialized and cannot be changed",
        )
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal_error", message)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Rpc(#[from] RpcError),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("toml serialization error: {0}")]
    Toml(#[from] toml::ser::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Error {
    pub fn invalid_name(message: impl Into<String>) -> Self {
        RpcError::invalid_name(message).into()
    }

    pub fn invalid_model_config(message: impl Into<String>) -> Self {
        RpcError::invalid_model_config(message).into()
    }

    pub fn internal(message: impl Into<String>) -> Self {
        RpcError::internal(message).into()
    }
}

impl From<toml::de::Error> for Error {
    fn from(value: toml::de::Error) -> Self {
        Self::internal(format!("failed to parse config.toml: {value}"))
    }
}

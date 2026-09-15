use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, RpcError};

pub const PROTOCOL_VERSION: i64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub version: i64,
    pub request_id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl RpcRequest {
    pub fn validate(&self) -> Result<(), Error> {
        if self.version != PROTOCOL_VERSION {
            return Err(Error::Rpc(RpcError::new(
                "invalid_request",
                "unsupported protocol version",
            )));
        }
        if self.request_id.is_empty() {
            return Err(Error::Rpc(RpcError::new(
                "invalid_request",
                "request_id cannot be empty",
            )));
        }
        if self.method == "initialize" || self.method == "initialized" {
            return Err(Error::Rpc(RpcError::new(
                "disabled_by_policy",
                "handshake is managed by CodexDDSAgent",
            )));
        }
        if disabled_login_method(&self.method) {
            return Err(Error::Rpc(RpcError::new(
                "disabled_by_policy",
                "account login is disabled",
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcResponse {
    pub version: i64,
    pub request_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl RpcResponse {
    pub fn success(request_id: impl Into<String>, result: Value) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(request_id: impl Into<String>, error: RpcError) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

pub fn disabled_login_method(method: &str) -> bool {
    matches!(
        method,
        "account/login/start"
            | "account/login/cancel"
            | "account/logout"
            | "account/bedrock/discover"
            | "account/bedrock/setup"
            | "account/chatgptAuthTokens/refresh"
    ) || method.starts_with("account/sessions/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_handshake_and_account_login() {
        for method in [
            "initialize",
            "initialized",
            "account/login/start",
            "account/sessions/read",
        ] {
            let request = RpcRequest {
                version: 1,
                request_id: "1".to_string(),
                method: method.to_string(),
                params: Value::Null,
            };
            assert!(request.validate().is_err());
        }
    }

    #[test]
    fn allows_read_only_auth_methods() {
        let request = RpcRequest {
            version: 1,
            request_id: "1".to_string(),
            method: "getAuthStatus".to_string(),
            params: Value::Null,
        };
        request.validate().unwrap();
    }
}

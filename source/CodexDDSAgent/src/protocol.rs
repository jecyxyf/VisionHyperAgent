use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::OnceLock};

use crate::error::{Error, RpcError};
use codex_app_server_protocol::{ClientRequest, JSONRPCRequest, RequestId};

pub const PROTOCOL_VERSION: i64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub version: i64,
    pub request_id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Deserialize)]
struct ProtocolExports {
    json_schema: HashMap<String, String>,
}

const CODEX_PROTOCOL_EXPORTS: &[u8] = include_bytes!(
    "../../depends/codex/codex-rs/app-server-protocol/schema/precomputed/app-server-exports-experimental.json.zst"
);

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
        if !client_request_methods()
            .iter()
            .any(|method| method == &self.method)
        {
            return Err(Error::Rpc(RpcError::new(
                "unknown_method",
                "method is not part of the fixed Codex protocol",
            )));
        }
        let request = JSONRPCRequest {
            id: RequestId::String(self.request_id.clone()),
            method: self.method.clone(),
            params: Some(self.params.clone()),
            trace: None,
        };
        ClientRequest::try_from(request).map_err(|_| {
            Error::Rpc(RpcError::new(
                "invalid_request",
                "method params do not match the fixed Codex protocol",
            ))
        })?;
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

fn client_request_methods() -> &'static Vec<String> {
    static METHODS: OnceLock<Vec<String>> = OnceLock::new();
    METHODS.get_or_init(|| protocol_schema_methods("ClientRequest.json"))
}

pub fn is_server_request_method(method: &str) -> bool {
    static METHODS: OnceLock<Vec<String>> = OnceLock::new();
    METHODS
        .get_or_init(|| protocol_schema_methods("ServerRequest.json"))
        .iter()
        .any(|candidate| candidate == method)
}

pub fn is_server_notification_method(method: &str) -> bool {
    static METHODS: OnceLock<Vec<String>> = OnceLock::new();
    METHODS
        .get_or_init(|| protocol_schema_methods("ServerNotification.json"))
        .iter()
        .any(|candidate| candidate == method)
}

fn protocol_schema_methods(file_name: &str) -> Vec<String> {
    protocol_schema(file_name)
        .get("oneOf")
        .and_then(Value::as_array)
        .map(|variants| {
            variants
                .iter()
                .filter_map(|variant| {
                    variant
                        .pointer("/properties/method/enum/0")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn protocol_schema(file_name: &str) -> Value {
    let decompressed = zstd::stream::decode_all(&CODEX_PROTOCOL_EXPORTS[..])
        .expect("decode Codex protocol exports");
    let exports: ProtocolExports =
        serde_json::from_slice(&decompressed).expect("decode Codex protocol export index");
    let schema = exports
        .json_schema
        .get(file_name)
        .expect("find Codex protocol schema");
    serde_json::from_str(schema).expect("decode Codex protocol schema")
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
            method: "account/usage/read".to_string(),
            params: Value::Null,
        };
        request.validate().unwrap();
    }

    #[test]
    fn validates_method_params_with_codex_types() {
        let request = RpcRequest {
            version: 1,
            request_id: "1".to_string(),
            method: "memory/reset".to_string(),
            params: Value::Null,
        };
        request.validate().unwrap();

        let request = RpcRequest {
            version: 1,
            request_id: "1".to_string(),
            method: "thread/start".to_string(),
            params: Value::Null,
        };
        let error = request.validate().unwrap_err();
        assert!(matches!(
            error,
            Error::Rpc(RpcError {
                code: "invalid_request",
                ..
            })
        ));
    }

    #[test]
    fn rejects_unknown_method_before_params_validation() {
        let request = RpcRequest {
            version: 1,
            request_id: "1".to_string(),
            method: "not/a/codex/method".to_string(),
            params: Value::Null,
        };
        let error = request.validate().unwrap_err();
        assert!(matches!(
            error,
            Error::Rpc(RpcError {
                code: "unknown_method",
                ..
            })
        ));
    }

    #[test]
    fn protocol_counts_match_codex_submodule() {
        assert_eq!(
            protocol_schema("ClientRequest.json")["oneOf"]
                .as_array()
                .unwrap()
                .len(),
            159
        );
        assert_eq!(
            protocol_schema("ClientNotification.json")["oneOf"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            protocol_schema("ServerRequest.json")["oneOf"]
                .as_array()
                .unwrap()
                .len(),
            11
        );
        assert_eq!(
            protocol_schema("ServerNotification.json")["oneOf"]
                .as_array()
                .unwrap()
                .len(),
            81
        );
        assert_eq!(client_request_methods().len(), 159);
    }
}

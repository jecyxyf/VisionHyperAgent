//! 应用配置。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub listen_addr: String,
    pub codex_websocket_url: String,
    pub data_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:8420".into(),
            codex_websocket_url: "ws://127.0.0.1:8080/ws".into(),
            data_dir: "./data".into(),
        }
    }
}

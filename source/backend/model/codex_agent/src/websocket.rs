//! Codex App Server WebSocket 客户端。

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub websocket_url: String,
    pub request_timeout_secs: u64,
    pub reconnect_interval_secs: u64,
    pub max_reconnect_attempts: u32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            websocket_url: "ws://127.0.0.1:8080/ws".into(),
            request_timeout_secs: 30,
            reconnect_interval_secs: 3,
            max_reconnect_attempts: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Stopped,
    Starting,
    Running,
    Reconnecting,
    Error(String),
}

pub struct CodexAgent {
    config: AgentConfig,
    status: std::sync::Arc<std::sync::RwLock<AgentStatus>>,
}

impl CodexAgent {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            status: std::sync::Arc::new(std::sync::RwLock::new(AgentStatus::Stopped)),
        }
    }

    pub fn status(&self) -> AgentStatus {
        self.status.read().unwrap().clone()
    }

    pub async fn start(&self) -> Result<(), String> {
        *self.status.write().unwrap() = AgentStatus::Starting;
        // TODO: 建立连接、握手、启动接收循环
        *self.status.write().unwrap() = AgentStatus::Running;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        *self.status.write().unwrap() = AgentStatus::Stopped;
        Ok(())
    }
}

use std::time::Duration;

/// Transport configuration; executable and process policy belong to the host.
#[derive(Clone)]
pub struct AgentConfig {
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

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            websocket_url: "ws://127.0.0.1:8421".into(),
            auth_token: None,
            experimental_api: false,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            reconnect_interval: Duration::from_millis(500),
            max_reconnect_attempts: 20,
            approval_timeout: Duration::from_secs(300),
            event_capacity: 1024,
        }
    }
}

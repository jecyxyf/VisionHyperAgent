use std::net::{Ipv4Addr, SocketAddr};

/// Fixed network configuration for the local host process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    /// The address bound by the HTTP server. The server is local-only.
    pub listen_addr: SocketAddr,
}

impl AppConfig {
    pub fn local() -> Self {
        Self {
            listen_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, Self::PORT)),
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.listen_addr)
    }

    pub const PORT: u16 = 8420;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_config_only_listens_on_loopback() {
        let config = AppConfig::local();

        assert_eq!(config.listen_addr.ip().to_string(), "127.0.0.1");
        assert_eq!(config.listen_addr.port(), 8420);
        assert_eq!(config.base_url(), "http://127.0.0.1:8420");
    }
}

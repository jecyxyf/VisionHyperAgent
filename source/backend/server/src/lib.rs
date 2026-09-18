//! Backend-owned runtime services, separate from the native tray entry point.
pub mod agent_api;
pub mod agent_runtime;
pub mod agent_service;
pub mod app_config;
pub mod attachments;
pub mod codex_config;
mod executable;
pub mod http_server;
mod model_gateway;
pub mod shutdown;

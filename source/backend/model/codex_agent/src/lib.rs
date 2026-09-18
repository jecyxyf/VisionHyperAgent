//! Codex App Server communication and process primitives.
//!
//! The host owns CodexProcess. CodexAgent never starts or kills a process.
mod client;
pub mod config;
mod error;
pub mod executable;
pub mod process;
mod types;
mod websocket;

pub use client::CodexAgent;
pub use config::{
    prepare, AgentConfig, AppConfig, CodexConfig, LoadedAgentConfig, ModelConfig, PreparedAgent,
    ProviderConfig, ProviderWireApi, ResolvedAgentConfig, ResolvedModel, TransportConfig,
};
pub use error::{AgentError, Result};
pub use process::{CodexProcess, ProcessConfig};
pub use types::*;

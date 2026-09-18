//! Codex App Server communication and process primitives.
//!
//! The host owns `CodexProcess`. `CodexAgent` never starts or kills a process.
mod client;
mod config;
mod error;
pub mod process;
mod types;
mod websocket;

pub use client::CodexAgent;
pub use config::AgentConfig;
pub use error::{AgentError, Result};
pub use process::{CodexProcess, ProcessConfig};
pub use types::*;

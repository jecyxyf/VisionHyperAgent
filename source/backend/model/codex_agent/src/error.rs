pub type Result<T> = std::result::Result<T, AgentError>;

/// Display excludes upstream bodies, which can contain credentials or prompts.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AgentError {
    #[error("Codex is not connected")]
    Disconnected,
    #[error("Codex connection failed")]
    ConnectionFailed,
    #[error("Codex request timed out; execution may already have started")]
    Timeout,
    #[error("Codex transport failed")]
    Transport,
    #[error("invalid Codex protocol message")]
    Protocol,
    #[error("Codex request capacity exceeded")]
    Busy,
    #[error("unsupported Codex method")]
    UnsupportedMethod,
    #[error("request has expired or has already been answered")]
    StaleRequest,
    #[error("Codex RPC error ({code})")]
    Rpc { code: i64, message: String },
    #[error("invalid agent configuration: {0}")]
    Configuration(&'static str),
}

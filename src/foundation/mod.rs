//! 基础层日志、配置及共享错误上下文，不接入界面或业务服务。
//!
//! ```no_run
//! use vision_hyper_agent::foundation::{self, CONFIG, LOGGER, ConfigLoadStatus};
//! # fn main() -> foundation::Result<()> {
//! let status = foundation::init()?;
//! if let ConfigLoadStatus::Recovered(problem) = status {
//!     LOGGER.error_with_context("config", "已备份并重建配置", &problem)?;
//! }
//! CONFIG.update(|config| config.agent.model = "model-name".into())?;
//! CONFIG.save()?;
//! LOGGER.info("app", "配置已保存")?;
//! LOGGER.flush()?;
//! # Ok(())
//! # }
//! ```
mod config_manager;
mod logger;

pub use config_manager::{AgentConfig, AppConfig, CONFIG, ConfigLoadStatus, ConfigManager};
pub use logger::{LOGGER, LogLevel, Logger};

use std::{
    error::Error,
    fmt, io,
    panic::Location,
    path::{Path, PathBuf},
};

/// 源码位置，在错误创建处或日志调用处捕获。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: &'static str,
    pub line: u32,
}
impl SourceLocation {
    #[track_caller]
    pub fn caller() -> Self {
        let location = Location::caller();
        Self {
            file: location.file(),
            line: location.line(),
        }
    }
}

/// 跨层传递时保留原始位置。调用方须仅放入已经脱敏的原因。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorContext {
    pub source: SourceLocation,
    pub reason: String,
    pub causes: Vec<String>,
    pub data_file: Option<PathBuf>,
    pub data_line: Option<usize>,
    pub data_column: Option<usize>,
}
impl ErrorContext {
    #[track_caller]
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            source: SourceLocation::caller(),
            reason: reason.into(),
            causes: Vec::new(),
            data_file: None,
            data_line: None,
            data_column: None,
        }
    }
    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.causes.push(cause.into());
        self
    }
    pub fn with_data_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.data_file = Some(path.into());
        self
    }
    pub fn with_json_position(mut self, line: usize, column: usize) -> Self {
        self.data_line = Some(line);
        self.data_column = Some(column);
        self
    }
}
impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)?;
        for cause in &self.causes {
            write!(f, "；原因：{cause}")?;
        }
        if let Some(path) = &self.data_file {
            write!(f, "；文件：{}", path.display())?;
        }
        if let Some(line) = self.data_line {
            write!(f, "；JSON 行：{line}")?;
        }
        if let Some(column) = self.data_column {
            write!(f, "，列：{column}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundationErrorKind {
    Io,
    NotInitialized,
    LockPoisoned,
    InvalidInput,
}

/// JSON 解析错误不保留 serde 的原始错误文本，避免其包含 KEY 值。
#[derive(Debug)]
pub struct FoundationError {
    kind: FoundationErrorKind,
    context: Box<ErrorContext>,
    io_source: Option<io::Error>,
}
impl FoundationError {
    #[track_caller]
    pub(crate) fn new(kind: FoundationErrorKind, reason: &str) -> Self {
        Self {
            kind,
            context: Box::new(ErrorContext::new(reason)),
            io_source: None,
        }
    }
    #[track_caller]
    pub(crate) fn io(reason: &str, path: Option<&Path>, source: io::Error) -> Self {
        let mut context = ErrorContext::new(reason).with_cause(source.to_string());
        if let Some(path) = path {
            context = context.with_data_file(path);
        }
        Self {
            kind: FoundationErrorKind::Io,
            context: Box::new(context),
            io_source: Some(source),
        }
    }
    pub fn kind(&self) -> FoundationErrorKind {
        self.kind
    }
    pub fn context(&self) -> &ErrorContext {
        &self.context
    }
    pub(crate) fn add_cause(mut self, cause: String) -> Self {
        self.context.causes.push(cause);
        self
    }
}
impl fmt::Display for FoundationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}:{}]",
            self.context, self.context.source.file, self.context.source.line
        )
    }
}
impl Error for FoundationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.io_source.as_ref().map(|e| e as &(dyn Error + 'static))
    }
}

pub type Result<T> = std::result::Result<T, FoundationError>;

pub(crate) fn executable_directory() -> Result<PathBuf> {
    let executable =
        std::env::current_exe().map_err(|e| FoundationError::io("无法定位可执行文件", None, e))?;
    executable.parent().map(PathBuf::from).ok_or_else(|| {
        FoundationError::new(FoundationErrorKind::InvalidInput, "可执行文件没有父目录")
    })
}

/// 初始化两个进程内单例；配置恢复结果交给调用方决定如何呈现或记录。
/// 重复调用不会重载内存配置。任一步失败都返回错误，不打印到终端。
pub fn init() -> Result<ConfigLoadStatus> {
    LOGGER.init()?;
    CONFIG.init()
}

#[cfg(test)]
#[path = "../../depoly/tests/foundation/support.rs"]
mod test_support;

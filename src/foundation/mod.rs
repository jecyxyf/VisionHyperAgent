//! 基础层日志、配置及共享错误上下文，不接入界面或业务服务。
//!
//! ```no_run
//! use vision_hyper_agent::foundation::{self, CONFIG, LOGGER, ConfigLoadStatus};
//! # fn main() -> foundation::Result<()> {
//! foundation::with_logging(|| {
//! let status = foundation::init()?;
//! if let ConfigLoadStatus::Recovered(problem) = status {
//!     LOGGER.error("config", &format!("已备份并重建配置；{problem}"))?;
//! }
//! CONFIG.write("agent", "model", "model-name")?;
//! CONFIG.save()?;
//! LOGGER.info("app", "配置已保存")?;
//! Ok(())
//! })
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
    QueueFull,
    WorkerStopped,
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

/// Runtime files are ordinary files, not links, devices or directories.
/// This is a static path guard, not protection against hostile concurrent filesystem changes.
#[track_caller]
pub(crate) fn validate_regular_file(path: &Path, writable: bool) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                return Err(FoundationError::io(
                    "目标不是普通文件",
                    Some(path),
                    io::Error::new(io::ErrorKind::InvalidInput, "拒绝符号链接、目录及特殊文件"),
                ));
            }
            if writable && metadata.permissions().readonly() {
                return Err(FoundationError::io(
                    "目标文件只读",
                    Some(path),
                    io::Error::new(io::ErrorKind::PermissionDenied, "不覆盖或追加只读文件"),
                ));
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(FoundationError::io("无法检查文件路径", Some(path), error)),
    }
}

#[track_caller]
pub(crate) fn validate_directory(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(FoundationError::io(
            "日志路径不是普通目录",
            Some(path),
            io::Error::new(io::ErrorKind::InvalidInput, "拒绝符号链接及其他非目录路径"),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(FoundationError::io("无法检查日志目录", Some(path), error)),
    }
}

/// Applied only to files just created by this module; existing user permissions are preserved.
pub(crate) fn secure_new_file(file: &std::fs::File) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

pub(crate) fn executable_directory() -> Result<PathBuf> {
    let executable =
        std::env::current_exe().map_err(|e| FoundationError::io("无法定位可执行文件", None, e))?;
    executable.parent().map(PathBuf::from).ok_or_else(|| {
        FoundationError::new(FoundationErrorKind::InvalidInput, "可执行文件没有父目录")
    })
}

/// 托管整个程序的日志生命周期，不初始化配置、不接入界面或业务。
/// 闭包返回后自动停止接收、排空队列、同步文件并等待写入线程结束。
/// 闭包错误会以 ERROR 文本入队，后台失败通过本函数返回；不打印到终端。
/// 不支持嵌套或并发托管；业务后台线程必须在闭包返回前结束日志调用。
/// panic 展开时也尽力收尾；强杀、abort、process::exit 和断电不在此保证内。
pub fn with_logging<T>(run: impl FnOnce() -> Result<T>) -> Result<T> {
    LOGGER.run_scoped(run)
}

/// 初始化两个进程内单例；应在 with_logging 托管的应用作用域内调用。
/// 配置恢复结果交给调用方决定如何呈现或记录。
/// 重复调用不会重载内存配置。任一步失败都返回错误，不打印到终端。
pub fn init() -> Result<ConfigLoadStatus> {
    LOGGER.init()?;
    CONFIG.init()
}

//! 文件日志单例：四个级别、调用位置、按本地日期追加与 90 日保留。
use super::{
    ErrorContext, FoundationError, FoundationErrorKind, Result, SourceLocation,
    executable_directory,
};
use chrono::{DateTime, Days, FixedOffset, Local, NaiveDate};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

pub static LOGGER: LazyLock<Logger> = LazyLock::new(Logger::new);
const RETENTION_DAYS: u64 = 90;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}
impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
        })
    }
}
struct LogState {
    directory: PathBuf,
    day: NaiveDate,
    file: File,
    cleaned_for: Option<NaiveDate>,
}
/// 通过 LOGGER 调用。构造本身不进行文件 I/O，所有失败通过 Result 返回。
pub struct Logger {
    state: Mutex<Option<LogState>>,
}
impl Logger {
    fn new() -> Self {
        Self {
            state: Mutex::new(None),
        }
    }
    pub fn init(&self) -> Result<()> {
        self.init_directory(
            executable_directory()?.join("logs"),
            Local::now().date_naive(),
        )
    }
    fn init_directory(&self, directory: PathBuf, day: NaiveDate) -> Result<()> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| FoundationError::new(FoundationErrorKind::LockPoisoned, "日志锁已损坏"))?;
        if guard.is_some() {
            return Ok(());
        }
        fs::create_dir_all(&directory)
            .map_err(|e| FoundationError::io("无法创建日志目录", Some(&directory), e))?;
        let file = open_daily(&directory, day)?;
        clean_expired(&directory, day)?;
        *guard = Some(LogState {
            directory,
            day,
            file,
            cleaned_for: Some(day),
        });
        Ok(())
    }
    #[track_caller]
    pub fn debug(&self, module: &str, message: &str) -> Result<()> {
        self.record(
            LogLevel::Debug,
            module,
            message,
            SourceLocation::caller(),
            Local::now().fixed_offset(),
        )
    }
    #[track_caller]
    pub fn info(&self, module: &str, message: &str) -> Result<()> {
        self.record(
            LogLevel::Info,
            module,
            message,
            SourceLocation::caller(),
            Local::now().fixed_offset(),
        )
    }
    #[track_caller]
    pub fn warning(&self, module: &str, message: &str) -> Result<()> {
        self.record(
            LogLevel::Warning,
            module,
            message,
            SourceLocation::caller(),
            Local::now().fixed_offset(),
        )
    }
    #[track_caller]
    pub fn error(&self, module: &str, message: &str) -> Result<()> {
        self.record(
            LogLevel::Error,
            module,
            message,
            SourceLocation::caller(),
            Local::now().fixed_offset(),
        )
    }
    /// 使用创建上下文时的源码位置，而不是记录日志时的上层位置。
    pub fn error_with_context(
        &self,
        module: &str,
        message: &str,
        context: &ErrorContext,
    ) -> Result<()> {
        self.record(
            LogLevel::Error,
            module,
            &format!("{message}；{context}"),
            context.source,
            Local::now().fixed_offset(),
        )
    }
    fn record(
        &self,
        level: LogLevel,
        module: &str,
        message: &str,
        source: SourceLocation,
        now: DateTime<FixedOffset>,
    ) -> Result<()> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| FoundationError::new(FoundationErrorKind::LockPoisoned, "日志锁已损坏"))?;
        let state = guard.as_mut().ok_or_else(|| {
            FoundationError::new(FoundationErrorKind::NotInitialized, "日志尚未初始化")
        })?;
        let day = now.date_naive();
        if state.day != day {
            let file = open_daily(&state.directory, day)?;
            // Drop the previous handle before retention cleanup, including on Windows.
            state.file = file;
            state.day = day;
        }
        if state.cleaned_for != Some(day) {
            clean_expired(&state.directory, day)?;
            state.cleaned_for = Some(day);
        }
        let line = format!(
            "{} [{}] [{}] [{}:{}] {}\n",
            now.format("%Y-%m-%d %H:%M:%S%.3f"),
            level,
            escape(module),
            escape(source.file),
            source.line,
            escape(message)
        );
        let path = state.directory.join(log_name(day));
        state
            .file
            .write_all(line.as_bytes())
            .map_err(|e| FoundationError::io("日志写入失败", Some(&path), e))
    }
    pub fn flush(&self) -> Result<()> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| FoundationError::new(FoundationErrorKind::LockPoisoned, "日志锁已损坏"))?;
        let state = guard.as_mut().ok_or_else(|| {
            FoundationError::new(FoundationErrorKind::NotInitialized, "日志尚未初始化")
        })?;
        let path = state.directory.join(log_name(state.day));
        state
            .file
            .sync_all()
            .map_err(|e| FoundationError::io("日志刷新失败", Some(&path), e))
    }
}
fn log_name(day: NaiveDate) -> String {
    format!("{}.log", day.format("%Y-%m-%d"))
}
fn open_daily(directory: &Path, day: NaiveDate) -> Result<File> {
    let path = directory.join(log_name(day));
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| FoundationError::io("无法打开日志文件", Some(&path), e))
}
fn clean_expired(directory: &Path, today: NaiveDate) -> Result<()> {
    let cutoff = today
        .checked_sub_days(Days::new(RETENTION_DAYS - 1))
        .ok_or_else(|| {
            FoundationError::new(FoundationErrorKind::InvalidInput, "日期超出日志保留范围")
        })?;
    let entries = fs::read_dir(directory)
        .map_err(|e| FoundationError::io("无法读取日志目录", Some(directory), e))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| FoundationError::io("无法读取日志目录项", Some(directory), e))?;
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(|e| FoundationError::io("无法检查日志文件类型", Some(&path), e))?
            .is_file()
        {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".log") else {
            continue;
        };
        let Ok(day) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") else {
            continue;
        };
        if log_name(day) == name && day < cutoff {
            fs::remove_file(&path)
                .map_err(|e| FoundationError::io("无法清理过期日志", Some(&path), e))?;
        }
    }
    Ok(())
}
fn escape(value: &str) -> String {
    value.chars().flat_map(|c| c.escape_debug()).collect()
}

#[cfg(test)]
#[path = "../../depoly/tests/foundation/logger.rs"]
mod tests;

//! Global file logger used by the desktop host process.
//!
//! Logging is independent from the HTTP server and tray implementation so
//! startup failures remain observable even when the process exits before a UI
//! is available.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::{env, fmt};

use chrono::Local;
use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

const DATE_FORMAT: &str = "%Y-%m-%d";
const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.3f";

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Configuration for the process-wide logger.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    app_name: String,
    directory: PathBuf,
    level: LevelFilter,
}

impl LoggingConfig {
    /// Creates logging configuration for an application directory.
    pub fn new(app_name: impl Into<String>, directory: impl Into<PathBuf>) -> Self {
        Self {
            app_name: app_name.into(),
            directory: directory.into(),
            level: default_level(),
        }
    }

    /// Overrides the maximum enabled level.
    pub fn with_level(mut self, level: LevelFilter) -> Self {
        self.level = level;
        self
    }

    /// Applies the optional `VHA_LOG_LEVEL` environment override.
    pub fn with_environment_level(mut self) -> Self {
        if let Ok(value) = env::var("VHA_LOG_LEVEL") {
            if let Some(level) = parse_level(value.trim()) {
                self.level = level;
            }
        }

        self
    }
}

/// Errors that can prevent the global logger from starting.
#[derive(Debug)]
pub enum LoggingInitError {
    InvalidApplicationName,
    CreateDirectory {
        directory: PathBuf,
        source: std::io::Error,
    },
    OpenFile {
        path: PathBuf,
        source: std::io::Error,
    },
    RegisterLogger(SetLoggerError),
}

impl fmt::Display for LoggingInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidApplicationName => write!(f, "application name must not be empty"),
            Self::CreateDirectory { directory, source } => write!(
                f,
                "failed to create log directory {}: {source}",
                directory.display()
            ),
            Self::OpenFile { path, source } => {
                write!(f, "failed to open log file {}: {source}", path.display())
            }
            Self::RegisterLogger(source) => {
                write!(f, "failed to register global logger: {source}")
            }
        }
    }
}

impl std::error::Error for LoggingInitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDirectory { source, .. } | Self::OpenFile { source, .. } => Some(source),
            Self::RegisterLogger(_) | Self::InvalidApplicationName => None,
        }
    }
}

/// Initializes the global logger and returns today's log file path.
///
/// Calling this function more than once is successful and returns the path
/// owned by the already-initialized singleton.
pub fn initialize(config: LoggingConfig) -> Result<PathBuf, LoggingInitError> {
    if let Some(logger) = LOGGER.get() {
        return Ok(logger.path().to_path_buf());
    }

    let app_name =
        sanitize_app_name(&config.app_name).ok_or(LoggingInitError::InvalidApplicationName)?;
    let logs_dir = config.directory.join("logs");
    std::fs::create_dir_all(&logs_dir).map_err(|source| LoggingInitError::CreateDirectory {
        directory: logs_dir.clone(),
        source,
    })?;

    let logger = Logger::open(&app_name, &logs_dir, config.level)?;
    let logger = LOGGER.get_or_init(|| logger);
    log::set_logger(logger).map_err(LoggingInitError::RegisterLogger)?;
    log::set_max_level(config.level);
    install_panic_hook();

    Ok(logger.path().to_path_buf())
}

/// Creates portable logging configuration based beside the current executable.
pub fn config_for_executable(app_name: &str) -> LoggingConfig {
    LoggingConfig::new(app_name, application_directory())
        .with_level(default_level())
        .with_environment_level()
}

fn application_directory() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn default_level() -> LevelFilter {
    if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    }
}

fn parse_level(value: &str) -> Option<LevelFilter> {
    match value.to_ascii_lowercase().as_str() {
        "error" => Some(LevelFilter::Error),
        "warning" | "warn" => Some(LevelFilter::Warn),
        "info" => Some(LevelFilter::Info),
        "debug" => Some(LevelFilter::Debug),
        "trace" => Some(LevelFilter::Trace),
        _ => None,
    }
}

fn sanitize_app_name(value: &str) -> Option<String> {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();

    (!sanitized.trim_matches('-').is_empty()).then_some(sanitized)
}

fn install_panic_hook() {
    let previous_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        if let Some(logger) = LOGGER.get() {
            logger.log_panic(panic_info);
        }

        previous_hook(panic_info);
    }));
}

struct Logger {
    app_name: String,
    logs_dir: PathBuf,
    path: PathBuf,
    date: Mutex<String>,
    file: Mutex<File>,
    max_level: AtomicUsize,
}

impl Logger {
    fn open(app_name: &str, logs_dir: &Path, level: LevelFilter) -> Result<Self, LoggingInitError> {
        let date = Local::now().format(DATE_FORMAT).to_string();
        let path = log_path(app_name, logs_dir, &date);
        let file = open_log_file(&path)?;

        Ok(Self {
            app_name: app_name.to_string(),
            logs_dir: logs_dir.to_path_buf(),
            path,
            date: Mutex::new(date),
            file: Mutex::new(file),
            max_level: AtomicUsize::new(level as usize),
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write_record(&self, record: &Record<'_>) -> std::io::Result<()> {
        self.rotate_if_needed()?;

        let mut file = self.file.lock().expect("log file mutex poisoned");
        file.write_all(format_record(record).as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()
    }

    fn rotate_if_needed(&self) -> std::io::Result<()> {
        let date = Local::now().format(DATE_FORMAT).to_string();
        let mut current_date = self.date.lock().expect("log date mutex poisoned");

        if *current_date == date {
            return Ok(());
        }

        let path = log_path(&self.app_name, &self.logs_dir, &date);
        let file = open_log_file_for_rotation(&path)?;
        *self.file.lock().expect("log file mutex poisoned") = file;
        *current_date = date;

        Ok(())
    }

    fn log_panic(&self, panic_info: &std::panic::PanicHookInfo<'_>) {
        let Some(location) = panic_info.location() else {
            self.log_unknown_panic(panic_info);
            return;
        };

        let payload = panic_payload(panic_info);
        let message = format!("panic captured: {payload}");
        let arguments = format_args!("{message}");
        let record = Record::builder()
            .args(arguments)
            .level(Level::Error)
            .target("panic")
            .file(Some(location.file()))
            .line(Some(location.line()))
            .build();

        let _ = self.write_record(&record);
    }

    fn log_unknown_panic(&self, panic_info: &std::panic::PanicHookInfo<'_>) {
        let payload = panic_payload(panic_info);
        let message = format!("panic captured: {payload}");
        let arguments = format_args!("{message}");
        let record = Record::builder()
            .args(arguments)
            .level(Level::Error)
            .target("panic")
            .build();

        let _ = self.write_record(&record);
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        // Network dependency diagnostics can include complete frames, headers and prompts.
        // The application logs classified connection/request failures instead.
        let root = metadata.target().split("::").next().unwrap_or_default();
        !matches!(
            root,
            "tungstenite"
                | "tokio_tungstenite"
                | "reqwest"
                | "hyper"
                | "hyper_util"
                | "h2"
                | "rustls"
                | "axum"
                | "multer"
                | "tower_http"
        ) && metadata.level() as usize <= self.max_level.load(Ordering::Relaxed)
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        if let Err(error) = self.write_record(record) {
            eprintln!("failed to write log record: {error}");
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

fn open_log_file(path: &Path) -> Result<File, LoggingInitError> {
    open_log_file_for_rotation(path).map_err(|source| LoggingInitError::OpenFile {
        path: path.to_path_buf(),
        source,
    })
}

fn open_log_file_for_rotation(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn panic_payload(panic_info: &std::panic::PanicHookInfo<'_>) -> String {
    panic_info
        .payload()
        .downcast_ref::<&str>()
        .map(|value| (*value).to_string())
        .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic payload".to_string())
}

fn log_path(app_name: &str, logs_dir: &Path, date: &str) -> PathBuf {
    logs_dir.join(format!("{app_name}.{date}.log"))
}

fn format_record(record: &Record<'_>) -> String {
    let timestamp = Local::now().format(TIMESTAMP_FORMAT);
    let level = format_level(record.level());

    if matches!(record.level(), Level::Debug | Level::Warn | Level::Error) {
        let source = match (record.file(), record.line()) {
            (Some(_), Some(line)) => format!("[{}:{line}] ", record.target()),
            (Some(_), None) => format!("[{}] ", record.target()),
            (None, Some(line)) => format!("[{}:{line}] ", record.target()),
            _ => String::new(),
        };

        format!("{timestamp} [{level}] {source}{}", record.args())
    } else {
        format!("{timestamp} [{level}] {}", record.args())
    }
}

fn format_level(level: Level) -> &'static str {
    match level {
        Level::Error => "ERROR",
        Level::Warn => "WARNING",
        Level::Info => "INFO",
        Level::Debug => "DEBUG",
        Level::Trace => "TRACE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_uses_one_file_per_day() {
        let path = log_path("VisionHyperAgent", Path::new("/app/logs"), "2026-09-16");

        assert_eq!(path, Path::new("/app/logs/VisionHyperAgent.2026-09-16.log"));
    }

    #[test]
    fn parses_user_friendly_level_names() {
        assert_eq!(parse_level("error"), Some(LevelFilter::Error));
        assert_eq!(parse_level("WARNING"), Some(LevelFilter::Warn));
        assert_eq!(parse_level("info"), Some(LevelFilter::Info));
        assert_eq!(parse_level("debug"), Some(LevelFilter::Debug));
        assert_eq!(parse_level("unknown"), None);
    }

    #[test]
    fn formats_error_with_source_file_and_line() {
        let record = Record::builder()
            .args(format_args!("startup failed"))
            .level(Level::Error)
            .target("vha_server::main")
            .file(Some("/app/source/main.rs"))
            .line(Some(42))
            .build();
        let line = format_record(&record);

        assert!(line.ends_with(" [ERROR] [vha_server::main:42] startup failed"));
    }

    #[test]
    fn formats_info_without_source_location() {
        let record = Record::builder()
            .args(format_args!("server started"))
            .level(Level::Info)
            .target("vha_server::http_server")
            .build();
        let line = format_record(&record);

        assert!(line.ends_with(" [INFO] server started"));
    }

    #[test]
    fn formats_debug_and_warning_with_source_location() {
        let debug_record = Record::builder()
            .args(format_args!("debug self-test"))
            .level(Level::Debug)
            .target("vha_server::main")
            .file(Some("/app/source/main.rs"))
            .line(Some(31))
            .build();
        let warning_record = Record::builder()
            .args(format_args!("warning self-test"))
            .level(Level::Warn)
            .target("vha_server::main")
            .file(Some("/app/source/main.rs"))
            .line(Some(36))
            .build();

        assert!(format_record(&debug_record)
            .ends_with(" [DEBUG] [vha_server::main:31] debug self-test"));
        assert!(format_record(&warning_record)
            .ends_with(" [WARNING] [vha_server::main:36] warning self-test"));
    }
    #[test]
    fn transport_frames_never_enter_logs_even_at_trace_level() {
        let root = std::env::temp_dir().join(format!(
            "vha-log-privacy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let logger = Logger::open("Privacy", &root, LevelFilter::Trace).unwrap();
        let path = logger.path().to_path_buf();
        for target in [
            "tungstenite::protocol",
            "reqwest::connect",
            "hyper::proto",
            "axum::rejection",
        ] {
            logger.log(
                &Record::builder()
                    .args(format_args!("SENSITIVE_FRAME_MARKER"))
                    .level(Level::Error)
                    .target(target)
                    .build(),
            );
        }
        logger.log(
            &Record::builder()
                .args(format_args!("application diagnostic retained"))
                .level(Level::Debug)
                .target("vha_server::agent")
                .build(),
        );
        logger.flush();
        drop(logger);
        let content = std::fs::read_to_string(path).unwrap();
        assert!(!content.contains("SENSITIVE_FRAME_MARKER"));
        assert!(content.contains("application diagnostic retained"));
        std::fs::remove_dir_all(root).unwrap();
    }
}

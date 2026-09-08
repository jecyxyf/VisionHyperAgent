//! 异步文件日志单例；入口托管生命周期，正常退出时自动等待后台写完。
use super::{
    FoundationError, FoundationErrorKind, Result, SourceLocation, executable_directory,
    secure_new_file, validate_directory, validate_regular_file,
};
use chrono::{DateTime, Days, FixedOffset, Local, NaiveDate};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
};

pub static LOGGER: LazyLock<Logger> = LazyLock::new(Logger::new);
const RETENTION_DAYS: u64 = 90;
// 有界队列限制积压条数；满时明确返回错误，不阻塞业务线程或静默丢弃。
const QUEUE_CAPACITY: usize = 4096;

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
struct LogRecord {
    day: NaiveDate,
    text: String,
}
struct Worker {
    sender: SyncSender<LogRecord>,
    thread: JoinHandle<Result<()>>,
    failure: Arc<Mutex<Option<String>>>,
    managed: bool,
}
impl Worker {
    fn check(&self) -> Result<()> {
        let failure = self.failure.lock().map_err(|_| lock_error())?;
        if let Some(reason) = failure.as_ref() {
            return Err(worker_stopped().add_cause(reason.clone()));
        }
        if self.thread.is_finished() {
            return Err(worker_stopped());
        }
        Ok(())
    }
}
enum LoggerState {
    Idle,
    Running(Worker),
    Closing,
}
/// 通过 LOGGER 调用；四级方法成功仅表示已入队，不表示已完成文件写入。
/// 主程序须通过 foundation::with_logging 托管整个运行期及退出收尾。
pub struct Logger {
    state: Mutex<LoggerState>,
}
impl Logger {
    fn new() -> Self {
        Self {
            state: Mutex::new(LoggerState::Idle),
        }
    }
    /// 同步检查目录、打开文件并启动后台线程；运行中重复调用不重置队列。
    /// 单独初始化不等于托管退出，应用入口应使用 foundation::with_logging。
    pub fn init(&self) -> Result<()> {
        self.init_directory(
            executable_directory()?.join("logs"),
            Local::now().date_naive(),
        )
    }
    fn init_directory(&self, directory: PathBuf, day: NaiveDate) -> Result<()> {
        let mut guard = self.state.lock().map_err(|_| lock_error())?;
        match &*guard {
            LoggerState::Running(worker) => return worker.check(),
            LoggerState::Closing => return Err(not_initialized()),
            LoggerState::Idle => {}
        }
        let mut files = LogState::open(directory.clone(), day)?;
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let failure = Arc::new(Mutex::new(None));
        let worker_failure = Arc::clone(&failure);
        let thread = thread::Builder::new()
            .name("vha-logger".into())
            .spawn(move || {
                let result = write_records(&receiver, &mut files);
                if let Err(error) = &result {
                    // 仅保存诊断；不能递归向已经失效的文件写日志。
                    *worker_failure.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(error.to_string());
                }
                result
            })
            .map_err(|e| FoundationError::io("无法启动日志写入线程", Some(&directory), e))?;
        *guard = LoggerState::Running(Worker {
            sender,
            thread,
            failure,
            managed: false,
        });
        Ok(())
    }
    /// 入队 DEBUG 日志，自动捕获当前调用位置；队列满或服务失败时返回错误。
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
    /// 入队 INFO 日志，自动捕获当前调用位置；不执行文件 I/O。
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
    /// 入队 WARNING 日志，自动捕获当前调用位置；不执行文件 I/O。
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
    /// 入队 ERROR 日志；原因直接作为脱敏文本传入，不另设上下文写入接口。
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
    fn record(
        &self,
        level: LogLevel,
        module: &str,
        message: &str,
        source: SourceLocation,
        now: DateTime<FixedOffset>,
    ) -> Result<()> {
        // 在线程切换前固定时间、位置和文本，后台不重新捕获调用栈或借用业务数据。
        let text = format!(
            "{} [{}] [{}] [{}:{}] {}\n",
            now.format("%Y-%m-%d %H:%M:%S%.3f"),
            level,
            escape(module),
            escape(source.file),
            source.line,
            escape(message)
        );
        let guard = self.state.lock().map_err(|_| lock_error())?;
        let LoggerState::Running(worker) = &*guard else {
            return Err(not_initialized());
        };
        worker.check()?;
        // 与关闭入口共用状态锁；返回成功的记录必定处于待收尾队列内。
        match worker.sender.try_send(LogRecord {
            day: now.date_naive(),
            text,
        }) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(FoundationError::new(
                FoundationErrorKind::QueueFull,
                "日志队列已满，本条日志未入队",
            )),
            Err(TrySendError::Disconnected(_)) => Err(worker_stopped()),
        }
    }

    pub(super) fn run_scoped<T>(&self, run: impl FnOnce() -> Result<T>) -> Result<T> {
        self.init()?;
        let lifetime = {
            let mut state = self.state.lock().map_err(|_| lock_error())?;
            let LoggerState::Running(worker) = &mut *state else {
                return Err(not_initialized());
            };
            if worker.managed {
                return Err(FoundationError::new(
                    FoundationErrorKind::InvalidInput,
                    "日志生命周期不支持嵌套或并发托管",
                ));
            }
            worker.managed = true;
            LifetimeGuard {
                logger: self,
                active: true,
            }
        };
        let outcome = match run() {
            Ok(value) => Ok(value),
            Err(error) => match self.error("app", &error.to_string()) {
                Ok(()) => Err(error),
                Err(report_error) => Err(error.add_cause(report_error.to_string())),
            },
        };
        let completed = lifetime.complete();
        match (outcome, completed) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(close_error)) => Err(error.add_cause(close_error.to_string())),
        }
    }

    fn stop_worker(&self) -> Result<()> {
        let (worker, poisoned) = {
            // 锁中毒时仍取回线程所有权收尾，但不把损坏状态视为正常成功。
            let (mut state, poisoned) = match self.state.lock() {
                Ok(state) => (state, false),
                Err(error) => (error.into_inner(), true),
            };
            match std::mem::replace(&mut *state, LoggerState::Closing) {
                LoggerState::Running(worker) => (worker, poisoned),
                previous => {
                    *state = previous;
                    return Err(not_initialized());
                }
            }
        };
        // 先停止接收新记录，再断开发送端。接收端读完剩余队列后才结束。
        // join 时不持有日志锁，也没有定时丢弃尾部记录的退出策略。
        drop(worker.sender);
        let completed = worker
            .thread
            .join()
            .unwrap_or_else(|_| Err(worker_stopped()));
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = LoggerState::Idle;
        if poisoned {
            return match completed {
                Ok(()) => Err(lock_error()),
                Err(error) => Err(error.add_cause(lock_error().to_string())),
            };
        }
        completed
    }
}

struct LifetimeGuard<'a> {
    logger: &'a Logger,
    active: bool,
}
impl LifetimeGuard<'_> {
    fn complete(mut self) -> Result<()> {
        self.active = false;
        self.logger.stop_worker()
    }
}
impl Drop for LifetimeGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            // 正常路径由 complete 返回收尾错误；这里只为 panic 展开提供尽力收尾。
            let _ = self.logger.stop_worker();
        }
    }
}

fn write_records(receiver: &Receiver<LogRecord>, files: &mut LogState) -> Result<()> {
    for record in receiver {
        files.write(record)?;
    }
    files.synchronize()
}

impl LogState {
    fn open(directory: PathBuf, day: NaiveDate) -> Result<Self> {
        validate_directory(&directory)?;
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let mut builder = builder;
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&directory) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).map_err(
                        |e| FoundationError::io("无法设置日志目录权限", Some(&directory), e),
                    )?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(FoundationError::io("无法创建日志目录", Some(&directory), e)),
        }
        validate_directory(&directory)?;
        let file = open_daily(&directory, day)?;
        clean_expired(&directory, day)?;
        Ok(Self {
            directory,
            day,
            file,
            cleaned_for: Some(day),
        })
    }

    fn write(&mut self, record: LogRecord) -> Result<()> {
        if self.day != record.day {
            let file = open_daily(&self.directory, record.day)?;
            self.synchronize()?;
            // 替换并关闭旧句柄之后再清理，兼容 Windows 的文件共享限制。
            self.file = file;
            self.day = record.day;
        }
        if self.cleaned_for != Some(record.day) {
            clean_expired(&self.directory, record.day)?;
            self.cleaned_for = Some(record.day);
        }
        let path = self.directory.join(log_name(record.day));
        self.file
            .write_all(record.text.as_bytes())
            .map_err(|e| FoundationError::io("日志写入失败", Some(&path), e))
    }

    fn synchronize(&self) -> Result<()> {
        let path = self.directory.join(log_name(self.day));
        self.file
            .sync_all()
            .map_err(|e| FoundationError::io("日志文件同步失败", Some(&path), e))
    }
}

#[track_caller]
fn not_initialized() -> FoundationError {
    FoundationError::new(
        FoundationErrorKind::NotInitialized,
        "日志尚未初始化或已进入退出阶段",
    )
}
#[track_caller]
fn lock_error() -> FoundationError {
    FoundationError::new(FoundationErrorKind::LockPoisoned, "日志锁已损坏")
}
#[track_caller]
fn worker_stopped() -> FoundationError {
    FoundationError::new(FoundationErrorKind::WorkerStopped, "日志写入线程已停止")
}
fn log_name(day: NaiveDate) -> String {
    format!("{}.log", day.format("%Y-%m-%d"))
}
fn open_daily(directory: &Path, day: NaiveDate) -> Result<File> {
    validate_directory(directory)?;
    let path = directory.join(log_name(day));
    validate_regular_file(&path, true)?;
    let mut options = OpenOptions::new();
    options.create_new(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(file) => {
            secure_new_file(&file)
                .map_err(|e| FoundationError::io("无法设置日志文件权限", Some(&path), e))?;
            Ok(file)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            validate_regular_file(&path, true)?;
            OpenOptions::new()
                .append(true)
                .open(&path)
                .map_err(|e| FoundationError::io("无法打开日志文件", Some(&path), e))
        }
        Err(error) => Err(FoundationError::io("无法创建日志文件", Some(&path), error)),
    }
}

fn clean_expired(directory: &Path, today: NaiveDate) -> Result<()> {
    validate_directory(directory)?;
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
            validate_regular_file(&path, true)?;
            fs::remove_file(&path)
                .map_err(|e| FoundationError::io("无法清理过期日志", Some(&path), e))?;
        }
    }
    Ok(())
}
fn escape(value: &str) -> String {
    value.chars().flat_map(|c| c.escape_debug()).collect()
}

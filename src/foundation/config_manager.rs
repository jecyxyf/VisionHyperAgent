//! 全局 Agent 配置：显式保存、损坏备份恢复及无敏感内容的诊断。
use super::{
    ErrorContext, FoundationError, FoundationErrorKind, Result, executable_directory,
    secure_new_file, validate_regular_file,
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        LazyLock, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

pub static CONFIG: LazyLock<ConfigManager> = LazyLock::new(ConfigManager::new);
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}
impl fmt::Debug for AgentConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .finish()
    }
}
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub agent: AgentConfig,
}

/// 恢复状态只携带安全诊断，不携带 JSON 原文；调用方可将问题交给 LOGGER。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigLoadStatus {
    AlreadyInitialized,
    Loaded,
    Created,
    Recovered(Box<ErrorContext>),
}
struct ConfigState {
    path: PathBuf,
    value: AppConfig,
}
/// CONFIG 是公开单例。锁顺序固定为 io -> state，参数通过模块名和参数名访问。
pub struct ConfigManager {
    io: Mutex<()>,
    state: RwLock<Option<ConfigState>>,
}
impl ConfigManager {
    fn new() -> Self {
        Self {
            io: Mutex::new(()),
            state: RwLock::new(None),
        }
    }
    pub fn init(&self) -> Result<ConfigLoadStatus> {
        self.init_file(executable_directory()?.join("config.json"))
    }
    fn init_file(&self, path: PathBuf) -> Result<ConfigLoadStatus> {
        let _io = self.io.lock().map_err(|_| lock_error())?;
        if self.state.read().map_err(|_| lock_error())?.is_some() {
            return Ok(ConfigLoadStatus::AlreadyInitialized);
        }
        let (value, status) = read_or_recover(&path)?;
        *self.state.write().map_err(|_| lock_error())? = Some(ConfigState { path, value });
        Ok(status)
    }
    /// 重新加载会替换尚未保存的内存修改；I/O 失败不改变现有内存快照。
    pub fn load(&self) -> Result<ConfigLoadStatus> {
        let _io = self.io.lock().map_err(|_| lock_error())?;
        let path = self
            .state
            .read()
            .map_err(|_| lock_error())?
            .as_ref()
            .ok_or_else(|| not_initialized())?
            .path
            .clone();
        let (value, status) = read_or_recover(&path)?;
        *self.state.write().map_err(|_| lock_error())? = Some(ConfigState { path, value });
        Ok(status)
    }
    /// 读取指定参数的独立字符串；模块名和参数名区分大小写，未知项返回错误。
    /// api_key 按要求返回明文，调用方不得将其写入日志或诊断输出。
    pub fn read(&self, module: &str, parameter: &str) -> Result<String> {
        let guard = self.state.read().map_err(|_| lock_error())?;
        let agent = &guard.as_ref().ok_or_else(|| not_initialized())?.value.agent;
        let value = match (module, parameter) {
            ("agent", "base_url") => &agent.base_url,
            ("agent", "api_key") => &agent.api_key,
            ("agent", "model") => &agent.model,
            _ => return Err(unknown_parameter()),
        };
        Ok(value.clone())
    }
    /// 只修改指定参数的内存值，不自动创建未知项；持久化时另行调用 save。
    /// 三个参数均为字符串，允许将已知参数设置为空字符串。
    pub fn write(&self, module: &str, parameter: &str, value: &str) -> Result<()> {
        let mut guard = self.state.write().map_err(|_| lock_error())?;
        let agent = &mut guard.as_mut().ok_or_else(|| not_initialized())?.value.agent;
        let target = match (module, parameter) {
            ("agent", "base_url") => &mut agent.base_url,
            ("agent", "api_key") => &mut agent.api_key,
            ("agent", "model") => &mut agent.model,
            _ => return Err(unknown_parameter()),
        };
        *target = value.to_owned();
        Ok(())
    }
    /// 串行保存调用取得的快照。保存过程中发生的新修改仍需下一次显式保存。
    pub fn save(&self) -> Result<()> {
        let _io = self.io.lock().map_err(|_| lock_error())?;
        let (path, value) = {
            let guard = self.state.read().map_err(|_| lock_error())?;
            let state = guard.as_ref().ok_or_else(|| not_initialized())?;
            (state.path.clone(), state.value.clone())
        };
        atomic_write(&path, &serialize(&value)?)
    }
}
#[track_caller]
fn lock_error() -> FoundationError {
    FoundationError::new(FoundationErrorKind::LockPoisoned, "配置锁已损坏")
}
#[track_caller]
fn not_initialized() -> FoundationError {
    FoundationError::new(FoundationErrorKind::NotInitialized, "配置尚未初始化")
}
#[track_caller]
fn unknown_parameter() -> FoundationError {
    // 不回显传入内容，避免调用方误将 KEY 当作参数名时泄漏到诊断中。
    FoundationError::new(FoundationErrorKind::InvalidInput, "未知配置模块或参数")
}
fn serialize(value: &AppConfig) -> Result<Vec<u8>> {
    let mut data = serde_json::to_vec_pretty(value)
        .map_err(|_| FoundationError::new(FoundationErrorKind::InvalidInput, "配置序列化失败"))?;
    data.push(b'\n');
    Ok(data)
}
fn read_or_recover(path: &Path) -> Result<(AppConfig, ConfigLoadStatus)> {
    read_or_recover_with(path, atomic_write)
}

fn read_or_recover_with(
    path: &Path,
    mut persist: impl FnMut(&Path, &[u8]) -> Result<()>,
) -> Result<(AppConfig, ConfigLoadStatus)> {
    validate_regular_file(path, false)?;
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let value = AppConfig::default();
            persist(path, &serialize(&value)?)?;
            return Ok((value, ConfigLoadStatus::Created));
        }
        Err(error) => return Err(FoundationError::io("配置读取失败", Some(path), error)),
    };
    match serde_json::from_slice::<AppConfig>(&data) {
        Ok(value) => Ok((value, ConfigLoadStatus::Loaded)),
        Err(error) => {
            // serde's Data errors may embed values (including a KEY). Keep only category/position.
            let reason = match error.classify() {
                serde_json::error::Category::Io => "JSON 读取失败",
                serde_json::error::Category::Syntax => "JSON 语法错误",
                serde_json::error::Category::Data => "配置字段类型不匹配",
                serde_json::error::Category::Eof => "JSON 内容不完整",
            };
            let context = ErrorContext::new(reason)
                .with_data_file(path)
                .with_json_position(error.line(), error.column());
            let backup = path.with_file_name("config.json.back");
            persist(&backup, &data).map_err(|e| e.add_cause(context.to_string()))?;
            let value = AppConfig::default();
            persist(path, &serialize(&value)?)
                .map_err(|e| e.add_cause(format!("原配置已备份到 {}", backup.display())))?;
            Ok((value, ConfigLoadStatus::Recovered(Box::new(context))))
        }
    }
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_write_with(path, |file| file.write_all(bytes))
}
fn atomic_write_with(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> Result<()> {
    validate_regular_file(path, true)?;
    let parent = path.parent().ok_or_else(|| {
        FoundationError::new(FoundationErrorKind::InvalidInput, "配置路径没有父目录")
    })?;
    let name = path.file_name().ok_or_else(|| {
        FoundationError::new(FoundationErrorKind::InvalidInput, "配置路径没有文件名")
    })?;
    let mut temporary = None;
    for _ in 0..64 {
        let mut temp_name = OsString::from(".");
        temp_name.push(name);
        temp_name.push(format!(
            ".tmp-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let temp = parent.join(temp_name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temp) {
            Ok(file) => {
                temporary = Some((temp, file));
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(FoundationError::io("无法创建配置临时文件", Some(&temp), e)),
        }
    }
    let (temp, mut file) = temporary
        .ok_or_else(|| FoundationError::new(FoundationErrorKind::Io, "无法分配唯一配置临时文件"))?;
    let written = secure_new_file(&file)
        .and_then(|()| write(&mut file))
        .and_then(|()| file.sync_all());
    drop(file); // Required before replacement/cleanup on platforms with restrictive sharing.
    let result = written
        .map_err(|e| FoundationError::io("配置临时文件写入失败", Some(&temp), e))
        .and_then(|()| {
            validate_regular_file(path, true)?;
            fs::rename(&temp, path)
                .map_err(|e| FoundationError::io("配置文件替换失败", Some(path), e))
        });
    if let Err(error) = result {
        return match fs::remove_file(&temp) {
            Ok(()) => Err(error),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(error),
            Err(e) => Err(error.add_cause(format!("临时文件清理失败：{e}"))),
        };
    }
    Ok(())
}

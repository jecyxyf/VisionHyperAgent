//! 配置功能的 ViewModel：只负责配置初始化、读取、修改与保存。
use super::Result;
use crate::foundation::{CONFIG, ConfigLoadStatus};

/// 可独立于 MainWindowViewModel 和任何 UI 使用。
#[derive(Default)]
pub struct ConfigViewModel;

impl ConfigViewModel {
    pub fn new() -> Self {
        Self
    }

    /// 初始化配置；恢复损坏文件时返回安全提示，日志或呈现由调用方协调。
    pub fn init(&self) -> Result<Option<String>> {
        CONFIG.init().map(recovery_notice).map_err(Into::into)
    }

    /// 显式重读磁盘，替换未保存的内存参数；恢复时返回安全提示。
    pub fn load(&self) -> Result<Option<String>> {
        CONFIG.load().map(recovery_notice).map_err(Into::into)
    }

    /// 读取单个参数；KEY 返回明文，调用方不得写入日志。
    pub fn read(&self, module: &str, parameter: &str) -> Result<String> {
        CONFIG.read(module, parameter).map_err(Into::into)
    }

    /// 只修改内存参数；未知模块或参数返回错误，不自动保存。
    pub fn write(&self, module: &str, parameter: &str, value: &str) -> Result<()> {
        CONFIG.write(module, parameter, value).map_err(Into::into)
    }

    /// 显式保存配置，失败通过 Result 返回。
    pub fn save(&self) -> Result<()> {
        CONFIG.save().map_err(Into::into)
    }
}

fn recovery_notice(status: ConfigLoadStatus) -> Option<String> {
    match status {
        ConfigLoadStatus::Recovered(problem) => Some(format!(
            "配置已备份并恢复默认值；{problem} [{}:{}]",
            problem.source.file, problem.source.line,
        )),
        ConfigLoadStatus::Created
        | ConfigLoadStatus::Loaded
        | ConfigLoadStatus::AlreadyInitialized => None,
    }
}

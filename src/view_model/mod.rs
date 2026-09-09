//! 按功能划分的 ViewModel：各自管理对应界面的交互逻辑，不依赖具体 UI。
use crate::foundation::{FoundationError, FoundationErrorKind};
use std::{error::Error, fmt};

mod config_view_model;
mod main_window_view_model;

pub use config_view_model::ConfigViewModel;
pub use main_window_view_model::{MainPage, MainWindowViewModel};

/// View 和其他调用方只使用运行核心的错误接口，不直接构造基础层错误。
#[derive(Debug)]
pub struct ViewModelError(FoundationError);

impl ViewModelError {
    /// 将调用方的执行失败交给运行核心处理；原因必须是可安全记录的文本。
    #[track_caller]
    pub fn operation(message: &str, cause: impl fmt::Display) -> Self {
        Self(
            FoundationError::new(FoundationErrorKind::Platform, message)
                .add_cause(cause.to_string()),
        )
    }
}

impl fmt::Display for ViewModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for ViewModelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl From<FoundationError> for ViewModelError {
    fn from(error: FoundationError) -> Self {
        Self(error)
    }
}

pub type Result<T> = std::result::Result<T, ViewModelError>;

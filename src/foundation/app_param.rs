//! 软件自动维护的内部参数；不包含用户配置，不依赖 UI 类型。
use super::{
    FoundationError, FoundationErrorKind, Result, atomic_write, executable_directory,
    validate_regular_file,
};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf, sync::Mutex};

pub static APP_PARAM: AppParam = AppParam {
    state: Mutex::new(None),
};

/// 逻辑窗口尺寸与分隔偏好。长度按窗口对应轴归一化，原生比例原样保存。
/// None 表示沿用该分隔条默认值，不把自动布局固化成像素尺寸。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutGroup {
    pub window_width: f32,
    pub window_height: f32,
    pub navigation_ratio: Option<f32>,
    pub agent_ratio: Option<f32>,
    pub inference_records_ratio: Option<f32>,
    pub models_details_ratio: Option<f32>,
    pub annotation_list_ratio: Option<f32>,
    pub preannotation_editor_ratio: Option<f32>,
    pub preannotation_analysis_ratio: Option<f32>,
    pub training_parameters_ratio: Option<f32>,
    pub training_log_ratio: Option<f32>,
    pub training_loss_ratio: Option<f32>,
    pub chat_composer_ratio: Option<f32>,
}

impl LayoutGroup {
    pub fn validate(&self) -> Result<()> {
        let valid_size = [self.window_width, self.window_height]
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0);
        let valid_ratios = [
            self.navigation_ratio,
            self.agent_ratio,
            self.inference_records_ratio,
            self.models_details_ratio,
            self.annotation_list_ratio,
            self.preannotation_editor_ratio,
            self.preannotation_analysis_ratio,
            self.training_parameters_ratio,
            self.training_log_ratio,
            self.training_loss_ratio,
            self.chat_composer_ratio,
        ]
        .into_iter()
        .flatten()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(&value));
        if valid_size && valid_ratios {
            Ok(())
        } else {
            Err(FoundationError::new(
                FoundationErrorKind::InvalidInput,
                "布局参数无效：窗口尺寸须为有限正数，分隔比例须在 0 到 1 之间",
            ))
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Parameters {
    layout: Option<LayoutGroup>,
}

struct State {
    path: PathBuf,
    value: Parameters,
}

/// 进程内单例。参数修改只更新内存，由宿主在正常退出时保存。
pub struct AppParam {
    state: Mutex<Option<State>>,
}

impl AppParam {
    /// 加载程序目录下的 parameters.json；首次运行不写盘。
    /// 损坏内容先备份，再返回安全恢复提示；I/O 错误不会被当作损坏忽略。
    pub fn init(&self) -> Result<Option<String>> {
        let mut state = self.state.lock().map_err(|_| lock_error())?;
        if state.is_some() {
            return Ok(None);
        }
        let path = executable_directory()?.join("parameters.json");
        validate_regular_file(&path, false)?;
        let (value, notice) = match fs::read(&path) {
            Ok(bytes) => {
                let decoded = serde_json::from_slice::<Parameters>(&bytes).map_err(|error| {
                    FoundationError::new(
                        FoundationErrorKind::InvalidInput,
                        "软件参数 JSON 格式或字段类型错误",
                    )
                    .add_cause(format!(
                        "行 {}，列 {}",
                        error.line(),
                        error.column()
                    ))
                });
                let decoded = decoded.and_then(|value| {
                    if let Some(layout) = &value.layout {
                        layout.validate()?;
                    }
                    Ok(value)
                });
                match decoded {
                    Ok(value) => (value, None),
                    Err(error) => {
                        let backup = path.with_file_name("parameters.json.back");
                        atomic_write(&backup, &bytes)?;
                        (
                            Parameters::default(),
                            Some(format!(
                                "软件参数已备份至 {}，使用默认布局；{error}",
                                backup.display()
                            )),
                        )
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (Parameters::default(), None),
            Err(error) => {
                return Err(FoundationError::io("软件参数读取失败", Some(&path), error));
            }
        };
        *state = Some(State { path, value });
        Ok(notice)
    }

    pub fn layout(&self) -> Result<Option<LayoutGroup>> {
        let state = self.state.lock().map_err(|_| lock_error())?;
        Ok(state
            .as_ref()
            .ok_or_else(not_initialized)?
            .value
            .layout
            .clone())
    }

    /// 仅修改布局组，不写文件；其他参数组不应通过此接口更新。
    pub fn set_layout(&self, layout: LayoutGroup) -> Result<()> {
        layout.validate()?;
        let mut state = self.state.lock().map_err(|_| lock_error())?;
        state.as_mut().ok_or_else(not_initialized)?.value.layout = Some(layout);
        Ok(())
    }

    /// 序列化后以同目录临时文件替换，复用基础层已有的受保护写入。
    pub fn save(&self) -> Result<()> {
        let state = self.state.lock().map_err(|_| lock_error())?;
        let state = state.as_ref().ok_or_else(not_initialized)?;
        let mut bytes = serde_json::to_vec_pretty(&state.value).map_err(|_| {
            FoundationError::new(FoundationErrorKind::InvalidInput, "软件参数序列化失败")
        })?;
        bytes.push(b'\n');
        atomic_write(&state.path, &bytes)
    }
}

#[track_caller]
fn lock_error() -> FoundationError {
    FoundationError::new(FoundationErrorKind::LockPoisoned, "软件参数锁已损坏")
}

#[track_caller]
fn not_initialized() -> FoundationError {
    FoundationError::new(FoundationErrorKind::NotInitialized, "软件参数尚未初始化")
}

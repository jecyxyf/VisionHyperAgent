//! VisionHyperAgent 内部模块入口。
//!
//! View 连接按功能划分的 ViewModel：主窗口负责导航与协调，配置由 ConfigViewModel 管理。
//! 模型、相机、通信及训练运行环境尚未接入。

pub mod agent;
pub mod drivers;
pub mod foundation;
pub mod view;
pub mod view_model;

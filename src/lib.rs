//! VisionHyperAgent 内部模块入口。
//!
//! 主程序已接入 Slint 窗口并托管日志生命周期；
//! 配置初始化、模型和外部运行环境尚未接入主程序。

pub mod agent;
pub mod application;
pub mod drivers;
pub mod foundation;
pub mod view;
pub mod view_model;

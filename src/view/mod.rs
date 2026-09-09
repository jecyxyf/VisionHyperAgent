//! 原生 Slint 窗口入口；当前只承载已有界面，不接入训练、推理或 Agent 服务。
use crate::foundation::{FoundationError, FoundationErrorKind, Result};

slint::include_modules!();

/// 在调用线程中创建主窗口并运行事件循环；返回后交由应用入口自动收尾日志。
pub fn run() -> Result<()> {
    let window = MainWindow::new().map_err(|error| {
        FoundationError::new(FoundationErrorKind::Platform, "无法创建主窗口")
            .add_cause(error.to_string())
    })?;
    window.run().map_err(|error| {
        FoundationError::new(FoundationErrorKind::Platform, "界面事件循环执行失败")
            .add_cause(error.to_string())
    })
}

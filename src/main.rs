#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::process::ExitCode;
use vision_hyper_agent::{foundation, view};

fn main() -> ExitCode {
    let result = foundation::with_logging(view::run);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        // 业务错误由托管入口写入日志；日志本身不可写时不伪装成功或回退到终端。
        Err(_) => ExitCode::FAILURE,
    }
}

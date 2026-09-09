#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::process::ExitCode;
use vision_hyper_agent::view;

fn main() -> ExitCode {
    match view::run() {
        Ok(()) => ExitCode::SUCCESS,
        // 运行核心负责记录错误和基础服务收尾，入口只返回退出状态。
        Err(_) => ExitCode::FAILURE,
    }
}

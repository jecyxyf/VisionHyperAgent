use std::process::ExitCode;
use vision_hyper_agent::foundation::{self, LOGGER};

fn main() -> ExitCode {
    let result = foundation::with_logging(|| {
        LOGGER.info("app", "程序启动")?;
        println!("VisionHyperAgent 工程骨架已初始化；业务功能尚未实现。");
        LOGGER.info("app", "程序正常退出")?;
        Ok(())
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        // 业务错误由托管入口写入日志；日志本身不可写时不伪装成功或回退到终端。
        Err(_) => ExitCode::FAILURE,
    }
}

//! VisionHyperAgent local host process.
//!
//! This stage intentionally contains only the local HTTP server and system
//! tray lifecycle. Agent and Codex integration will be added in later stages.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_config;
mod browser;
mod http_server;
mod shutdown;
mod tray;

use crate::app_config::AppConfig;

fn main() {
    let logging_config = vha_common::logging::config_for_executable("VisionHyperAgent");
    let log_file = match vha_common::logging::initialize(logging_config) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to initialize logging: {error}");
            std::process::exit(1);
        }
    };

    log::info!(
        "VisionHyperAgent starting, version={}, os={}, arch={}, pid={}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::process::id()
    );
    log::debug!("logger initialized, file={}", log_file.display());
    log::debug!(target: "vha_server::main", "startup log self-test: DEBUG");
    log::info!("startup log self-test: INFO");
    log::warn!(target: "vha_server::main", "startup log self-test: WARNING");
    log::error!(target: "vha_server::main", "startup log self-test: ERROR");
    log::debug!(
        "executable={}, working_directory={}",
        std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("<unavailable: {error}>")),
        std::env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("<unavailable: {error}>"))
    );

    let config = AppConfig::local();
    log::info!(
        "local address configured, listen_addr={}",
        config.listen_addr
    );

    let server = match http_server::start(config.listen_addr) {
        Ok(server) => server,
        Err(error) => {
            log::error!("failed to start local server: {error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = tray::run(&config.base_url(), server) {
        log::error!("failed to run VisionHyperAgent: {error}");
        std::process::exit(1);
    }

    log::info!("VisionHyperAgent stopped");
}

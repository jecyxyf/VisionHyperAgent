//! VisionHyperAgent local host process.
//!
//! The backend owns Codex; browser tabs never own application lifetime.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod browser;
mod tray;

use vha_server::{app_config::AppConfig, codex_config::CodexSettings, http_server};

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

    let app_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let settings = CodexSettings::load(&app_dir);
    let server = match http_server::start_with_codex(config.listen_addr, settings) {
        Ok(server) => server,
        Err(error) => {
            log::error!("failed to start local server: {error}");
            std::process::exit(1);
        }
    };

    let result = if std::env::args().any(|arg| arg == "--headless") {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("signal runtime");
        if let Err(error) = runtime.block_on(wait_for_signal()) {
            log::error!("failed to wait for shutdown signal: {error}");
        }
        server.stop()
    } else {
        tray::run(&config.base_url(), server)
    };
    if let Err(error) = result {
        log::error!("failed to run VisionHyperAgent: {error}");
        std::process::exit(1);
    }

    log::info!("VisionHyperAgent stopped");
}

async fn wait_for_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {_ = terminate.recv()=>Ok(()), result=tokio::signal::ctrl_c()=>result}
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

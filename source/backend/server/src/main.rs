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
    let config = AppConfig::local();

    let server = match http_server::start(config.listen_addr) {
        Ok(server) => server,
        Err(error) => {
            eprintln!("failed to start local server: {error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = tray::run(&config.base_url(), server) {
        eprintln!("failed to run VisionHyperAgent: {error}");
        std::process::exit(1);
    }
}

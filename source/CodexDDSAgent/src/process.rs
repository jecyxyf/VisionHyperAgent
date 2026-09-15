use std::{io::Write, process::Stdio};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

use crate::error::Error;

pub struct ProcessSupervisor;

impl ProcessSupervisor {
    pub async fn start(
        binary: &std::path::Path,
        codex_home: &std::path::Path,
        log_path: &std::path::Path,
    ) -> Result<(ManagedProcess, String), Error> {
        if !binary.is_file() {
            return Err(Error::Rpc(crate::error::RpcError::new(
                "process_start_failed",
                "built-in codex-app-server is missing",
            )));
        }
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        let mut command = Command::new(binary);
        command
            .arg("--listen")
            .arg("ws://127.0.0.1:0")
            .env("CODEX_HOME", codex_home)
            .env("RUST_LOG", "info")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|err| {
            Error::Rpc(crate::error::RpcError::new(
                "process_start_failed",
                format!("failed to spawn codex-app-server: {err}"),
            ))
        })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::internal("codex-app-server stderr is unavailable"))?;
        let mut lines = BufReader::new(stderr).lines();

        let mut websocket_url = None;
        let mut logged_lines = Vec::new();
        while let Some(line) = lines.next_line().await.map_err(Error::Io)? {
            let line = strip_ansi(&line);
            if websocket_url.is_none() {
                websocket_url = parse_websocket_url(&line);
            }
            logged_lines.push(line);
            if websocket_url.is_some() {
                break;
            }
        }

        let websocket_url = websocket_url.ok_or_else(|| {
            Error::Rpc(crate::error::RpcError::new(
                "process_start_failed",
                "codex-app-server did not report its websocket address",
            ))
        })?;
        tokio::spawn(async move {
            let mut writer = std::io::BufWriter::new(log_file);
            for line in logged_lines {
                let _ = writeln!(writer, "{line}");
            }
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = writeln!(writer, "{}", strip_ansi(&line));
            }
        });

        Ok((ManagedProcess { child }, websocket_url))
    }
}

pub struct ManagedProcess {
    child: Child,
}

impl ManagedProcess {
    pub async fn stop(&mut self) -> Result<(), Error> {
        self.child.start_kill().map_err(Error::Io)?;
        self.child.wait().await.map_err(Error::Io)?;
        Ok(())
    }

    pub async fn wait(&mut self) -> Result<std::process::ExitStatus, Error> {
        self.child.wait().await.map_err(Error::Io)
    }
}

fn parse_websocket_url(line: &str) -> Option<String> {
    line.split_whitespace()
        .find_map(|token| token.strip_prefix("ws://"))
        .and_then(|value| value.parse::<std::net::SocketAddr>().ok())
        .map(|address| format!("ws://{address}"))
}

fn strip_ansi(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_loopback_websocket_address() {
        assert_eq!(
            parse_websocket_url("info: listening on: ws://127.0.0.1:54321"),
            Some("ws://127.0.0.1:54321".to_string())
        );
        assert_eq!(parse_websocket_url("invalid ws://example.com"), None);
    }
}

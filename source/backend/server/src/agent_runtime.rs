//! The only application layer allowed to own the Codex child process.
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use vha_codex_agent::{
    prepare as prepare_agent, CodexAgent, CodexProcess, LoadedAgentConfig, PreparedAgent,
    ProcessConfig,
};

use crate::{agent_service::AgentService, attachments::AttachmentStore, shutdown::ShutdownSignal};

const MAX_RANDOM_PORT_ATTEMPTS: usize = 20;

pub struct AgentRuntime {
    pub service: Arc<AgentService>,
    process_config: Option<ProcessConfig>,
}

impl AgentRuntime {
    pub async fn prepare(
        loaded: Result<LoadedAgentConfig, String>,
        app_dir: &Path,
        addr: SocketAddr,
    ) -> Self {
        let prepared = async {
            let loaded = loaded?;
            let resolved = Arc::new(loaded.config);
            let prepared = prepare_agent(resolved, app_dir, addr)?;
            let process = prepared.process.clone();
            let agent = Arc::new(prepared);
            let uploads = AttachmentStore::new(&agent.workspace).await?;
            Ok::<_, String>((agent, process, uploads))
        }
        .await;
        match prepared {
            Ok((agent, process, uploads)) => {
                let client = CodexAgent::new(agent.transport_config());
                Self {
                    service: AgentService::new(client, Some(agent), Some(Arc::new(uploads)), None),
                    process_config: Some(process),
                }
            }
            Err(error) => Self {
                service: AgentService::new(
                    CodexAgent::new(vha_codex_agent::TransportConfig::default()),
                    None,
                    None,
                    Some(error),
                ),
                process_config: None,
            },
        }
    }

    pub async fn run(self, mut shutdown: ShutdownSignal) {
        let receiver = self.service.client.subscribe_events();
        let events = tokio::spawn(self.service.clone().pump(receiver));
        let mut process: Option<CodexProcess> = None;
        if self.process_config.is_some() {
            let Some(agent) = self.service.agent.as_ref() else {
                unreachable!("prepared agent implies service configuration");
            };
            let mut startup = StartupOutcome::Fatal(String::new());
            for attempt in 1..=MAX_RANDOM_PORT_ATTEMPTS {
                startup = start_owned_codex(&self.service, agent, attempt, &mut shutdown).await;
                if !matches!(startup, StartupOutcome::Retry(_)) {
                    break;
                }
            }

            match startup {
                StartupOutcome::Ready(child) => {
                    process = Some(child);
                    let child = process.as_mut().expect("ready Codex child was just stored");
                    self.service
                        .process_status("running", child.pid(), None)
                        .await;
                    let mut tick = tokio::time::interval(Duration::from_millis(100));
                    loop {
                        tokio::select! {
                            _ = shutdown.wait() => break,
                            _ = tick.tick() => match child.try_wait() {
                                Ok(None) => {},
                                result => {
                                    log::error!(
                                        "Owned Codex process exited unexpectedly; status_available={}",
                                        result.is_ok()
                                    );
                                    let _ = self.service.client.disconnect().await;
                                    drop(process.take());
                                    self.service.process_status(
                                        "error",
                                        None,
                                        Some("Codex 进程已退出，请查看日志并重新启动应用".into()),
                                    ).await;
                                    shutdown.wait().await;
                                    break;
                                }
                            }
                        }
                    }
                }
                StartupOutcome::Stopped => {}
                StartupOutcome::Retry(reason) => {
                    let message = format!(
                        "Codex 随机端口连续 {MAX_RANDOM_PORT_ATTEMPTS} 次启动失败：{reason}"
                    );
                    log::error!("Codex startup failed: {message}");
                    self.service
                        .process_status("error", None, Some(message))
                        .await;
                    shutdown.wait().await;
                }
                StartupOutcome::Fatal(message) => {
                    log::error!("Codex startup failed: {message}");
                    self.service
                        .process_status("error", None, Some(message))
                        .await;
                    shutdown.wait().await;
                }
            }
        } else {
            shutdown.wait().await;
        }

        self.service.begin_shutdown().await;
        self.service.interrupt_all().await;
        let _ = self.service.client.disconnect().await;
        if let Some(child) = process.as_mut() {
            if let Err(error) = child.shutdown(Duration::from_secs(2)).await {
                log::error!("Codex shutdown failed: {error}");
            }
        }
        events.abort();
        let _ = events.await;
        if let Some(uploads) = &self.service.uploads {
            if let Err(error) = uploads.cleanup_unused().await {
                log::warn!("Attachment shutdown cleanup: {error}");
            }
        }
        self.service.process_status("stopped", None, None).await;
    }
}

enum StartupOutcome {
    Ready(CodexProcess),
    Stopped,
    Retry(String),
    Fatal(String),
}

fn allocate_random_local_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| format!("无法分配 Codex 随机端口（{error}）"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("无法读取 Codex 随机端口（{error}）"))?
        .port();
    drop(listener);
    if port == 0 {
        return Err("系统返回了无效的 Codex 随机端口".into());
    }
    Ok(port)
}

fn is_listening_port_conflict(child: &CodexProcess) -> bool {
    child
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.contains("listening port is already in use"))
}

async fn abandon_attempt(client: &CodexAgent, child: &mut CodexProcess) {
    let _ = client.disconnect().await;
    if let Err(error) = child.shutdown(Duration::from_secs(2)).await {
        log::warn!("Failed to shut down a rejected Codex attempt: {error}");
    }
}

async fn start_owned_codex(
    service: &Arc<AgentService>,
    agent: &Arc<PreparedAgent>,
    attempt: usize,
    shutdown: &mut ShutdownSignal,
) -> StartupOutcome {
    let port = match allocate_random_local_port() {
        Ok(port) => port,
        Err(message) => return StartupOutcome::Fatal(message),
    };
    let process = match agent.rebind_port(port) {
        Ok(process) => process,
        Err(message) => return StartupOutcome::Fatal(message),
    };
    if let Err(error) = service.client.replace_websocket_url(agent.websocket_url()) {
        return StartupOutcome::Fatal(format!("无法切换 Codex 随机端口：{error}"));
    }
    log::info!(
        "Preparing owned Codex process, attempt={attempt}, port={port}, executable={}",
        process.executable.display()
    );
    let mut child = match CodexProcess::spawn(&process).await {
        Ok(child) => child,
        Err(error) => {
            return StartupOutcome::Fatal(format!(
                "无法启动 Codex（系统错误 {}）",
                error.raw_os_error().unwrap_or_default()
            ))
        }
    };
    service
        .process_status("starting", child.pid(), Some("正在连接 Codex…".into()))
        .await;
    let connection = service.client.connect();
    tokio::pin!(connection);
    let deadline = tokio::time::sleep(Duration::from_secs(20));
    tokio::pin!(deadline);
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let startup = loop {
        tokio::select! {
            biased;
            _ = shutdown.wait() => break Ok(false),
            _ = &mut deadline => break Err("Codex 初始化超时".to_string()),
            _ = tick.tick() => match child.try_wait() {
                Ok(Some(status)) => break Err(format!(
                    "Codex 提前退出（{status}）；{}",
                    child.diagnostics().join("；")
                )),
                Err(_) => break Err("无法读取 Codex 进程状态".into()),
                _ => {},
            },
            result = &mut connection => break match result {
                Ok(()) if matches!(child.try_wait(), Ok(None)) => Ok(true),
                _ => {
                    let exited = child.try_wait().ok().flatten().is_some();
                    let diagnostics = child.diagnostics().join("；");
                    let suffix = if exited && !diagnostics.is_empty() {
                        format!("；{diagnostics}")
                    } else {
                        String::new()
                    };
                    Err(format!(
                        "Codex 连接或协议握手失败，请检查安装版本和配置{suffix}"
                    ))
                }
            },
        }
    };

    match startup {
        Ok(true) => StartupOutcome::Ready(child),
        Ok(false) => {
            abandon_attempt(&service.client, &mut child).await;
            StartupOutcome::Stopped
        }
        Err(message) => {
            let exited_with_conflict =
                matches!(child.try_wait(), Ok(Some(_))) && is_listening_port_conflict(&child);
            abandon_attempt(&service.client, &mut child).await;
            if exited_with_conflict && attempt < MAX_RANDOM_PORT_ATTEMPTS {
                log::warn!(
                    "Codex rejected the random port; selecting another, attempt={attempt}, port={port}, reason={message}"
                );
                StartupOutcome::Retry(message)
            } else {
                StartupOutcome::Fatal(message)
            }
        }
    }
}

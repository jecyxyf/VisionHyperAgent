//! The only application layer allowed to own the Codex child process.
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use vha_codex_agent::{
    prepare as prepare_agent, CodexAgent, CodexProcess, LoadedAgentConfig, ProcessConfig,
};

use crate::{agent_service::AgentService, attachments::AttachmentStore, shutdown::ShutdownSignal};

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
        if let Some(config) = self.process_config.clone() {
            let Some(agent) = self.service.agent.as_ref() else {
                unreachable!("prepared agent implies service configuration");
            };
            let port = agent.config.codex.port;
            // Refuse an occupied port before spawning. The per-start capability token also
            // prevents accidentally connecting to a different process in the bind race window.
            let preflight =
                std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)));
            let start = match preflight {
                Err(_) => Err("Codex 端口已被占用，请修改配置或关闭对应实例".to_string()),
                Ok(probe) => {
                    drop(probe);
                    CodexProcess::spawn(&config).await.map_err(|e| {
                        format!(
                            "无法启动 Codex（系统错误 {}）",
                            e.raw_os_error().unwrap_or_default()
                        )
                    })
                }
            };
            match start {
                Ok(child) => process = Some(child),
                Err(message) => {
                    self.service
                        .process_status("error", None, Some(message))
                        .await
                }
            }
        }
        if let Some(child) = process.as_mut() {
            self.service
                .process_status("starting", child.pid(), Some("正在连接 Codex…".into()))
                .await;
            let connection = self.service.client.connect();
            tokio::pin!(connection);
            let deadline = tokio::time::sleep(Duration::from_secs(20));
            tokio::pin!(deadline);
            let mut tick = tokio::time::interval(Duration::from_millis(100));
            let startup = loop {
                tokio::select! {
                    biased;
                    _=shutdown.wait()=>break Ok(false),
                    _=&mut deadline=>break Err("Codex 初始化超时".to_string()),
                    _=tick.tick()=>match child.try_wait() {
                        Ok(Some(status))=>break Err(format!("Codex 提前退出（{status}）；{}",child.diagnostics().join("；"))),
                        Err(_)=>break Err("无法读取 Codex 进程状态".into()),
                        _=>{},
                    },
                    result=&mut connection=>break match result {
                        Ok(()) if matches!(child.try_wait(),Ok(None))=>Ok(true),
                        _=>Err("Codex 连接或协议握手失败，请检查安装版本和配置".to_string()),
                    },
                }
            };
            match startup {
                Ok(true) => {
                    self.service
                        .process_status("running", child.pid(), None)
                        .await;
                    loop {
                        tokio::select! {
                            _=shutdown.wait()=>break,
                            _=tick.tick()=>match child.try_wait() {
                                Ok(None)=>{},
                                result=>{
                                    log::error!("Owned Codex process exited unexpectedly; status_available={}",result.is_ok());
                                    let _=self.service.client.disconnect().await;
                                    drop(process.take());
                                    self.service.process_status("error",None,Some("Codex 进程已退出，请查看日志并重新启动应用".into())).await;
                                    shutdown.wait().await;break;
                                }
                            }
                        }
                    }
                }
                Ok(false) => {}
                Err(message) => {
                    log::error!("Codex startup failed: {message}");
                    let _ = self.service.client.disconnect().await;
                    let _ = child.shutdown(Duration::from_secs(2)).await;
                    drop(process.take());
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

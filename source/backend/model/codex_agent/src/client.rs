use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc, Mutex, RwLock,
};

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{timeout, Instant};

use crate::websocket::{Actor, Command, Request};
use crate::{
    AgentError, AgentEvent, AgentState, ApprovalDecision, ConnectionPhase, Model, Page, Result,
    ServerRequest, Thread, ThreadOptions, TransportConfig, Turn, TurnInput,
};

struct Running {
    commands: mpsc::Sender<Command>,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}

struct Inner {
    config: RwLock<TransportConfig>,
    state: watch::Sender<AgentState>,
    events: broadcast::Sender<AgentEvent>,
    running: Mutex<Option<Running>>,
    next_id: AtomicI64,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(running) = self.running.get_mut().unwrap().take() {
            let _ = running.stop.send(true);
            running.task.abort();
        }
    }
}

/// Cloneable client; transport ownership ends with the last clone or explicit disconnect.
#[derive(Clone)]
pub struct CodexAgent {
    inner: Arc<Inner>,
}

impl CodexAgent {
    pub fn new(config: TransportConfig) -> Self {
        let (state, _) = watch::channel(AgentState::default());
        let (events, _) = broadcast::channel(config.event_capacity.max(1));
        Self {
            inner: Arc::new(Inner {
                config: RwLock::new(config),
                state,
                events,
                running: Mutex::new(None),
                next_id: AtomicI64::new(1),
            }),
        }
    }

    pub fn status(&self) -> AgentState {
        self.inner.state.borrow().clone()
    }

    /// Lag is reported by the receiver; consumers must fetch a fresh snapshot after lag.
    pub fn subscribe_events(&self) -> broadcast::Receiver<AgentEvent> {
        self.inner.events.subscribe()
    }

    pub async fn connect(&self) -> Result<()> {
        let config = self.inner.config.read().unwrap().clone();
        if config.connect_timeout.is_zero() || config.request_timeout.is_zero() {
            return Err(AgentError::Configuration("timeouts must be positive"));
        }
        let mut state = self.inner.state.subscribe();
        {
            let mut running = self.inner.running.lock().unwrap();
            if self.status().phase == ConnectionPhase::Disconnecting {
                return Err(AgentError::Busy);
            }
            if running.as_ref().is_none_or(|run| run.task.is_finished()) {
                let (commands, receiver) = mpsc::channel(128);
                let (stop, cancel) = watch::channel(false);
                self.inner
                    .state
                    .send_modify(|s| s.phase = ConnectionPhase::Connecting);
                let actor = Actor {
                    config,
                    commands: receiver,
                    stop: cancel,
                    state: self.inner.state.clone(),
                    events: self.inner.events.clone(),
                };
                *running = Some(Running {
                    commands,
                    stop,
                    task: tokio::spawn(actor.run()),
                });
            }
        }
        loop {
            match state.borrow_and_update().phase {
                ConnectionPhase::Ready => return Ok(()),
                ConnectionPhase::Error => return Err(AgentError::ConnectionFailed),
                ConnectionPhase::Disconnected | ConnectionPhase::Disconnecting => {
                    return Err(AgentError::Disconnected)
                }
                _ => {}
            }
            state
                .changed()
                .await
                .map_err(|_| AgentError::Disconnected)?;
        }
    }

    pub async fn disconnect(&self) -> Result<()> {
        let running = {
            let mut running = self.inner.running.lock().unwrap();
            self.inner
                .state
                .send_modify(|state| state.phase = ConnectionPhase::Disconnecting);
            running.take()
        };
        if let Some(mut running) = running {
            let _ = running.stop.send(true);
            if timeout(std::time::Duration::from_secs(3), &mut running.task)
                .await
                .is_err()
            {
                running.task.abort();
                let _ = running.task.await;
            }
        }
        if self.inner.running.lock().unwrap().is_none() {
            self.inner
                .state
                .send_modify(|state| state.phase = ConnectionPhase::Disconnected);
        }
        Ok(())
    }

    /// Replaces only the transport endpoint between isolated startup attempts.
    ///
    /// The caller must disconnect first. Capacity and timeout semantics stay fixed for
    /// the lifetime of this client, which keeps active event receivers valid.
    pub fn replace_websocket_url(&self, websocket_url: String) -> Result<()> {
        if websocket_url.trim().is_empty() {
            return Err(AgentError::Configuration("websocket URL must not be empty"));
        }
        let running = self.inner.running.lock().unwrap();
        if running.as_ref().is_some_and(|run| !run.task.is_finished()) {
            return Err(AgentError::Busy);
        }
        self.inner.config.write().unwrap().websocket_url = websocket_url;
        Ok(())
    }

    async fn call<T: DeserializeOwned>(&self, method: &'static str, params: Value) -> Result<T> {
        let state = self.status();
        if state.phase != ConnectionPhase::Ready {
            return Err(AgentError::Disconnected);
        }
        let tx = self.commands()?;
        let (reply, rx) = oneshot::channel();
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let request = Request {
            id,
            method,
            params,
            connection_id: state.connection_id,
            deadline: Instant::now() + self.inner.config.read().unwrap().request_timeout,
            reply,
        };
        let request_timeout = self.inner.config.read().unwrap().request_timeout;
        let value = timeout(request_timeout, async {
            tx.send(Command::Request(request))
                .await
                .map_err(|_| AgentError::Disconnected)?;
            rx.await.map_err(|_| AgentError::Disconnected)?
        })
        .await
        .map_err(|_| AgentError::Timeout)??;
        serde_json::from_value(value).map_err(|_| AgentError::Protocol)
    }

    fn commands(&self) -> Result<mpsc::Sender<Command>> {
        self.inner
            .running
            .lock()
            .unwrap()
            .as_ref()
            .map(|run| run.commands.clone())
            .ok_or(AgentError::Disconnected)
    }

    pub async fn list_models(&self, cursor: Option<&str>) -> Result<Page<Model>> {
        self.call("model/list", json!({"cursor":cursor,"limit":100}))
            .await
    }

    pub async fn create_thread(&self, options: &ThreadOptions) -> Result<Thread> {
        #[derive(serde::Deserialize)]
        struct Response {
            thread: Thread,
        }
        Ok(self
            .call::<Response>(
                "thread/start",
                serde_json::to_value(options).map_err(|_| AgentError::Protocol)?,
            )
            .await?
            .thread)
    }

    pub async fn resume_thread(&self, id: &str, options: &ThreadOptions) -> Result<Thread> {
        #[derive(serde::Deserialize)]
        struct Response {
            thread: Thread,
        }
        let mut params = serde_json::to_value(options).map_err(|_| AgentError::Protocol)?;
        params["threadId"] = json!(id);
        Ok(self.call::<Response>("thread/resume", params).await?.thread)
    }

    pub async fn read_thread(&self, id: &str) -> Result<Thread> {
        #[derive(serde::Deserialize)]
        struct Response {
            thread: Thread,
        }
        Ok(self
            .call::<Response>("thread/read", json!({"threadId":id,"includeTurns":true}))
            .await?
            .thread)
    }

    pub async fn list_threads(&self, cursor: Option<&str>, cwd: &str) -> Result<Page<Thread>> {
        self.call(
            "thread/list",
            json!({"cursor":cursor,"limit":30,"cwd":cwd,"archived":false,"modelProviders":[]}),
        )
        .await
    }

    pub async fn archive_thread(&self, id: &str) -> Result<()> {
        self.call::<Value>("thread/archive", json!({"threadId":id}))
            .await
            .map(|_| ())
    }

    /// Release a never-submitted draft, which may not yet have a rollout to archive.
    pub async fn unsubscribe_thread(&self, id: &str) -> Result<()> {
        self.call::<Value>("thread/unsubscribe", json!({"threadId":id}))
            .await
            .map(|_| ())
    }

    pub async fn start_turn(&self, input: &TurnInput) -> Result<Turn> {
        #[derive(serde::Deserialize)]
        struct Response {
            turn: Turn,
        }
        Ok(self
            .call::<Response>(
                "turn/start",
                serde_json::to_value(input).map_err(|_| AgentError::Protocol)?,
            )
            .await?
            .turn)
    }

    pub async fn interrupt_turn(&self, thread_id: &str, turn_id: &str) -> Result<()> {
        self.call::<Value>(
            "turn/interrupt",
            json!({"threadId":thread_id,"turnId":turn_id}),
        )
        .await
        .map(|_| ())
    }

    pub async fn list_skills(&self, cwd: &str) -> Result<Value> {
        self.call("skills/list", json!({"cwds":[cwd]})).await
    }

    pub async fn respond(&self, request: &ServerRequest, result: Result<Value>) -> Result<()> {
        let (reply, rx) = oneshot::channel();
        let command = Command::Respond {
            request: request.clone(),
            result,
            reply,
        };
        let request_timeout = self.inner.config.read().unwrap().request_timeout;
        timeout(request_timeout, async {
            self.commands()?
                .send(command)
                .await
                .map_err(|_| AgentError::Disconnected)?;
            rx.await.map_err(|_| AgentError::Disconnected)?.map(|_| ())
        })
        .await
        .map_err(|_| AgentError::Timeout)?
    }

    pub async fn approve(&self, request: &ServerRequest, decision: ApprovalDecision) -> Result<()> {
        if !matches!(
            request.method.as_str(),
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
        ) {
            return Err(AgentError::UnsupportedMethod);
        }
        self.respond(request, Ok(json!({"decision":decision})))
            .await
    }
}

impl CodexAgent {
    pub fn experimental_api_enabled(&self) -> bool {
        self.inner.config.read().unwrap().experimental_api
    }

    pub async fn list_background_terminals(
        &self,
        thread_id: &str,
        cursor: Option<&str>,
    ) -> Result<Page<crate::BackgroundTerminal>> {
        if !self.inner.config.read().unwrap().experimental_api {
            return Err(AgentError::UnsupportedMethod);
        }
        self.call(
            "thread/backgroundTerminals/list",
            json!({"threadId":thread_id,"cursor":cursor,"limit":100}),
        )
        .await
    }

    pub async fn terminate_background_terminal(
        &self,
        thread_id: &str,
        process_id: &str,
    ) -> Result<bool> {
        if !self.inner.config.read().unwrap().experimental_api {
            return Err(AgentError::UnsupportedMethod);
        }
        #[derive(serde::Deserialize)]
        struct Response {
            terminated: bool,
        }
        Ok(self
            .call::<Response>(
                "thread/backgroundTerminals/terminate",
                json!({"threadId":thread_id,"processId":process_id}),
            )
            .await?
            .terminated)
    }
}

//! Application-facing Agent operations. No method here starts or kills a process.
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;
use vha_codex_agent::{
    AgentError, AgentEvent, ApprovalPolicy, CodexAgent, ConnectionPhase, Model, PreparedAgent,
    SandboxMode, ServerRequest, Thread, ThreadOptions, Turn, TurnInput, UserInput,
};

use crate::attachments::AttachmentStore;

#[derive(Debug, Clone, Serialize)]
pub struct ServiceError {
    pub code: &'static str,
    pub message: String,
}
pub type ServiceResult<T> = Result<T, ServiceError>;
impl ServiceError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl From<AgentError> for ServiceError {
    fn from(error: AgentError) -> Self {
        log::warn!("Agent operation failed: {error}");
        match error {
            AgentError::Timeout | AgentError::Disconnected => Self::new(
                "outcome_unknown",
                "连接中断或请求超时，任务可能已经开始。请先同步状态，不要重复发送。",
            ),
            AgentError::StaleRequest => {
                Self::new("stale_interaction", "该请求已失效或已被其他页面处理")
            }
            AgentError::Busy => Self::new("busy", "Agent 正忙，请稍后再试"),
            AgentError::Rpc { code, message } => {
                let lower = message.to_lowercase();
                if lower.contains("effort") {
                    Self::new("codex_rpc", "Codex 拒绝了当前 Effort，请选择模型支持的档位")
                } else {
                    Self::new(
                        "codex_rpc",
                        format!("Codex 请求失败（错误码 {code}）；请检查模型配置和当前会话状态"),
                    )
                }
            }
            _ => Self::new("codex_error", error.to_string()),
        }
    }
}

struct ActiveTurn {
    ticket: Uuid,
    turn: Option<Turn>,
    uncertain: bool,
    stop_requested: bool,
}

struct State {
    process_phase: &'static str,
    message: Option<String>,
    pid: Option<u32>,
    last_connection: u64,
    active: HashMap<String, ActiveTurn>,
    archiving: HashSet<String>,
    cancelling: HashSet<String>,
    command_items: HashMap<(String, String), HashSet<String>>,
    // Newly created empty threads need not have a persisted rollout yet.
    empty_threads: HashMap<String, Thread>,
    interactions: HashMap<String, ServerRequest>,
    models: Vec<Model>,
}

pub struct AgentService {
    pub client: CodexAgent,
    pub agent: Option<Arc<PreparedAgent>>,
    pub uploads: Option<Arc<AttachmentStore>>,
    state: Mutex<State>,
    events: broadcast::Sender<Value>,
    pub closing: AtomicBool,
    resyncing: AtomicBool,
}

impl AgentService {
    pub fn new(
        client: CodexAgent,
        agent: Option<Arc<PreparedAgent>>,
        uploads: Option<Arc<AttachmentStore>>,
        error: Option<String>,
    ) -> Arc<Self> {
        let models = agent.as_ref().map(|agent| agent.config.browser_models());
        let (events, _) = broadcast::channel(512);
        Arc::new(Self {
            client,
            agent,
            uploads,
            events,
            closing: AtomicBool::new(false),
            resyncing: AtomicBool::new(false),
            state: Mutex::new(State {
                process_phase: if error.is_some() { "error" } else { "starting" },
                message: error,
                pid: None,
                last_connection: 0,
                active: HashMap::new(),
                archiving: HashSet::new(),
                cancelling: HashSet::new(),
                command_items: HashMap::new(),
                empty_threads: HashMap::new(),
                interactions: HashMap::new(),
                models: models.unwrap_or_default(),
            }),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }

    pub fn sanitize(&self, mut value: Value) -> Value {
        if let Some(agent) = &self.agent {
            agent.redact(&mut value);
        }
        value
    }

    pub fn emit(&self, value: Value) {
        let _ = self.events.send(self.sanitize(value));
    }

    pub async fn process_status(
        &self,
        phase: &'static str,
        pid: Option<u32>,
        message: Option<String>,
    ) {
        let mut state = self.state.lock().await;
        state.process_phase = phase;
        state.pid = pid;
        state.message = message;
        let lost = if matches!(phase, "error" | "stopped") && pid.is_none() {
            let ids: Vec<_> = state.active.keys().cloned().collect();
            state.active.clear();
            state.interactions.clear();
            state.cancelling.clear();
            ids
        } else {
            vec![]
        };
        drop(state);
        if !lost.is_empty() {
            self.emit(json!({"type":"process_lost","threads":lost}));
        }
        self.publish_status().await;
    }

    pub async fn snapshot(&self) -> Value {
        let state = self.state.lock().await;
        let connection = self.client.status();
        let phase = if state.process_phase == "running" {
            match connection.phase {
                ConnectionPhase::Ready => "ready",
                ConnectionPhase::Error => "error",
                ConnectionPhase::Reconnecting => "reconnecting",
                _ => "connecting",
            }
        } else {
            state.process_phase
        };
        let mut active:HashMap<_,_>=state.active.iter().map(|(id,slot)|(id,json!({"turn":slot.turn,"uncertain":slot.uncertain,"stopRequested":slot.stop_requested}))).collect();
        for id in &state.cancelling {
            active
                .entry(id)
                .or_insert(json!({"turn":null,"uncertain":false,"stopRequested":true}));
        }
        let interactions: Vec<_> = state
            .interactions
            .iter()
            .map(|(key, request)| json!({"key":key,"request":request}))
            .collect();
        self.sanitize(json!({
            "phase":phase,"message":state.message,"pid":state.pid,"connection":connection,
            "models":state.models,"activeTurns":active,"interactions":interactions,
        }))
    }

    async fn publish_status(&self) {
        self.emit(json!({"type":"status","data":self.snapshot().await}));
    }

    fn options(&self) -> ServiceResult<ThreadOptions> {
        let agent = self
            .agent
            .as_ref()
            .ok_or_else(|| ServiceError::new("configuration", "Agent 尚未配置"))?;
        Ok(ThreadOptions {
            model: Some(agent.config.active_model_id.clone()),
            cwd: agent.workspace.to_string_lossy().into_owned(),
            approval_policy: ApprovalPolicy::OnRequest,
            sandbox: SandboxMode::WorkspaceWrite,
        })
    }

    async fn ensure_ready(&self) -> ServiceResult<()> {
        if self.closing.load(Ordering::Acquire) {
            return Err(ServiceError::new("closing", "程序正在退出"));
        }
        if self.state.lock().await.process_phase != "running"
            || self.client.status().phase != ConnectionPhase::Ready
        {
            return Err(ServiceError::new(
                "not_ready",
                "Agent 尚未就绪，请检查连接状态和后端配置",
            ));
        }
        Ok(())
    }

    pub async fn call(self: &Arc<Self>, method: &str, params: Value) -> ServiceResult<Value> {
        if method == "status" {
            return Ok(self.snapshot().await);
        }
        if method == "turn.interrupt" {
            return self.interrupt(&identifier(&params, "threadId")?).await;
        }
        self.ensure_ready().await?;
        let result = match method {
            "models.list" => {
                json!({"data":self.state.lock().await.models.clone()})
            }
            "thread.create" => {
                let thread = self.client.create_thread(&self.options()?).await?;
                let mut state = self.state.lock().await;
                if state.empty_threads.len() >= 128 {
                    if let Some(id) = state
                        .empty_threads
                        .values()
                        .min_by_key(|t| t.created_at)
                        .map(|t| t.id.clone())
                    {
                        state.empty_threads.remove(&id);
                    }
                }
                state
                    .empty_threads
                    .insert(thread.id.clone(), thread.clone());
                json!({"thread":thread})
            }
            "thread.list" => {
                let cursor = params.get("cursor").and_then(Value::as_str);
                if cursor.is_some_and(|s| s.len() > 2048) {
                    return Err(ServiceError::new("bad_request", "历史分页参数过长"));
                }
                let mut page = self
                    .client
                    .list_threads(cursor, &self.options()?.cwd)
                    .await?;
                if cursor.is_none() {
                    let ids: HashSet<_> = page.data.iter().map(|t| t.id.clone()).collect();
                    let state = self.state.lock().await;
                    page.data.extend(
                        state
                            .empty_threads
                            .values()
                            .filter(|t| !ids.contains(&t.id))
                            .cloned(),
                    );
                    page.data.sort_by_key(|t| std::cmp::Reverse(t.updated_at));
                }
                json!(page)
            }
            "thread.read" | "thread.resume" => {
                let id = identifier(&params, "threadId")?;
                let thread = self.read_thread(&id, method == "thread.resume").await?;
                json!({"thread":thread})
            }
            "thread.archive" => {
                let id = identifier(&params, "threadId")?;
                self.archive(&id).await?;
                json!({})
            }
            "turn.start" => {
                self.start_turn(
                    serde_json::from_value(params)
                        .map_err(|_| ServiceError::new("bad_request", "消息参数格式错误"))?,
                )
                .await?
            }
            "interaction.reply" => self.reply_interaction(params).await?,
            "skills.list" => self.client.list_skills(&self.options()?.cwd).await?,
            _ => return Err(ServiceError::new("unknown_method", "不支持的 Agent 操作")),
        };
        Ok(self.sanitize(result))
    }

    async fn archive(&self, id: &str) -> ServiceResult<()> {
        {
            let mut state = self.state.lock().await;
            if state.active.contains_key(id)
                || state.cancelling.contains(id)
                || !state.archiving.insert(id.into())
            {
                return Err(ServiceError::new("busy", "请先停止会话中的任务，再归档"));
            }
        }
        let result = async {
            self.read_thread(id, false).await?;
            if self.state.lock().await.active.contains_key(id) {
                return Err(ServiceError::new("busy", "会话仍有运行中的任务"));
            }
            let draft = self.state.lock().await.empty_threads.contains_key(id);
            if draft {
                return self
                    .client
                    .unsubscribe_thread(id)
                    .await
                    .map_err(ServiceError::from);
            }
            self.client
                .archive_thread(id)
                .await
                .map_err(ServiceError::from)
        }
        .await;
        let mut state = self.state.lock().await;
        state.archiving.remove(id);
        if result.is_ok() {
            state.empty_threads.remove(id);
        }
        result
    }

    async fn read_thread(&self, id: &str, resume: bool) -> ServiceResult<Thread> {
        let empty = self.state.lock().await.empty_threads.get(id).cloned();
        let thread = if let Some(thread) = empty {
            thread
        } else {
            self.client.read_thread(id).await?
        };
        let configured = &self
            .agent
            .as_ref()
            .ok_or_else(|| ServiceError::new("configuration", "Agent 尚未配置"))?
            .workspace;
        if Path::new(&thread.cwd).canonicalize().ok().as_ref() != Some(configured) {
            return Err(ServiceError::new(
                "wrong_workspace",
                "该会话不属于当前工作目录",
            ));
        }
        let thread = if resume && !self.state.lock().await.empty_threads.contains_key(id) {
            self.client.resume_thread(id, &self.options()?).await?
        } else {
            thread
        };
        self.observe_thread(&thread).await;
        Ok(thread)
    }

    async fn start_turn(self: &Arc<Self>, params: SendTurn) -> ServiceResult<Value> {
        validate_id(&params.thread_id)?;
        if let Some(id) = &params.client_message_id {
            validate_id(id)?;
        }
        if params.text.len() > 64 * 1024 {
            return Err(ServiceError::new("bad_request", "单条输入不能超过 64 KiB"));
        }
        if params.text.trim().is_empty() && params.attachments.is_empty() {
            return Err(ServiceError::new("bad_request", "消息和附件不能同时为空"));
        }
        let agent = self
            .agent
            .as_ref()
            .ok_or_else(|| ServiceError::new("configuration", "Agent 尚未配置"))?;
        let Some(model) = agent.config.model(&params.model_id) else {
            return Err(ServiceError::new(
                "bad_request",
                "当前模型未在后端配置中启用",
            ));
        };
        if !model.supported_efforts.contains(&params.effort) {
            return Err(ServiceError::new("bad_request", "当前模型不支持该 Effort"));
        }
        let ticket = Uuid::new_v4();
        {
            let mut state = self.state.lock().await;
            if state.active.contains_key(&params.thread_id)
                || state.archiving.contains(&params.thread_id)
                || state.cancelling.contains(&params.thread_id)
                || state.active.len() >= 64
            {
                return Err(ServiceError::new("busy", "此会话已有任务运行或状态待同步"));
            }
            state.active.insert(
                params.thread_id.clone(),
                ActiveTurn {
                    ticket,
                    turn: None,
                    uncertain: false,
                    stop_requested: false,
                },
            );
        }
        // Reserve before any RPC await so Stop during resume/validation is not lost.
        if let Err(error) = self.read_thread(&params.thread_id, true).await {
            let mut state = self.state.lock().await;
            if state
                .active
                .get(&params.thread_id)
                .is_some_and(|slot| slot.ticket == ticket && slot.turn.is_none())
            {
                state.active.remove(&params.thread_id);
            }
            drop(state);
            self.publish_status().await;
            return Err(error);
        }
        {
            let mut state = self.state.lock().await;
            if state
                .active
                .get(&params.thread_id)
                .is_some_and(|slot| slot.turn.is_some())
            {
                return Err(ServiceError::new("busy", "会话中已有未结束的任务"));
            }
            if state
                .active
                .get(&params.thread_id)
                .is_some_and(|slot| slot.stop_requested)
            {
                state.active.remove(&params.thread_id);
                drop(state);
                self.publish_status().await;
                return Err(ServiceError::new("cancelled", "发送已取消，尚未启动新任务"));
            }
        }
        let mut input = Vec::new();
        if !params.text.trim().is_empty() {
            input.push(UserInput::Text {
                text: params.text.clone(),
            });
        }
        if !params.attachments.is_empty() {
            let uploaded = match &self.uploads {
                Some(store) => {
                    store
                        .inputs(&params.attachments, model.supports_images)
                        .await
                }
                None => Err("附件服务不可用".into()),
            };
            match uploaded {
                Ok(items) => input.extend(items),
                Err(message) => {
                    self.state.lock().await.active.remove(&params.thread_id);
                    self.publish_status().await;
                    return Err(ServiceError::new("attachment", message));
                }
            }
        }
        self.publish_status().await;
        let result = self
            .client
            .start_turn(&TurnInput {
                thread_id: params.thread_id.clone(),
                client_user_message_id: params.client_message_id.clone(),
                input,
                model: Some(params.model_id),
                effort: Some(params.effort),
            })
            .await;
        let mut state = self.state.lock().await;
        let mut stop = None;
        match &result {
            Ok(turn) => {
                state.empty_threads.remove(&params.thread_id);
                if let Some(slot) = state
                    .active
                    .get_mut(&params.thread_id)
                    .filter(|slot| slot.ticket == ticket)
                {
                    slot.turn = Some(turn.clone());
                    slot.uncertain = false;
                    if slot.stop_requested {
                        stop = Some(turn.id.clone());
                    }
                    if is_finished(&turn.status) {
                        state.active.remove(&params.thread_id);
                    }
                }
            }
            Err(AgentError::Timeout | AgentError::Disconnected) => {
                state.empty_threads.remove(&params.thread_id);
                if let Some(slot) = state
                    .active
                    .get_mut(&params.thread_id)
                    .filter(|slot| slot.ticket == ticket)
                {
                    slot.uncertain = true;
                }
            }
            Err(_) => {
                if state
                    .active
                    .get(&params.thread_id)
                    .is_some_and(|slot| slot.ticket == ticket && slot.turn.is_none())
                {
                    state.active.remove(&params.thread_id);
                }
            }
        }
        drop(state);
        if let Some(turn) = stop {
            self.queue_interrupt(params.thread_id.clone(), turn);
        }
        self.publish_status().await;
        result.map(|turn| json!({"turn":turn})).map_err(Into::into)
    }

    async fn interrupt(self: &Arc<Self>, id: &str) -> ServiceResult<Value> {
        let mut state = self.state.lock().await;
        let Some(active) = state.active.get_mut(id) else {
            return Ok(json!({"requested":false}));
        };
        active.stop_requested = true;
        let turn = active.turn.as_ref().map(|turn| turn.id.clone());
        drop(state);
        if let Some(turn) = turn {
            if self.client.status().phase == ConnectionPhase::Ready {
                self.cancel_turn(id, &turn).await?;
            }
        }
        self.publish_status().await;
        Ok(json!({"requested":true}))
    }

    async fn cancel_turn(&self, thread: &str, turn: &str) -> ServiceResult<()> {
        if !self.state.lock().await.cancelling.insert(thread.into()) {
            return Ok(());
        }
        self.publish_status().await;
        let result =
            tokio::time::timeout(Duration::from_secs(5), self.cancel_turn_inner(thread, turn))
                .await
                .unwrap_or_else(|_| {
                    Err(ServiceError::new(
                        "terminal_cleanup",
                        "停止操作未能在期限内确认，请同步后检查任务状态",
                    ))
                });
        self.state.lock().await.cancelling.remove(thread);
        self.publish_status().await;
        result
    }

    async fn cancel_turn_inner(&self, thread: &str, turn: &str) -> ServiceResult<()> {
        self.client.interrupt_turn(thread, turn).await?;
        if !self.client.experimental_api_enabled() {
            return Ok(());
        }
        // Unified-exec sessions outlive turn/interrupt. Only cancel items from this turn.
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
            let items = self
                .state
                .lock()
                .await
                .command_items
                .get(&(thread.into(), turn.into()))
                .cloned()
                .unwrap_or_default();
            if items.is_empty() {
                continue;
            }
            let mut cursor = None;
            for _ in 0..16 {
                let page = self
                    .client
                    .list_background_terminals(thread, cursor.as_deref())
                    .await?;
                for terminal in page.data {
                    if items.contains(&terminal.item_id) {
                        if attempt == 2 {
                            return Err(ServiceError::new(
                                "terminal_cleanup",
                                "回合已中断，但仍检测到本轮命令，请检查后台状态",
                            ));
                        }
                        let _ = self
                            .client
                            .terminate_background_terminal(thread, &terminal.process_id)
                            .await?;
                    }
                }
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
        }
        Ok(())
    }

    fn queue_interrupt(self: &Arc<Self>, thread: String, turn: String) {
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = service.cancel_turn(&thread, &turn).await {
                log::warn!("Deferred turn interrupt failed, code={}", error.code);
            }
        });
    }

    async fn reply_interaction(&self, params: Value) -> ServiceResult<Value> {
        let key = params
            .get("key")
            .and_then(Value::as_str)
            .ok_or_else(|| ServiceError::new("bad_request", "缺少审批请求标识"))?;
        let request = self
            .state
            .lock()
            .await
            .interactions
            .get(key)
            .cloned()
            .ok_or_else(|| ServiceError::new("stale_interaction", "审批已失效或已被处理"))?;
        let result = if request.method == "item/tool/requestUserInput" {
            let answer = params
                .get("answers")
                .and_then(Value::as_object)
                .ok_or_else(|| ServiceError::new("bad_request", "缺少问题答案"))?;
            let questions = request
                .params
                .get("questions")
                .and_then(Value::as_array)
                .ok_or_else(|| ServiceError::new("bad_request", "问题格式不支持"))?;
            let ids: HashSet<_> = questions
                .iter()
                .filter_map(|q| q.get("id").and_then(Value::as_str))
                .collect();
            if answer.len() != ids.len()
                || answer.iter().any(|(id, v)| {
                    !ids.contains(id.as_str())
                        || v.as_str()
                            .is_none_or(|s| s.trim().is_empty() || s.len() > 4096)
                })
            {
                return Err(ServiceError::new(
                    "bad_request",
                    "请回答所有问题，且每项不超过 4096 字节",
                ));
            }
            let answers: serde_json::Map<_, _> = answer
                .iter()
                .map(|(id, v)| (id.clone(), json!({"answers":[v]})))
                .collect();
            self.client
                .respond(&request, Ok(json!({"answers":answers})))
                .await
        } else {
            let decision = params
                .get("decision")
                .cloned()
                .ok_or_else(|| ServiceError::new("bad_request", "缺少审批决定"))?;
            if !(decision.is_string() || decision.is_object()) {
                return Err(ServiceError::new("bad_request", "审批决定格式错误"));
            }
            if let Some(allowed) = request
                .params
                .get("availableDecisions")
                .and_then(Value::as_array)
            {
                if !allowed.contains(&decision) {
                    return Err(ServiceError::new("bad_request", "Codex 未允许该审批选项"));
                }
            }
            self.client
                .respond(&request, Ok(json!({"decision":decision})))
                .await
        };
        if result.is_ok()
            || matches!(
                result,
                Err(AgentError::StaleRequest | AgentError::Disconnected)
            )
        {
            self.state.lock().await.interactions.remove(key);
        }
        self.publish_status().await;
        result?;
        Ok(json!({}))
    }

    pub async fn begin_shutdown(&self) {
        self.closing.store(true, Ordering::Release);
        if let Some(uploads) = &self.uploads {
            uploads.begin_shutdown();
        }
        let pid = self.state.lock().await.pid;
        self.process_status("stopping", pid, None).await;
    }

    pub async fn interrupt_all(&self) {
        let turns: Vec<_> = self
            .state
            .lock()
            .await
            .active
            .iter()
            .filter_map(|(thread, slot)| {
                slot.turn
                    .as_ref()
                    .map(|turn| (thread.clone(), turn.id.clone()))
            })
            .collect();
        let tasks = turns
            .iter()
            .map(|(thread, turn)| self.cancel_turn(thread, turn));
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            futures_util::future::join_all(tasks),
        )
        .await;
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SendTurn {
    #[serde(default)]
    client_message_id: Option<String>,
    thread_id: String,
    text: String,
    model_id: String,
    effort: String,
    #[serde(default)]
    attachments: Vec<String>,
}

fn validate_id(id: &str) -> ServiceResult<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err(ServiceError::new("bad_request", "会话标识格式无效"));
    }
    Ok(())
}
fn identifier(params: &Value, key: &str) -> ServiceResult<String> {
    let id = params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ServiceError::new("bad_request", "缺少会话标识"))?;
    validate_id(id)?;
    Ok(id.into())
}
fn is_finished(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "interrupted")
}
fn interaction_key(request: &ServerRequest) -> String {
    format!(
        "{}:{}",
        request.connection_id,
        serde_json::to_string(&request.id).unwrap_or_default()
    )
}

#[path = "agent_events.rs"]
mod events;

#[cfg(test)]
#[path = "agent_service_tests.rs"]
pub(crate) mod tests;

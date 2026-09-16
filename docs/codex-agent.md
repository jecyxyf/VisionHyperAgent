# CodexAgent 模块边界

| 项目 | 内容 |
|------|------|
| 源码位置 | backend/model/codex_agent/ |
| 职责 | 将 Codex App Server 的 WebSocket / JSON-RPC 能力转为后端进程内接口 |

## 1. 状态机

~~~
stopped → starting → running
running → reconnecting → running
running → stopping → stopped
任意状态 → error
~~~

## 2. 对外接口

~~~rust
pub trait CodexAgent {
    async fn start(&self, config: AgentConfig) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn status(&self) -> AgentStatus;

    async fn create_thread(&self) -> Result<ThreadId>;
    async fn resume_thread(&self, id: &ThreadId) -> Result<()>;
    async fn list_threads(&self) -> Result<Vec<ThreadSummary>>;
    async fn start_turn(&self, thread: &ThreadId, input: TurnInput) -> Result<TurnId>;
    async fn interrupt_turn(&self, thread: &ThreadId) -> Result<()>;
    async fn list_skills(&self) -> Result<Vec<Skill>>;
    async fn subscribe_events(&self, cb: EventCallback) -> SubscriptionId;
}
~~~

## 3. 方法白名单

CodexAgent 只允许调用以下 Codex 方法：

~~~
thread/start
thread/resume
thread/list
thread/read
turn/start
turn/interrupt
skills/list
~~~

## 4. 事件映射

| Codex 通知 | 进程内事件 |
|-----------|----------|
| thread/started | agent.thread_started |
| turn/started | agent.turn_started |
| item/agentMessage/delta | agent.message_delta |
| item/completed | agent.item_completed |
| turn/completed | agent.turn_completed |
| error | agent.error |

## 5. 不属于 CodexAgent 的职责

- 不保存业务数据
- 不做 UI 渲染
- 不管理训练任务
- 不直接读写文件系统

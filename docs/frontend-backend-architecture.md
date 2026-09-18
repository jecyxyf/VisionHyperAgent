# Agent 前后端实现架构

| 项目 | 内容 |
| --- | --- |
| 更新日期 | 2026-09-17 |
| 状态 | 当前已实现代码的架构说明 |
| 范围 | 浏览器 Agent 面板、Rust 本地后端、Codex 进程与协议调用链 |
| 不包含 | 训练、标注、推理等后续业务模块 |

本文按“运行关系 → 前端模块 → 通信接口 → 后端模块 → Codex 调用库”的顺序描述。接口名以当前源码为准；docs/architecture.md 仍保留项目长期架构基线。

## 1. 运行视图

~~~text
┌────────────────────────────────────────────────────┐
│ 浏览器                                             │
│ Svelte + TypeScript                               │
│ ┌──────────────────────────────────────────────┐   │
│ │ AgentPanel                                    │   │
│ │ 聊天 / 历史 / 模型 / Effort / 附件 / 审批      │   │
│ └───────┬──────────────────────────────┬───────┘   │
│         │ HTTP 附件上传/删除             │ WebSocket │
└─────────┼──────────────────────────────┼───────────┘
          ▼                              ▼
┌────────────────────────────────────────────────────┐
│ Rust 主进程 vha-server                              │
│                                                    │
│ HTTP 静态资源 + /api/agent/* + /ws                  │
│ AgentService：白名单业务方法、状态、事件、并发控制    │
│ AttachmentStore：附件落盘与 opaque ID               │
│ AgentRuntime：唯一拥有并监督 Codex 子进程            │
│ ModelGateway：可选 Chat Completions 兼容层          │
│ Tray / Browser / Logging / Shutdown                │
└───────┬──────────────────────────────┬────────────┘
        │ 本机 WebSocket + token          │ 模型 HTTP
        ▼                              ▼
┌────────────────────────┐  ┌──────────────────────┐
│ Codex App Server 子进程 │  │ 配置的模型服务         │
│ 工具 / 会话 / 审批 / 回合│  │ MiniMax-M3 测试入口   │
└────────────────────────┘  └──────────────────────┘
~~~

关键所有权：

- **后端拥有 Codex 进程**：主程序启动时自动启动 Codex，托盘退出或后端关闭时中断任务并回收子进程。
- **浏览器不拥有进程**：关闭、刷新网页不会停止后端和 Codex。
- **前端“停止”只中断当前回合**：不结束 Codex，也不影响其他会话的回合。
- **CodexAgent 不管理进程**：它只是 Codex App Server 的 WebSocket/JSON-RPC 客户端；进程启动和停止由 AgentRuntime 调用 CodexProcess 完成。
- **浏览器不接触模型密钥**：密钥只存在于后端私有配置和进程环境中；前端响应和日志会做脱敏。

## 2. 当前目录边界

~~~text
source/frontend/
├── src/lib/api/websocket.ts                  # Agent WebSocket RPC 客户端
├── src/lib/components/AgentPanel.svelte       # Agent 业务容器
├── src/lib/components/agent/
│   ├── AgentHistoryPanel.svelte               # 历史会话面板
│   ├── AgentAttachmentList.svelte              # 发送前附件列表
│   ├── AgentInteractionCard.svelte             # 审批 / 追问卡片
│   ├── messages.ts                             # Codex item 与聊天消息转换
│   └── types.ts                                # 前端协议类型
└── tests/                                      # Vitest / Playwright

source/backend/
├── common/                                    # 全局日志单例、通用 JSON 配置文件读写
├── core/                                      # 事件总线与共享领域事件
├── server/
│   ├── src/main.rs                            # 可执行入口、启动顺序、托盘/headless 分支
│   ├── src/http_server.rs                     # HTTP 服务、静态资源、关闭编排
│   ├── src/agent_api.rs                       # 浏览器 WS/HTTP 接口与白名单转发
│   ├── src/agent_service.rs                   # Agent 业务服务
│   ├── src/agent_events.rs                    # Codex 事件归并和状态修复
│   ├── src/agent_runtime.rs                   # Codex 进程监督者
│   ├── src/attachments.rs                     # 附件存储
│   ├── src/model_gateway/                     # 可选模型协议兼容层
│   ├── src/tray.rs / browser.rs / shutdown.rs # 托盘、浏览器、关闭信号
│   └── tests/                                 # Rust 集成测试
└── model/codex_agent/
    ├── src/client.rs                          # CodexAgent
    ├── src/websocket.rs                       # WebSocket actor / JSON-RPC 关联
    ├── src/process.rs / process/windows.rs    # CodexProcess 与平台进程控制
    ├── src/config.rs / executable.rs          # app_config 解析与 Codex 可执行文件
    ├── src/types.rs / error.rs                # 协议类型、错误
    └── tests/                                 # 协议和进程测试
~~~

## 3. 前端架构与模块接口

### 3.1 AgentSocket

位置：source/frontend/src/lib/api/websocket.ts。

职责：维护唯一浏览器 WebSocket，封装 RPC 请求、心跳、指数退避重连和事件分发。断线时拒绝所有未完成请求，不会重放可能产生副作用的请求。

| 接口 | 签名 | 说明 |
| --- | --- | --- |
| 构造 | new AgentSocket(createSocket?) | 默认创建浏览器 WebSocket；测试可注入工厂 |
| 连接 | connect(url?) | 默认基于当前页面协议推导 ws://host/ws 或 wss://host/ws |
| 请求 | request<T>(method, params?, timeoutMs = 35000) | 发送 id/method/params；返回 result 或抛出 AgentRequestError |
| 订阅 | on(type, listener): () => void | 返回取消订阅函数 |
| 断开 | disconnect() | 清理心跳、重连和待处理请求 |

公开错误类型：

~~~ts
class AgentRequestError extends Error {
  code: string;
}
~~~

内置行为：

- 每 10 秒发送 type=ping。
- 重连延迟从 500ms 指数增长，最大 5 秒。
- 连接断开时，未完成请求以 outcome_unknown 拒绝；提示任务可能已开始，需同步状态。
- open / close 事件用于驱动前端重新初始化。

### 3.2 AgentPanel

AgentPanel.svelte 是 Agent 页面业务编排容器，内部状态包括：

| 状态 | 作用 |
| --- | --- |
| snapshot | 后端 Agent 状态快照 |
| sessions / historyCursor | 历史会话与分页游标 |
| activeSessionId | 当前会话，持久在 localStorage 的 vha.agent.activeThread |
| messageCache | 每个会话的本地消息缓存 |
| selectedModel / selectedEffort | 模型与 Effort 选择 |
| attachments | 拖入但尚未发送的文件 |
| sendPhase | preparing / uploading / submitting / null |
| interactions | 当前会话可见的审批或追问 |

主要内部流程：

| 流程 | 行为 |
| --- | --- |
| 初始化 | 等待 ready，然后 thread.list，恢复活动会话或 thread.create |
| 同步 | status、thread.list、thread.resume，用于重连和事件丢失 |
| 发送 | 先上传附件得到 ID，乐观插入用户消息，再 turn.start，成功后清空输入 |
| 停止 | 上传前中止本地流程；已提交后调用 turn.interrupt |
| 流式渲染 | 处理 agentMessage delta、item started/completed、turn completed |
| 附件 | 面板拖拽接收，无额外上传按钮；发送前可删除 |
| 历史 | 打开面板、分页加载、切换、新建、归档 |

该组件不向其他前端组件暴露 props，通过 AgentSocket 与后端通信。

### 3.3 Agent 子组件

AgentHistoryPanel props：

~~~ts
type Props = {
  sessions: AgentHistorySession[];
  activeSessionId: string;
  loading?: boolean;
  hasMore?: boolean;
  onLoadMore?: () => void;
  onClose?: () => void;
  onSelect?: (id: string) => void;
  onCreate?: () => void;
  onDelete?: (id: string) => void;
};
~~~

职责：展示历史列表、当前会话、加载更多、新建和归档按钮。

AgentAttachmentList props：

~~~ts
type Props = {
  attachments: AgentAttachment[];
  onRemove?: (id: string) => void;
};
~~~

职责：显示图片预览或文件图标、文件名、大小和删除按钮。

AgentInteractionCard props：

~~~ts
type Props = {
  interaction: AgentInteraction;
  context?: string;
  onReply: (params: object) => Promise<void>;
};
~~~

职责：

- 命令执行、文件修改等审批：提交 accept / decline / cancel。
- item/tool/requestUserInput：按问题 ID 收集答案并提交。
- 如果底层请求不支持 decline，“拒绝”退化为 cancel，并明确提示会结束本轮。
- Codex 提出执行策略修正时，展示“本次允许并应用提议权限”；只原样提交 Codex 给出的 `acceptWithExecpolicyAmendment`，不能由前端改写修正内容。

### 3.4 消息转换模块 messages.ts

| 接口 | 输入 → 输出 | 说明 |
| --- | --- | --- |
| itemMessage(item, turnId, streaming?) | Codex item → AgentChatMessage 或 null | 支持 agentMessage、userMessage、commandExecution、fileChange 和工具类 item |
| threadMessages(thread) | Codex thread → AgentChatMessage[] | 历史恢复，附加中断或失败系统消息 |
| errorText(error) | unknown → string | 提取可展示错误，避免暴露内部堆栈 |
| historySession(thread) | Codex thread → 历史列表项 | 生成标题、摘要和时间 |
| mergeHydrated(incoming, current, busy) | 两条消息序列 → 合并序列 | 保护历史同步期间已收到的流式增量 |

渲染约束：模型返回内容按 Svelte 文本插值显示，不作为 HTML 执行。

### 3.5 前端类型 types.ts

核心类型：

~~~text
AgentChatMessage
AgentAttachment
AgentHistorySession
AgentModel
CodexItem
CodexTurn
CodexThread
AgentInteraction
AgentSnapshot
ThreadPage
~~~

`AgentModel.inputModalities` 决定是否允许拖入图片。Effort 选项完全来自当前模型的 `supportedReasoningEfforts`；切换模型后自动使用该模型 `defaultReasoningEffort`，不做静默映射。

## 4. 浏览器与后端接口

### 4.1 HTTP

| 方法 | 路径 | 请求 | 成功响应 | 说明 |
| --- | --- | --- | --- | --- |
| GET | /api/health | 无 | ok | 健康检查 |
| GET | /api/agent/status | 无 | AgentSnapshot | 当前状态 |
| POST | /api/agent/attachments | multipart，字段名必须为 file | {"attachment": Attachment} | 单文件上传，最大 16 MiB |
| DELETE | /api/agent/attachments/{id} | 路径参数为后端 UUID | {} | 删除未被会话引用的附件 |

Attachment 结构：

~~~json
{
  "id": "uuid",
  "name": "原始文件名（已清理）",
  "size": 123,
  "isImage": true
}
~~~

安全与限流：

- 服务仅监听 127.0.0.1。
- 带 Origin 的请求必须在允许列表内：生产 127.0.0.1:8420 / localhost:8420，开发 127.0.0.1:5173 / localhost:5173。
- 同时最多 2 个上传请求，上传读取超时 30 秒。
- 每次只能上传一个文件字段，请求体限制为附件限制加 64 KiB。

### 4.2 WebSocket /ws

连接建立后，后端先推送一次：

~~~json
{"type":"status","data":AgentSnapshot}
~~~

请求格式：

~~~json
{"id":"client-generated-id","method":"turn.start","params":{}}
~~~

成功响应：

~~~json
{"id":"同请求 id","result":{}}
~~~

失败响应：

~~~json
{
  "id":"同请求 id",
  "error":{"code":"busy","message":"..."}
}
~~~

约束：单连接最多 16 个在途请求，全局最多 32 个业务调用和 32 个 WebSocket 连接；消息上限 128 KiB。请求被后端接受后会脱离当前浏览器连接继续执行，因此关闭网页不会取消它。

#### 业务方法白名单

| 方法 | 参数 | 结果 | 说明 |
| --- | --- | --- | --- |
| status | {} | AgentSnapshot | 读取状态，不要求 Agent ready |
| models.list | {} | {"data": AgentModel[]} | 返回后端启用的模型 |
| thread.create | {} | {"thread": CodexThread} | 新会话 / 新上下文 |
| thread.list | {"cursor": string 或 null} | ThreadPage | 分页读取未归档历史 |
| thread.read | {"threadId": string} | {"thread": CodexThread} | 读取会话 |
| thread.resume | {"threadId": string} | {"thread": CodexThread} | 恢复并观察会话 |
| thread.archive | {"threadId": string} | {} | 归档；未提交空草稿走 unsubscribe |
| turn.start | TurnStartParams | {"turn": CodexTurn} | 发送当前回合 |
| turn.interrupt | {"threadId": string} | {"requested": boolean} | 请求停止当前回合 |
| interaction.reply | 审批参数或答案参数 | {} | 回复服务端请求 |
| skills.list | {} | Codex skills 响应 | 能力查询 |

TurnStartParams：

~~~json
{
  "threadId": "thread-id",
  "text": "用户输入，最大 64 KiB",
  "modelId": "前端显示且全局唯一的模型 ID",
  "effort": "low|medium|high|xhigh|max|ultra",
  "attachments": ["attachment-id"],
  "clientMessageId": "前端生成的乐观消息 ID"
}
~~~

规则：

- 文本与附件不能同时为空。
- 每条消息最多 16 个附件。
- 模型必须是后端配置启用的模型。
- 同一 thread 同时只允许一个活动回合。
- 后端在 resume、附件校验和真正 turn/start 之间保留活动槽，避免停止请求丢失。

#### AgentSnapshot

~~~json
{
  "phase": "starting|connecting|reconnecting|ready|error|stopping|stopped",
  "message": "可选错误或提示",
  "pid": 123,
  "connection": {
    "phase": "disconnected|connecting|initializing|ready|reconnecting|disconnecting|error",
    "connectionId": 1
  },
  "models": [],
  "activeTurns": {
    "thread-id": {
      "turn": {},
      "uncertain": false,
      "stopRequested": false
    }
  },
  "interactions": [
    {
      "key": "connectionId:requestId",
      "request": {}
    }
  ]
}
~~~

#### 推送事件

| type | data | 前端行为 |
| --- | --- | --- |
| status | AgentSnapshot | 更新状态、模型、活动回合、审批 |
| codex | {"event": AgentEvent} | 流式增量、工具、回合完成、错误 |
| process_lost | {"threads":[id]} | Codex 退出，将流式消息标记为未确认 |
| resync_required | 无 | 重新同步状态和历史 |
| closing | 无 | 显示主程序退出提示 |
| pong | 无 | 心跳响应 |

常见错误码：not_ready、busy、bad_request、attachment、configuration、outcome_unknown、stale_interaction、codex_rpc、terminal_cleanup、closing、unknown_method。

## 5. Rust 后端架构与模块接口

### 5.1 main.rs

启动序列：

1. 初始化全局日志。
2. 以可执行文件目录为应用目录，调用 `vha_codex_agent::config::load()` 读取或恢复 `app_config.json`。
3. 固定监听 `127.0.0.1:8420`；配置错误不会退出，会转换为可观察状态。
4. GUI 模式创建托盘并打开浏览器；`--headless` 模式等待信号，用于自动化测试。

### 5.2 common/config.rs

~~~rust
pub struct LoadedConfig<T> { pub value: T; pub created: bool; pub recovered_from: Option<PathBuf>; }
pub fn load_or_create<T>(path: &Path) -> Result<LoadedConfig<T>, ConfigError>;
pub fn save<T>(path: &Path, value: &T) -> Result<(), ConfigError>;
~~~

职责：创建缺失默认 JSON、备份损坏 JSON、pretty 序列化、临时文件原子替换、Unix `0600` 权限。它不理解 Agent 字段。

### 5.3 http_server.rs

| 接口 | 说明 |
| --- | --- |
| `local_addr() -> SocketAddr` | 返回固定 `127.0.0.1:8420` |
| `start(addr)` | 无模型运行时的可观察服务 |
| `start_with_codex(addr, app_dir, loaded)` | 启动 HTTP 并准备 AgentRuntime |
| `ServerHandle::address()` / `stop()` | 返回实际地址 / 优雅停止并等待 |

路由包含 `/api/health`、`/api/agent/*`、`/ws`、内部模型网关和打包前端静态资源。

### 5.4 AgentRuntime

~~~rust
pub struct AgentRuntime { pub service: Arc<AgentService> }

impl AgentRuntime {
    pub async fn prepare(
        loaded: Result<LoadedAgentConfig, String>,
        app_dir: &Path,
        addr: SocketAddr,
    ) -> Self;
    pub async fn run(self, shutdown: ShutdownSignal);
}
~~~

职责：调用 `prepare()`、创建 `CodexAgent` 和 `AgentService`、预检 Codex 端口、启动并监督 Codex 子进程、退出时中断任务并回收进程。它是应用层唯一拥有 Codex 生命周期的模块。

### 5.5 AgentService

~~~rust
pub struct AgentService {
    pub client: CodexAgent,
    pub agent: Option<Arc<PreparedAgent>>,
    pub uploads: Option<Arc<AttachmentStore>>,
}
~~~

主要接口：`new()`、`subscribe()`、`sanitize()`、`emit()`、`process_status()`、`snapshot()`、`call()`、`begin_shutdown()`、`interrupt_all()`。

`call()` 是浏览器白名单入口，负责 ready / closing 检查、参数校验、`modelId` O(1) 查找、Effort 与图片能力校验、活动回合互斥、附件转换、超时不确定状态和响应脱敏。

### 5.6 agent_events.rs / agent_api.rs / AttachmentStore

`agent_events.rs` 消费 Codex 状态、通知、审批和服务端请求，维护活动回合与后台命令关联。

`agent_api.rs` 注册 WebSocket 和附件 API，执行同源校验、消息大小、并发、心跳和关闭处理。

`AttachmentStore` 将文件保存到 workspace 的 `.vha-attachments`，使用 UUID、安全扩展名、`.part` 原子替换，限制单文件 16 MiB、单消息 16 个，退出时只清理未引用文件。

### 5.7 model_gateway

| 模块 | 职责 |
| --- | --- |
| `mod.rs` | 鉴权、Origin 拒绝、按请求 `modelId` 选择 ResolvedModel、替换上游 `modelName`、转发 |
| `request.rs` | Responses 请求转 Chat Completions 请求 |
| `protocol.rs` | Chat SSE 转 Responses SSE、Responses 透传流 redaction |

内部端点为 `POST /internal/model/v1/responses`。`responses` Provider 流式透传；`chat_completions` Provider 做协议转换。上游错误不回显 body，Provider 密钥不出现在响应中。

### 5.8 托盘、浏览器与关闭信号

`tray.rs` 提供托盘菜单“打开界面 / 退出”；`browser.rs` 打开系统浏览器；`shutdown.rs` 广播退出信号。浏览器连接断开不触发退出。

## 6. model/codex_agent 库架构

### 6.1 crate 导出

lib.rs 导出：

~~~text
CodexAgent
AppConfig / AgentConfig / ProviderConfig / ModelConfig / CodexConfig
LoadedAgentConfig / ResolvedAgentConfig / ResolvedModel / PreparedAgent
ProviderWireApi / TransportConfig
AgentError / Result
CodexProcess / ProcessConfig
types.rs 中的全部协议类型
~~~

内部模块：

| 模块 | 职责 |
| --- | --- |
| client.rs | 面向业务的异步客户端和协议方法封装 |
| websocket.rs | 连接 actor、握手、请求响应关联、通知分发、重连 |
| process.rs | 跨平台子进程启动、等待、优雅停止、强杀 |
| process/windows.rs | Windows Job Object 细节 |
| config.rs | app_config.json、多 Provider 模型索引、Codex 启动准备 |
| executable.rs | 原生 Codex 可执行文件解析 |
| types.rs | 协议 DTO 与事件 |
| error.rs | 错误类型 |

### 6.2 配置与准备

`AgentConfig::resolve()` 校验配置并生成 `ResolvedAgentConfig`：

~~~rust
pub struct ResolvedAgentConfig {
    pub active_model_id: String,
    pub active_model: Arc<ResolvedModel>,
    pub models: Arc<HashMap<String, Arc<ResolvedModel>>>,
    pub codex: CodexConfig,
}

impl ResolvedAgentConfig {
    pub fn model(&self, model_id: &str) -> Option<Arc<ResolvedModel>>;
    pub fn browser_models(&self) -> Vec<Model>;
}
~~~

`ResolvedModel` 合并 Provider URL、密钥、wire API、上游模型名、支持 Effort 和图片能力，建立 O(1) 索引。

`prepare(config, app_dir, address)` 创建 workspace 和独立 `CODEX_HOME`，生成随机 WebSocket / Gateway token，写入不含 Provider 密钥的 Codex `config.toml`，并返回 `ProcessConfig`。`PreparedAgent::redact()` 负责响应脱敏。

`TransportConfig` 只描述 WebSocket 客户端连接参数，由 `PreparedAgent::transport_config()` 从 Codex 配置生成。

### 6.3 CodexAgent

| Rust 接口 | Codex 协议方法 | 说明 |
| --- | --- | --- |
| new(config) | 无 | 创建客户端，不连接 |
| connect() | initialize + initialized | 建立连接和握手 |
| disconnect() | 无 | 断开客户端，不停止进程 |
| status() | 无 | 返回 AgentState |
| subscribe_events() | 无 | 订阅状态、通知和服务端请求 |
| list_models(cursor) | model/list | 模型分页 |
| create_thread(options) | thread/start | 创建会话 |
| resume_thread(id, options) | thread/resume | 恢复会话 |
| read_thread(id) | thread/read | 读取完整会话 |
| list_threads(cursor, cwd) | thread/list | 历史列表 |
| archive_thread(id) | thread/archive | 归档已持久化会话 |
| unsubscribe_thread(id) | thread/unsubscribe | 释放未提交空草稿 |
| start_turn(input) | turn/start | 发送回合 |
| interrupt_turn(threadId, turnId) | turn/interrupt | 中断回合 |
| list_skills(cwd) | skills/list | 技能列表 |
| respond(request, result) | 原服务端请求 ID | 回复用户输入；服务层也会用它原样转发已校验的复杂审批决定 |
| approve(request, decision) | 原服务端请求 ID | 回复字符串审批 |
| experimental_api_enabled() | 无 | 是否启用实验 API |
| list_background_terminals(threadId, cursor) | thread/backgroundTerminals/list | 列出后台命令 |
| terminate_background_terminal(threadId, processId) | thread/backgroundTerminals/terminate | 结束指定后台命令 |

传输保证：

- JSON-RPC 请求 ID 关联，支持并发。
- 连接代际 ID 防止旧连接响应污染新连接。
- 通知、服务端请求与普通响应分开处理。
- 断线拒绝待处理请求，不自动重放副作用请求。
- 重连后重新握手，并通过 resync_required 触发上层状态同步。
- 通道和事件容量有界。

### 6.4 协议类型

| 类型 | 用途 |
| --- | --- |
| ConnectionPhase | Codex 连接阶段 |
| AgentState | 连接阶段和连接代际 |
| AgentEvent | status、notification、server request、expired |
| ServerRequest | Codex 主动发起的审批或追问 |
| ThreadOptions | cwd、模型、审批策略、沙箱 |
| Thread / Turn / Page<T> | 会话、回合和分页 |
| Model / ReasoningEffortOption | 模型与 Effort 能力 |
| UserInput | text、localImage、mention |
| TurnInput | 回合完整输入 |
| ApprovalDecision | accept、decline、cancel |
| BackgroundTerminal | 后台命令 item、process、PID |

### 6.5 CodexProcess

~~~rust
pub struct ProcessConfig {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, OsString)>,
}

impl CodexProcess {
    pub async fn spawn(config: &ProcessConfig) -> io::Result<Self>;
    pub fn pid(&self) -> Option<u32>;
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
    pub fn diagnostics(&self) -> Vec<&'static str>;
    pub async fn shutdown(&mut self, grace: Duration) -> io::Result<ExitStatus>;
    pub async fn kill(&mut self) -> io::Result<ExitStatus>;
}
~~~

平台策略：

- Linux：自有进程组；启动时处理父进程死亡信号；正常退出时清理本进程组。
- Windows：挂起创建，加入 KILL_ON_JOB_CLOSE 的 Job Object，再恢复执行；加入失败会回收子进程。
- 只操作本应用创建并持有的子进程，禁止按进程名批量杀掉其他 Codex。
- stdout 和 stderr 会被读取，避免管道写满阻塞子进程，但不会把原始敏感内容写入日志。

## 7. 核心数据流

### 7.1 发送消息

~~~text
拖拽文件到 AgentPanel
→ POST /api/agent/attachments
→ AttachmentStore 返回 UUID
→ 前端插入 pending 用户消息
→ WS turn.start(text, modelId, effort, attachmentIds, clientMessageId)
→ AgentService 校验并 reserve 活动回合
→ AttachmentStore.inputs() 转成 UserInput
→ CodexAgent turn/start
→ Codex 通知事件
→ AgentService 状态归并和 broadcast
→ 前端流式渲染
→ turn/completed 后刷新历史
~~~

### 7.2 停止当前回合

~~~text
前端点击停止
→ 若仍在上传：中止本地 AbortController，未发送 turn
→ 若已提交：WS turn.interrupt
→ AgentService 标记 stopRequested
→ CodexAgent turn/interrupt
→ 列出后台 terminals，只终止当前 turn 关联 command item
→ turn/completed(interrupted)
→ 前端恢复可输入
~~~

### 7.3 重连与状态修复

~~~text
浏览器断线
→ AgentSocket 拒绝 pending 请求，不重放
→ 后端任务继续运行
→ 前端重连 /ws
→ 收到 status
→ resync_required 或主动 synchronize
→ thread.list + thread.resume
→ mergeHydrated 合并历史与流式缓存
~~~

### 7.4 应用退出

~~~text
托盘退出
→ ShutdownController
→ HTTP graceful shutdown，WS 收到 closing
→ AgentService.begin_shutdown
→ interrupt_all
→ CodexAgent.disconnect
→ CodexProcess.shutdown(2s)，必要时 kill
→ 清理未引用附件
→ 状态 stopped
→ 托盘移除，主进程退出
~~~

## 8. 测试入口

当前与架构直接相关的测试：

| 位置 | 覆盖 |
| --- | --- |
| model/codex_agent/tests | 模拟 WebSocket、协议、真实 Codex 忽略测试、进程生命周期 |
| server/src/agent_api_tests.rs | 浏览器 API、Origin、并发、关闭行为 |
| server/src/agent_service_tests.rs | 业务方法、停止、审批、状态一致性 |
| server/src/model_gateway/http_tests.rs 与 protocol_tests.rs | 协议转换、SSE、安全与失败语义 |
| server/tests | 服务和端到端生命周期 |
| frontend/tests/unit | 消息转换、状态与组件 |
| frontend/tests/browser | 真实后端、Codex 和浏览器场景 |

测试结论和历史证据见 docs/codex-agent-test-report.md。Windows 交叉编译通过，但托盘和 Job Object 仍需 Windows 实机复验。

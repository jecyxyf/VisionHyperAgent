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
├── common/                                    # 全局日志单例
├── core/                                      # 预留的通用配置 / 事件总线
├── server/
│   ├── src/main.rs                            # 可执行入口、启动顺序、托盘/headless 分支
│   ├── src/http_server.rs                     # HTTP 服务、静态资源、关闭编排
│   ├── src/agent_api.rs                       # 浏览器 WS/HTTP 接口与白名单转发
│   ├── src/agent_service.rs                   # Agent 业务服务
│   ├── src/agent_events.rs                    # Codex 事件归并和状态修复
│   ├── src/agent_runtime.rs                   # Codex 进程监督者
│   ├── src/attachments.rs                     # 附件存储
│   ├── src/codex_config.rs                    # 私有配置与独立 CODEX_HOME
│   ├── src/executable.rs                      # Codex 原生可执行文件发现
│   ├── src/model_gateway/                     # 可选模型协议兼容层
│   ├── src/tray.rs / browser.rs / shutdown.rs # 托盘、浏览器、关闭信号
│   └── tests/                                 # Rust 集成测试
└── model/codex_agent/
    ├── src/client.rs                          # CodexAgent
    ├── src/websocket.rs                       # WebSocket actor / JSON-RPC 关联
    ├── src/process.rs / process/windows.rs    # CodexProcess 与平台进程控制
    ├── src/config.rs / types.rs / error.rs     # 配置、协议类型、错误
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

Effort 固定为：

~~~text
low / medium / high / xhigh / max / ultra
~~~

如果模型返回能力列表，则禁用未支持选项；能力未知时不做静默替换。

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
  "model": "后端已启用模型",
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

职责：应用入口，确定启动顺序。

启动序列：

1. 初始化全局日志。
2. 读取 AppConfig::local()。
3. 从可执行文件目录读取私有 Codex 配置。
4. 启动本地 HTTP 服务；配置错误不会直接退出 GUI，状态仍可展示。
5. GUI 模式进入托盘；--headless 模式等待信号，便于自动化测试。

### 5.2 AppConfig

~~~rust
pub struct AppConfig {
    pub listen_addr: SocketAddr,
}

impl AppConfig {
    pub fn local() -> Self;
    pub fn base_url(&self) -> String;
    pub const PORT: u16; // 8420
}
~~~

### 5.3 http_server.rs

| 接口 | 说明 |
| --- | --- |
| start(addr) -> Result<ServerHandle> | 无模型配置时启动可观察的本地服务 |
| start_with_codex(addr, settings) -> Result<ServerHandle> | 启动 HTTP，并在后台准备 Agent 运行时 |
| ServerHandle::address() | 返回实际绑定地址 |
| ServerHandle::stop() | 请求关闭并等待服务线程结束 |
| Drop for ServerHandle | 兜底触发关闭 |

路由组合：

- /api/health
- /api/agent/*
- /ws
- 可选 /internal/model/v1/responses
- frontend/dist 静态资源与 SPA fallback

关闭顺序：

~~~text
收到退出信号
→ 停止接收新请求
→ AgentService 标记 closing
→ AgentRuntime 中断活动回合、断开 Codex、停止子进程
→ 关闭 HTTP / WebSocket
→ 等待或超时中止 supervisor
~~~

### 5.4 AgentRuntime

~~~rust
pub struct AgentRuntime {
    pub service: Arc<AgentService>,
    process_config: Option<ProcessConfig>,
}

impl AgentRuntime {
    pub async fn prepare(settings: Result<CodexSettings, String>, addr: SocketAddr) -> Self;
    pub async fn run(self, shutdown: ShutdownSignal);
}
~~~

职责：

- 读取并准备私有配置、独立 CODEX_HOME、工作区和附件目录。
- 创建 CodexAgent 与 AgentService。
- 预检 Codex 端口，避免误连已有进程。
- 启动 CodexProcess，等待 WebSocket 初始化握手，20 秒启动超时。
- 监控子进程退出，释放进程 guard，更新错误状态。
- 退出时中断活动任务、断开客户端、优雅停止子进程并清理未引用附件。

AgentRuntime 是应用层唯一允许拥有 Codex 子进程的模块。

### 5.5 AgentService

~~~rust
pub struct AgentService {
    pub client: CodexAgent,
    pub settings: Option<Arc<CodexSettings>>,
    pub uploads: Option<Arc<AttachmentStore>>,
    // 内部状态与事件通道
}

impl AgentService {
    pub fn new(...) -> Arc<Self>;
    pub fn subscribe(&self) -> broadcast::Receiver<Value>;
    pub fn sanitize(&self, value: Value) -> Value;
    pub fn emit(&self, value: Value);
    pub async fn process_status(...);
    pub async fn snapshot(&self) -> Value;
    pub async fn call(&self, method: &str, params: Value) -> ServiceResult<Value>;
    pub async fn begin_shutdown(&self);
    pub async fn interrupt_all(&self);
}
~~~

call(method, params) 是浏览器方法白名单的唯一入口，负责：

- ready 检查与关闭检查；
- 参数长度、ID、Effort、附件数量和模型合法性校验；
- 活动回合、归档、取消状态互斥；
- 附件 ID 到 UserInput 的转换；
- 超时或断线时保留 uncertain 状态，不自动重放；
- 响应脱敏。

### 5.6 agent_events.rs

该文件是 AgentService 的实现扩展，主要接口为 crate 内部：

| 接口 | 说明 |
| --- | --- |
| pump(receiver) | 消费 CodexAgent 事件并更新服务状态 |
| observe_thread(thread) | 从历史线程恢复活动回合和后台命令 item 关联 |
| refresh_models() | 读取模型信息，并按后端配置修正图片能力 |

处理内容：

- 连接状态变化：标记不确定、要求重同步。
- 服务端请求：保存 interactions，等待前端回复。
- turn started/completed：维护活动回合。
- command item：记录 thread、turn、item 关联，用于停止时清理本轮后台命令。
- broadcast 消费落后：发送 resync_required。

### 5.7 agent_api.rs

| 接口 | 说明 |
| --- | --- |
| routes(service, shutdown, addr) -> Router | 注册 /ws 与附件 HTTP API |

内部能力：

- 同源 Origin 校验；
- WebSocket 连接和并发调用限流；
- JSON 请求反序列化与未知字段拒绝；
- 心跳、Ping、Pong 和超时处理；
- 后端业务调用与浏览器连接解耦；
- 关闭时推送 closing 并结束长连接；
- 待处理响应过多时主动关闭连接，防止无界排队。

### 5.8 AttachmentStore

~~~rust
pub const MAX_ATTACHMENT_BYTES: usize;       // 16 MiB
pub const MAX_ATTACHMENTS_PER_TURN: usize;   // 16

impl AttachmentStore {
    pub async fn new(workspace: &Path) -> Result<Self, String>;
    pub async fn store(&self, name: &str, bytes: &[u8]) -> Result<Attachment, String>;
    pub async fn inputs(&self, ids: &[String], allow_images: bool)
        -> Result<Vec<UserInput>, String>;
    pub async fn remove(&self, id: &str) -> Result<(), String>;
    pub fn begin_shutdown(&self);
    pub async fn cleanup_unused(&self) -> Result<(), String>;
}
~~~

存储位置：

~~~text
<配置的 Agent workspace>/.vha-attachments/<uuid>.<安全扩展名>
~~~

规则：

- 浏览器只拿到 UUID，不能提供任意服务器路径。
- 文件名去路径和控制符并截断；扩展名只允许有限 ASCII 字母数字。
- 先写 .part 再原子 rename。
- 单次运行最多登记 1024 个附件。
- 图片通过魔数识别，仅在配置明确启用图片能力时允许发送。
- 已被回合引用的附件不能删除；退出时只清理未引用文件。

### 5.9 CodexSettings

公开接口：

~~~rust
impl CodexSettings {
    pub fn from_toml(app_dir: &Path, text: &str) -> Result<Self, String>;
    pub fn load(app_dir: &Path) -> Result<Self, String>;
    pub fn websocket_url(&self) -> String;
    pub fn prepare(&mut self) -> Result<ProcessConfig, String>;
    pub fn redact(&self, value: &mut Value);
}
~~~

配置来源：

1. 环境变量：VHA_CODEX_PATH、VHA_CODEX_BASE_URL、VHA_CODEX_MODEL、VHA_CODEX_API_KEY、VHA_CODEX_WORKSPACE、VHA_CODEX_PORT、VHA_CODEX_WIRE_API。
2. 可执行文件旁的 config.local.toml（Git 忽略，权限受限）。

职责：

- 校验模型服务 URL、端口、模型名和凭据存在性。
- 生成每次启动随机 Codex WebSocket capability token 与模型兼容层 token。
- 创建独立 data/codex 作为 CODEX_HOME，不修改用户 ~/.codex。
- 写入隔离的 Codex config.toml。
- 构造 Codex 启动参数和环境。
- 对异常外发的 JSON 做密钥替换。

wire_api 支持两种：

- responses：Codex 直接调用配置服务的 Responses API。
- chat_completions：后端启动进程内兼容层，将 Codex Responses 请求转换为服务端 Chat Completions。

### 5.10 executable.rs

~~~rust
pub fn resolve_codex(configured: Option<&Path>, app_dir: &Path)
    -> Result<PathBuf, String>;
~~~

解析顺序：

1. 配置的 codex_path。
2. 应用目录 codex 或 codex.exe。
3. 应用目录 depends/codex。
4. 系统 PATH。

该模块识别原生二进制（ELF、Mach-O、MZ），并支持从 npm 包装器附近解析平台专属 vendor 二进制；不会把 Node wrapper 当成进程所有者。

### 5.11 model_gateway

| 模块 | 接口 | 说明 |
| --- | --- | --- |
| mod.rs | routes(settings) -> Result<Router, String> | 仅 Chat 模式注册内部路由 |
| request.rs | convert_request(input) -> Result<ChatRequest, String> | Responses 请求转换为 Chat Completions 请求 |
| protocol.rs | ChatStream::new / with_tool_names / begin / push / finish / fail | 流式事件转换 |
| protocol.rs | SseDecoder::push(bytes) | 字节流转为 SSE event |

内部端点：

~~~text
POST /internal/model/v1/responses
Authorization: Bearer <每次启动随机 token>
~~~

该端点不是浏览器 API：

- 拒绝任何带 Origin 的请求。
- 使用随机 gateway token，不把真实模型密钥暴露给 Codex 配置或前端。
- 只接受后端配置的模型名。
- 将 Chat 流转换为 Responses SSE。
- 必须同时看到终止 finish reason 和 DONE 标记才认为完成；截断流不会伪造成功。
- 保留本地 Codex 工具调用、审批和沙箱；禁用服务端托管 web search，因为 Chat 协议无法承载该工具。

### 5.12 托盘、浏览器与关闭信号

| 模块 | 接口 | 说明 |
| --- | --- | --- |
| tray.rs | run(url, server) -> Result<(), String> | 创建托盘、菜单和事件循环 |
| browser.rs | open(url) -> Result<(), String> | 打开系统浏览器 |
| shutdown.rs | ShutdownController::new() / request_shutdown() | 广播关闭请求 |
| shutdown.rs | ShutdownSignal::from_controller() / wait() | 异步等待关闭 |

托盘菜单：

- 打开界面
- 退出

选择退出会调用 ServerHandle::stop()，随后由 HTTP 服务和 AgentRuntime 完成 Codex 回收。

### 5.13 公共日志 common

| 接口 | 说明 |
| --- | --- |
| LoggingConfig::new(app_name, directory) | 指定应用与日志目录 |
| LoggingConfig::with_level(level) | 指定级别 |
| LoggingConfig::with_environment_level() | 读取 VHA_LOG_LEVEL |
| initialize(config) -> Result<PathBuf, LoggingInitError> | 初始化全局单例 |
| config_for_executable(app_name) | 使用可执行文件目录下的 logs |

日志文件：

~~~text
<应用目录>/logs/VisionHyperAgent.YYYY-MM-DD.log
~~~

格式：

~~~text
YYYY-MM-DD HH:mm:ss.SSS [级别] [模块:行号] 内容
~~~

DEBUG、WARNING、ERROR 包含位置；INFO 默认不包含。日志不记录密钥、用户正文、附件内容和模型完整流式响应。

### 5.14 core 预留模块

当前 Agent 链路没有把 core 作为运行时依赖，它保留给后续训练、标注、推理等业务使用。

Config：

~~~rust
pub struct Config {
    pub listen_addr: String,
    pub codex_websocket_url: String,
    pub data_dir: String,
}

impl Default for Config { ... }
~~~

EventBus：

~~~rust
pub struct Event {
    pub topic: String,
    pub payload: serde_json::Value,
}

pub struct EventBus;

impl EventBus {
    pub fn publish(&self, topic: impl Into<String>, payload: serde_json::Value);
    pub fn subscribe(&self) -> broadcast::Receiver<Event>;
}
~~~

Agent 当前使用 server 内的 broadcast 与 ShutdownSignal，避免把本功能耦合到未来业务事件总线。

## 6. model/codex_agent 库架构

### 6.1 crate 导出

lib.rs 导出：

~~~text
CodexAgent
AgentConfig
AgentError
Result
CodexProcess
ProcessConfig
types.rs 中的全部协议类型
~~~

内部模块：

| 模块 | 职责 |
| --- | --- |
| client.rs | 面向业务的异步客户端和协议方法封装 |
| websocket.rs | 连接 actor、握手、请求响应关联、通知分发、重连 |
| process.rs | 跨平台子进程启动、等待、优雅停止、强杀 |
| process/windows.rs | Windows Job Object 细节 |
| config.rs | 超时、重连、事件容量等连接配置 |
| types.rs | 协议 DTO 与事件 |
| error.rs | 错误类型 |

### 6.2 AgentConfig

~~~rust
pub struct AgentConfig {
    pub websocket_url: String,
    pub auth_token: Option<String>,
    pub experimental_api: bool,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub reconnect_interval: Duration,
    pub max_reconnect_attempts: u32,
    pub approval_timeout: Duration,
    pub event_capacity: usize,
}
~~~

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
| respond(request, result) | 原服务端请求 ID | 回复用户输入等请求 |
| approve(request, decision) | 原服务端请求 ID | 回复审批 |
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
→ WS turn.start(text, model, effort, attachmentIds, clientMessageId)
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

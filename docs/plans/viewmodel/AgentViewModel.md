# AgentViewModel

> **状态：接口设计草案，尚未实现、编译或运行验证。** 本文补充 Agent 通信及宿主接入链路；具体签名、消息字段、并发与关闭策略需评审后实现。Python 相关工作继续暂缓。没有内容的栏目保留空白。
>
> 本类负责聊天与会话交互的运行逻辑，不依赖 Slint 窗口、控件、事件循环或 UI 模型类型。View 绑定代码处理回调连接、数据转换和 UI 线程更新。

## 变更记录

| 版本 | 日期 | 作者 | 变更说明 |
|------|------|------|----------|
| 0.1.0 | 2026-09-10 | Codex | 初始通信与接入接口草案，待确认后实现 |
| 0.1.1 | 2026-09-10 | Codex | 明确宿主 Client 与后台 Server 两端角色，保持 Agent 公开接口调用 |
| 0.1.2 | 2026-09-10 | Codex | 补齐清空显示、故障分层和固定运行目录的宿主契约 |

---

## 1. 类型定义与关系

### 1.1 定义

**说明**：连接宿主交互与通用 Agent 库：提交用户操作、接收 AgentEvent、维护纯 Rust 交互状态，并向 View 提供状态更新通知。业务工具交给宿主已注册的真实能力，不在本类另写训练或相机实现。

**类型声明**：

```rust
pub struct AgentViewModel { /* 私有 Agent、交互状态与运行任务 */ }
```

### 1.2 命名空间／所属模块

- **模块路径**：拟在 `vision_hyper_agent::view_model` 导出；文档位于宿主 ViewModel 目录，不放入通用库 API 目录。
- **所属层**：宿主按功能划分的 ViewModel。
- **源码位置**：规划归属 `src/view_model/`，当前尚未实现。现有共享 ViewModelError 与 Result 继续沿用，不在本轮新增共享错误类型。

### 1.3 基类／继承关系

### 1.4 派生类型

### 1.5 实现接口／Trait

### 1.6 组合与依赖

| 依赖项 | 关系类型 | 版本／固定提交 | 说明 |
| --- | --- | --- | --- |
| [Agent](../agent/api/Agent.md) | 拥有／使用 |  | 负责该交互模块的 Agent 生命周期和公开事件接收端 |
| 各 Agent Manager | 功能接口 |  | 使用 sessions、tasks、skills、tools，不直接使用 ZMQ Socket 或 Codex 类型 |
| 宿主工具处理入口 | 注入的业务能力 |  | 执行与按钮入口相同的业务路径，不经 View 执行业务 |
| MainWindowViewModel | 宿主协调关系 |  | 协调子模块启动与退出，不代理全部聊天或业务方法 |
| View 绑定代码 | 使用方 |  | 订阅状态并更新 Slint；本类不反向持有 View |

---

## 2. 构造／初始化

### 2.1 `new(agent, tool_handler)`

**说明**：接收已装配但尚未启动的 Agent 与宿主工具处理入口，构造纯 Rust 交互状态。

**定义**：

```rust
pub fn new(agent: Agent, tool_handler: Option<HostToolHandler>) -> Self
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| agent | Agent | 该 ViewModel 使用的运行入口 | 构造阶段不调用 initialize，也不读取 CONFIG |
| tool_handler | `Option<HostToolHandler>` | 宿主的非 UI 工具处理能力 | 缺失时不能承诺已注册工具可执行；不通过 View 补做业务 |

**返回值**：尚未初始化的 AgentViewModel。

**错误**：

**备注**：

- 连接配置由宿主已有配置路径读取并组装 AgentConfig，不在 View 中读写参数文件。
- 本软件由宿主按可执行文件所在目录组装路径：skills_dirs 只包含该目录下的 Skill，storage_dir 为该目录下的 Chat。保留原有连接配置字段，不另增用户配置文件中的路径选项，也不使用启动工作目录替代应用程序目录。
- MainWindowViewModel 负责协调，不因全局聊天面板而成为所有 Agent 功能的转发器。

**示例**：

**适用范围**：VisionHyperAgent 宿主的 ViewModel；不属于可复用 Agent 库。

**另请参阅**：

### 2.2 `initialize()`

**说明**：取得 Agent 的唯一事件接收端并启动独立消费，再异步初始化 Agent。

**定义**：

```rust
pub async fn initialize(&self) -> crate::view_model::Result<()>
```

**参数**：

**返回值**：初始化的真实结果，不表示远端模型已经完成能力验证。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `ViewModelError` | 配置、状态、Agent 调用或宿主执行失败 | 不伪造成功，不把 UI 更新失败当作业务成功 | 返回安全原因并按实际状态显示或收尾 |

**备注**：

- 事件消费必须独立于 UI 订阅；没有 View 时仍能处理模型状态、工具请求和错误。
- 初始化时核对工具注册集合与宿主处理入口是否匹配；已声明可执行工具却没有处理入口时必须明确报告，不能等到模型调用后让请求无限挂起。
- 不得在 Slint 回调中同步等待本操作结束。
- 重复初始化与失败后的重建规则沿用 Agent 生命周期待确认事项，不暗中反复创建运行实例。
- Agent 模块初始化失败不等于整个桌面程序必须退出。宿主应保留其他本地功能并呈现模块错误，不把模型连接成功作为主界面启动前提。

**示例**：

**适用范围**：VisionHyperAgent 宿主的 ViewModel；不属于可复用 Agent 库。

**另请参阅**：

---

## 3. 字段与属性

### 3.1 字段

| 字段名 | 类型 | 访问级别 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| agent | Agent | private |  | 控制入口；View 不直接访问它 |
| state | AgentViewState | private |  | 由本类维护的交互状态，不包含 Slint 类型 |
| agent_events | AgentEventReceiver | private |  | 由本类独占消费；不能让 View 与本类竞争读取 |
| tool_handler | `Option<HostToolHandler>` | private |  | 宿主真实业务执行能力 |
| 运行任务句柄 | 内部类型 | private |  | 控制事件消费及必要的工具处理任务生命周期；具体调度实现待确认 |

### 3.2 属性

| 属性名 | 类型 | 访问级别 | 读写方式 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| state | AgentViewState | public | state() 返回只读快照 |  | 不向 View 暴露可变内部状态或锁守卫 |

---

## 4. 方法

### 4.1 关联函数／静态方法

### 4.2 实例方法

#### 4.2.1 `load_sessions(query) / create_session()`

**说明**：通过 SessionManager 查询或创建会话，并更新必要的交互状态。

**定义**：

```rust
pub async fn load_sessions(&self, query: SessionQuery)
    -> crate::view_model::Result<SessionPage>
pub async fn create_session(&self) -> crate::view_model::Result<SessionInfo>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| query | SessionQuery | 列表分页条件 | 创建会话方法没有该参数 |

**返回值**：SessionPage 或 SessionInfo；创建会话不等于已经发送消息。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `ViewModelError` | 配置、状态、Agent 调用或宿主执行失败 | 不伪造成功，不把 UI 更新失败当作业务成功 | 返回安全原因并按实际状态显示或收尾 |

**备注**：

- 是否在创建成功后自动选中会话属于交互细节，本草案不默认决定；选择由 select_session 明确表达。
- 不维护另一套持久化聊天数据库。

**示例**：

**适用范围**：VisionHyperAgent 宿主的 ViewModel；不属于可复用 Agent 库。

**另请参阅**：

#### 4.2.2 `select_session(session_id)`

**说明**：加载或恢复指定会话用于当前交互展示，维护宿主的选中会话状态。

**定义**：

```rust
pub async fn select_session(&self, session_id: SessionId)
    -> crate::view_model::Result<()>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| session_id | SessionId | 用户选择的会话 | 不改变已提交请求绑定的会话 |

**返回值**：所选会话的交互状态准备结果。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `ViewModelError` | 配置、状态、Agent 调用或宿主执行失败 | 不伪造成功，不把 UI 更新失败当作业务成功 | 返回安全原因并按实际状态显示或收尾 |

**备注**：

- 切换页面或会话不能丢失用户草稿；具体草稿组织与历史分页展示字段后续细化。
- 较早的异步加载结果不能覆盖较新的选择，需保留选择操作的关联标识。
- 失败时不静默创建替代会话，不重放历史任务。
- 会话元数据读取、历史分页和后台上下文恢复是不同操作，分别对应 SessionManager.read、history、resume；何时组合调用仍需确认，不把切换显示等同于重放或重建会话。

**示例**：

**适用范围**：VisionHyperAgent 宿主的 ViewModel；不属于可复用 Agent 库。

**另请参阅**：

#### 4.2.3 `send_message(message)`

**说明**：将用户图文输入提交到已明确选中的会话。

**定义**：

```rust
pub async fn send_message(&self, message: AgentMessage)
    -> crate::view_model::Result<TaskId>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| message | AgentMessage | 纯 Rust 的消息与附件数据 | 不传入 Slint Image、ModelRc 或窗口句柄 |

**返回值**：已受理请求的 TaskId；完成、失败和流式内容通过事件更新。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `ViewModelError` | 配置、状态、Agent 调用或宿主执行失败 | 不伪造成功，不把 UI 更新失败当作业务成功 | 返回安全原因并按实际状态显示或收尾 |

**备注**：

- 提交前捕获明确的 SessionId，不能在异步过程中改用后来切换的会话。
- 没有可用选中会话或受理失败时明确报告；不把未提交输入当作已发送。
- 消息转换与附件选择由 View 配合完成，是否可发送及实际提交由本类负责。

**示例**：

**适用范围**：VisionHyperAgent 宿主的 ViewModel；不属于可复用 Agent 库。

**另请参阅**：

#### 4.2.4 `cancel_task(task_id) / retry_task(task_id)`

**说明**：将用户明确的取消或重试操作交给 TaskManager。

**定义**：

```rust
pub async fn cancel_task(&self, task_id: TaskId) -> crate::view_model::Result<()>
pub async fn retry_task(&self, task_id: TaskId) -> crate::view_model::Result<TaskId>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| task_id | TaskId | 准确的目标请求 | 不能仅靠当前显示位置或当前会话确定目标 |

**返回值**：取消受理结果，或新重试请求的 TaskId。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `ViewModelError` | 配置、状态、Agent 调用或宿主执行失败 | 不伪造成功，不把 UI 更新失败当作业务成功 | 返回安全原因并按实际状态显示或收尾 |

**备注**：

- 正在停止与已经取消分别展示；工具是否已经停止依据宿主真实结果。
- 重试由用户发起；不在界面刷新或切页时自动触发重试。

**示例**：

**适用范围**：VisionHyperAgent 宿主的 ViewModel；不属于可复用 Agent 库。

**另请参阅**：

#### 4.2.5 `state()`

**说明**：读取当前交互状态的纯 Rust 快照。

**定义**：

```rust
pub fn state(&self) -> AgentViewState
```

**参数**：

**返回值**：可用于生成界面显示的 AgentViewState，不带 UI 对象和可变内部引用。

**错误**：

**备注**：

- 状态快照不是业务事实的另一来源；会话历史、Agent 生命周期和请求状态仍分别以其所属模块为准。

**示例**：

**适用范围**：VisionHyperAgent 宿主的 ViewModel；不属于可复用 Agent 库。

**另请参阅**：

#### 4.2.6 `take_updates()`

**说明**：向根 View 绑定代码交付交互状态更新通知接收端。

**定义**：

```rust
pub fn take_updates(&self) -> crate::view_model::Result<AgentViewUpdateReceiver>
```

**参数**：

**返回值**：独立的更新接收端；不是原始 AgentEventReceiver。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `ViewModelError` | 配置、状态、Agent 调用或宿主执行失败 | 不伪造成功，不把 UI 更新失败当作业务成功 | 返回安全原因并按实际状态显示或收尾 |

**备注**：

- 本草案按一个根 View 接收端组织，多处展示由根绑定代码分发；重新订阅及多订阅者需求尚待确认。
- 未绑定 View 或接收端被关闭，不能阻塞 Agent 的事件消费和工具执行。
- View 更新通道不传递必须由 UI 执行的业务工具请求。

**示例**：

**适用范围**：VisionHyperAgent 宿主的 ViewModel；不属于可复用 Agent 库。

**另请参阅**：

#### 4.2.7 `shutdown()`

**说明**：由宿主退出协调调用，组织 Agent 和本类后台消费任务的收尾。

**定义**：

```rust
pub async fn shutdown(&self) -> crate::view_model::Result<()>
```

**参数**：

**返回值**：真实关闭结果，不能以关闭 UI 通知端代替 Agent 已关闭。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `ViewModelError` | 配置、状态、Agent 调用或宿主执行失败 | 不伪造成功，不把 UI 更新失败当作业务成功 | 返回安全原因并按实际状态显示或收尾 |

**备注**：

- 等待 Agent 收尾时继续消费其必要事件；不能先停止接收任务，再等待需要该任务处理的反馈。
- 由宿主在基础日志运行作用域结束前协调收尾。正在运行的宿主工具如何取消或等待，仍需明确规则。

**示例**：

**适用范围**：VisionHyperAgent 宿主的 ViewModel；不属于可复用 Agent 库。

**另请参阅**：

#### 4.2.8 `clear_display()`

**说明**：清空当前聊天显示投影，不改变 Agent 会话、任务或未发送草稿。对应需求文档已经确认的清空显示行为，签名仍为接口草案。

**定义**：

```rust
pub fn clear_display(&self) -> crate::view_model::Result<()>
```

**参数**：

**返回值**：当前显示投影的更新结果，不表示会话历史被删除或任务已停止。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| ViewModelError | 本地交互状态无法更新 | 不伪造显示已清空，不修改后台状态 | 返回安全原因 |

**备注**：

- 只处理宿主交互投影，不发送清会话、重置或取消命令。
- 保留选中会话、当前任务及队列的控制标识，保留草稿；不删除 Chat 数据目录，也不重建 Agent 或 Manager。
- 清空后如何呈现后续流式输出和重新加载的历史，属于仍需确认的显示规则，不能通过删除后台数据来解决。

**示例**：

**适用范围**：宿主 ViewModel 的交互行为，不属于 Agent 库会话 API。

**另请参阅**：[Agent 需求文档](../agent/Agent.md)、[SessionManager](../agent/api/SessionManager.md)、[TaskManager](../agent/api/TaskManager.md)

---

## 5. 事件与消息处理

### 5.1 事件与通知

| 名称 | 载荷类型 | 触发条件 | 说明 |
| --- | --- | --- | --- |
| StateChanged | 更新标识及状态读取依据 | 交互状态发生变化 | View 根据通知读取最新快照；具体增量策略待确认 |
| Diagnostic | 安全错误信息 | 需要宿主呈现的失败或提示 | 不携带 KEY 或未经筛选的协议原文 |
| Agent 内部事件 | 不直接向 View 转交执行责任 | 本类接收并转换 | ToolCallRequested 交给宿主业务入口，不等待 UI 执行业务 |

### 5.2 消息处理与回调

```text
用户操作
→ Slint 回调
→ src/view/ 中的绑定代码
→ AgentViewModel 的交互方法
→ Agent / Manager 公开接口
→ AgentTransport（宿主侧）→ ZmqClient
→ inproc（两端共享同一个底层 Context）
→ ZmqServer → AgentTransport（后台侧）→ Runtime → CodexAdapter

后台结果
→ AgentEventReceiver（由 AgentViewModel 独立消费）
→ 更新纯 Rust 交互状态
→ AgentViewUpdateReceiver
→ View 绑定代码安排 UI 线程更新
→ Slint 属性／模型刷新
```

工具调用走另一条非 UI 执行路径：AgentEvent 中的 ToolCallRequested → 本类注入的 HostToolHandler → 宿主同一业务能力 → ToolManager.respond。该路径在没有窗口时也必须工作，不能放进 View 回调，也不能在等待工具完成期间停止消费其他 Agent 事件。

Agent 原始事件端与 View 更新端的故障必须分开：没有 View 订阅不能阻塞核心；原始 AgentEventReceiver 异常却意味着状态或工具请求可能无法继续处理，应明确报告并进入约定的故障／收尾流程，不能当作普通 View 解绑忽略。

后台事件按自己的 SessionId、TaskId 和输出条目标识更新对应投影，不能一律追加到当前选中会话。整体 AgentState 只使用库发布的真实状态，不根据某条回复文本、一个工具结果或一次传输调用自行推断。

这里的“ViewModel 端 ZMQ 对象”是 Agent 对外入口内部的 ZmqClient；本类通过 Agent／Manager 使用它，不再重复创建客户端或 Context。Agent 后台端由 ZmqServer 承担，两端对象独立，底层实现共用。

---

## 6. 其他成员

### 6.1 常量

### 6.2 枚举与嵌套类型

#### 6.2.1 `AgentViewState`

归属该 ViewModel 的交互状态投影，不作为新的 Agent 历史或业务事实存储。字段随 UI 契约进一步确认。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| 选中会话 | `Option<SessionId>` | 宿主交互选择，不覆盖请求提交时的 SessionId |
| 会话列表与历史投影 | SessionInfo、HistoryPage 等已有类型 | 来自 SessionManager；不另写历史保存规则 |
| 请求与回复投影 | TaskInfo 及流式内容 | 依真实任务事件更新 |
| 草稿 | 宿主交互数据 | 不因切页或缩放丢失；具体字段与保存规则待确认 |
| Agent 状态与安全提示 | AgentState 等 | Ready 不能显示成模型已联网验证成功 |

#### 6.2.2 `AgentViewUpdate / AgentViewUpdateReceiver`

归属 ViewModel 的纯 Rust 状态通知类型，与 Agent 库事件分开。

```rust
pub struct AgentViewUpdateReceiver { /* 私有通知接收端 */ }

impl AgentViewUpdateReceiver {
    pub async fn next(&mut self)
        -> crate::view_model::Result<Option<AgentViewUpdate>>;
}
```

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| StateChanged | 状态更新标识 | 通知 View 获取或应用状态，不带业务执行回调 |
| Diagnostic | 安全原因 | 需展示的失败或提示；不重分类为模型任务成功 |
| 接收端结束 | Option 为 None | 表示该通知端结束，不代表 Agent 任务已结束 |

#### 6.2.3 `HostToolHandler / HostToolFuture`

宿主注入的非 UI 业务入口，归属本 ViewModel 的集成契约，不放入通用 Agent 库。以下为异步回传形式的草案。

```rust
pub type HostToolFuture = std::pin::Pin<Box<
    dyn std::future::Future<Output = ToolResult> + Send + 'static
>>;

pub type HostToolHandler = std::sync::Arc<
    dyn Fn(ToolCall) -> HostToolFuture + Send + Sync
>;
```

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| 执行职责 | 宿主提供 | 调用与按钮相同的业务能力，不让 AgentViewModel 复制训练或相机逻辑 |
| 缺失或不支持 | 明确失败 | 不能把请求交给 UI 等待处理，也不能无限挂起；由库允许的回传路径报告失败 |
| 资源与取消 | 待确认 | 并发、时限和取消契约后续明确；不预设任意工具都能停止 |

#### 6.2.4 `ViewModelError / Result`

沿用当前 view_model 模块已有共享错误接口；AgentError 在宿主边界转换为可安全记录的原因，不在本次文档工作中修改既有错误类型。

### 6.3 索引器与运算符

### 6.4 扩展成员

### 6.5 析构／终结器

关闭由宿主显式协调。View 解绑不直接等于取消任务；真正退出时必须处理 Agent、事件消费和宿主工具任务的生命周期，不只释放一个窗口引用。

---

## 7. 使用示例与约束

### 7.1 使用示例

```text
宿主装配配置、Agent 与工具处理入口
→ 创建 AgentViewModel
→ 根 View 绑定交互回调并取得更新接收端
→ 宿主异步初始化 AgentViewModel
→ AgentViewModel 独立消费 AgentEvent
→ View 的更新接收任务用 Slint 的事件循环投递机制安排刷新
→ 退出时宿主协调 shutdown，再结束基础服务运行作用域
```

View 绑定层可使用所选 Slint 版本的 invoke_from_event_loop 或 Weak::upgrade_in_event_loop；这些调用及窗口弱引用必须留在 View 层，不进入本 ViewModel。投递失败也需按 View 的生命周期处理或明确报告，不能静默改写任务结果。

### 7.2 使用约束

- View 只连接本 ViewModel，不直接使用 CONFIG、驱动、AgentTransport 或 Codex。
- 本类不持有 Slint Window、Weak、ModelRc、Image 或事件循环对象；界面专用类型转换属于 View。
- 事件接收与业务工具执行独立于 View 是否存在；UI 更新断开不能变成后端执行阻塞点。
- 不使用 ZeroMQ 连接 UI 与 ViewModel。ZMQ 只位于 Agent 库内已确认的线程边界。
- MainWindowViewModel 负责子模块协调，本类负责聊天和会话交互，其他功能 ViewModel 各自负责业务。
- 保持已有导航、全局聊天、草稿与布局要求；本轮不修改 UI 行为或布局保存逻辑。

**待确认事项：**

- 状态投影字段、增量更新及根 View 的订阅／解绑规则。
- 宿主异步任务的组织、工具处理并发和取消、退出收尾顺序与时限。
- 会话选择、历史分页、草稿与发送受理失败的交互细节；不得损坏已有草稿保留规则。
- 清空显示后的增量展示与历史重载规则；后台会话和 Chat 数据保持不变。
- Skill／Chat 的宿主路径定位、准备和权限失败处理；根目录已确认，不回退到其他位置。

### 7.3 适用范围与依赖版本

目标平台为 Linux、Windows。Slint 使用项目固定的 1.17.1，但 Slint 依赖仅出现在 View 绑定层；本类的运行逻辑应能脱离窗口使用。Agent 与 ZMQ 版本沿用各自文档，不另设一套依赖基线。

本软件运行时目录固定为可执行文件所在目录下的 Skill 和 Chat。源码资源目录不随之迁移；目录无法使用时报告错误，不自动提权或修改全局环境，也不把相对启动位置当作存储根。

### 7.4 验证现状

已只读核对现有 View 绑定入口和共享 ViewModel 错误类型，并参考固定 Slint 版本的事件循环投递文档。尚未实现 AgentViewModel、UI 绑定或后台事件消费，没有编译、预览或运行验证。

### 7.5 另请参阅

- [Agent](../agent/api/Agent.md)
- [AgentTransport](../agent/api/AgentTransport.md)
- [SessionManager](../agent/api/SessionManager.md)
- [TaskManager](../agent/api/TaskManager.md)
- [ToolManager](../agent/api/ToolManager.md)
- [模块规范模板](../../模块规范模板.md)
- [软件框架](../../软件框架.md)
- [当前 View 绑定入口](../../../src/view/mod.rs)
- [当前 ViewModel 共享类型](../../../src/view_model/mod.rs)
- [Slint 1.17.1 事件循环投递](https://docs.rs/slint/1.17.1/slint/fn.invoke_from_event_loop.html)
- [ZmqClient](../drivers/ZmqClient.md)
- [ZmqServer](../drivers/ZmqServer.md)

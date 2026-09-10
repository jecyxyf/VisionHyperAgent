# Agent

> **状态：接口设计草案，尚未实现、编译或运行验证。** 职责依据[Agent 需求文档](../Agent.md)及已确认的功能分工与通信边界；具体签名、数据字段和状态细节是待评审的设计建议，不等同于已确认的公开接口。Python 相关内容继续暂缓。空栏目保留，不补造无关成员。
>
> 本类属于库对外接口；不依赖 Slint 窗口、控件、事件循环或 VisionHyperAgent 的全局配置／日志对象。

## 变更记录

| 版本 | 日期 | 作者 | 变更说明 |
|------|------|------|----------|
| 0.1.0 | 2026-09-10 | Codex | 初始接口设计草案，待确认后实现 |
| 0.1.1 | 2026-09-10 | Codex | 补充通信层与宿主接入文档关联，明确职责边界 |
| 0.1.2 | 2026-09-10 | Codex | 明确 ZmqClient 与 ZmqServer 的两端角色关联 |
| 0.1.3 | 2026-09-10 | Codex | 统一整体生命周期、事件出口及 Skill／Chat 目录规则 |

---

## 1. 类型定义与关系

### 1.1 定义

**说明**：通用 Agent 库的统一入口。负责接收配置、装配功能句柄、控制运行生命周期并交付事件接收端；会话、对话请求、Skills 和工具规则分别由对应模块负责。

Agent 是宿主使用的门面及生命周期协调者，不是后台运行对象本身。各 Manager 是面向同一运行实例的功能句柄，具体功能状态归其模块维护；Runtime 驱动这些状态，不复制第二套队列、注册表或历史。

**类型声明**：

```rust
pub struct Agent { /* 私有实现 */ }
```

### 1.2 命名空间／所属模块

- **模块路径**：拟在 `vision_hyper_agent::agent` 重导出；具体子模块文件划分待实现阶段确认。
- **所属层**：通用 Agent 接入能力；保持单 Cargo 包，不按类拆分进程或独立库。
- **源码位置**：当前 `src/agent/mod.rs` 仅有职责占位，本文所述类型尚未落地，本文仅定义待评审的接口。

### 1.3 基类／继承关系

### 1.4 派生类型

### 1.5 实现接口／Trait

### 1.6 组合与依赖

| 依赖项 | 关系类型 | 版本／固定提交 | 说明 |
| --- | --- | --- | --- |
| [SessionManager](SessionManager.md) | 组合 |  | 会话与历史访问句柄 |
| [TaskManager](TaskManager.md) | 组合 |  | 对话请求、队列与取消句柄 |
| [SkillManager](SkillManager.md) | 组合 |  | 已配置 Skills 的查询与接入句柄 |
| [ToolManager](ToolManager.md) | 组合 |  | 工具定义注册及执行结果回传句柄 |
| [AgentTransport](AgentTransport.md) | 内部通信上下文 |  | 封装命令、响应和事件链路，宿主侧使用 ZmqClient，后台侧使用 ZmqServer |
| [AgentRuntime](AgentRuntime.md) | 拥有运行生命周期 |  | 唯一后台运行上下文；不为每个 Manager 单独启动线程 |

---

## 2. 构造／初始化

### 2.1 `new(config)`

**说明**：构造运行入口并保存宿主传入的配置，不读取文件、不联网，也不启动模型请求。

**定义**：

```rust
pub fn new(config: AgentConfig) -> Self
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| config | AgentConfig | 本实例的模型配置与受控资源位置 | 由宿主提供，不读取开发者个人 Codex 配置 |

**返回值**：尚未初始化的 Agent。

**错误**：

**备注**：
- 构造与初始化分开；需要访问外部环境的检查交给 initialize 返回结果。
- 配置为本实例参数，不是另一套 ConfigManager；不修改全局环境。
- 逻辑装配时为同一运行实例准备 Host 与 Runtime 两端通信配置，实例标识一致、角色不同；不在构造阶段打开 Socket 或启动 Codex。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：

### 2.2 `initialize()`

**说明**：在后台建立本地运行上下文、加载允许的资源并完成 Codex 进程内客户端初始化。

**定义**：

```rust
pub async fn initialize(&self) -> Result<(), AgentError>
```

**参数**：

**返回值**：本地运行上下文达到可受理条件；不表示远端模型服务已经通过真实请求验证。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `InvalidInput` | 必需配置不完整或不合法 | 未进入 Ready | 修正配置 |
| `Backend / Transport / Io` | 上游调用、通信或存储失败 | 以实际结果为准，不能推定已回滚 | 接收错误并按业务规则处理，不自动重放有副作用的请求 |

**备注**：
- 不得阻塞 UI；只在运行初始化阶段进行必要 I/O。
- 初始化必须落实配置隔离与内置执行工具限制；无法保证限制时不得当作初始化成功。
- 一次模型请求的鉴权或网络失败不应被混同为所有本地初始化均失败。
- 重复初始化、并发初始化及失败后能否在同一对象重试尚待确认。
- 初始化阶段只创建一份供该链路使用的底层 Context，再将共享句柄交给两端所属上下文。Ready 由本类的生命周期协调逻辑在两端通信及后台初始化均成功后确认，不能只凭某一端连接成功设置。
- Context 和本地任务句柄通过启动装配传入，不序列化进 ZMQ 帧。若 inproc 尚未建立就发生错误，必须能通过启动任务的结果回报，不能等待一个尚不可用的通道返回初始化失败。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：[AgentRuntime](AgentRuntime.md)、[CodexAdapter](CodexAdapter.md)

---

## 3. 字段与属性

### 3.1 字段

### 3.2 属性

| 属性名 | 类型 | 访问级别 | 读写方式 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| state | AgentState | public | state() 只读 |  | 反映生命周期协调逻辑综合两端与后台结果维护的整体状态，不等同于当前对话请求状态 |
| sessions / tasks / skills / tools | 对应 Manager 句柄 | public | 访问器只读 |  | 返回关联同一运行实例的轻量句柄，不能直接改写内部状态 |

---

## 4. 方法

### 4.1 关联函数／静态方法

### 4.2 实例方法

#### 4.2.1 `sessions() / tasks() / skills() / tools()`

**说明**：提供独立功能入口，让外部通过 agent.sessions().create(...) 等方式使用能力。

**定义**：

```rust
pub fn sessions(&self) -> SessionManager
pub fn tasks(&self) -> TaskManager
pub fn skills(&self) -> SkillManager
pub fn tools(&self) -> ToolManager
```

**参数**：

**返回值**：各自的可克隆功能句柄；获取句柄不创建新 Agent、会话引擎或后台线程。

**错误**：

**备注**：
- 初始化前可以获取句柄，但需要运行时的操作仍须返回未就绪错误。
- 工具定义的启动前注册规则见 ToolManager；不因此允许提前发起模型请求。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：[SessionManager](SessionManager.md)、[TaskManager](TaskManager.md)、[SkillManager](SkillManager.md)、[ToolManager](ToolManager.md)

#### 4.2.2 `state()`

**说明**：读取当前运行实例的生命周期快照。

**定义**：

```rust
pub fn state(&self) -> AgentState
```

**参数**：

**返回值**：AgentState 快照；查询不执行文件或网络访问。

**错误**：

**备注**：
- 快照不能当作后续调用一定成功的保证；异步调用仍独立返回真实结果。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：

#### 4.2.3 `take_events()`

**说明**：取得统一事件接收端，将流式内容、任务变化及工具请求交给宿主处理。

**定义**：

```rust
pub fn take_events(&self) -> Result<AgentEventReceiver, AgentError>
```

**参数**：

**返回值**：独立持有的 AgentEventReceiver，不借用 Agent 到整个等待期间。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `InvalidState` | 事件接收端已经取走 | 不创建第二份事件流 | 使用已有接收端 |

**备注**：
- 本草案采用单一接收端，便于保持事件顺序；宿主按自身需求分发给界面等使用方。是否需要库内多订阅者仍待确认。
- 接收端独立于控制入口，等待事件时仍能提交任务、回传工具结果或发起关闭。
- AgentEventReceiver 是本实例统一事件出口：归并通信返回的后台业务事件和生命周期协调逻辑生成的 StateChanged，不是另一套独立事件引擎。在 VisionHyperAgent 中由 AgentViewModel 独占消费，View 不再竞争读取原始 Agent 事件。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：[AgentRuntime](AgentRuntime.md)、[ToolManager](ToolManager.md)

#### 4.2.4 `shutdown()`

**说明**：请求后台停止受理新工作，并完成运行时及资源的正常收尾。

**定义**：

```rust
pub async fn shutdown(&self) -> Result<(), AgentError>
```

**参数**：

**返回值**：Agent 生命周期协调逻辑确认的整体关闭结果，不以成功投递关闭消息或后台循环结束代替整条链路已收尾。

成功条件须同时覆盖 Runtime／Codex 结束、两端通信收尾及本实例拥有的资源释放；单独收到后台结束通知不能代表 shutdown 成功。具体关闭握手、等待时限和宿主工具策略仍待确认。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `Backend / Transport / Io` | 上游调用、通信或存储失败 | 以实际结果为准，不能推定已回滚 | 接收错误并按业务规则处理，不自动重放有副作用的请求 |

**备注**：
- 关闭期间仍需处理必要的取消反馈、工具结果和最终事件，不能先切断所有通道。
- 等待任务的处理策略、当前任务的停止规则、关闭时限与重复调用行为尚待确认；不填写默认超时。
- 会话保存复用 Codex；不重新保存一套聊天记录。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：[AgentRuntime](AgentRuntime.md)

---

## 5. 事件与消息处理

### 5.1 事件与通知

| 名称 | 载荷类型 | 触发条件 | 说明 |
| --- | --- | --- | --- |
| StateChanged | AgentState | 生命周期发生变化 | 统一通过 AgentEvent 输出 |
| TaskChanged / MessageDelta / TaskFinished | TaskManager 中定义的载荷 | 对话请求变化、流式输出或终态 | 携带 TaskId 与 SessionId |
| ToolCallRequested | ToolCall | 上游请求已注册的宿主工具 | 宿主负责实际执行，见 ToolManager |
| Diagnostic | 安全诊断信息 | 出现需要宿主处理的运行问题 | 不包含 KEY、原始配置或无关数据 |

### 5.2 消息处理与回调

宿主持续接收 AgentEvent，将需要执行的工具请求交给自身业务能力，再通过 ToolManager 返回结果。界面更新与 UI 线程切换由宿主的 View 适配代码承担；库不调用 Slint。

在 VisionHyperAgent 中的具体接入关系为：

```text
Slint UI ↔ View 绑定代码 ↔ AgentViewModel
                              ↕ Manager 接口／纯 Rust 状态与事件
                            Agent
                              ↕
                     AgentTransport（宿主侧）
                              ↕ ZmqClient
                    inproc（同一个底层 Context）
                              ↕ ZmqServer
                     AgentTransport（后台侧）
                              ↕
                         AgentRuntime → CodexAdapter → Codex
```

请求、响应和后台事件分别由 AgentTransport 关联。AgentViewModel 管理交互状态并独立处理宿主工具调用，View 绑定代码仅转换显示数据与安排 UI 线程刷新。其他非 UI 宿主可以直接使用 Agent 事件接口，不需要依赖 AgentViewModel。

ZmqClient 与 ZmqServer 是两端分别持有的角色对象，共用驱动实现和底层 Context，但不共用 Socket。Agent 的公开入口仍封装宿主端通信，ViewModel 不需要另外创建客户端或改变 agent.sessions().create(...) 的调用形式。

---

## 6. 其他成员

### 6.1 常量

### 6.2 枚举与嵌套类型

#### 6.2.1 `AgentConfig`

归属 Agent 的运行参数；没有自动继承个人环境的行为。字段结构为当前草案，其中 VisionHyperAgent 的 Skill／Chat 路径规则已经确认，不属于可以自行变更的隐含默认值。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| base_url / api_key / model | String | 宿主提供的 Responses API 兼容服务配置；KEY 不进入 Debug、日志、事件或诊断 |
| work_dir | PathBuf | 受控任务工作目录，不作为应用程序目录，也不改变 Skill／Chat 位置 |
| storage_dir | PathBuf | 会话记录数据根；VisionHyperAgent 固定传入可执行文件所在目录下的 `Chat`。不把该字段默认等同于整个 Codex Home 或凭据目录 |
| skills_dirs | `Vec<PathBuf>` | 通用库显式接收来源；VisionHyperAgent 只传入可执行文件所在目录下的 `Skill`，不据此开放任意文件访问 |

这两条产品路径已经确认，具体映射见[需求文档运行时目录](../Agent.md)。由宿主按路径组件组装，不以进程当前工作目录拼接。Skill／Chat 的大小写保持不变；目录准备、权限错误和 Codex 内部布局的实现仍需确认，不静默回退到其他位置。

#### 6.2.2 `AgentState`

Agent 对外可见的整体生命周期状态，由 Agent 的生命周期协调逻辑基于两端通信与 Runtime 的真实报告维护一份。Runtime 可以保留局部运行阶段，但不能另维护一份可独立改写的整体 AgentState。以下是评审用状态集合，失败恢复及并发转换规则尚未全部确认。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| Created / Initializing | 生命周期 | 已构造／后台初始化中 |
| Ready | 生命周期 | 本地可受理；不代表模型服务始终联网可用 |
| Stopping / Stopped | 生命周期 | 整条链路正在收尾／本实例约定的关闭条件均已满足；不只表示模型或后台循环停止 |
| Failed | 生命周期 | 运行实例无法继续工作；不用于表达普通单次请求失败 |

#### 6.2.3 `AgentEvent`

库对外事件契约的草案，不直接暴露 Codex 协议或 Slint 类型。它是统一出口，不表示当前字段已经冻结。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| StateChanged | AgentState | 由整体生命周期协调逻辑发布，不由传输层或某个功能模块自行推断 |
| TaskChanged / TaskFinished | TaskInfo | 请求快照或最终结果，见 TaskManager |
| MessageDelta | TaskId、SessionId、所属条目标识与消息片段 | 区分同一请求的不同输出项；具体公开条目类型待确认，不等于终态成功 |
| ToolCallRequested | ToolCall | 宿主需要执行的已注册能力，见 ToolManager |
| Diagnostic | 安全错误上下文 | 与具体请求有关时带上关联标识 |

#### 6.2.4 `AgentEventReceiver`

归属 Agent 的事件通道类型，不是额外的事件管理器。

```rust
pub struct AgentEventReceiver { /* 私有接收端 */ }

impl AgentEventReceiver {
    pub async fn next(&mut self) -> Result<Option<AgentEvent>, AgentError>;
}
```

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| next() 返回 Some | AgentEvent | 取到一条事件 |
| next() 返回 None | 事件流正常结束 | 不代表某个未收到终态的任务自动成功 |
| next() 返回 Err | AgentError | 事件通道异常，必须交给宿主处理 |

#### 6.2.5 `AgentError`

库共用的公开错误类型，归属 Agent 模块，不依赖 VisionHyperAgent 的 Foundation 单例。下列为分类草案，原始错误链须保留但敏感信息必须过滤。

涉及 ZMQ 传输时，原始错误对象及错误链留在产生端，线上只传可序列化且脱敏的分类、消息和必要关联；接收端据此还原对外错误含义，不承诺跨传输保留原 Rust 错误对象。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| InvalidInput / NotInitialized / InvalidState | 调用错误 | 参数不合法、尚未就绪或当前状态不允许操作 |
| NotFound / Conflict | 标识与注册错误 | 目标不存在或定义冲突 |
| Backend / Transport / Io | 外部错误 | Codex／模型服务、线程通信或本地存储失败；不能包装成成功 |

### 6.3 索引器与运算符

### 6.4 扩展成员

### 6.5 析构／终结器

正常退出使用显式 shutdown 并等待结果。对象释放时的兜底策略尚待确认，不把异步收尾寄托于未定义的析构行为。

---

## 7. 使用示例与约束

### 7.1 使用示例

```rust
use vision_hyper_agent::agent::{Agent, AgentConfig, AgentError, AgentEventReceiver};

fn prepare_agent(config: AgentConfig) -> Result<(Agent, AgentEventReceiver), AgentError> {
    let agent = Agent::new(config);
    let events = agent.take_events()?;
    Ok((agent, events))
}
```

仅为接口使用示意，未编译或执行；调用方需提供已声明的输入和异步运行上下文，不依赖 Slint 事件循环。

宿主取得控制入口与事件接收端后，先安排独立的事件消费，再异步初始化并使用各功能句柄。不能等到 shutdown 返回后才开始消费事件；本文暂不选定宿主的异步调度实现。

### 7.2 使用约束

- Agent 是运行入口，不是会话对象，也不是训练业务控制器。
- 所有 Manager 句柄共享同一运行实例；传输方式属于内部细节，不将 ZeroMQ Socket 暴露给宿主。
- Manager 通过 AgentTransport 使用内部通信；不自行创建 Context、拼接传输帧或直接调用底层驱动。
- 控制入口和事件消费必须能并行工作；关闭时仍需处理必要的最终事件与工具反馈。
- Agent 拥有运行生命周期；Runtime 不反向持有 Agent 或宿主对象。Manager 副本只是访问句柄，关闭后不能自动创建替代运行时。
- 整体 AgentState 只由生命周期协调逻辑维护；Runtime 的局部阶段、TaskState、工具调用状态及 ViewModel 显示状态分别归属，不互相冒充。
- 清空聊天显示不提供为会话重置接口；会话、请求和界面状态分别处理。
- 不新增 Python 执行、动态编译或运行环境安装接口。

**待确认事项：**

- 初始化重复调用和失败恢复规则。
- 关闭时排队请求与正在执行请求的处理策略、时限及资源释放保证。
- 单事件接收端是否满足宿主需要，以及事件背压、缓存容量和丢失防护。
- 通信资源策略的具体字段与配置入口；不默认由 Runtime 读取全局 APP_PARAM 或自行选择数值。
- 固定 Skill／Chat 根目录与 Codex 发现、历史及索引存储的映射，目录不可用时的处理，以及其他内部数据的独立位置；不重新选择产品根目录。

### 7.3 适用范围与依赖版本

目标平台为 Linux、Windows，尚未开展跨平台验证。软件版本与运行环境兼容范围沿用项目基线，不在本文填写未经确认的最低版本或资源限额。

Codex 固定提交为 `73a1148c9c775c2a4616ce5096291740a00ed68a`；项目侧 ZeroMQ 固定提交为 `5d78967001abb1aece2fba878d6151cb66cd1767`。完整来源见[依赖基线](../../../../src/depends/README.md)，不跟随分支自动升级。

### 7.4 验证现状

本轮只核对需求、类间职责、文档引用及可见的上游接口；没有创建 Rust 实现，没有编译、执行示例或进行模型、工具、平台兼容性验证。

### 7.5 另请参阅

- [Agent 需求文档](../Agent.md)
- [模块规范模板](../../../模块规范模板.md)
- [SessionManager](SessionManager.md)
- [TaskManager](TaskManager.md)
- [SkillManager](SkillManager.md)
- [ToolManager](ToolManager.md)
- [AgentRuntime](AgentRuntime.md)
- [CodexAdapter](CodexAdapter.md)
- [AgentTransport](AgentTransport.md)
- [宿主 AgentViewModel](../../viewmodel/AgentViewModel.md)
- [ZmqClient](../../drivers/ZmqClient.md)
- [ZmqServer](../../drivers/ZmqServer.md)

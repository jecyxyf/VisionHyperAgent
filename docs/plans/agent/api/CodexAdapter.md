# CodexAdapter

> **状态：接口设计草案，尚未实现、编译或运行验证。** 职责依据[Agent 需求文档](../Agent.md)及已确认的功能分工与通信边界；具体签名、数据字段和状态细节是待评审的设计建议，不等同于已确认的公开接口。Python 相关内容继续暂缓。空栏目保留，不补造无关成员。
>
> 本类只供库内部使用，宿主通过 Agent 及其功能管理类访问。

## 变更记录

| 版本 | 日期 | 作者 | 变更说明 |
|------|------|------|----------|
| 0.1.0 | 2026-09-10 | Codex | 初始接口设计草案，待确认后实现 |
| 0.1.1 | 2026-09-10 | Codex | 补充通信层与宿主接入文档关联，明确职责边界 |
| 0.1.2 | 2026-09-10 | Codex | 补充上游事件缓存风险及 Skill／Chat 的适配验证项 |

---

## 1. 类型定义与关系

### 1.1 定义

**说明**：封装固定版本 Codex 进程内客户端，转换本库请求、响应及事件。仅承担上游接入和协议差异，不管理宿主业务，不重写会话引擎、模型请求重试引擎或工具执行器。

**类型声明**：

```rust
pub(crate) struct CodexAdapter { /* 私有实现 */ }
```

### 1.2 命名空间／所属模块

- **模块路径**：拟在 `vision_hyper_agent::agent` 内部使用，不对外重导出。
- **所属层**：通用 Agent 接入能力；保持单 Cargo 包，不按类拆分进程或独立库。
- **源码位置**：当前 `src/agent/mod.rs` 仅有职责占位，本文所述类型尚未落地，本文仅定义待评审的接口。

### 1.3 基类／继承关系

### 1.4 派生类型

### 1.5 实现接口／Trait

### 1.6 组合与依赖

| 依赖项 | 关系类型 | 版本／固定提交 | 说明 |
| --- | --- | --- | --- |
| Codex in-process client | 直接上游 | 73a1148c9c775c2a4616ce5096291740a00ed68a | 使用所选提交的 InProcessAppServerClient，而非另起 CLI 子进程替代既定方案 |
| Codex app-server protocol | 直接上游 | 同一固定提交 | 以可见的类型和实验性字段要求为准 |
| [AgentRuntime](AgentRuntime.md) | 内部使用方 |  | 接收规范化请求与事件，不把上游类型泄露给宿主 |

---

## 2. 构造／初始化

### 2.1 `new()`

**说明**：构造尚未连接上游的适配对象，不执行 I/O。

**定义**：

```rust
pub(crate) fn new() -> Self
```

**参数**：

**返回值**：未启动的 CodexAdapter。

**错误**：

**备注**：
- 适配器生命周期由 Runtime 持有；每个 Manager 不各自创建一个 Codex 客户端。

**示例**：

**适用范围**：库内部使用，不对宿主公开。

**另请参阅**：

---

## 3. 字段与属性

### 3.1 字段

| 字段名 | 类型 | 访问级别 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| client | `Option<InProcessAppServerClient>` | private |  | 启动后持有固定版本客户端；上游类型不对宿主公开 |
| 协议关联 | 内部数据 | private |  | 关联请求 ID、thread／turn 与工具调用；不等同于业务队列 |

### 3.2 属性

---

## 4. 方法

### 4.1 关联函数／静态方法

### 4.2 实例方法

#### 4.2.1 `start(config)`

**说明**：构建隔离的上游配置，启动进程内运行时并完成初始化握手。

**定义**：

```rust
pub(crate) async fn start(&mut self, config: &AgentConfig) -> Result<(), AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| config | &AgentConfig | 宿主传入的连接和资源配置 | 不继承开发者个人 Codex 配置或密钥环境 |

**返回值**：本地客户端初始化完成；不声称模型服务兼容性已经验证。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `InvalidInput / Backend / Io` | 配置映射、实验能力协商或本地资源启动失败 | 不进入可用状态 | 返回安全诊断并清理已建立资源 |

**备注**：
- 固定源码的启动参数还需要调用方身份、路径、状态与环境等对象；这些是接入验证项，不由文档假装已经装配完成。
- 指定的 model 不可用时不得静默切换另一个模型。
- 必须明确禁止模型使用内置通用终端和任意文件操作；不能把“传入动态工具”误当作已经关闭其他工具。
- 所需动态工具字段是当前固定协议中的实验性字段，其初始化开关与完整路径必须在实现阶段验证。

**示例**：

**适用范围**：库内部使用，不对宿主公开。

**另请参阅**：

#### 4.2.2 `request_handle()`

**说明**：取得独立持有的受控请求句柄，使持续事件读取不独占所有控制操作。

**定义**：

```rust
pub(crate) fn request_handle(&self) -> Result<CodexRequestHandle, AgentError>
```

**参数**：

**返回值**：CodexRequestHandle；它属于适配模块，不是新增功能管理器。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `NotInitialized / InvalidState` | 客户端尚未启动或已经关闭 | 没有创建替代运行时 | 由 Runtime 处理 |

**备注**：
- 此处定义的是本库封装句柄，不宣称上游 request handle 已经包含本文全部包装方法。
- 请求发送可以复用上游已有句柄能力；工具回复等控制如何安全转交持有客户端的上下文仍需实现验证。

**示例**：

**适用范围**：库内部使用，不对宿主公开。

**另请参阅**：

#### 4.2.3 `next_event()`

**说明**：持续读取并归一化上游通知和服务端请求，保留必要的原始关联标识。

**定义**：

```rust
pub(crate) async fn next_event(&mut self) -> Result<Option<CodexEvent>, AgentError>
```

**参数**：

**返回值**：Some 为一个内部事件，None 表示上游事件流结束；异常结束还需根据运行状态报告错误。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `Backend / Transport` | 协议不符合预期或事件链路失败 | 不伪造任务终态 | 交给 Runtime 明确处理 |

**备注**：
- 没有新的事件时异步等待，不阻塞 UI。
- 与独立请求句柄并行使用，不能因为等事件而阻止工具结果或取消请求发送。
- 重要事件不能静默丢弃；协议扩展的兼容策略待确认，不能把未知错误统一当作可忽略通知。

**示例**：

**适用范围**：库内部使用，不对宿主公开。

**另请参阅**：

#### 4.2.4 `shutdown()`

**说明**：结束上游进程内运行时并释放持有的客户端资源。

**定义**：

```rust
pub(crate) async fn shutdown(&mut self) -> Result<(), AgentError>
```

**参数**：

**返回值**：实际关闭结果。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `Backend / Transport / Io` | 底层收尾失败或未按预期完成 | 不记录为无条件正常退出 | 向 Runtime 返回真实结果 |

**备注**：
- 上游文档描述有限等待与中止行为；本项目仍需明确如何区分正常关闭、超时与异常终止。
- 不在本适配层叠加另一个未确认的自动重启或重试策略。

**示例**：

**适用范围**：库内部使用，不对宿主公开。

**另请参阅**：

---

## 5. 事件与消息处理

### 5.1 事件与通知

| 名称 | 载荷类型 | 触发条件 | 说明 |
| --- | --- | --- | --- |
| 会话／轮次事件 | CodexEvent | 上游发送对应通知 | 携带 thread／turn 等关联信息 |
| 工具调用请求 | CodexEvent 的工具请求载荷 | item/tool/call | 保留原始服务端请求 ID，不直接执行业务 |
| 诊断／结束 | 安全内部事件 | 上游警告、错误或事件流结束 | 由 Runtime 判断影响范围并转为公开事件 |

### 5.2 消息处理与回调

适配器只接入本期需要的协议能力。收到禁止或未支持的执行／审批请求时，必须通过上游允许的拒绝或错误路径结束该请求并报告原因，不能自动授权，也不能无限等待。项目侧 ZMQ 收发止于 AgentTransport；Runtime 通过普通 Rust 接口使用本适配层，本类不操作 ZMQ 驱动，上游进程内类型化通道保持原样。

---

## 6. 其他成员

### 6.1 常量

### 6.2 枚举与嵌套类型

#### 6.2.1 `CodexRequestHandle`

适配模块的内部请求通道类型，供 Runtime 在独立事件读取期间发起操作；不直接暴露给宿主。

```rust
pub(crate) struct CodexRequestHandle { /* 私有请求通道 */ }

impl CodexRequestHandle {
    pub(crate) async fn request(&self, request: CodexRequest)
        -> Result<CodexResponse, AgentError>;
    pub(crate) async fn respond_tool(&self, request: BackendToolRequest,
        result: ToolResult) -> Result<(), AgentError>;
}
```

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| request | CodexRequest → CodexResponse | 受理与最终任务完成分开；模型请求不得阻塞事件读取 |
| respond_tool | BackendToolRequest + ToolResult | 回复原始服务端请求；完成发送不等于模型已处理结果 |

#### 6.2.2 `CodexRequest / CodexResponse`

适配层内部的受控请求与结果类型，只覆盖本期功能。以下是与所选源码可见接口的映射，不是本项目已验证的功能清单。

| 本库能力 | 上游接口／字段 | 映射要求 |
| --- | --- | --- |
| 创建／恢复会话 | thread/start、thread/resume | 保持持久化目标、指定模型与工具限制，不静默使用临时会话或替代模型 |
| 会话列表与元数据 | thread/list、thread/read | 只访问本实例隔离范围；分页和错误如实转换 |
| 历史分页 | thread/turns/list、thread/items/list | 所选源码对分页历史推荐分开读取；对外 HistoryPage 的组合映射仍需确认 |
| 提交／停止模型轮次 | turn/start、turn/interrupt | 与 TaskId 显式对应，取消受理不等于任务已经结束 |
| 技能查询／来源设置 | skills/list、skills/extraRoots/set | 本软件来源固定为应用程序目录下的 Skill；设置额外来源不等于已排除全部默认来源，需核查实际加载路径 |
| 工具声明 | thread/start.dynamicTools | 在会话创建／恢复边界处理允许的定义，实验能力要求需验证 |
| 工具结果 | item/tool/call 的响应 | 结构化结果映射至上游内容项；success 保留真实业务含义 |

#### 6.2.3 `CodexEvent`

保留上游身份与时序的内部事件。公开 AgentEvent 的生成和业务请求关联由 Runtime 与对应功能模块处理，不在此伪造业务状态。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| 原始会话／轮次／条目标识 | 上游标识 | 用于与 SessionId、TaskId 对应 |
| 事件载荷 | 类型化内部数据 | 流式内容、轮次结束、服务端请求或安全诊断 |
| 上游请求身份 | BackendToolRequest 等内部引用 | 不外泄给宿主，不与 ToolCallId 混用 |

#### 6.2.4 `BackendToolRequest`

持有回复原始工具调用所需的内部关联信息；既保留 call_id，也保留原始服务端请求 ID。具体上游类型封装在适配模块内，不作为宿主可构造的数据。

### 6.3 索引器与运算符

### 6.4 扩展成员

### 6.5 析构／终结器

---

## 7. 使用示例与约束

### 7.1 使用示例

协议转换示意：

```text
TaskManager 的请求进入执行阶段
→ Runtime 交给 CodexRequestHandle
→ Adapter 映射 turn/start
→ 响应建立 TaskId 与 turn ID 的关联
→ 持续读取内容和工具请求事件
→ 宿主工具结果按原始请求身份回复
→ 上游真实终态交给 Runtime
```

流程尚未完成本项目集成验证。

### 7.2 使用约束

- 不直接依赖 Slint、CONFIG、LOGGER 或 APP_PARAM；宿主状态不能被适配器直接改写。
- 不安装依赖、不编译 Codex、不修改模型选择或开发者全局环境。
- 原始上游错误可能包含请求与配置内容，转换为公开错误前必须过滤敏感信息，同时保留足够的可定位上下文。
- 不额外包装 stdio／WebSocket 子进程作为当前默认方案；当前已选进程内接入。
- 不承诺动态工具和 Skills 在限制内置工具后已经正常工作；这是实现前的重要验证门槛。
- 固定版本客户端说明指出：命令及嵌入运行时通道有界，但 facade 的本地消费者事件队列无界。设置一个 channel_capacity 不能证明整个链路有界；必须持续消费，并单独确认事件压力与资源保护策略。
- 业务轮次、工具服务端请求及普通协议请求的身份分别关联；未知或迟到事件不能被归到“当前任务”。原始错误链保留在产生端，传给宿主的只能是必要且脱敏的错误信息。

**待确认事项：**

- 固定版本启动参数的完整装配、私有配置隔离和模型服务配置映射。
- 禁用内置工具后的 Skills、动态工具与服务端请求拒绝路径。
- 并行请求／事件读取／工具回复的资源与借用组织，以及退出保证。
- 历史分页归一化、ToolResult 的内容项映射和上游错误分类。
- 本软件已固定 Skill／Chat 两个根目录。需核查技能来源限制，以及会话历史、索引和附件如何实际落到 Chat；不能只修改显示路径、直接假定 Chat 等于整个 Codex Home，或在不支持时静默换目录。

### 7.3 适用范围与依赖版本

目标平台为 Linux、Windows，尚未开展跨平台验证。软件版本与运行环境兼容范围沿用项目基线，不在本文填写未经确认的最低版本或资源限额。

Codex 固定提交为 `73a1148c9c775c2a4616ce5096291740a00ed68a`；项目侧 ZeroMQ 固定提交为 `5d78967001abb1aece2fba878d6151cb66cd1767`。完整来源见[依赖基线](../../../../src/depends/README.md)，不跟随分支自动升级。

### 7.4 验证现状

本轮只核对需求、类间职责、文档引用及可见的上游接口；没有创建 Rust 实现，没有编译、执行示例或进行模型、工具、平台兼容性验证。

### 7.5 另请参阅

- [Agent 需求文档](../Agent.md)
- [模块规范模板](../../../模块规范模板.md)
- [AgentRuntime](AgentRuntime.md)
- [SessionManager](SessionManager.md)
- [SkillManager](SkillManager.md)
- [ToolManager](ToolManager.md)
- [固定版本进程内客户端说明](../../../../src/depends/codex/codex-rs/app-server-client/README.md)
- [固定版本客户端源码](../../../../src/depends/codex/codex-rs/app-server-client/src/lib.rs)
- [固定版本协议入口](../../../../src/depends/codex/codex-rs/app-server-protocol/src/protocol/common.rs)
- [OpenAI 官方 App Server 文档](https://developers.openai.com/codex/app-server/)
- [AgentTransport](AgentTransport.md)

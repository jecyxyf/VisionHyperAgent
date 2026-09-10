# ToolManager

> **状态：接口设计草案，尚未实现、编译或运行验证。** 职责依据[Agent 需求文档](../Agent.md)及已确认的功能分工与通信边界；具体签名、数据字段和状态细节是待评审的设计建议，不等同于已确认的公开接口。Python 相关内容继续暂缓。空栏目保留，不补造无关成员。
>
> 本类属于库对外接口；不依赖 Slint 窗口、控件、事件循环或 VisionHyperAgent 的全局配置／日志对象。

## 变更记录

| 版本 | 日期 | 作者 | 变更说明 |
|------|------|------|----------|
| 0.1.0 | 2026-09-10 | Codex | 初始接口设计草案，待确认后实现 |
| 0.1.1 | 2026-09-10 | Codex | 补充通信层与宿主接入文档关联，明确职责边界 |
| 0.1.2 | 2026-09-10 | Codex | 统一注册集合和宿主结果回传的职责与通信侧别 |

---

## 1. 类型定义与关系

### 1.1 定义

**说明**：管理对外声明的自定义工具及待回传调用。负责注册、校验、分发和结果关联；宿主负责工具的真实业务执行，Agent 不直接承担训练、相机或代码运行。

**类型声明**：

```rust
pub struct ToolManager { /* 私有实现 */ }
```

### 1.2 命名空间／所属模块

- **模块路径**：拟在 `vision_hyper_agent::agent` 重导出；具体子模块文件划分待实现阶段确认。
- **所属层**：通用 Agent 接入能力；保持单 Cargo 包，不按类拆分进程或独立库。
- **源码位置**：当前 `src/agent/mod.rs` 仅有职责占位，本文所述类型尚未落地，本文仅定义待评审的接口。

### 1.3 基类／继承关系

### 1.4 派生类型

### 1.5 实现接口／Trait

拟支持 Clone；所有句柄共享同一注册表和调用关联，不重复注册或重复执行工具。

### 1.6 组合与依赖

| 依赖项 | 关系类型 | 版本／固定提交 | 说明 |
| --- | --- | --- | --- |
| [Agent](Agent.md) | 功能入口与事件出口 |  | 通过 agent.tools() 访问 |
| [AgentTransport](AgentTransport.md) | 事件与结果控制通道 |  | 工具回传不排入等待当前对话结束的普通请求队列 |
| [AgentRuntime](AgentRuntime.md) | 后台驱动 |  | 在模型请求执行期间仍接收工具返回及取消控制 |
| [CodexAdapter](CodexAdapter.md) | 协议适配 |  | 映射 dynamicTools、item/tool/call 及原始服务端请求回复 |
| 宿主业务能力 | 实际执行者 |  | 复用按钮使用的同一套非 UI 业务逻辑 |

---

## 2. 构造／初始化

由 Agent 装配。当前草案先覆盖启动配置阶段注册工具，避免在已有会话中隐式改变能力集合；运行中增删工具及其生效范围尚待确认，不据此提前实现热更新。

初始化前的定义与后台使用的注册集合属于同一个工具模块，不为每个句柄复制独立注册表。如何交付不可变快照或共享受控状态是内部实现细节；不得导致宿主查询列表与模型可调用工具各自变成不同事实。

---

## 3. 字段与属性

### 3.1 字段

### 3.2 属性

---

## 4. 方法

### 4.1 关联函数／静态方法

### 4.2 实例方法

#### 4.2.1 `register(definition)`

**说明**：登记一个将向 Codex 声明的自定义工具，不在注册时执行任何业务。

**定义**：

```rust
pub fn register(&self, definition: ToolDefinition) -> Result<(), AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| definition | ToolDefinition | 工具名称、说明与输入契约 | 宿主必须具备与之对应的真实执行能力 |

**返回值**：工具定义已进入本实例的注册集合，不表示已有会话已更新能力。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `InvalidInput / Conflict` | 定义无效或工具名称冲突 | 不静默覆盖原定义 | 修正定义后显式注册 |
| `InvalidState` | 当前状态不支持变更工具集合 | 不隐式修改已有会话 | 等待注册时机规则确认 |

**备注**：
- 本草案将注册与执行回调解耦：执行请求通过事件交给宿主，再用 respond 回传结果。此接口形态需评审确认。
- 输入契约的格式按上游 JSON Schema 要求适配；名称限制和所用校验依赖不在本文凭空选择。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：

#### 4.2.2 `list()`

**说明**：返回本实例已登记的工具定义快照。

**定义**：

```rust
pub fn list(&self) -> Vec<ToolDefinition>
```

**参数**：

**返回值**：工具元数据列表，不包含业务执行对象、凭据或可变的内部注册表。

**错误**：

**备注**：
- 查询本地定义不发起模型调用；列表中的工具是否已绑定某个会话是另一项状态，不混为一谈。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：

#### 4.2.3 `respond(call_id, result)`

**说明**：将宿主实际执行得到的结果，关联并回传给原始工具调用。

**定义**：

```rust
pub async fn respond(&self, call_id: ToolCallId, result: ToolResult) -> Result<(), AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| call_id | ToolCallId | 事件中收到的调用标识 | 不能用当前会话或当前任务代替 |
| result | ToolResult | 真实成功或失败结果 | 业务失败必须保留失败含义，不能包装成成功 |

**返回值**：回传请求已通过本地检查并交付给内部回传链路；不等于另一次业务执行，也不保证模型已消费结果。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `NotFound / InvalidState` | 调用未知、已终结或不允许再次回传 | 不回传到其他请求，不重复执行业务 | 核对原调用状态 |
| `Backend / Transport / Io` | 上游、通信或受控文件访问失败 | 保留真实错误，不伪造成功 | 交由宿主处理 |

**备注**：
- 工具回传不是新的普通对话消息，不排在等待当前请求结束的用户队列后面。
- 本方法在宿主侧通过 Host 端 AgentTransport.request 提交工具结果命令，Runtime 再按原始调用身份回传上游；不要与仅供 Runtime 端使用的 AgentTransport.respond 混用。
- 失败后的重发安全性与重复提交行为需确认，不能在链路状态不明时自动重复执行业务。
- 某个工具返回失败后，Codex 仍可能继续处理当前用户请求；只有整条请求最终失败时才按 TaskManager 的失败规则处理。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：[TaskManager](TaskManager.md)、[CodexAdapter](CodexAdapter.md)

---

## 5. 事件与消息处理

### 5.1 事件与通知

| 名称 | 载荷类型 | 触发条件 | 说明 |
| --- | --- | --- | --- |
| ToolCallRequested | ToolCall | 收到可关联且已注册的工具调用 | 由宿主运行核心接收，随后调用真实业务能力 |

只有已注册且符合允许范围的调用才能作为宿主工具请求交付。未知工具和被禁止的内置执行请求必须明确拒绝或报告，不能自动批准，也不能挂起等待一个永远不会处理的回调。

### 5.2 消息处理与回调

调用链为 Codex → Adapter → Runtime／ToolManager → AgentEvent → 宿主业务能力 → ToolManager.respond → 原始 Codex 服务端请求。库中的 ToolCallId、Codex call_id 与原始请求 ID 必须有明确映射，不把不同标识互相替用。

库内事件传输和结果回传由 AgentTransport 承担。在 VisionHyperAgent 中，AgentViewModel 将工具请求交给注入的非 UI 宿主业务入口，不经 View 执行业务；ToolManager 不直接操作 Socket。

---

## 6. 其他成员

### 6.1 常量

### 6.2 枚举与嵌套类型

#### 6.2.1 `ToolDefinition`

向模型声明的工具契约，不包含任何脚本执行实现。

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| name | String | 本实例内用于定位的工具名；具体限制待映射上游要求 |
| description | String | 用途与适用条件 |
| input_schema | serde_json::Value | 结构化输入契约；校验方案需在实现前确认 |

#### 6.2.2 `ToolCallId`

库对外提供的工具调用标识，由运行侧生成或映射；不要求宿主持有上游 RPC 请求对象。

#### 6.2.3 `ToolCall`

交给宿主的执行请求，必须能精确关联到发起它的会话和用户任务。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| id / session_id / task_id | ToolCallId / SessionId / TaskId | 调用、会话及任务的关联 |
| name | String | 已注册的工具名称 |
| arguments | serde_json::Value | 已做必要结构校验的参数，宿主仍须执行真实业务校验 |

#### 6.2.4 `ToolResult`

宿主提供的实际结果；下面以 JSON 数据作为文本／结构化结果的接口草案，不预设二进制、图片或音频结果协议。

```rust
pub struct ToolResult {
    pub success: bool,
    pub output: serde_json::Value,
}
```

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| success | bool | 反映真实执行结果，不等于消息发送成功 |
| output | serde_json::Value | 结果或失败说明；敏感内容必须过滤，图片等附加数据的回传方式另行确认 |

### 6.3 索引器与运算符

### 6.4 扩展成员

### 6.5 析构／终结器

---

## 7. 使用示例与约束

### 7.1 使用示例

```rust
use vision_hyper_agent::agent::{AgentError, ToolCallId, ToolManager, ToolResult};

// 仅在宿主已经确认工具执行成功后使用；本函数不执行业务。
async fn return_success(tools: &ToolManager, call_id: ToolCallId,
    output: serde_json::Value) -> Result<(), AgentError>
{
    tools.respond(call_id, ToolResult { success: true, output }).await
}
```

仅为接口使用示意，未编译或执行；调用方需提供已声明的输入和异步运行上下文，不依赖 Slint 事件循环。

### 7.2 使用约束

- 库注册工具入口不等于 Agent 自己执行业务；宿主调用与按钮共享的真实业务能力。
- 不提供通用终端、任意文件访问或 Python 执行接口；Python 相关固定工具继续暂缓。
- 取消 Agent 请求不自动撤销宿主副作用，也不等于工具已经停止。宿主工具是否可取消及其通知契约仍需明确。
- 当前不提供 unregister、replace 或动态改写已存在会话工具集的接口。
- 元数据、参数及结果不得包含无关文件、明文 KEY 或不属于当前任务授权的数据。

**待确认事项：**

- 工具注册允许的时机、已有会话的工具定义与宿主能力不一致时的策略。
- 宿主事件回传式执行契约，以及是否还需要其他注册形式。
- 输入 Schema 校验实现、工具取消、超时、结果重发与重复提交语义。
- 会话恢复后持久化的工具定义与本次宿主处理能力是否一致；不得静默宣称历史工具仍可执行或擅自更换其含义。
- 禁用内置执行工具后的完整动态工具调用链路；仅有协议类型并不代表安全限制已生效。

### 7.3 适用范围与依赖版本

目标平台为 Linux、Windows，尚未开展跨平台验证。软件版本与运行环境兼容范围沿用项目基线，不在本文填写未经确认的最低版本或资源限额。

Codex 固定提交为 `73a1148c9c775c2a4616ce5096291740a00ed68a`；项目侧 ZeroMQ 固定提交为 `5d78967001abb1aece2fba878d6151cb66cd1767`。完整来源见[依赖基线](../../../../src/depends/README.md)，不跟随分支自动升级。

### 7.4 验证现状

本轮只核对需求、类间职责、文档引用及可见的上游接口；没有创建 Rust 实现，没有编译、执行示例或进行模型、工具、平台兼容性验证。

### 7.5 另请参阅

- [Agent 需求文档](../Agent.md)
- [模块规范模板](../../../模块规范模板.md)
- [Agent](Agent.md)
- [TaskManager](TaskManager.md)
- [CodexAdapter](CodexAdapter.md)
- [固定版本工具调用协议](../../../../src/depends/codex/codex-rs/app-server-protocol/src/protocol/v2/item.rs)
- [固定版本会话工具声明](../../../../src/depends/codex/codex-rs/app-server-protocol/src/protocol/v2/thread.rs)
- [AgentTransport](AgentTransport.md)
- [宿主 AgentViewModel](../../viewmodel/AgentViewModel.md)

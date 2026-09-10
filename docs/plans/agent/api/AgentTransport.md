# AgentTransport

> **状态：接口设计草案，尚未实现、编译或运行验证。** 本文补充 Agent 通信及宿主接入链路；具体签名、消息字段、并发与关闭策略需评审后实现。Python 相关工作继续暂缓。没有内容的栏目保留空白。
>
> 本类处理 Agent 的命令、响应和事件协议；不直接操作 Slint，不拥有宿主业务状态，不替代驱动层的 ZeroMQ 封装。

## 变更记录

| 版本 | 日期 | 作者 | 变更说明 |
|------|------|------|----------|
| 0.1.0 | 2026-09-10 | Codex | 初始通信与接入接口草案，待确认后实现 |
| 0.1.1 | 2026-09-10 | Codex | 明确宿主 Client 与后台 Server 两端对象及共用底层 |
| 0.1.2 | 2026-09-10 | Codex | 区分 Host／Runtime 实例和操作权限，校准响应与关闭契约 |

---

## 1. 类型定义与关系

### 1.1 定义

**说明**：连接 Agent 的对外功能句柄与后台 Runtime，负责请求关联、响应投递、事件传输及通信生命周期。宿主侧通过 ZmqClient，后台侧通过 ZmqServer；两类复用驱动模块公共实现，业务请求排队仍属于 TaskManager。

本草案将同一类按角色装配为 Host 与 Runtime 两端实例。两端使用相同的协议及运行实例标识，各自只持有本端驱动的控制句柄；共享 Context，不共享 Socket，也不在两个线程同时使用同一个可变端点对象。对外 Manager 只访问 Host 端，后台 Runtime 只访问 Runtime 端。

**类型声明**：

```rust
pub(crate) struct AgentTransport { /* 私有通信上下文与端点控制句柄 */ }
```

### 1.2 命名空间／所属模块

- **模块路径**：拟在 `vision_hyper_agent::agent` 内部使用，不对宿主暴露 ZMQ 端点。
- **所属层**：通用 Agent 库的通信适配。
- **源码位置**：规划归属 `src/agent/`；当前没有该类型的实现，本轮不创建 Rust 文件。

### 1.3 基类／继承关系

### 1.4 派生类型

### 1.5 实现接口／Trait

### 1.6 组合与依赖

| 依赖项 | 关系类型 | 版本／固定提交 | 说明 |
| --- | --- | --- | --- |
| [ZmqClient](../../drivers/ZmqClient.md) | 宿主端驱动接口 | 5d78967001abb1aece2fba878d6151cb66cd1767 | 发送宿主数据并接收返回数据；本模块解释其业务协议 |
| [ZmqServer](../../drivers/ZmqServer.md) | 后台端驱动接口 | 同一固定版本 | 接收宿主数据并发送返回数据；不直接执行 Agent 业务 |
| [Agent](Agent.md) | 拥有生命周期与公开事件出口 |  | 对外保持 Manager 方法和 AgentEventReceiver，不暴露传输协议 |
| 各功能 Manager | 命令提交方 |  | 提交各自的类型化操作；不会各建一套 Socket 管理逻辑 |
| [AgentRuntime](AgentRuntime.md) | 运行端 |  | 接收命令并驱动业务规则，返回响应及事件 |

---

## 2. 构造／初始化

### 2.1 `new(config)`

**说明**：构造指定角色的单端逻辑通信上下文，不立即绑定或连接 Socket。

**定义**：

```rust
pub(crate) fn new(config: AgentTransportConfig) -> Self
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| config | AgentTransportConfig | 本实例通信参数 | 端点命名、策略和容量不得从全局环境隐式取得 |

**返回值**：尚未启动的通信上下文。

**错误**：

**备注**：

- Agent 构造仍保持不执行 I/O；实际端点初始化在后台启动阶段进行。
- 同一 Agent 实例的各 Manager 取得 Host 端访问句柄；另一个 Runtime 端实例交给后台运行核心，不由每个 Manager 重建。
- config.side 决定端点角色，两端的 instance_key 必须相同；不在运行中切换角色。

**示例**：

**适用范围**：Agent 库内部；宿主继续使用 Agent 与各 Manager 的类型化接口。

**另请参阅**：

### 2.2 `start(context)`

**说明**：使用共享 Context 启动当前角色的通信端点；两端分别调用，不由本方法越过线程归属同时操作对端 Socket。

**定义**：

```rust
pub(crate) async fn start(&self, context: ZmqContext) -> Result<(), AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| context | ZmqContext | 由驱动层提供的共享上下文句柄 | 同一 inproc 链路的两端必须关联同一个底层 Context |

**返回值**：本端必要端点已建立的结果。Agent 的整体就绪须同时考虑对端和 Codex 初始化，不由本端自行宣称。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `Transport / InvalidInput` | 端点建立、选项配置或通信参数失败 | 不报告通信就绪 | 清理部分初始化资源并返回安全原因 |

**备注**：

- Socket 在各自的 I/O 所属线程创建、使用和关闭；公开异步调用不得直接在任意调用线程操作 Socket。
- 在宿主端所属线程创建 ZmqClient，在后台端所属线程创建 ZmqServer，并传入同一底层 Context 的共享句柄。两端分别持有对象和 Socket，不共同操作一个 Socket。
- 具体采用何种 Socket 模式、多少端点和 I/O 调度方式尚待确认，不把逻辑通道数当成已选 Socket 数量。

**示例**：

**适用范围**：Agent 库内部；宿主继续使用 Agent 与各 Manager 的类型化接口。

**另请参阅**：

---

## 3. 字段与属性

### 3.1 字段

| 字段名 | 类型 | 访问级别 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| config | AgentTransportConfig | private |  | 本实例的通信配置与标识 |
| pending_calls | 调用关联表 | private |  | Host 端维护 CommandId 与本地等待者；不是用户对话队列 |
| endpoint_handle | 当前角色端点的控制句柄 | private |  | Host 对应 ZmqClient，Runtime 对应 ZmqServer；一个实例不同时持有两端 Socket |

### 3.2 属性

---

## 4. 方法

### 4.1 关联函数／静态方法

### 4.2 实例方法

| 操作 | 允许角色 | 说明 |
| --- | --- | --- |
| request | Host | 提交命令并等待相应响应 |
| next_request / respond / publish | Runtime | 接收命令、回应调用、发送后台业务事件 |
| start / close | 两端各自调用 | 只操作本端资源；整体顺序由 Agent 生命周期协调 |

角色不匹配属于 InvalidState，必须在触及端点前拒绝。内部访问句柄及异步借用的具体实现仍待确认，不能把这些签名解释成可跨线程共享原始 Socket。

#### 4.2.1 `request(command)`

**说明**：发送一项类型化命令，并等待与该次调用对应的响应。

**定义**：

```rust
pub(crate) async fn request(&self, command: AgentCommand)
    -> Result<AgentCommandResult, AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| command | AgentCommand | 会话、任务、技能或工具结果等操作 | 只传受控的数据载荷，不序列化闭包、指针或 UI 对象 |

**返回值**：该命令的结果；submit 的结果只是 TaskId，不等待整条模型回复结束。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `NotInitialized / InvalidState` | 通信未就绪或已进入不允许新命令的阶段 | 命令不被执行 | 由调用方处理 |
| `Transport` | 收发、解码或等待响应失败 | 发送后失败可能意味着结果未知，不保证未执行 | 报告状态，不自动重放有副作用的命令 |

**备注**：

- CommandId 由通信层关联一次接口调用，与 TaskId、SessionId、ToolCallId 分开。
- 取消、关闭及工具结果属于控制操作，不能等待普通对话队列腾空。
- 传输超时不等于任务取消，是否撤销业务请求必须通过对应任务接口明确处理。

**示例**：

**适用范围**：Agent 库内部；宿主继续使用 Agent 与各 Manager 的类型化接口。

**另请参阅**：

#### 4.2.2 `next_request()`

**说明**：供运行端取得下一项已校验的命令。

**定义**：

```rust
pub(crate) async fn next_request(&mut self)
    -> Result<Option<ReceivedAgentCommand>, AgentError>
```

**参数**：

**返回值**：Some 为命令及响应关联信息；None 仅表示约定的通道正常结束。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `Transport` | 帧结构、协议字段或关联信息非法 | 不执行未知操作 | 返回错误或安全诊断，不能伪装为通道正常结束 |
| `InvalidState` | 在非 Runtime 端调用 | 不消费另一端的帧 | 使用正确角色的实例 |

**备注**：

- 待处理通信命令与待执行的用户请求是不同的队列。
- Runtime 不能在等待模型完整回复期间停止接收控制命令。

**示例**：

**适用范围**：Agent 库内部；宿主继续使用 Agent 与各 Manager 的类型化接口。

**另请参阅**：

#### 4.2.3 `respond(command_id, result)`

**说明**：回传指定命令的成功结果或明确错误。

**定义**：

```rust
pub(crate) async fn respond(&self, command_id: CommandId,
    result: Result<AgentCommandResult, AgentError>) -> Result<(), AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| command_id | CommandId | 收到命令时的调用标识 | 不能用当前任务或当前会话替代 |
| result | `Result<AgentCommandResult, AgentError>` | 真实处理结果 | 只编码可传输且脱敏后的字段，不传原始错误对象或指针 |

**返回值**：响应进入发送路径的结果；不保证对端已经消费。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `InvalidState / Transport` | 响应关联已失效或回传失败 | 不改投给其他调用 | 记录安全原因并结束对应等待关系 |

**备注**：

- 重复响应、迟到响应和关闭期间响应的处理需明确，不能静默变成另一项命令的结果。
- 此方法只用于 Runtime 端回复通信调用。宿主的 ToolManager.respond 是另一层业务接口，其数据通过 Host 端 request 作为工具结果命令提交，不能直接调用本方法。

**示例**：

**适用范围**：Agent 库内部；宿主继续使用 Agent 与各 Manager 的类型化接口。

**另请参阅**：

#### 4.2.4 `publish(event)`

**说明**：由 Runtime 端将后台业务事件交给 Agent 的统一事件出口，不由传输层判断整体生命周期。

**定义**：

```rust
pub(crate) async fn publish(&self, event: AgentEvent) -> Result<(), AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| event | AgentEvent | 状态、流式内容、终态或工具调用通知 | 保留必要的会话、任务与调用关联 |

**返回值**：事件进入投递路径的结果，不等于 UI 已显示。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `Transport` | 事件通道不可用或发送失败 | 不能伪造任务成功或悄悄丢弃关键终态 | 报告并由 Runtime 判断影响范围 |

**备注**：

- 不默认使用可能丢失首条或关键消息的发布订阅方案；具体传输模式和背压规则需确认。
- UI 的合并刷新发生在 View 绑定层，不在这里丢弃业务事件。
- 后台业务事件与整体 AgentState 分开；Runtime 的阶段结果交给 Agent 生命周期协调逻辑，整体 StateChanged 由该逻辑生成并汇入统一出口。

**示例**：

**适用范围**：Agent 库内部；宿主继续使用 Agent 与各 Manager 的类型化接口。

**另请参阅**：

#### 4.2.5 `close()`

**说明**：按整体关闭顺序结束本端资源和等待关系，不直接关闭另一端。

**定义**：

```rust
pub(crate) async fn close(&self) -> Result<(), AgentError>
```

**参数**：

**返回值**：本端实际通信收尾结果，不代表对端或整个 Agent 已经结束。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `Transport` | 端点关闭、等待者收尾或上下文释放失败 | 不宣称全部资源已正常释放 | 向 Agent 生命周期控制返回真实原因 |

**备注**：

- 先完成必要的 Runtime／Codex 收尾，再结束仍需回传结果和最终事件的通道。
- 不得销毁仍被运行端使用的共享 Context。
- Linger、截止时间、剩余消息和未完成调用如何处置尚待确认，不填默认超时或默认丢弃策略。
- 整体 shutdown 由 Agent 协调两端和 Runtime／Codex 结果；本端失败不能被另一个端点的成功覆盖。

**示例**：

**适用范围**：Agent 库内部；宿主继续使用 Agent 与各 Manager 的类型化接口。

**另请参阅**：

---

## 5. 事件与消息处理

### 5.1 事件与通知

| 名称 | 载荷类型 | 触发条件 | 说明 |
| --- | --- | --- | --- |
| AgentEvent | 既有公开事件类型 | 后台产生状态或内容变化 | 由 AgentEventReceiver 交付宿主，不直接通知 UI |
| 通信失败 | AgentError | 端点、帧、关联或通道发生错误 | 区分命令调用失败与运行实例不可继续工作 |

### 5.2 消息处理与回调

调用方向：Manager → AgentTransport 的宿主侧协议处理 → ZmqClient → inproc → ZmqServer → AgentTransport 的后台侧协议处理 → Runtime。返回方向分别是命令响应与 AgentEvent；两者不能混用。UI／ViewModel 不参与序列化、Socket 选择或原始帧收发。

Client 与 Server 表示两端角色，不表示独立进程，也不表示已选用同名的原生 Socket 类型。两端的命令、响应和事件仍使用同一份协议定义，不各写一套编解码规则。

命令响应由 Host 端内部的响应分派完成，后台事件进入同一运行实例的公开事件出口；它们不能竞争读取同一原始 Socket。具体调度和背压方案待确认，但不能让 request 等待期间停止处理对应响应、取消控制或工具返回。

---

## 6. 其他成员

### 6.1 常量

### 6.2 枚举与嵌套类型

#### 6.2.1 `AgentTransportConfig`

通信配置的数据类型归本模块管理。资源参数及其配置入口尚未最终确认，不给未确认项设置默认值。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| instance_key | 实例标识 | 由同一 Agent 的装配逻辑生成并提供给两端，不允许每端各生成一个互不关联的值 |
| side | TransportSide | Host 或 Runtime；构造后不动态切换 |
| 通信策略 | 尚待细化 | Socket 组合、容量、帧限制、等待期限与关闭策略；不表示已选择某个固定值 |

#### 6.2.2 `CommandId`

一次通信调用的唯一关联标识，不是会话 ID、用户请求 TaskId 或工具调用 ID。作用于本运行实例及对应的通道生命周期。

#### 6.2.3 `ReceivedAgentCommand / AgentCommandResult`

AgentCommand 和业务操作结果仍按 Runtime 与所属功能模块定义；本模块增加通信关联，不复制业务规则。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| ReceivedAgentCommand.command_id | CommandId | 用于回传本次接口调用的结果 |
| ReceivedAgentCommand.command | AgentCommand | 受控操作数据；本地等待者不写入通信帧 |
| AgentCommandResult | 对应操作的成功结果 | 由 Runtime 及功能模块维护，定义说明见 AgentRuntime |

#### 6.2.4 `通信封装`

以下为逻辑消息结构，不是已经确定的线协议。编码、版本及兼容性在实现前确认。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| Command | CommandId + 操作载荷 | 请求对端执行一项已知操作 |
| Reply | CommandId + 成功结果或安全错误 | 完成对应的本地等待关系 |
| Event | AgentEvent | 独立的后台变化；不冒充某个调用的返回值 |

这里的 Result 和 AgentError 是进程内接口表达，不表示直接序列化任意 Rust 错误对象。线协议只能传明确的数据字段和安全错误信息；原始错误链、等待通道、future 和指针留在本地。完整字段、大小限制及版本仍待确认。

#### 6.2.5 `TransportSide`

驱动角色对应的内部通信侧别，不是新的管理器或进程类型。

| 取值 | 驱动角色 | 使用方 |
| --- | --- | --- |
| Host | ZmqClient | Agent 对外入口及各 Manager 的访问句柄 |
| Runtime | ZmqServer | 后台 AgentRuntime |

### 6.3 索引器与运算符

### 6.4 扩展成员

### 6.5 析构／终结器

析构仅作为兜底，不替代显式 close。固定 ZMQ 依赖的 Socket 析构与 Context 终止存在特殊限制，详见 [ZmqClient 的共用约束](../../drivers/ZmqClient.md)；不能简单 drop 后就报告安全关闭。

---

## 7. 使用示例与约束

### 7.1 使用示例

```text
AgentViewModel 调用 agent.tasks().submit(session_id, message)
→ TaskManager 经 Transport 发出带 CommandId 的命令
→ 宿主端 ZmqClient 将数据交给同一 Context 下的后台端 ZmqServer
→ Runtime 交给 TaskManager 的后台状态受理并返回 TaskId
→ 响应按 CommandId 回到原调用
→ 后续 MessageDelta / TaskFinished 独立进入 AgentEventReceiver
```

这是接口关系示意，尚未实现或验证。

### 7.2 使用约束

- 本模块属于 Agent 库内部，不作为对外网络服务，不处理产品对外图片识别协议。
- 同一进程内链路共享底层 Context；不能每个端点创建独立 Context 后仅靠同名 inproc 地址连接。
- ZmqClient／ZmqServer 只提供两端通信角色，并复用驱动模块公共实现；本模块负责 Agent 消息格式、关联和传输错误转换，不直接调用驱动内部实现。
- 异步方法不能通过直接执行无限等待的同步 recv 来冒充非阻塞；I/O 执行位置须在实现中明确。
- 本模块没有第二个模型请求队列，不自动重试宿主工具或重放任务。

**待确认事项：**

- Socket 模式和端点布局、I/O 调度方式、协议编码与版本。
- 命令响应等待、事件背压、大小与数量限制、重复及迟到消息策略。
- 初始化失败和关闭阶段的端点回收、Linger 与最后一个 Context 引用的释放顺序。

### 7.3 适用范围与依赖版本

目标平台为 Linux、Windows，当前仅约定 Agent 链路使用 inproc。Rust ZeroMQ 固定为 `5d78967001abb1aece2fba878d6151cb66cd1767`；不调整 Cargo 依赖或上游版本。

### 7.4 验证现状

本轮仅阅读固定版本源码及官方 inproc 说明，完成接口文档整理；没有实现通信协议、运行收发测试或验证 Windows/Linux 行为。

### 7.5 另请参阅

- [Agent](Agent.md)
- [AgentRuntime](AgentRuntime.md)
- [AgentViewModel](../../viewmodel/AgentViewModel.md)
- [模块规范模板](../../../模块规范模板.md)
- [固定依赖基线](../../../../src/depends/README.md)
- [官方 inproc 说明](https://libzmq.readthedocs.io/en/latest/zmq_inproc.html)
- [ZmqClient](../../drivers/ZmqClient.md)
- [ZmqServer](../../drivers/ZmqServer.md)

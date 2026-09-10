# ZmqClient

> **状态：接口设计草案，尚未实现、编译或运行验证。** 本文补充 Agent 通信及宿主接入链路；具体签名、消息字段、并发与关闭策略需评审后实现。Python 相关工作继续暂缓。没有内容的栏目保留空白。
>
> 本类是宿主／ViewModel的通信角色对象。Client／Server 是使用端角色，不表示已选定 ZeroMQ 的原生 CLIENT／SERVER、REQ／REP 等 Socket 模式，也不产生独立后端进程。

## 变更记录

| 版本 | 日期 | 作者 | 变更说明 |
|------|------|------|----------|
| 0.1.0 | 2026-09-10 | Codex | 初始通信与接入接口草案，待确认后实现 |
| 0.1.1 | 2026-09-10 | Codex | 合并共用契约，取消独立底层类文档，保持两端接口 |
| 0.1.2 | 2026-09-10 | Codex | 明确应用线程归属及端点状态不代表整体 Agent 状态 |

---

## 1. 类型定义与关系

### 1.1 定义

**说明**：发送宿主侧数据并接收后台返回的数据。ZmqClient 与 ZmqServer 是两端独立对象，分别持有端点资源；收发、参数检查、错误转换和关闭逻辑通过驱动模块的公共内部实现复用，不各写一套，也不要求另设第三个驱动类。

**类型声明**：

```rust
pub struct ZmqClient { /* 私有的共用驱动端点与资源引用 */ }
```

### 1.2 命名空间／所属模块

- **模块路径**：拟在 `vision_hyper_agent::drivers` 导出 `ZmqClient` 及必要的共用数据／资源类型；底层公共实现不作为额外的对外驱动入口。
- **所属层**：驱动／适配层的角色接口。
- **源码位置**：规划归属 `src/drivers/`，当前仍只有职责占位；本轮只编写 API 文档，不创建实现文件。

### 1.3 基类／继承关系

### 1.4 派生类型

### 1.5 实现接口／Trait

本草案不提供角色端点的 Clone。底层 Context 可以通过 ZmqContext 克隆共享，但不能因此并发共享同一个 Socket。

### 1.6 组合与依赖

| 依赖项 | 关系类型 | 版本／固定提交 | 说明 |
| --- | --- | --- | --- |
| 驱动模块公共内部实现 | 私有复用 | 5d78967001abb1aece2fba878d6151cb66cd1767 | 统一处理端点收发、校验与释放，不要求独立成类 |
| ZmqContext | 共享资源 |  | 与通信另一端引用同一个底层 Context；不能分别 new 独立 Context 来组成 inproc 链路 |
| [ZmqServer](ZmqServer.md) | 通信对端 |  | 只通过通信数据交互，不相互持有对端对象 |
| [AgentTransport](../agent/api/AgentTransport.md) | 上层使用方 |  | 由其定义命令、响应和事件的含义；本类不依赖 Agent 类型 |

---

## 2. 构造／初始化

### 2.1 `connect(context, endpoint, kind, options)`

**说明**：在所属线程中使用共享 Context 创建端点并连接地址，成功后交付本角色对象。

**定义**：

```rust
pub fn connect(context: ZmqContext, endpoint: &str,
    kind: ZmqSocketKind, options: ZmqSocketOptions) -> Result<Self, ZmqError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| context | ZmqContext | 共享上下文句柄 | inproc 对端必须来自同一个底层 Context |
| endpoint | &str | 连接地址 | 地址及 NUL 检查复用共用驱动，不绕过验证 |
| kind | ZmqSocketKind | 实际 Socket 模式 | 由上层方案确认，不根据类名自动选定 |
| options | ZmqSocketOptions | 端点配置 | 不隐含填写超时、容量或 Linger 默认值 |

**返回值**：已完成端点连接操作的 ZmqClient；不意味着对端已处理请求，也不意味着 Agent 已就绪。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `InvalidInput / Backend` | 参数无效、端点创建或连接失败 | 不交付假就绪对象；部分资源按共用关闭策略处理 | 返回真实原因，不重试业务操作 |

**备注**：

- 在宿主／ViewModel对应的 I/O 所有者上下文中创建和使用，不在 UI 回调中等待网络或 Socket。
- 本草案用单个地址表达一对端点的角色；最终 Socket 数量与控制／事件通道组合仍由 AgentTransport 方案决定，不承诺全库只能存在两个 Socket。
- 当前不增加自动重连或运行中改换 Context 的行为。
- 原生资源关闭的错误保证尚未实现，不能在初始化失败清理时吞掉底层问题。

**示例**：

**适用范围**：驱动层的宿主／ViewModel角色端点；由 AgentTransport 使用，不包含 Agent 或 UI 业务协议。

**另请参阅**：

---

## 3. 字段与属性

### 3.1 字段

| 字段名 | 类型 | 访问级别 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| endpoint | 驱动模块内部端点资源 | private |  | 不暴露给 ViewModel 或 AgentRuntime；具体实现形式待落地时确认 |
| context | 共享 Context 的保活关系 | private |  | 由共用底层维护生命周期，不另造一个独立 Context |
| endpoint | 端点信息 | private |  | 记录本对象的连接目标；具体存储形式不在本轮锁定 |

### 3.2 属性

---

## 4. 方法

### 4.1 关联函数／静态方法

### 4.2 实例方法

#### 4.2.1 `send(message, wait)`

**说明**：发送宿主侧原始数据。它可以承载上层命令，但本类不解释命令内容。

**定义**：

```rust
pub fn send(&mut self, message: &ZmqMessage,
    wait: ZmqWait) -> Result<(), ZmqError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| message | &ZmqMessage | 原始多帧消息 | 保留帧边界，复用共用驱动的消息校验 |
| wait | ZmqWait | 本次等待策略 | 不得通过无限等待阻塞 UI 或控制处理 |

**返回值**：共用驱动的发送结果，不保证对端已经接收或执行业务。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `InvalidInput / WouldBlock / TimedOut / Backend` | 输入、等待或底层发送失败 | 不伪造已送达，不自动重复发送业务消息 | 返回错误，由上层协议决定后续处理 |

**备注**：

- 不在本类生成 CommandId、解析 TaskId 或维护等待模型结束的队列。
- 发送、容量限制与部分失败处理只在驱动模块公共实现中写一次。

**示例**：

**适用范围**：驱动层的宿主／ViewModel角色端点；由 AgentTransport 使用，不包含 Agent 或 UI 业务协议。

**另请参阅**：

#### 4.2.2 `receive(wait)`

**说明**：接收后台端返回的数据，交给 AgentTransport 区分响应和异步事件。

**定义**：

```rust
pub fn receive(&mut self, wait: ZmqWait) -> Result<ZmqMessage, ZmqError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| wait | ZmqWait | 等待策略 | 由上层已确认的调度规则提供 |

**返回值**：一条完整原始多帧消息；超时或通道异常不使用空消息代替。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `WouldBlock / TimedOut / LimitExceeded / Backend` | 暂不可读、等待超时、超限或接收失败 | 不返回伪造或截断的成功消息 | 使用共用错误契约处理 |

**备注**：

- 空帧与通道结束不能混用。
- 不将接收结果直接交给 Slint；驱动不知道 View、ViewModel 或 AgentEvent 类型。

**示例**：

**适用范围**：驱动层的宿主／ViewModel角色端点；由 AgentTransport 使用，不包含 Agent 或 UI 业务协议。

**另请参阅**：

#### 4.2.3 `poll(interest, wait)`

**说明**：复用共用驱动等待或检查可读／可写状态。

**定义**：

```rust
pub fn poll(&mut self, interest: ZmqInterest,
    wait: ZmqWait) -> Result<ZmqReadiness, ZmqError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| interest | ZmqInterest | 关注的就绪条件 | 不代表业务完成状态 |
| wait | ZmqWait | 等待方式 | 范围检查与时间单位转换复用共用底层 |

**返回值**：ZmqReadiness；没有就绪事件与底层调用错误分开处理。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `InvalidInput / Backend` | 等待条件不合法或 poll 失败 | 不伪造可读／可写 | 向上层返回原因 |

**备注**：

- 本类提供同步的低层端点接口；AgentTransport 负责把它放入不会阻塞 UI 的 I/O 调度。
- 不为 Client 和 Server 分别实现不同的轮询错误规则。

**示例**：

**适用范围**：驱动层的宿主／ViewModel角色端点；由 AgentTransport 使用，不包含 Agent 或 UI 业务协议。

**另请参阅**：

#### 4.2.4 `close()`

**说明**：通过共用底层显式关闭本端点，不关闭通信另一端仍使用的 Context。

**定义**：

```rust
pub fn close(&mut self) -> Result<(), ZmqError>
```

**参数**：

**返回值**：目标契约为真实关闭结果；当前尚未实现或验证该保证。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `Backend / InvalidState` | 共用底层关闭失败或端点状态不明确 | 不返回假成功，也不强制销毁对端 Context | 由上层完成错误报告与收尾协调 |

**备注**：

- Linger、剩余消息及原生 Drop 可能 panic 的限制统一见 [ZmqClient 的共用约束](ZmqClient.md)；本类不能用一层包装掩盖这些限制。
- 两端各自关闭 Socket，再结束最终共享 Context 所有权。
- 重复关闭、失败后是否可再次关闭及超时策略尚待确认。

**示例**：

**适用范围**：驱动层的宿主／ViewModel角色端点；由 AgentTransport 使用，不包含 Agent 或 UI 业务协议。

**另请参阅**：

---

## 5. 事件与消息处理

### 5.1 事件与通知

### 5.2 消息处理与回调

ViewModel 通过 Agent 的公开接口使用宿主端：ViewModel → Agent／Manager → AgentTransport → ZmqClient。接收方向反向返回，由 AgentTransport 解析并交付响应或 AgentEventReceiver。ViewModel 不另建第二个客户端，不直接拼帧或使用原生 Socket。

---

## 6. 其他成员

### 6.1 常量

### 6.2 枚举与嵌套类型

本节集中记录两个角色共用的驱动模块类型。这里只是文档归集，不表示它们嵌套在 ZmqClient 内，也不表示 ZmqServer 需要持有或调用客户端对象。具体共用实现仍属于驱动模块，不复制两套类型或资源管理逻辑。

#### 6.2.1 `ZmqContext`

本模块管理的可克隆上下文资源句柄。克隆应共享同一个原生 Context，而不是新建一个 Context。

```rust
pub struct ZmqContext { /* 共享底层上下文 */ }

impl ZmqContext {
    pub fn new() -> Self;
}

// 拟支持 Clone；创建与销毁仍须遵守本模块生命周期规则。
```

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| 所有权 | 共享资源 | 不能因某个端点关闭而终止其他端点使用的 Context |
| 释放 | 生命周期协调 | 所有相关端点完成收尾后才结束最终上下文所有权；不向任意克隆暴露强制销毁入口 |

#### 6.2.2 `ZmqMessage`

原始多帧消息的数据类型；不在驱动层序列化业务对象。

```rust
pub type ZmqMessage = Vec<Vec<u8>>;
```

#### 6.2.3 `ZmqSocketKind / ZmqSocketOptions`

通信模式与 Socket 选项的数据类型归驱动模块。实际暴露的模式集合、容量限制和关闭参数仍需确认，不默认承诺封装所有原生功能。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| Socket 模式 | 由上层明确选择 | AgentTransport 的模式尚未确定，不由驱动擅自选择 |
| 收发与容量 | 待确认的受控配置 | 不能把原生默认值自动当作项目已确认策略 |
| Linger 与关闭 | 待确认的受控配置 | 区分等待发送、丢弃未发送数据及可能阻塞的后果 |

#### 6.2.4 `ZmqWait / ZmqInterest / ZmqReadiness`

等待和就绪类型的候选表达，不预置具体时间数值。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| ZmqWait | NonBlocking / Timeout(Duration) / Blocking | 底层语义选择；上层是否允许无限等待另行约束 |
| ZmqInterest | Readable / Writable / ReadWrite | 关注的原生就绪条件 |
| ZmqReadiness | 可读、可写标志 | 不代表完整业务响应已经到达 |

#### 6.2.5 `ZmqError`

归属驱动层，不依赖 AgentError、ViewModelError 或 Slint。上层在各自边界转换错误，保留操作和安全上下文，不记录原始消息内容。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| InvalidInput / InvalidState | 输入与状态错误 | 地址、模式、选项、等待范围或端点状态不合法 |
| WouldBlock / TimedOut | 等待结果 | 明确区分非阻塞暂不可用和有限等待超时 |
| LimitExceeded | 资源限制错误 | 超出已确认的消息限制 |
| Backend | 原生错误 | 保留可定位的错误码和原因，不把未知错误变成成功 |

### 6.3 索引器与运算符

### 6.4 扩展成员

### 6.5 析构／终结器

只复用共用底层的释放规则，不另写一套原生关闭或 Context 销毁流程。显式 close 的实现保证仍需核查，不能把析构完成当作消息已送达或对端已停止。

---

## 7. 使用示例与约束

### 7.1 使用示例

```text
同一个 ZmqContext
├─ 宿主端所属线程：ZmqClient::connect(context.clone(), ...)
└─ 后台端所属线程：ZmqServer::bind(context.clone(), ...)

两端分别持有端点对象，互不共享 Socket。
AgentTransport 解释消息，ZmqClient / ZmqServer 只负责各自端的通信。
```

仅为角色关系示意，不在此确定启动顺序、Socket 模式或通道数量。

### 7.2 使用约束

- 对外驱动角色只保留 Client 与 Server；共用能力留在驱动模块内部，不再要求独立的第三个驱动类或文档。
- 采用组合复用共用底层，不虚构继承关系；同一能力只实现一次。
- 两端都可在所选 Socket 模式允许的范围内收发，不能把角色名误认为只能单向通信。
- 同一 Context 与独立 Socket 同时成立；不同 Context 不能仅靠同名 inproc 地址互通。
- 本文的 I/O 所属线程指应用中实际持有和操作端点的执行上下文，不是把 ZeroMQ 内部线程数固定为某个值；不能因提供了同步接口就在 UI 线程进行阻塞等待。
- 不通过这些类向 UI 或 Agent 暴露通用终端、Python 执行或业务权限。
- 具体是否需要多端点组合留给上层通信方案，不提前新增多通道管理框架。
- 通信就绪与业务就绪分开，本类不设置 AgentState，也不把 connect 或 poll 成功解释为整个 Agent 初始化成功。

**两端共用的收发与关闭约束：**

- 绑定、连接、解绑和断连等端点地址须先校验 NUL 等非法输入。固定绑定在端点字符串转换处使用 unwrap，不能让可预期输入错误触发该路径。
- 零帧消息应作为非法发送输入处理；固定版本 send_multipart 对空迭代器直接返回 Ok，不能据此声称已经发出消息。包含一个空帧与零帧消息不同。
- 接收时保留多帧边界。暂不可读、超时和链路错误不能用空消息替代；有效空帧也不能当作通道结束。
- 单帧大小、帧数和累计大小的保护应在接收过程中进行，不把全部分配后再检查称为已经完成资源防护。具体限制仍待确认。
- poll 的就绪与实际业务完成分开；等待参数转换须检查范围。同步驱动调用不能直接阻塞 UI 或占住需要处理控制消息的执行器。
- 发送失败不表示对端一定未收到，不能自动重放可能有副作用的业务消息；部分发送和超限后的端点恢复行为仍需明确。
- close 是目标接口而非已验证保证：固定版本 Socket::Drop 在原生关闭失败时会 panic，简单 drop 后返回 Ok 不能兑现错误回传承诺。
- 显式关闭的实现路径、必要的底层操作和所有权处理须先评审，不得因此擅自更换固定依赖或吞掉错误。
- 两端各自完成 Socket 收尾后才能释放最终共享 Context 所有权；不能从任意 Context 克隆强制 destroy 仍有端点在使用的上下文。Linger、剩余消息、超时及失败恢复规则仍待确认。

**待确认事项：**

- Socket 模式、端点组合和应用线程内的 I/O 调度。
- 超时、容量、Linger 与关闭失败恢复规则。
- 共用底层显式关闭的实现路径与错误保证。

### 7.3 适用范围与依赖版本

Rust ZeroMQ 固定提交为 `5d78967001abb1aece2fba878d6151cb66cd1767`，包版本字段为 `0.10.0`；不更新依赖。目标为 Linux、Windows，当前 Agent 链路使用 inproc，尚未进行平台或通信验证。

### 7.4 验证现状

本轮仅完成角色接口和文档关联整理，没有创建 Rust 类、修改 Cargo、编译驱动或运行收发／关闭测试。

### 7.5 另请参阅

- [ZmqServer](ZmqServer.md)
- [AgentTransport](../agent/api/AgentTransport.md)
- [宿主 AgentViewModel](../viewmodel/AgentViewModel.md)
- [模块规范模板](../../模块规范模板.md)
- [固定依赖基线](../../../src/depends/README.md)
- [固定 rust-zmq 源码](../../../src/depends/rust-zmq/src/lib.rs)

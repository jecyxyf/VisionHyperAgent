# ZmqServer

> **状态：接口设计草案，尚未实现、编译或运行验证。** 本文补充 Agent 通信及宿主接入链路；具体签名、消息字段、并发与关闭策略需评审后实现。Python 相关工作继续暂缓。没有内容的栏目保留空白。
>
> 本类是Agent 后台的通信角色对象。Client／Server 是使用端角色，不表示已选定 ZeroMQ 的原生 CLIENT／SERVER、REQ／REP 等 Socket 模式，也不产生独立后端进程。

## 变更记录

| 版本 | 日期 | 作者 | 变更说明 |
|------|------|------|----------|
| 0.1.0 | 2026-09-10 | Codex | 初始通信与接入接口草案，待确认后实现 |
| 0.1.1 | 2026-09-10 | Codex | 合并共用契约，取消独立底层类文档，保持两端接口 |
| 0.1.2 | 2026-09-10 | Codex | 明确服务端收尾与整体生命周期的边界 |

---

## 1. 类型定义与关系

### 1.1 定义

**说明**：接收宿主侧数据并向宿主发送返回数据。ZmqServer 与 ZmqClient 是两端独立对象，分别持有端点资源；收发、参数检查、错误转换和关闭逻辑通过驱动模块的公共内部实现复用，不各写一套，也不要求另设第三个驱动类。

**类型声明**：

```rust
pub struct ZmqServer { /* 私有的共用驱动端点与资源引用 */ }
```

### 1.2 命名空间／所属模块

- **模块路径**：拟在 `vision_hyper_agent::drivers` 导出 `ZmqServer` 及必要的共用数据／资源类型；底层公共实现不作为额外的对外驱动入口。
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
| [ZmqClient](ZmqClient.md) | 通信对端 |  | 只通过通信数据交互，不相互持有对端对象 |
| [AgentTransport](../agent/api/AgentTransport.md) | 上层使用方 |  | 由其定义命令、响应和事件的含义；本类不依赖 Agent 类型 |

---

## 2. 构造／初始化

### 2.1 `bind(context, endpoint, kind, options)`

**说明**：在所属线程中使用共享 Context 创建端点并绑定地址，成功后交付本角色对象。

**定义**：

```rust
pub fn bind(context: ZmqContext, endpoint: &str,
    kind: ZmqSocketKind, options: ZmqSocketOptions) -> Result<Self, ZmqError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| context | ZmqContext | 共享上下文句柄 | inproc 对端必须来自同一个底层 Context |
| endpoint | &str | 绑定地址 | 地址及 NUL 检查复用共用驱动，不绕过验证 |
| kind | ZmqSocketKind | 实际 Socket 模式 | 由上层方案确认，不根据类名自动选定 |
| options | ZmqSocketOptions | 端点配置 | 不隐含填写超时、容量或 Linger 默认值 |

**返回值**：已完成端点绑定操作的 ZmqServer；不意味着对端已处理请求，也不意味着 Agent 已就绪。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `InvalidInput / Backend` | 参数无效、端点创建或绑定失败 | 不交付假就绪对象；部分资源按共用关闭策略处理 | 返回真实原因，不重试业务操作 |

**备注**：

- 在Agent 后台对应的 I/O 所有者上下文中创建和使用，不在 UI 回调中等待网络或 Socket。
- 本草案用单个地址表达一对端点的角色；最终 Socket 数量与控制／事件通道组合仍由 AgentTransport 方案决定，不承诺全库只能存在两个 Socket。
- 当前不增加自动重连或运行中改换 Context 的行为。
- 原生资源关闭的错误保证尚未实现，不能在初始化失败清理时吞掉底层问题。

**示例**：

**适用范围**：驱动层的Agent 后台角色端点；由 AgentTransport 使用，不包含 Agent 或 UI 业务协议。

**另请参阅**：

---

## 3. 字段与属性

### 3.1 字段

| 字段名 | 类型 | 访问级别 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| endpoint | 驱动模块内部端点资源 | private |  | 不暴露给 ViewModel 或 AgentRuntime；具体实现形式待落地时确认 |
| context | 共享 Context 的保活关系 | private |  | 由共用底层维护生命周期，不另造一个独立 Context |
| endpoint | 端点信息 | private |  | 记录本对象的绑定目标；具体存储形式不在本轮锁定 |

### 3.2 属性

---

## 4. 方法

### 4.1 关联函数／静态方法

### 4.2 实例方法

#### 4.2.1 `send(message, wait)`

**说明**：向宿主端发送原始数据。响应和后台通知如何区分由上层协议决定。

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

**适用范围**：驱动层的Agent 后台角色端点；由 AgentTransport 使用，不包含 Agent 或 UI 业务协议。

**另请参阅**：

#### 4.2.2 `receive(wait)`

**说明**：接收宿主端数据，交给 AgentTransport 解析后再由 Runtime 分派。

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

**适用范围**：驱动层的Agent 后台角色端点；由 AgentTransport 使用，不包含 Agent 或 UI 业务协议。

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

**适用范围**：驱动层的Agent 后台角色端点；由 AgentTransport 使用，不包含 Agent 或 UI 业务协议。

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

**适用范围**：驱动层的Agent 后台角色端点；由 AgentTransport 使用，不包含 Agent 或 UI 业务协议。

**另请参阅**：

---

## 5. 事件与消息处理

### 5.1 事件与通知

### 5.2 消息处理与回调

Agent 后台通过运行端协议接口使用服务端：ZmqServer 收到原始帧 → AgentTransport 解析 → Runtime 分派；操作结果和事件经 AgentTransport 编码后由本类发送。不在服务端驱动中执行业务或调用 Codex。

---

## 6. 其他成员

### 6.1 常量

### 6.2 枚举与嵌套类型

#### 6.2.1 `共用驱动数据类型`

ZmqContext、ZmqMessage、ZmqSocketKind、ZmqSocketOptions、ZmqWait、ZmqInterest、ZmqReadiness 和 ZmqError 归驱动模块的公共部分管理，定义统一见 [ZmqClient 的共用类型说明](ZmqClient.md)。本类不重新声明这些类型，也不依赖或创建客户端对象来获得共用能力。

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
- 服务端只报告本端收发及资源状态，不设置整体 AgentState，也不把本端关闭解释为宿主端或 Codex 已结束。应用线程归属与 ZeroMQ 内部线程配置分别处理。
- 不通过这些类向 UI 或 Agent 暴露通用终端、Python 执行或业务权限。
- 具体是否需要多端点组合留给上层通信方案，不提前新增多通道管理框架。
- 两端共用的地址校验、零帧消息、增量接收限制、等待与错误、关闭及 Context 生命周期规则，统一遵守 [ZmqClient 的共用约束](ZmqClient.md)，不再复制另一套说明或实现。

**待确认事项：**

- Socket 模式、端点组合和应用线程内的 I/O 调度。
- 超时、容量、Linger 与关闭失败恢复规则。
- 共用底层显式关闭的实现路径与错误保证。

### 7.3 适用范围与依赖版本

Rust ZeroMQ 固定提交为 `5d78967001abb1aece2fba878d6151cb66cd1767`，包版本字段为 `0.10.0`；不更新依赖。目标为 Linux、Windows，当前 Agent 链路使用 inproc，尚未进行平台或通信验证。

### 7.4 验证现状

本轮仅完成角色接口和文档关联整理，没有创建 Rust 类、修改 Cargo、编译驱动或运行收发／关闭测试。

### 7.5 另请参阅

- [ZmqClient](ZmqClient.md)
- [AgentTransport](../agent/api/AgentTransport.md)
- [宿主 AgentViewModel](../viewmodel/AgentViewModel.md)
- [模块规范模板](../../模块规范模板.md)
- [固定依赖基线](../../../src/depends/README.md)

# SessionManager

> **状态：接口设计草案，尚未实现、编译或运行验证。** 职责依据[Agent 需求文档](../Agent.md)及已确认的功能分工与通信边界；具体签名、数据字段和状态细节是待评审的设计建议，不等同于已确认的公开接口。Python 相关内容继续暂缓。空栏目保留，不补造无关成员。
>
> 本类属于库对外接口；不依赖 Slint 窗口、控件、事件循环或 VisionHyperAgent 的全局配置／日志对象。

## 变更记录

| 版本 | 日期 | 作者 | 变更说明 |
|------|------|------|----------|
| 0.1.0 | 2026-09-10 | Codex | 初始接口设计草案，待确认后实现 |
| 0.1.1 | 2026-09-10 | Codex | 补充通信层与宿主接入文档关联，明确职责边界 |
| 0.1.2 | 2026-09-10 | Codex | 明确 Chat 数据根及历史与未执行请求的边界 |

---

## 1. 类型定义与关系

### 1.1 定义

**说明**：对外提供会话及历史管理能力。会话是对话上下文，不等同于 Agent 运行实例或一次排队请求；持久化历史以 Codex 为唯一来源。

**类型声明**：

```rust
pub struct SessionManager { /* 私有实现 */ }
```

### 1.2 命名空间／所属模块

- **模块路径**：拟在 `vision_hyper_agent::agent` 重导出；具体子模块文件划分待实现阶段确认。
- **所属层**：通用 Agent 接入能力；保持单 Cargo 包，不按类拆分进程或独立库。
- **源码位置**：当前 `src/agent/mod.rs` 仅有职责占位，本文所述类型尚未落地，本文仅定义待评审的接口。

### 1.3 基类／继承关系

### 1.4 派生类型

### 1.5 实现接口／Trait

拟支持 Clone；克隆得到的是同一运行实例的访问句柄，不创建新的会话存储或后台运行时。

### 1.6 组合与依赖

| 依赖项 | 关系类型 | 版本／固定提交 | 说明 |
| --- | --- | --- | --- |
| [Agent](Agent.md) | 由其装配与提供 |  | 通过 agent.sessions() 取得 |
| [AgentTransport](AgentTransport.md) | 内部通信访问 |  | 关联命令与响应，不向本类暴露 Socket |
| [AgentRuntime](AgentRuntime.md) | 后台协作 |  | 协调会话操作及其真实结果 |
| [CodexAdapter](CodexAdapter.md) | 间接依赖 |  | 映射会话与历史协议，不自行访问个人 Codex 数据目录 |

---

## 2. 构造／初始化

不提供宿主直接调用的独立构造函数。由 Agent 装配，通过 agent.sessions() 获取；涉及 Codex 的操作须在初始化完成后调用。

---

## 3. 字段与属性

### 3.1 字段

### 3.2 属性

| 属性名 | 类型 | 访问级别 | 读写方式 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| 运行实例关联 | 内部句柄 | private | 不可由外部替换 |  | 所有会话操作使用已配置的数据根；VisionHyperAgent 为 `应用程序目录/Chat` |

---

## 4. 方法

### 4.1 关联函数／静态方法

### 4.2 实例方法

#### 4.2.1 `create()`

**说明**：创建新的持久会话，不自动发送用户消息或重放历史任务。

**定义**：

```rust
pub async fn create(&self) -> Result<SessionInfo, AgentError>
```

**参数**：

**返回值**：包含新 SessionId 的 SessionInfo。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `NotInitialized / InvalidState` | 未完成初始化、正在停止或已关闭 | 请求未被执行 | 等待可用状态或结束使用该实例 |
| `Backend / Transport / Io` | 上游调用、通信或存储失败 | 以实际结果为准，不能推定已回滚 | 接收错误并按业务规则处理，不自动重放有副作用的请求 |

**备注**：
- 按 Agent 配置使用模型、工作目录和允许的能力；不创建临时会话。
- 工具定义在创建会话时如何绑定由 ToolManager 与适配器协作，不能悄悄开放未注册能力。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：[ToolManager](ToolManager.md)

#### 4.2.2 `list(query)`

**说明**：分页查询本实例可访问的持久会话列表。

**定义**：

```rust
pub async fn list(&self, query: SessionQuery) -> Result<SessionPage, AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| query | SessionQuery | 游标及页大小 | 游标为不透明值，不自行拼接或解释 |

**返回值**：当前页 SessionInfo 及可选后续游标。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `NotInitialized / InvalidState` | 未完成初始化、正在停止或已关闭 | 请求未被执行 | 等待可用状态或结束使用该实例 |
| `Backend / Transport / Io` | 上游调用、通信或存储失败 | 以实际结果为准，不能推定已回滚 | 接收错误并按业务规则处理，不自动重放有副作用的请求 |

**备注**：
- 只访问宿主配置的会话数据根；本软件固定为应用程序目录下的 Chat，不混入开发者个人 Codex 历史。
- 页大小为空时保留固定版本上游的分页行为，不在文档中假定某个数量；产品层默认页大小另行确认。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：

#### 4.2.3 `read(session_id)`

**说明**：读取指定会话的基本信息，不启动对话请求。

**定义**：

```rust
pub async fn read(&self, session_id: SessionId) -> Result<SessionInfo, AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| session_id | SessionId | 目标会话标识 | 由该实例可访问的会话返回 |

**返回值**：SessionInfo；历史内容使用 history 分页读取。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `NotInitialized / InvalidState` | 未完成初始化、正在停止或已关闭 | 请求未被执行 | 等待可用状态或结束使用该实例 |
| `NotFound` | 会话不存在或不在允许访问范围 | 没有创建替代会话 | 由调用方处理 |
| `Backend / Transport / Io` | 上游调用、通信或存储失败 | 以实际结果为准，不能推定已回滚 | 接收错误并按业务规则处理，不自动重放有副作用的请求 |

**备注**：
- 读取元数据与加载完整历史分开，避免把所有会话内容一次性塞进返回值。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：

#### 4.2.4 `history(session_id, query)`

**说明**：分页读取会话历史，用于展示或检查已有对话内容。

**定义**：

```rust
pub async fn history(&self, session_id: SessionId, query: HistoryQuery) -> Result<HistoryPage, AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| session_id | SessionId | 目标会话 | 不能用它访问另一宿主的历史 |
| query | HistoryQuery | 不透明游标与页大小 | 历史条目与轮次的分页映射需在固定版本验证 |

**返回值**：历史条目页及后续游标；不触发任务执行。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `NotInitialized / InvalidState` | 未完成初始化、正在停止或已关闭 | 请求未被执行 | 等待可用状态或结束使用该实例 |
| `NotFound` | 目标历史不可用 | 不以空历史伪装成功 | 报告实际原因 |
| `Backend / Transport / Io` | 上游调用、通信或存储失败 | 以实际结果为准，不能推定已回滚 | 接收错误并按业务规则处理，不自动重放有副作用的请求 |

**备注**：
- 保留文字、图片资源引用及必要的工具／任务结果含义，不仅保存界面渲染文本。
- 不得因历史里存在工具调用而执行它。
- 历史页与轮次／条目的具体数据映射仍需确认，不提前承诺完整离线附件布局。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：

#### 4.2.5 `resume(session_id)`

**说明**：恢复已有会话上下文，使其可继续接收明确绑定该 SessionId 的新请求。

**定义**：

```rust
pub async fn resume(&self, session_id: SessionId) -> Result<SessionInfo, AgentError>
```

**参数**：

| 参数名 | 类型 | 说明 | 约束 |
| --- | --- | --- | --- |
| session_id | SessionId | 要恢复的会话 | 失败时不静默创建新会话 |

**返回值**：恢复后的会话信息；不表示历史任务已重新执行。

**错误**：

| 错误或失败状态 | 触发条件 | 失败后的状态 | 调用方处理 |
| --- | --- | --- | --- |
| `NotInitialized / InvalidState` | 未完成初始化、正在停止或已关闭 | 请求未被执行 | 等待可用状态或结束使用该实例 |
| `NotFound` | 会话不存在 | 未创建新会话 | 由调用方明确选择后续操作 |
| `Backend / Transport / Io` | 上游调用、通信或存储失败 | 以实际结果为准，不能推定已回滚 | 接收错误并按业务规则处理，不自动重放有副作用的请求 |

**备注**：
- 不修改一个供所有排队任务隐式读取的全局“当前会话”。
- 已排队请求继续使用各自提交时的 SessionId。
- 已加载会话的重复恢复行为与工具集合兼容性仍需核查。

**示例**：

**适用范围**：当前接口设计草案；运行条件见第 7 节。

**另请参阅**：

---

## 5. 事件与消息处理

### 5.1 事件与通知

### 5.2 消息处理与回调

这些查询与控制操作经 AgentTransport 交由 Runtime 执行，结果按通信调用标识返回原调用方。不得占用普通对话请求队列等待当前模型回复结束，也不得复制一套会话引擎或在本类直接操作底层驱动。

---

## 6. 其他成员

### 6.1 常量

### 6.2 枚举与嵌套类型

#### 6.2.1 `SessionId`

库公开的不透明会话标识，内部映射 Codex thread ID；不将磁盘路径当作会话标识。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| 内部值 | String | 外部可持有和比较，不解析内部结构或跨实例伪造关联 |

#### 6.2.2 `SessionInfo`

会话元数据快照，不包含可变的运行时所有权。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| id | SessionId | 会话标识 |
| title | `Option<String>` | 已有标题；缺失时不伪造名称 |

#### 6.2.3 `SessionQuery / SessionPage`

会话列表分页参数与结果。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| SessionQuery.cursor | `Option<String>` | 上次返回的不透明游标 |
| SessionQuery.page_size | `Option<u32>` | 显式页大小；允许范围及产品默认值待确认 |
| SessionPage.items | `Vec<SessionInfo>` | 当前页会话 |
| SessionPage.next_cursor | `Option<String>` | 存在时可继续读取 |

#### 6.2.4 `HistoryQuery / HistoryPage`

历史分页类型归属会话模块；不向外暴露 Codex 原始协议结构。

| 成员 | 类型／取值 | 说明 |
| --- | --- | --- |
| HistoryQuery.cursor / page_size | `Option<String> / Option<u32>` | 历史分页请求 |
| HistoryPage.items | `Vec<HistoryEntry>` | 当前页历史条目 |
| HistoryPage.next_cursor | `Option<String>` | 后续游标；分页映射待实现前确认 |

#### 6.2.5 `HistoryEntry`

表示一个历史条目，需保持来源角色、内容种类及必要的轮次关联；详细字段在历史契约确认后补齐，不能假定所有条目都是纯文本。

### 6.3 索引器与运算符

### 6.4 扩展成员

### 6.5 析构／终结器

---

## 7. 使用示例与约束

### 7.1 使用示例

```rust
use vision_hyper_agent::agent::{AgentError, SessionManager, SessionInfo};

async fn create_and_read(sessions: &SessionManager) -> Result<SessionInfo, AgentError> {
    let created = sessions.create().await?;
    sessions.read(created.id).await
}
```

仅为接口使用示意，未编译或执行；调用方需提供已声明的输入和异步运行上下文，不依赖 Slint 事件循环。

### 7.2 使用约束

- 会话持久化不等于排队任务持久化，更不等于重启后自动重放任务。
- 不提供清空显示对应的删除或重置接口。删除、归档等未确认能力不加入当前接口。
- history 读取的是 Codex 已持久化的会话内容，不保证包含尚未发给 Codex 的排队消息或所有失败记录；这些与 TaskManager 记录的整合及跨重启保存仍待确认。
- 历史与附件由库在隔离范围内通过 Codex 管理；不得把临时外部路径永久可用作为默认前提。
- 在 VisionHyperAgent 中使用固定的 `应用程序目录/Chat`。清空聊天显示不删除该目录，切换会话或任务工作目录也不更换数据根。

**待确认事项：**

- 历史条目字段、附件长期可用性及轮次／条目分页映射。
- 恢复已加载会话、已有工具定义与宿主注册表不一致时的行为。
- 重启是否恢复上次选中会话，以及历史删除规则。
- Chat 内部的历史、索引和附件布局、目录准备与权限失败，以及多个程序或 Agent 实例同时访问该目录的规则；不默认承诺多进程协调。

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
- [固定版本会话协议](../../../../src/depends/codex/codex-rs/app-server-protocol/src/protocol/v2/thread.rs)
- [AgentTransport](AgentTransport.md)

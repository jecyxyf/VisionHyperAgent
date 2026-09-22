# agent-service API 文档

## 1. 概述

`agent-service` 将 Codex Agent 接入视觉业务，负责会话、上下文、规则建议、训练建议、阶段计划和审批记录。它只能通过项目与 ML 业务服务落库或创建任务，不能绕过确认直接修改数据。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | backend |
| 语言 | Rust |
| 公开边界 | 对话命令、流式消息、计划确认和审计记录 |
| 外部依赖 | Codex Agent、用户配置的模型服务 |
| 安全边界 | API Key 只在后端使用；上下文按最小必要原则构建 |

## 3. 数据结构

| 类型 | 字段 | 说明 |
| --- | --- | --- |
| `Conversation` | id、project_id、title、model_id、effort | Agent 会话 |
| `ChatInput` | conversation_id、text、attachments | 用户消息 |
| `AgentMessage` | id、role、content、status | 消息快照 |
| `ProjectContext` | goal、rule、categories、dataset_stats、task_summary | 脱敏上下文 |
| `ExecutionPlan` | id、stage、steps、parameters、risk、status | 阶段级计划 |
| `PlanApproval` | plan_id、approved_by、approved_at | 用户确认记录 |
| `AgentEvent` | kind、message_id、delta、plan_id | 流式事件 |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `ready` | `bool` | `false` | Codex 运行环境是否可用 |
| `default_model_id` | `ModelId` | 配置值 | 默认模型 |
| `default_effort` | `Effort` | `medium` | 默认推理强度 |
| `active_conversations` | `usize` | 0 | 活跃会话数，只读 |

## 5. 方法

### `create_conversation(project_id, options) -> Conversation`

创建会话并记录模型与 Effort；不发送消息。

### `list_conversations(project_id) -> Vec<ConversationSummary>`

返回历史摘要，不加载全部消息正文。

### `send(input) -> AgentStream`

构建上下文并向 Codex 发送消息；返回流式句柄。发送不因前端连接状态失败，失败由流返回明确结果。

### `stop(conversation_id) -> StopReceipt`

请求停止当前回合，停止后保留已收到的部分消息。

### `approve_plan(approval) -> PlanReceipt`

记录用户对阶段计划和参数的明确确认；只有确认后业务服务才允许执行。

### `get_context(project_id) -> ProjectContext`

读取最小业务摘要供 Agent 使用，不包含密钥、原图和模型权重。

### `reload_config() -> Result<()>`

重新读取单例 Agent 配置，更新可用模型、Provider 和 Codex 参数；活动会话保留旧快照。

## 6. 消息与事件

| 事件 | 说明 |
| --- | --- |
| `message.started` | 回合已接受 |
| `message.delta` | 增量文本 |
| `message.tool_request` | Agent 提出外部动作或计划 |
| `message.completed` | 正常完成 |
| `message.failed` | 发送或服务失败 |
| `message.stopped` | 用户请求停止 |
| `plan.awaiting_approval` | 等待自然语言确认 |
| `plan.approved` | 计划已确认 |

事件顺序按 message_id 保证；断线后通过历史和回合状态恢复。

## 7. 委托与回调

`AgentEventSink = Fn(AgentEvent) + Send + Sync`。回调在事件分发任务中串行执行，不能直接执行训练或写项目；需要动作时调用 `approve_plan` 和业务服务。Codex 连接回调只接收脱敏事件。

## 8. 错误处理

| 错误码 | 说明 |
| --- | --- |
| `agent_not_ready` | Codex 尚未就绪 |
| `model_config_invalid` | 模型或 Provider 配置无效 |
| `conversation_not_found` | 会话不存在 |
| `context_unavailable` | 项目摘要暂时不可读 |
| `plan_not_approved` | 业务动作缺少确认 |
| `agent_timeout` | 上游响应超时 |
| `agent_stopped` | 回合已停止 |
| `agent_transport_error` | WebSocket 或进程通信失败 |

失败消息保存为可恢复状态；不把外部响应的原始密钥和内部路径返回前端。

## 9. 生命周期与线程安全

服务启动后等待 Codex 可用；会话方法可并发调用，不同会话互不共享可变消息缓冲。单个会话同一时间只允许一个发送回合。停止、关闭和配置重载均通过内部命令队列串行化。服务关闭时先停止新回合，再回收流和连接。

## 10. 依赖关系

依赖现有 `model/codex_agent`、`project-service`、`ml-service`、`storage` 和 `common-services`；被 `api-gateway` 调用。不得直接依赖 `mask-rcnn-worker`。

## 11. 调用示例

```rust
// 创建 Agent 服务和项目会话
let agent = AgentService::new(codex, projects, ml, storage, config);
let conversation = agent.create_conversation(project_id, ConversationOptions::default()).await?;
// 发送目标描述并消费流式事件
let mut stream = agent.send(ChatInput::text(conversation.id, "识别每个独立零件")).await?;
while let Some(event) = stream.next().await {
    // 等待计划或规则结果
    if let AgentEvent::PlanAwaitingApproval(plan) = event? {
        let receipt = agent.approve_plan(PlanApproval::for_user(plan.id)).await?;
        assert!(receipt.accepted);
        // 结束当前计划确认分支
    }
}
// 保存会话历史并可在需要时停止后续回合
let history = agent.list_conversations(project_id).await?;
let context = agent.get_context(project_id).await?;
// 停止当前回合并刷新配置
agent.stop(conversation.id).await?;
agent.reload_config().await?;
// 校验上下文和历史都来自当前会话
assert!(!context.goal.is_empty());
assert!(history.iter().any(|item| item.id == conversation.id));
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `new` / `create_conversation` / `send` | 初始化和发送段 |
| `approve_plan` | 计划确认段 |
| `get_context` / `stop` / `reload_config` | 收尾段 |
| `list_conversations` | 历史查询代码 |

## 12. 2119 测试要求

### 验证范围

规格应覆盖会话、流式回合、计划确认、配置重载和错误恢复等调用方可观察行为。重点验证：

| 行为 | 必须验证 | 反例测试 |
| --- | --- | --- |
| 会话与发送 | 消息被接受并最终产生完成、停止或失败事件 | 模型无效、上游超时、传输断开时返回明确失败 |
| 计划确认 | 未确认的计划不能创建或修改业务数据 | 直接调用执行路径必须被拒绝 |
| 上下文隔离 | 只发送最小项目摘要，不泄漏密钥、原图和权重 | 脱敏检查拒绝敏感字段 |
| 配置重载 | 新会话使用新配置，活动会话保留旧快照 | 重载期间的并发回合不互相覆盖 |

测试应在 `agent-service` 集成边界标注对应 REQ-ID，并同时覆盖停止、取消和历史恢复。Codex mock 只能替代外部传输，不能替代计划确认和错误状态断言。

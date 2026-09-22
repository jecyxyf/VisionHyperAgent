# task-runtime API 文档

## 1. 概述

`task-runtime` 是长任务状态机、事件总线、取消、恢复和子进程监督的唯一权威。业务服务只提交结构化任务，不能自行启动 Worker 或伪造成功状态。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | backend |
| 语言 | Rust |
| 公开边界 | 任务提交、快照、事件、取消和启动恢复 |
| 管理对象 | Python ML Worker 进程及任务目录 |
| 状态 | created、queued、running、succeeded、failed、cancelled、interrupted |

## 3. 数据结构

| 类型 | 字段 | 说明 |
| --- | --- | --- |
| `TaskSpec` | id、kind、input、output_dir、timeout | 结构化任务定义 |
| `TaskState` | kind、status、completed、total、error | 任务状态快照 |
| `TaskEvent` | sequence、task_id、kind、payload | 事件载荷 |
| `Cancellation` | requested_at、deadline、force_kill | 取消策略 |
| `CompletionProof` | manifest、hashes、finished_at | Worker 完成凭证 |
| `RecoveryReport` | resumed、interrupted、verified | 启动恢复结果 |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `queue_capacity` | `usize` | 配置值 | 等待队列上限 |
| `running_task` | `Option<TaskId>` | `None` | 当前 GPU 任务 |
| `event_capacity` | `usize` | 1024 | 事件缓存容量 |
| `shutdown` | `bool` | `false` | 是否停止接收新任务 |

## 5. 方法

### `submit(spec) -> TaskReceipt`

校验任务 ID 幂等性和资源策略后入队；不表示已经开始执行。

### `snapshot(task_id) -> TaskState`

返回当前状态副本；找不到任务返回 `task_not_found`。

### `subscribe(task_id, cursor) -> TaskSubscription`

订阅任务事件；游标过期时返回同步提示。

### `cancel(task_id, reason) -> CancellationReceipt`

先向 Worker 发送取消命令，超时后回收进程树；最终状态由 Worker 结果和凭证决定。

### `recover() -> RecoveryReport`

启动时核对数据库状态、进程和完成凭证，把无法证明完成的任务标记 `interrupted`。

### `shutdown() -> ShutdownReport`

停止新任务、取消运行任务、等待或强制回收 Worker，最后写入状态。

## 6. 消息与事件

发布 `task.created`、`task.queued`、`task.running`、`task.progress`、`task.metric`、`task.cancel-requested`、`task.cancelled`、`task.succeeded`、`task.failed`、`task.interrupted`。事件只携带摘要和相对产物路径。

## 7. 委托与回调

`TaskEventSink` 接收按序事件；`WorkerFactory` 创建受控子进程；`CompletionValidator` 验证凭证和哈希。回调必须幂等，不能在事件回调中同步等待同一任务终态。

## 8. 错误处理

| 错误码 | 说明 |
| --- | --- |
| `task_exists` | 同 ID 任务已存在且参数不同 |
| `queue_full` | 队列已满 |
| `task_conflict` | 资源策略冲突 |
| `task_not_found` | 任务不存在 |
| `cancel_timeout` | 取消后 Worker 未及时退出 |
| `worker_exit` | Worker 异常退出 |
| `completion_invalid` | 完成凭证或哈希不合法 |
| `runtime_shutdown` | 运行时已停止接收新任务 |

## 9. 生命周期与线程安全

任务状态转移只能由运行时内部状态机执行；公开方法通过 Tokio channel 串行化命令。事件订阅句柄可跨线程持有。关闭顺序固定为拒绝新任务→请求取消→等待→强制回收→持久化终态。网页断开不影响任务。

## 10. 依赖关系

依赖 `storage`、`common-services` 和 ML Worker 协议；被 `ml-service`、`api-gateway`、`desktop-host` 调用。不得承载项目规则和 UI 状态。

## 11. 调用示例

```rust
// 创建运行时并恢复历史任务
let runtime = TaskRuntime::new(storage, worker_factory, RuntimeConfig::default());
let recovery = runtime.recover().await?;
// 提交训练任务并订阅事件
let receipt = runtime.submit(TaskSpec::training(task_id, manifest, output_dir)).await?;
let mut events = runtime.subscribe(receipt.task_id, None).await?;
while let Some(event) = events.next().await {
    // 任务完成后退出事件循环
    if event?.is_terminal() { break; }
}
// 用户请求停止或应用退出时走统一清理
runtime.cancel(receipt.task_id, "用户停止").await?;
let final_state = runtime.snapshot(receipt.task_id).await?;
let report = runtime.shutdown().await?;
// 验证恢复、进程回收和任务终态
assert!(recovery.interrupted.len() >= 0);
assert!(report.remaining_processes.is_empty());
// 终态必须是可解释的任务状态
assert!(final_state.status.is_terminal());
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `new` / `recover` / `submit` / `subscribe` | 初始化、恢复和任务段 |
| `cancel` / `snapshot` / `shutdown` | 清理段 |

## 12. 2119 测试要求

### 验证范围

规格应覆盖任务状态机、事件顺序、取消、恢复和进程监督：

| 行为 | 必须验证 | 反例测试 |
| --- | --- | --- |
| 状态转移 | 任务只按允许路径进入终态，事件顺序可追踪 | 重复完成、跳过 running 或伪造成功必须拒绝 |
| 取消 | 先协作取消，超时再回收，并记录最终原因 | Worker 不响应时不能无限等待 |
| 恢复 | 启动时核对状态、进程和完成凭证 | 无凭证的 running 任务必须标记 interrupted |
| 事件订阅 | 游标缺口返回同步提示，网页断开不改变任务 | 过期事件不能静默丢失 |

测试应使用受控 Worker 和至少一次真实子进程回收；每个状态和拒绝路径标注对应 REQ-ID。

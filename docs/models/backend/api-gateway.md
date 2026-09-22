# api-gateway API 文档

## 1. 概述

`api-gateway` 是浏览器与 Rust 宿主之间的唯一协议边界，负责 HTTP、WebSocket、静态资源、文件上传、参数校验和内部错误映射。它不实现项目业务规则、不直接访问 SQLite，也不直接启动子进程。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | backend |
| 语言 | Rust |
| 公开边界 | HTTP 路由、WebSocket 会话、上传和稳定错误响应 |
| 线程模型 | axum/Tokio 异步运行；处理器不持有阻塞锁 |
| 安全边界 | 默认只绑定本机地址；密钥、绝对路径和内部堆栈不返回前端 |

## 3. 数据结构

| 类型 | 字段 | 说明 |
| --- | --- | --- |
| `RequestContext` | request_id、method、path | 请求追踪信息；载荷已脱敏 |
| `ApiResponse<T>` | request_id、data、error | 成功或失败的统一响应 |
| `ApiError` | code、message、retryable、details | 稳定错误码和中文提示 |
| `WsSubscribe` | topics、last_sequence | WebSocket 订阅主题和断线续传位置 |
| `WsEvent` | sequence、topic、event、payload | 任务、Agent 和状态事件 |
| `UploadReceipt` | upload_id、resource_id、accepted、rejected | 批量上传结果摘要 |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `listen_addr` | `SocketAddr` | `127.0.0.1:0` | 本地监听地址；实际端口由宿主分配 |
| `max_upload_bytes` | `u64` | 配置值 | 单个上传大小上限 |
| `event_capacity` | `usize` | 1024 | 每个订阅的事件缓存容量 |
| `started` | `bool` | `false` | 是否已绑定并接受连接；只读 |

## 5. 方法

### `new(state, limits) -> ApiGateway`

创建网关并注入应用服务。网关借用服务句柄，不取得数据库、Agent 或任务运行时的所有权。

### `router(&self) -> Router`

构造 HTTP、WebSocket、上传和静态资源路由。重复调用返回等价路由，不启动监听。

### `serve(self, listener) -> Result<(), GatewayError>`

在给定监听器上服务；收到优雅关闭信号后停止接收新请求并等待处理中请求结束。

### `subscribe(&self, request: WsSubscribe) -> Result<WsSession, ApiError>`

校验主题并创建 WebSocket 会话。`last_sequence` 过期时返回需要快照同步的错误，而不是静默丢事件。

### `handle_upload(&self, request: UploadRequest) -> UploadReceipt`

把上传内容写入临时目录并交由项目服务校验；单个文件失败不影响同批其他文件。

### `shutdown(&self) -> Result<(), GatewayError>`

停止监听并关闭订阅；不取消后台任务，任务由 `task-runtime` 管理。

## 6. 消息与事件

| 事件 | 方向 | 说明 |
| --- | --- | --- |
| `task.*` | 后端→前端 | 任务创建、排队、进度、完成、失败和中断 |
| `agent.message` | 后端→前端 | Agent 流式文本和消息状态 |
| `agent.status` | 后端→前端 | Agent 连接和配置状态 |
| `project.changed` | 后端→前端 | 项目、图片、标注或数据集快照变化 |
| `sync.required` | 后端→前端 | 事件缓存不足，需要重新读取快照 |
| `gateway.error` | 双向 | 协议错误；不含内部堆栈 |

事件序列号在单个服务实例内单调递增；客户端不得仅按到达时间推断状态。

## 7. 委托与回调

`EventSink = Fn(WsEvent) + Send + Sync`，由网关向订阅者发送事件。回调不得阻塞 Tokio 运行时；慢客户端进入有界队列，队列满则发送 `sync.required` 并断开订阅。上传完成回调只传递收据，不传递文件内容。

## 8. 错误处理

| 错误码 | 含义 | 可重试 |
| --- | --- | --- |
| `invalid_request` | JSON、路径或参数不合法 | 否 |
| `not_found` | 项目或资源不存在 | 否 |
| `payload_too_large` | 超过上传限制 | 否 |
| `upload_failed` | 临时文件或校验失败 | 是 |
| `service_unavailable` | 依赖服务暂不可用 | 是 |
| `event_cursor_expired` | 事件无法从缓存补齐 | 是，先同步 |
| `internal_error` | 未分类内部错误 | 视 details 决定 |

HTTP 状态码与稳定错误码同时返回；`details` 经过脱敏，日志使用 `request_id` 关联。

## 9. 生命周期与线程安全

启动顺序为构造状态 → 构造路由 → 绑定监听器 → 接受请求。处理器可并发运行，业务服务负责事务和状态一致性。`ApiGateway` 的配置在启动后只读；关闭时先拒绝新请求，再关闭 WebSocket，最后释放路由资源。订阅句柄可以跨线程移动，但发送操作必须由内部异步任务串行化。

## 10. 依赖关系

| 依赖 | 用途 |
| --- | --- |
| `project-service` | 项目、图片、标注和数据集请求 |
| `agent-service` | Agent 对话、历史和计划 |
| `ml-service` | 训练、评估、推理请求 |
| `task-runtime` | 任务快照和取消 |
| `storage` | 仅通过业务服务间接使用 |
| `axum` / `tokio` | HTTP、WebSocket 和异步运行时 |

网关不直接依赖 `mask-rcnn-worker` 和数据库连接。

## 11. 调用示例

```rust
// 构造网关并启动本地服务
let state = AppState::new(projects, agents, ml, tasks);
let gateway = ApiGateway::new(state, GatewayLimits::default());
let router = gateway.router();
let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
// 监听任务在收到关闭信号后结束
let server = tokio::spawn(gateway.serve(listener));

// 创建 WebSocket 订阅
let session = gateway.subscribe(WsSubscribe {
    topics: vec!["task.*".into(), "agent.*".into()],
    last_sequence: None,
}).await?;
// 上传文件并取得资源回执
let receipt = gateway.handle_upload(UploadRequest::from_path(image_path)).await?;
// 客户端断线时以回执和快照恢复
assert!(receipt.accepted.iter().all(|item| item.resource_id.is_some()));
// 关闭网关但不取消后台任务
session.close().await?;
gateway.shutdown().await?;
server.await??;
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `new` / `router` / `serve` | 前半段启动代码 |
| `subscribe` | WebSocket 订阅代码 |
| `handle_upload` | 上传回执代码 |
| `shutdown` | 最后一段关闭代码 |

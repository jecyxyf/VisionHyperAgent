# workspace-ui API 文档

## 1. 概述

`workspace-ui` 是 Svelte 5 + TypeScript 的主工作区，负责项目、图片、数据集、训练、评估、推理、设置和 Agent 页面。它只维护页面状态和用户交互，不实现业务规则、不直接写文件或数据库。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | frontend |
| 语言 | TypeScript / Svelte |
| 公开边界 | 页面状态、命令发送、事件订阅和视图模型 |
| 依赖边界 | 通过 `lib/api` 调用 Rust 网关；Canvas 通过事件交换几何数据 |
| 状态原则 | 后端快照是最终事实，本地状态只做展示和乐观交互 |

## 3. 数据结构

| 类型 | 字段 | 说明 |
| --- | --- | --- |
| `WorkspaceState` | route、project、selection、busy、error | 页面总状态 |
| `ProjectSummary` | id、name、image_count、updated_at | 项目列表项 |
| `ImageRow` | id、thumbnail_url、status、instance_count | 图片列表项 |
| `DatasetSummary` | id、image_count、class_stats、risk | 数据集摘要 |
| `TaskView` | id、kind、status、progress、message | 任务展示状态 |
| `AgentPanelState` | conversation_id、messages、model_id、effort、streaming | Agent 面板状态 |
| `UiCommand` | kind、payload、correlation_id | 由页面发出的命令 |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `currentRoute` | `Route` | `home` | 当前页面路由 |
| `currentProjectId` | `string | null` | `null` | 当前项目 |
| `busy` | `boolean` | `false` | 是否有页面级操作正在提交 |
| `connection` | `ConnectionState` | `connecting` | HTTP/WebSocket 状态 |
| `lastSyncSequence` | `number` | `0` | 最近同步事件序号 |

## 5. 方法

### `mount(target) -> UiController`

挂载应用外壳、读取初始快照并建立事件订阅；不会把本地草稿当作后端事实。

### `openProject(projectId) -> Promise<void>`

读取项目快照并切换工作区；项目不存在时保留当前页面并显示错误。

### `subscribe(listener) -> Unsubscribe`

订阅视图状态变化；监听器接收只读快照，组件卸载时调用返回的取消函数。

### `dispatch(command) -> Promise<CommandResult>`

把用户操作交给 API 客户端；成功后等待快照或事件确认，失败时恢复可编辑状态。

### `applyEvent(event) -> void`

按事件序号更新可见状态；发现缺口时触发快照同步。

### `restore() -> Promise<void>`

刷新或重新打开网页后恢复路由、项目、会话和任务状态。

### `destroy() -> void`

取消订阅、清理定时器和未完成的视图请求；不取消后端任务。

## 6. 消息与事件

消费 `project.changed`、`task.*`、`agent.*`、`sync.required`；向 `api-gateway` 发送项目命令、标注提交、任务控制和 Agent 消息。事件载荷只更新视图模型，资源大图通过 HTTP 按需加载。

## 7. 委托与回调

`ViewListener = (state: Readonly<WorkspaceState>) => void`；`CommandErrorHandler` 接收稳定错误码。回调应短小，不在 Svelte 响应式更新中同步执行长任务；组件卸载时必须解除监听。

## 8. 错误处理

| 错误码 | 页面处理 |
| --- | --- |
| `network_unavailable` | 保留输入草稿并显示重连提示 |
| `stale_snapshot` | 重新拉取后端快照 |
| `validation_failed` | 定位到字段或图片 |
| `task_failed` | 展示任务错误和重试入口 |
| `agent_not_ready` | Agent 区域显示配置修复提示 |
| `permission_denied` | 说明资源不可用，不清空当前状态 |

## 9. 生命周期与线程安全

浏览器主线程运行 UI；网络事件通过客户端 store 串行归并。WebSocket 断开不修改后端任务状态。所有异步请求需携带取消信号，组件销毁后忽略过期响应。状态更新使用不可变快照，避免并发覆盖。

## 10. 依赖关系

依赖 `lib/api`、`lib/stores`、`annotation-canvas` 和 Svelte；被浏览器入口使用。不得直接依赖 Rust crate、SQLite 或 Python Worker。

## 11. 调用示例

```ts
// 挂载工作区并监听后端事件
const controller = mount(document.querySelector('#app')!);
const unsubscribe = controller.subscribe((state) => renderShell(state));
// 恢复上次打开的项目和任务
await controller.restore();
await controller.openProject('project-001');
// 提交导入命令并等待事件确认
await controller.dispatch({ kind: 'import-images', files });
// 收到断线重连后的事件或同步请求
controller.applyEvent({ sequence: 42, event: 'task.progress', payload: progress });
// 用户关闭网页只销毁前端资源
unsubscribe();
controller.destroy();
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `mount` / `restore` / `openProject` | 初始化和恢复段 |
| `subscribe` / `dispatch` / `applyEvent` | 订阅、操作和事件段 |
| `destroy` | 网页关闭段 |

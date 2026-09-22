# desktop-host API 文档

## Overview

本文件同时是 `desktop-host` 的 API 契约和 2119 需求来源。需求约束宿主启动、浏览器连接、托盘退出和跨平台进程清理。

## Requirements

### 1: 宿主生命周期

1. 宿主 MUST 按日志、配置、服务、子进程监督和浏览器入口的顺序完成启动 [manual]
2. 浏览器连接断开时宿主 MUST 继续运行后台任务 [manual]
3. 托盘退出时宿主 MUST 回收应用拥有的 Codex、ML Worker 和进程树 [manual]
4. Windows 和 Linux 宿主 MUST 对端口冲突、权限失败和浏览器启动失败提供可诊断结果 [manual]


## 1. 概述

`desktop-host` 负责桌面启动顺序、系统托盘、浏览器打开、平台适配和退出清理。浏览器标签关闭只断开前端，不触发宿主退出；托盘退出回收应用拥有的 Codex 和 ML Worker。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | backend |
| 语言 | Rust |
| 公开边界 | 启动、界面入口、托盘命令和关闭报告 |
| 平台 | Windows 10/11、主流 Linux |
| 不负责 | 项目业务、模型计算和协议路由 |

## 3. 数据结构

| 类型 | 字段 | 说明 |
| --- | --- | --- |
| `HostConfig` | app_dir、data_dir、open_browser | 宿主配置 |
| `StartupReport` | address、tray_ready、agent_ready、recovered | 启动结果 |
| `TrayCommand` | Open、ShowLogs、Quit | 托盘动作 |
| `ShutdownReport` | codex_reaped、workers_reaped、database_closed | 退出清理结果 |
| `PlatformInfo` | os、arch、executable_dir | 平台信息 |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `running` | `bool` | `false` | 宿主是否运行 |
| `browser_url` | `Url` | 本地服务地址 | 可打开或聚焦的界面 |
| `tray_visible` | `bool` | `false` | 托盘是否可见 |
| `shutdown_requested` | `bool` | `false` | 是否已请求退出 |

## 5. 方法

### `start(config) -> StartupReport`

按日志→配置→存储→HTTP→托盘→Codex→恢复的顺序启动；Codex 失败不阻塞本地项目功能。

### `open_ui() -> Result<()>`

打开或聚焦本地浏览器页面；浏览器失败时仍保持宿主运行。

### `handle_tray(command) -> Result<()>`

处理打开界面、显示日志和退出命令；命令串行执行。

### `request_shutdown(reason) -> ShutdownHandle`

发起幂等关闭流程，停止新任务并回收子进程。

### `platform_info() -> PlatformInfo`

返回平台和可执行目录信息，不包含用户密钥。

### `wait_closed() -> ShutdownReport`

等待关闭流程完成并返回结果。

## 6. 消息与事件

`host.started`、`host.agent-status`、`host.tray-command`、`host.shutdown-requested`、`host.closed`。浏览器断开事件只更新连接计数，不发布退出事件。

## 7. 委托与回调

`TrayHandler = Fn(TrayCommand) + Send`；`ShutdownObserver` 接收阶段、超时和回收结果。回调不得阻塞托盘线程，耗时工作投递到异步运行时。

## 8. 错误处理

| 错误码 | 说明 |
| --- | --- |
| `tray_unavailable` | 托盘初始化失败，可退化为命令行入口 |
| `browser_launch_failed` | 浏览器打开失败，服务仍可访问 |
| `port_bind_failed` | 本地监听失败 |
| `child_reap_timeout` | 子进程未在超时内退出 |
| `startup_failed` | 关键服务启动失败 |
| `shutdown_in_progress` | 重复请求关闭 |

## 9. 生命周期与线程安全

`start` 只允许一次；托盘事件线程不直接操作业务服务。关闭流程使用一次性状态，重复退出请求返回同一个句柄。Windows 使用 Job Object 等价机制，Linux 使用进程组回收子树。

## 10. 依赖关系

装配 `common-services`、`storage`、`api-gateway`、`task-runtime` 和 Agent Runtime；使用 `tray-icon`、`tao`、`webbrowser` 等平台库。业务模块不反向依赖桌面宿主。

## 11. 调用示例

```rust
// 读取平台信息并启动宿主
let host = DesktopHost::new(HostConfig::from_executable_dir());
let platform = host.platform_info();
let report = host.start(config).await?;
// 托盘打开或聚焦界面
host.handle_tray(TrayCommand::Open).await?;
host.open_ui().await?;
// 托盘退出触发统一回收
let handle = host.request_shutdown("用户退出").await?;
handle.await_requested().await?;
let closed = host.wait_closed().await?;
// 验证宿主和数据库已完成关闭
assert!(closed.database_closed);
assert!(platform.executable_dir.exists());
// 验证服务仅绑定本机地址
assert!(report.address.ip().is_loopback());
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `new` / `platform_info` / `start` | 启动段 |
| `handle_tray` / `open_ui` | 界面段 |
| `request_shutdown` / `wait_closed` | 退出段 |

## 12. 2119 测试要求

| 2119 ID | 验证行为 | 当前证据 |
| --- | --- | --- |
| `desktop-host.1.1` | 启动顺序和入口可观察 | 规划验收；实现后补平台冒烟测试 |
| `desktop-host.1.2` | 浏览器断开不影响后台任务 | 规划验收；实现后补生命周期测试 |
| `desktop-host.1.3` | 托盘退出不遗留子进程 | 规划验收；实现后补 Windows/Linux 进程测试 |
| `desktop-host.1.4` | 平台启动失败可诊断 | 规划验收；实现后补端口和权限测试 |

平台测试应在真实宿主边界执行，使用 `// 2119: desktop-host.1.1` 标记。

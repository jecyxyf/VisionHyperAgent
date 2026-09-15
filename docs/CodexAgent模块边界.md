# CodexAgent 模块边界

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-15 |
| 状态 | 已确认边界，具体实现由用户逐段指挥 |
| 源码位置 | `source/Rust/model/codex_agent/` |

## 1. 目标

CodexAgent 是 VisionHyperAgentAPP 进程内的 Rust 库模块，以受控子线程运行，负责把 Codex App Server 的 WebSocket / JSON-RPC 能力转换为进程内接口和事件。

CodexAgent 只做通讯适配，不承载视觉业务，也不做独立产品进程。

## 2. 生命周期

### 2.1 状态

```text
stopped -> starting -> running
running -> reconnecting -> running
running -> stopping -> stopped
任意状态 -> error
```

状态由 CodexAgent 管理，并通过事件通知 Rust ViewModel。

### 2.2 启动

`start(config)` 负责建立连接并启动接收线程。配置至少包含：

| 配置 | 说明 |
| --- | --- |
| websocket_url | Codex App Server 地址 |
| request_timeout | 单次请求超时 |
| reconnect_interval | 重连间隔 |
| max_reconnect_attempts | 最大连续重连次数，成功后计数重置 |

配置由 Rust Core 按安装环境和用户配置解析，CodexAgent 不读取界面状态。

### 2.3 停止与重启

- `stop()`：取消未完成请求、关闭连接、等待线程退出；
- `restart()`：按停止再启动执行，保留调用方显式传入的新配置；
- 掉线时自动重连，连续失败后进入 `error`。

## 3. 对外接口

第一版接口形态：

```text
start(config)
stop()
restart()
status()

create_thread()
resume_thread(thread_id)
list_threads()
read_thread(thread_id)

start_turn(thread_id, input)
interrupt_turn(thread_id)

list_skills()
subscribe_events(callback)
```

接口返回请求结果或请求句柄；耗时等待不放在 Qt 主线程。

## 4. 方法白名单

CodexAgent 只允许调用以下 Codex 方法：

```text
thread/start
thread/resume
thread/list
thread/read
turn/start
turn/interrupt
skills/list
```

后续新增方法必须先扩展白名单和文档，不允许把任意方法名透传给 Codex。

## 5. 事件

CodexAgent 把 Codex 通知转换为进程内事件：

| Codex 通知 | 进程内事件 |
| --- | --- |
| `thread/started` | `agent.thread_started` |
| `turn/started` | `agent.turn_started` |
| `item/agentMessage/delta` | `agent.message_delta` |
| `item/completed` | `agent.item_completed` |
| `turn/completed` | `agent.turn_completed` |
| `error` | `agent.error` |

事件由 Rust Core 的事件总线分发。Python ViewModel 只消费 Rust ViewModel 整理后的状态。

## 6. 请求与线程

- 每个 JSON-RPC 请求保存关联 ID、超时和取消标记；
- 响应、通知和反向请求在 CodexAgent 接收线程解析；
- 回调经过线程安全通道转发，不直接触碰 Qt 对象；
- 反向审批请求交给 Rust Model，再进入用户确认流程；
- CodexAgent 不在内部吞掉错误，所有终态必须可观测。

## 7. 不属于 CodexAgent 的职责

- 不保存业务参数和业务事实；
- 不解析 rollout 文件作为会话管理入口；
- 不判断预标注、训练或部署规则；
- 不直接执行 Python、训练或推理；
- 不修改 Codex 子模块源码。

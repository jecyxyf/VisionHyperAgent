# CodexDDSAgent 模块边界

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-15 |
| 状态 | 进程定位、目录、端口和守护策略已确认；Zenoh 接口明细由用户逐段确认后补充 |
| 源码位置 | `source/CodexDDSAgent/` |
| 协议来源 | `source/depends/codex`，提交 `6b9826e3aa83b1a5947db50f4332cb9c65f1b340`，标签 `rust-v0.154.0` |

## 1. 目标

CodexDDSAgent 是独立的 Agent 服务进程，由 VisionHyperAgent 启动和守护。它把 codex-app-server 的 WebSocket / JSON-RPC 能力封装为 Zenoh 服务，供主程序的 Rust Model 调用。

~~~text
VisionHyperAgent
↔ Zenoh
CodexDDSAgent
↔ WebSocket / JSON-RPC
codex-app-server
~~~

## 2. 职责

CodexDDSAgent 负责：

- 接收主程序传入的 Zenoh 监听参数并启动服务；
- 启动、停止、守护内置 codex-app-server；
- 解析 codex-app-server 输出并建立 WebSocket 连接；
- 完成 initialize / initialized 握手；
- 将 VisionHyperAgent 请求转换为 Codex 请求；
- 将 Codex 响应、反向请求和服务端通知转发回 VisionHyperAgent；
- 处理请求超时、取消、错误映射和重连；
- 发布自身状态和 codex-app-server 状态。

## 3. 不负责

CodexDDSAgent 不负责：

- 预标注、标注、预训练、训练、推理和模型管理；
- 保存业务事实或用户确认状态；
- 自研会话数据库；
- 解析 rollout 文件作为会话管理入口；
- 直接执行训练、推理或 Python 脚本；
- 替代用户审批命令、文件修改或动态工具；
- 向界面暴露原始 WebSocket；
- 修改 Codex 子模块源码。

## 4. 目录与路径

~~~text
发布包/
├── VisionHyperAgent
│
└── CodexDDSAgent/
    ├── CodexDDSAgent
    └── agents/
        ├── codex-app-server
        └── logs/
~~~

- `CODEX_HOME = CodexDDSAgent/agents`。
- codex-app-server 固定使用内置路径。
- 不从 `PATH` 查找，不接受用户配置的外部可执行文件。
- codex-app-server 的 stdout / stderr 用于解析监听地址，并写入 `agents/logs/`。

## 5. 启动与守护

已确认的启动事实：

- CodexDDSAgent 由 VisionHyperAgent 自动拉起。
- VisionHyperAgent 退出时停止 CodexDDSAgent。
- Zenoh 端口保存在主程序配置中，默认 `18744`。
- 主程序把端口作为启动参数传给 CodexDDSAgent。
- 监听地址固定为本机回环。
- 启动参数不只有端口和日志级别；完整参数清单后续由用户逐段确认。

守护策略：

- CodexDDSAgent 异常退出后，主程序延迟 2 秒重启。
- 连续失败 5 次后停止自动重启并进入错误状态。
- 重启成功后失败计数清零。
- codex-app-server 异常退出后，CodexDDSAgent 重启进程、解析地址、重连 WebSocket 并重新握手。
- 未完成的请求按连接断开失败处理，不跨进程重启恢复。

## 6. Codex 协议能力范围

CodexDDSAgent 的目标是覆盖固定 Codex 版本 App Server 协议暴露的全部对外能力，而不是维护少量方法白名单。

当前协议统计：

| 类型 | 数量 |
| --- | ---: |
| 客户端请求 | 162 |
| 客户端通知 | 1 |
| Codex 反向请求 | 11 |
| Codex 服务端通知 | 83 |

约束：

- 协议类型以 `codex-app-server-protocol` 为准，避免手写字符串和第二套协议模型。
- 60 个请求是方法级实验能力，另有 9 个请求只有部分参数组合是实验能力。
- 握手默认声明 `experimentalApi: true`。
- 运行时或账号权限拒绝的能力必须返回可观测错误，不做能力缺口掩盖。
- VisionHyperAgent 可以只向界面暴露产品需要的场景，但 CodexDDSAgent 内部保持完整协议覆盖。

完整方法、通知和反向请求清单在实现协议适配层时以固定子模块为准生成，不在本文展开。

## 7. 会话管理

- Codex 会话和落盘数据由 Codex 在 `CODEX_HOME` 中管理。
- CodexDDSAgent 不创建第二套会话数据库。
- VisionHyperAgent 通过 CodexDDSAgent 调用 Codex 的列表、恢复和继续会话能力。
- 会话标识由 Codex 返回，业务层只保存必要的引用，不解释 rollout 内部结构。

## 8. 错误与状态

CodexDDSAgent 至少区分以下状态：

~~~text
stopped
starting
running
reconnecting
error
stopping
~~~

错误类型覆盖：

- 启动参数无效；
- 可执行文件缺失；
- 端口占用；
- 子进程启动失败；
- 输出地址解析失败；
- WebSocket 连接或握手失败；
- Codex 请求错误；
- 请求超时；
- 连接关闭；
- 内部通讯错误。

所有状态变化通过 Zenoh 发布给 VisionHyperAgent，并写入日志。

## 9. 版本维护

1. 固定 `source/depends/codex` 子模块版本。
2. 以该版本协议定义重新核对请求、反向请求和通知数量。
3. 更新握手能力和实验能力标记。
4. 为新增能力补充类型化接口和测试。
5. 对移除能力保留编译期错误，禁止静默兼容。

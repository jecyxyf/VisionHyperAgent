# CodexDDSAgent 模块边界

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-16 |
| 状态 | 多 Agent 实例服务方案已确认；本阶段只实现 CodexDDSAgent |
| 源码位置 | source/CodexDDSAgent/ |
| 协议来源 | source/depends/codex，提交 6b9826e3aa83b1a5947db50f4332cb9c65f1b340，标签 rust-v0.154.0 |

## 1. 目标与边界

CodexDDSAgent 是独立 Agent 服务进程。它管理局域网内的一个服务节点和多个 Agent 实例，并把每个 Agent 实例对应的 codex-app-server WebSocket 协议完整桥接到 Zenoh。

~~~text
CodexDDSAgent 可执行程序
├── 本机管理 CLI
└── 服务进程
    ├── Zenoh peer / 局域网发现
    ├── AgentInstance A
    │   ├── 独立请求队列
    │   ├── 独立 WebSocket
    │   └── 独立 codex-app-server
    ├── AgentInstance B
    └── AgentInstance N
~~~

CodexDDSAgent 负责：

- 本机创建、启动、关闭服务；
- 登记服务和 Agent 实例；
- 局域网服务发现；
- 创建、绑定、心跳、释放 Agent 实例；
- 在 Agent 初始化阶段生成独立的 URL / API key / model 接入配置；
- 为每个 Agent 实例启动独立 codex-app-server；
- 建立 WebSocket 连接并完成 initialize / initialized 握手；
- 按策略全量转发 Codex 客户端请求、服务端通知和反向请求；
- 发布服务与 Agent 状态；
- 处理队列、互斥、超时、重连和错误映射。

CodexDDSAgent 不负责：

- 预标注、标注、预训练、训练、推理和视觉业务模型管理；
- 保存视觉业务事实或用户确认状态；
- 自研第二套 Codex 会话数据库；
- 直接执行训练、推理或业务 Python 脚本；
- 替代用户做命令、文件修改或权限审批；
- 向界面暴露原始 WebSocket；
- 修改 Codex 子模块源码；
- 提供 ChatGPT 或 Bedrock 账号登录流程，也不通过 account/login/start 管理 API key；
- 提供远程删除服务或远程修改配置的能力。

本阶段不实现桌面应用。Python / QML、VisionHyperAgent 主程序接入方式属于后续阶段。

## 2. 决策原则

1. CodexDDSAgent 只做桥接和实例管理，不加入视觉业务逻辑。
2. 简单实现问题采用当前文档内的默认方案，不逐条向用户确认。
3. 只有破坏性操作、架构边界变化、协议不兼容和无法回退的数据行为才需要用户确认。
4. Codex 原生能力优先；CodexDDSAgent 不复制 Codex 已经提供的功能。
5. 名称是稳定身份，IP 和端口只是本次运行地址。
6. 删除登记、删除 CODEX_HOME、关闭服务只允许本机执行。

## 3. 模块分组

| 模块 | 职责 | 输入 | 输出 |
| --- | --- | --- | --- |
| LocalCli | 本机创建、启动、关闭、遍历、删除和配置管理 | 命令行参数 | 操作结果 |
| RegistryStore | 保存服务与 Agent 登记 | 登记、状态变更 | 登记记录 |
| DiscoveryService | Zenoh peer 发现和服务信息应答 | 服务发现查询 | 服务地址与状态 |
| ServiceManager | 服务生命周期、端口监听、名称冲突处理 | 服务配置 | 服务状态 |
| InstanceManager | Agent 创建、绑定、心跳、释放和删除 | 生命周期请求 | Agent 状态 |
| ModelConfigStore | 生成并锁定每个 Agent 的模型接入配置 | URL、API key、model | CODEX_HOME/config.toml |
| ProcessSupervisor | 每个 Agent 独立守护 codex-app-server | 启动参数 | 进程状态 |
| WebSocketTransport | 每个 Agent 独立 WebSocket 收发和重连 | 内部协议消息 | WebSocket 消息 |
| HandshakeController | initialize / initialized 握手 | 连接成功事件 | 协议可用状态 |
| ProtocolRegistry | 引用固定版本 Codex 协议类型并校验消息 | 原始 JSON | 类型化消息 |
| RequestQueue | Agent 内 FIFO 串行队列和跨 Agent 并行调度 | Zenoh 请求 | WebSocket 请求与响应 |
| NotificationBus | 服务端通知转 Zenoh 事件 | WebSocket notification | Zenoh publication |
| ReverseRequestBroker | 反向请求转发和结果回写 | WebSocket request | Zenoh 请求与 WebSocket response |
| StatusReporter | 汇总发布服务、进程、WebSocket、Agent 状态 | 各模块状态 | status key |

模块只承担上述职责，不得互相嵌入业务规则。

## 4. 目录与数据隔离

发布目录：

~~~text
CodexDDSAgent/
├── CodexDDSAgent
├── agents/
│   ├── codex-app-server
│   └── logs/
└── data/
    ├── registry.db
    └── codex/
        ├── agent-a/
        │   └── config.toml
        ├── agent-b/
        │   └── config.toml
        └── agent-c/
            └── config.toml
~~~

规则：

1. agents/codex-app-server 是唯一内置 Codex 服务程序。
2. 不从 PATH 查找外部可执行文件，不接受用户配置外部 Codex 程序。
3. data/registry.db 是单文件嵌入式登记数据库。
4. data/codex/{agent_name}/ 是该 Agent 独立的 CODEX_HOME。
5. data/codex/{agent_name}/config.toml 保存该 Agent 的模型接入配置和 API key。
6. config.toml 必须以 0600 权限创建，API key 明文写入该文件。
7. agents/logs/{agent_name}/ 保存该 Agent 对应子进程日志。
8. Agent 实例之间的队列、WebSocket、进程、通知、反向请求、CODEX_HOME 和模型接入配置完全隔离。
9. 心跳超时或 detach 只停止 runtime，不删除登记信息、CODEX_HOME 和模型接入配置。
10. 删除 Agent 时同时删除登记记录和对应 CODEX_HOME，操作不可恢复。

## 5. 名称与地址

### 5.1 名称规则

service_name 和 agent_name 都由用户输入。

~~~text
允许：字母、数字、下划线、中横线
长度：1～64
禁止：空格、中文、斜杠、冒号和其他路径字符
~~~

- service_name 在线局域网内唯一，创建后不可修改。
- agent_name 在服务内唯一，创建后不可修改。
- 不自动生成 agent_name。
- 不提供 rename。
- 已存在 agent_name 只能 attach，不能通过 create 抢占。

### 5.2 地址规则

稳定身份是：

~~~text
service_name / agent_name
~~~

IP 和端口只是本次运行连接地址。端口每次启动可以变化。桌面应用本地保存 service_name + agent_name，重连时优先 attach 原实例；找不到原服务或实例时，才提示选择或创建。

### 5.3 服务名冲突

没有中心数据库，只保证在线服务名唯一。

1. 创建时搜索局域网在线服务，重名则创建失败。
2. 两个同名服务可以离线创建，网络恢复后发现冲突。
3. created_at 较新的服务进入 name_conflict。
4. 较旧服务继续可用。
5. created_at 相同时，使用登记中的随机 conflict_id 决胜。
6. name_conflict 服务不提供 Agent 连接，只能本机删除登记后换名重建。

## 6. 登记数据

registry.db 保存长期登记，不代表 runtime 正在运行。

### 6.1 服务表

| 字段 | 说明 |
| --- | --- |
| service_name | 主键 |
| created_at | 创建时间 |
| config_json | 服务配置 |
| last_started_at | 最近启动时间 |
| last_listen_port | 最近监听端口 |
| conflict_id | 服务名冲突决胜值 |
| state | 服务登记状态 |

### 6.2 Agent 表

| 字段 | 说明 |
| --- | --- |
| service_name | 联合主键第一段 |
| agent_name | 联合主键第二段 |
| created_at | 创建时间 |
| last_attached_at | 最近绑定时间 |
| last_stopped_at | 最近停止时间 |
| state | Agent 登记状态 |
| model_config_initialized | 模型配置是否已写入并锁定 |

不保存：

- 桌面身份；
- 来源 IP；
- Codex 会话内容；
- 视觉业务数据；
- API key；API key 只写入对应 Agent 的 CODEX_HOME/config.toml。

## 7. 配置

### 7.1 启动配置

| 配置 | 默认值 | 说明 |
| --- | --- | --- |
| listen_host | 0.0.0.0 | Zenoh 监听地址 |
| port_mode | auto | auto 或 fixed |
| port | 无 | fixed 模式使用的端口 |
| port_range | 内置默认范围 | auto 模式选端口范围 |
| discovery_enabled | true | 是否启用局域网发现 |

### 7.2 运行配置

| 配置 | 默认值 |
| --- | --- |
| heartbeat_interval_ms | 1000 |
| heartbeat_timeout_ms | 5000 |

不提供 log_level、log_retention_days、最大 Agent 数、是否允许新建 Agent。

规则：

- 配置只能本机 CLI 读写。
- 局域网只能读取服务发现信息。
- 配置修改后重启服务生效。
- 端口和配置不是稳定身份。

## 8. 本机管理 CLI

~~~text
CodexDDSAgent service create --service-name NAME --agent-name NAME [配置参数]
CodexDDSAgent service start --service-name NAME
CodexDDSAgent service list
CodexDDSAgent service shutdown --service-name NAME

CodexDDSAgent agent list --service-name NAME
CodexDDSAgent agent delete --service-name NAME --agent-name NAME [--force]

CodexDDSAgent config get --service-name NAME
CodexDDSAgent config set --service-name NAME key=value
~~~

行为：

- service create：校验名称，写入服务与初始 Agent 登记，启动服务；初始 Agent 只登记，runtime 等待 attach。
- service start：启动已有登记服务，不创建新服务。
- service shutdown：停止全部 Agent runtime 和服务进程，登记保留。
- agent list：只遍历本机登记。
- agent delete：仅本机可删除；活跃实例默认拒绝；--force 先停止再删除。
- 服务和实例不支持局域网远程删除。
- 服务异常退出或电脑重启后不自动恢复，由本机 service start 恢复。

## 9. 局域网服务发现

使用 Zenoh peer 模式和自带 multicast scouting，不自研 UDP 发现协议。

查询：

~~~text
codex-dds/v1/service/info
~~~

模式：request/reply。

返回：

~~~json
{
  "version": 1,
  "protocol_version": 1,
  "service_name": "main-studio",
  "host": "192.168.2.10",
  "port": 18744,
  "state": "running"
}
~~~

返回内容只包含连接服务所需信息。不返回 Agent 列表、会话信息、本机路径、配置详情。

服务和实例的管理遍历、删除、关闭仅限本机 CLI。

## 10. Agent 生命周期

### 10.1 接口

| Key | 模式 |
| --- | --- |
| codex-dds/v1/{service_name}/agent/{agent_name}/create | request/reply |
| codex-dds/v1/{service_name}/agent/{agent_name}/attach | request/reply |
| codex-dds/v1/{service_name}/agent/{agent_name}/detach | request/reply |
| codex-dds/v1/{service_name}/agent/{agent_name}/heartbeat | request/reply |

名称已经存在于 key 中，载荷不重复携带 service_name 和 agent_name。

### 10.2 create

create 只能创建新实例。同名实例存在时返回 agent_exists。

create 请求必须携带模型初始化配置：

~~~json
{
  "version": 1,
  "model": {
    "default_provider": "main",
    "providers": [
      {
        "id": "main",
        "base_url": "https://api.example.com/v1",
        "api_key": "example-key",
        "default_model": "model-a",
        "models": ["model-a", "model-b"]
      }
    ]
  }
}
~~~

模型配置校验规则：

1. providers 不能为空。
2. provider id 只允许字母、数字、下划线、中横线，长度 1～64。
3. provider id 在同一 Agent 内唯一。
4. base_url 必须是合法 URL。
5. api_key 不能为空。
6. models 不能为空，default_model 必须存在于 models。
7. default_provider 必须指向 providers 中的某个 id。

创建成功后模型配置立即写入该 Agent 的 config.toml 并锁定，后续不允许修改。

create 的模型配置必须先完整校验；校验失败时不写入 Agent 登记和 config.toml，同名 Agent 可以随后用有效配置重新 create。

成功响应：

~~~json
{
  "version": 1,
  "heartbeat_interval_ms": 1000,
  "heartbeat_timeout_ms": 5000,
  "model_config_locked": true,
  "state": "running"
}
~~~

创建成功即视为当前连接绑定，模型配置初始化完成，runtime 立即启动，登记信息长期保留。

### 10.3 attach

attach 只能绑定已存在实例：

- 不判断连接来源；
- 不做 owner_token；
- 已有活跃心跳时返回 agent_busy；
- 无活跃心跳时允许 attach；
- runtime 已停止时自动重启；
- 首次 attach 尚未初始化模型配置的登记实例时，必须携带与 create 相同结构的 model 配置；
- 模型配置一旦写入，后续 attach 不允许修改，携带不同配置返回 model_config_locked；
- 成功后按返回的心跳参数持续发送 heartbeat。

### 10.4 heartbeat

heartbeat 按 agent_name 刷新心跳计时。默认 1 秒一次，超过 5 秒未收到时：

1. 停止 Agent runtime；
2. 停止对应 codex-app-server；
3. 清空请求队列；
4. 未完成请求返回 agent_stopped；
5. 反向请求返回 agent_stopped；
6. 登记信息和 CODEX_HOME 保留。

### 10.5 detach

detach 表示主动释放连接：

- 停止 runtime；
- 停止 codex-app-server；
- 清空未执行队列；
- 登记信息和 CODEX_HOME 保留。

## 11. Runtime 与 codex-app-server

每个 Agent 实例拥有独立队列、WebSocket 和 codex-app-server。

启动规则：

1. 服务启动时只加载登记，不自动启动全部 Agent runtime。
2. create 或 attach 成功后启动 runtime。
3. CODEX_HOME 固定为 data/codex/{agent_name}。
4. codex-app-server 固定使用内置程序。
5. WebSocket 启动参数固定为 --listen ws://127.0.0.1:0。
6. 系统随机分配本机回环端口。
7. CodexDDSAgent 从子进程输出解析实际地址。
8. WebSocket 地址不暴露给外部。

启动流程：

~~~text
create / attach
→ 启动 codex-app-server
→ 解析 WebSocket 地址
→ 建立 WebSocket
→ initialize
→ initialized
→ Agent running
~~~

异常恢复：

- 心跳仍活跃时，codex-app-server 异常退出后自动重启，config.toml 中的模型配置保持不变。
- 自动重连 WebSocket 并重新握手。
- 重启期间队列暂停，不丢失已排队请求。
- 连续失败 5 次后停止自动重启，Agent 进入 error。
- 成功恢复后失败计数清零。
- 心跳超时或 detach 不做自动重启，直接停止 runtime。

## 12. 模型接入与账户策略

CodexDDSAgent 不支持账户登录。模型接入只在 Agent 初始化阶段配置，使用 URL、API key 和 model。

### 12.1 多模型接入

Codex 支持在一个 CODEX_HOME/config.toml 中定义多个 model provider。限制是：一个 thread 同时只能选择一个 provider 和一个 model。

规则：

1. 多组 URL / API key / model 可以同时配置。
2. 每个 thread 通过 thread/start 的 model_provider 和 model 选择一组接入。
3. 不传时使用 config.toml 中的默认 provider 和默认 model。
4. 同一个 thread 内不动态切换模型。
5. 需要并行使用多个模型时，创建多个 thread 或多个 Agent 实例。

### 12.2 config.toml 生成

ModelConfigStore 把 create 或首次 attach 传入的模型配置写入：

~~~text
data/codex/{agent_name}/config.toml
~~~

示例：

~~~toml
model_provider = "main"
model = "model-a"

[model_providers.main]
name = "main"
base_url = "https://api.example.com/v1"
experimental_bearer_token = "example-key"
requires_openai_auth = false
wire_api = "responses"

[model_providers.fast]
name = "fast"
base_url = "https://fast.example.com/v1"
experimental_bearer_token = "fast-key"
requires_openai_auth = false
wire_api = "responses"
~~~

固定规则：

1. API key 必须写入 config.toml。
2. Codex 字段名使用 experimental_bearer_token。
3. 不使用环境变量保存 API key。
4. 不使用 account/login/start 的 apiKey 登录流程。
5. 所有自定义 provider 固定 requires_openai_auth = false。
6. 当前 Codex 版本只支持 wire_api = "responses"，模型服务必须兼容 OpenAI Responses API。
7. config.toml 以 0600 权限保存。
8. API key 不写入 registry.db，不写入日志，不通过服务发现和状态接口返回。
9. 模型配置写入后锁定；更换配置只能删除 Agent 后重建。

### 12.3 禁用账户登录

以下请求由 CodexDDSAgent 直接拒绝，返回 disabled_by_policy，不转发给 codex-app-server：

~~~text
account/login/start
account/login/cancel
account/logout
account/bedrock/discover
account/bedrock/setup
account/sessions/*
~~~

反向请求 account/chatgptAuthTokens/refresh 同样直接返回 disabled_by_policy。

getAuthStatus、account/read、account/rateLimits/read、account/usage/read 等只读接口保持透传；在无账户登录模式下由 Codex 返回实际状态或错误。

## 13. initialize 握手

initialize 和 initialized 由 CodexDDSAgent 内部管理，不通过 Zenoh 暴露。每个 Agent 实例独立握手。

~~~json
{
  "clientInfo": {
    "name": "CodexDDSAgent",
    "title": "VisionHyperAgent Codex DDS Agent",
    "version": "内置版本号"
  },
  "capabilities": {
    "experimentalApi": true,
    "requestAttestation": true,
    "mcpServerOpenaiFormElicitation": true,
    "optOutNotificationMethods": null,
    "extensions": null
  }
}
~~~

每次 codex-app-server 重启后必须重新握手。

## 14. Zenoh 接口总表

| Key | 模式 | 方向 | 说明 |
| --- | --- | --- | --- |
| codex-dds/v1/service/info | request/reply | 客户端 → 服务 | 局域网服务发现 |
| codex-dds/v1/{service_name}/agent/{agent_name}/create | request/reply | 客户端 → 服务 | 创建 Agent |
| codex-dds/v1/{service_name}/agent/{agent_name}/attach | request/reply | 客户端 → 服务 | 绑定 Agent |
| codex-dds/v1/{service_name}/agent/{agent_name}/detach | request/reply | 客户端 → 服务 | 释放 Agent |
| codex-dds/v1/{service_name}/agent/{agent_name}/heartbeat | request/reply | 客户端 → 服务 | 心跳 |
| codex-dds/v1/{service_name}/status | pub/sub | 服务 → 客户端 | 服务状态变化 |
| codex-dds/v1/{service_name}/status/get | request/reply | 客户端 → 服务 | 服务状态快照 |
| codex-dds/v1/{service_name}/{agent_name}/rpc | request/reply | 客户端 → 服务 | 全部可透传 Codex 请求 |
| codex-dds/v1/{service_name}/{agent_name}/event | pub/sub | 服务 → 客户端 | 全部 Codex 服务端通知 |
| codex-dds/v1/{service_name}/{agent_name}/reverse/request | pub/sub | 服务 → 客户端 | 全部 Codex 反向请求 |
| codex-dds/v1/{service_name}/{agent_name}/reverse/response | request/reply | 客户端 → 服务 | 反向请求结果 |
| codex-dds/v1/{service_name}/{agent_name}/status | pub/sub | 服务 → 客户端 | Agent 状态变化 |
| codex-dds/v1/{service_name}/{agent_name}/status/get | request/reply | 客户端 → 服务 | Agent 状态快照 |

service_name 和 agent_name 只放在 key 中，载荷不重复携带。唯一例外是 service/info 的返回值必须包含 service_name，因为查询 key 不包含具体服务名。

## 15. RPC 载荷与请求队列

### 15.1 请求

~~~json
{
  "version": 1,
  "request_id": "uuid",
  "method": "thread/start",
  "params": {}
}
~~~

### 15.2 成功响应

~~~json
{
  "version": 1,
  "request_id": "uuid",
  "ok": true,
  "result": {}
}
~~~

### 15.3 失败响应

~~~json
{
  "version": 1,
  "request_id": "uuid",
  "ok": false,
  "error": {
    "code": "Codex 原始错误码",
    "message": "error message",
    "data": {}
  }
}
~~~

规则：

1. 保留 Codex 原始 method 和 params。
2. 不按 Codex 方法拆 Zenoh key。
3. 除 initialize、initialized 和账户登录相关禁用方法外，其余客户端请求按策略透传。
4. 外层不新增 timeout_ms。
5. Codex 自己的 timeout 参数和默认超时规则原样生效。
6. Zenoh 调用方本地超时不导致请求取消。
7. 调用方不再接收结果时，结果丢弃并写日志。
8. request_id 必须由调用方生成，服务只做回填和关联。

### 15.4 队列与互斥

- 每个 Agent 一个 FIFO 队列。
- 同一 Agent 内串行执行。
- 不同 Agent 完全并行。
- 队列等待不超时。
- 队列不设容量上限。
- 不提供通用取消接口。
- runtime 停止时清空队列，未执行请求返回 agent_stopped。
- 中断行为走 Codex 原生 turn/interrupt、process 或 command 相关方法。
- 入队通道本身不设 1024 等固定容量；等待中的请求只受 runtime 停止影响。

## 16. 服务端通知

全部通知发布到：

~~~text
codex-dds/v1/{service_name}/{agent_name}/event
~~~

载荷：

~~~json
{
  "version": 1,
  "event_id": "uuid",
  "method": "turn/started",
  "params": {},
  "received_at": "2026-09-16T00:00:00Z"
}
~~~

规则：

1. 固定版本 81 个服务端通知全量转发。
2. 不合并、不节流、不解释业务含义。
3. 不同 Agent 事件互不转发。
4. 不保存通知历史。
5. 断线期间通知不补发。
6. 重连后通过 Codex 历史接口恢复上下文。

## 17. 会话历史与提示词队列

CodexDDSAgent 不自建会话数据库，也不复制提示词队列。主程序通过原生 RPC 访问 Codex 已有能力。

历史相关方法：

- thread/list；
- thread/read；
- thread/turns/list；
- thread/items/list；
- thread/timeline/list。

提示词队列直接使用 Codex 原生 thread queue：

- 新增：thread/queue/add；
- 查询：thread/queue/list；
- 修改：thread/queue/update；
- 删除：thread/queue/delete；
- 排序：thread/queue/reorder；
- 启动：thread/queue/start；
- 变化通知：thread/queue/changed；
- 清空：调用方先 list，再逐条 delete。

CodexDDSAgent 不为这些方法增加额外语义。

## 18. 反向请求

固定版本共有 11 个 Codex 反向请求，全部桥接：

| 功能 | WebSocket method |
| --- | --- |
| 命令审批 | item/commandExecution/requestApproval |
| 文件修改审批 | item/fileChange/requestApproval |
| 工具用户输入 | item/tool/requestUserInput |
| MCP 补充信息 | mcpServer/elicitation/request |
| 权限审批 | item/permissions/requestApproval |
| 动态工具调用 | item/tool/call |
| 凭证刷新 | account/chatgptAuthTokens/refresh |
| 证明生成 | attestation/generate |
| 当前时间读取 | currentTime/read |
| 遗留补丁审批 | applyPatchApproval |
| 遗留命令审批 | execCommandApproval |

流程：

~~~text
codex-app-server 发起 WebSocket request
→ CodexDDSAgent 生成 reverse_id
→ 发布 reverse/request
→ 客户端展示审批或输入界面
→ 客户端调用 reverse/response
→ CodexDDSAgent 映射回 Codex 原 id
→ WebSocket response 返回 codex-app-server
~~~

规则：

1. 同一 Agent 内一次只处理一个反向请求。
2. 不同 Agent 的反向请求互不影响。
3. 不自动审批、不自动补输入。
4. CodexDDSAgent 不增加反向请求外层超时，等待规则遵循 Codex 内部默认规则。
5. runtime 停止时反向请求立即失败。
6. reverse_id 不存在或已处理时返回 not_found或 invalid_state。

## 19. 状态协议

服务状态：

~~~text
stopped / starting / running / error / stopping / name_conflict
~~~

Agent 状态：

~~~text
registered / starting / running / stopping / stopped / error
~~~

Agent 状态快照包含：

- runtime 状态；
- WebSocket 状态；
- codex-app-server 状态；
- 心跳是否活跃；
- 是否正在处理请求；
- 当前错误。

示例：

~~~json
{
  "version": 1,
  "service_name": "main-studio",
  "agent_name": "desktop-a",
  "state": "running",
  "websocket_state": "connected",
  "codex_process_state": "running",
  "heartbeat_active": true,
  "model_config_initialized": true,
  "processing_request": false,
  "failure_count": 0,
  "error": null,
  "changed_at": "2026-09-16T00:00:00Z"
}
~~~

状态变化发布到对应 status key，也可通过 status/get 获取当前快照。服务状态载荷不包含 Agent 列表。

## 20. 协议完整性

固定 Codex 版本协议规模：

| 类型 | 数量 |
| --- | ---: |
| 客户端请求 | 159 |
| 客户端通知 | 1 |
| Codex 反向请求 | 11 |
| 服务端通知 | 81 |

客户端请求功能分组：

| 功能组 | 数量 | WebSocket 方法范围 | Zenoh 协议 |
| --- | ---: | --- | --- |
| 系统与用户验证 | 7 | initialize；server/diagnostics；userVerification/*；mock/experimentalMethod | rpc，method + params 原样传递 |
| 会话与历史 | 37 | thread/*，排除 thread/queue/*、thread/realtime/*、thread/timeline/list；threadSection/* | rpc |
| 队列、项目与记忆 | 14 | thread/queue/*；project/*；memory/reset | rpc |
| Turn 与实时语音 | 12 | turn/*；thread/realtime/*；thread/timeline/list；review/start | rpc |
| 技能、插件与应用 | 23 | skills/*；hooks/list；marketplace/*；plugin/*；app/* | rpc |
| 文件、命令、进程与搜索 | 21 | fs/*；command/*；process/*；fuzzyFileSearch* | rpc |
| 模型、实验能力与远程环境 | 16 | model/*；modelProvider/*；experimentalFeature/*；permissionProfile/*；remoteControl/*；collaborationMode/list；environment/* | rpc |
| MCP 与 Windows 沙箱 | 9 | mcpServer/*；mcpServerStatus/*；config/mcpServer/reload；windowsSandbox/* | rpc |
| 账号、配置与遗留能力 | 20 | account/*；feedback/upload；config/*，排除 config/mcpServer/reload；configRequirements/read；externalAgentConfig/* | rpc |

特殊规则：

1. initialize 由 HandshakeController 内部调用，不对外透传。
2. initialized 是唯一客户端通知，也由内部握手流程发送。
3. 除账户登录相关禁用方法外，其余可透传客户端请求全部通过 rpc 传递。
4. ProtocolRegistry 必须引用 codex-app-server-protocol 生成的类型，不维护手写字符串白名单。
5. 构建测试必须核对请求 159、反向请求 11、通知 81。
6. Codex 子模块版本变化时重新核对并更新本文。

## 21. 错误映射

| 来源 | 错误码 |
| --- | --- |
| 名称格式错误 | invalid_name |
| 服务不存在 | service_not_found |
| Agent 不存在 | agent_not_found |
| Agent 已存在 | agent_exists |
| Agent 心跳活跃 | agent_busy |
| runtime 已停止 | agent_stopped |
| 服务名冲突 | name_conflict |
| 模型配置已锁定 | model_config_locked |
| 模型配置校验失败 | invalid_model_config |
| 账户登录能力被禁用 | disabled_by_policy |
| 请求参数无法反序列化 | invalid_request |
| Codex 方法不属于固定协议版本 | unknown_method |
| Codex 返回错误 | 原样映射 Codex code / message / data |
| WebSocket 断开 | connection_closed |
| codex-app-server 启动失败 | process_start_failed |
| 监听地址解析失败 | server_address_invalid |
| CodexDDSAgent 内部错误 | internal_error |

错误不吞掉。无法确定错误码时使用 internal_error，并在日志中保留原始上下文。

## 22. 验收与实施顺序

验收要求：

1. 本机 CLI 可以完成服务创建、启动、遍历、关闭、配置读写和 Agent 删除。
2. Zenoh 可以发现在线服务并获取可连接地址。
3. 多个 Agent 实例可以同时运行且互不串扰。
4. 同一 Agent 请求串行，不同 Agent 请求并行。
5. 心跳 5 秒丢失后 runtime 立即销毁，登记和模型配置保留。
6. codex-app-server 异常退出后在心跳活跃时自动恢复。
7. 除 initialize / initialized 和账户登录禁用方法外，Codex 协议按策略全量透传。
8. 服务端通知和反向请求不丢失 method、params 和错误信息。
9. 构建测试核对协议数量。
10. 删除行为仅限本机，并符合登记与 CODEX_HOME 处理规则。

建议实施顺序：

1. 目录、登记数据库和本机 CLI；
2. Zenoh 服务发现与状态；
3. Agent 生命周期、心跳和模型配置生成；
4. codex-app-server 进程管理、WebSocket 与握手；
5. RPC 队列和错误映射；
6. 服务端通知；
7. 反向请求；
8. 协议数量测试与多实例集成测试。

具体实施步骤仍由用户逐段指挥。

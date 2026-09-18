# VisionHyperAgent 架构基线

| 项目 | 内容 |
|------|------|
| 日期 | 2026-09-16 |
| 状态 | Web 架构已确认 |
| 核心决策 | 浏览器前端 + Rust 本地后端，放弃 PySide6/QML 桌面方案 |
| 当前实现 | Agent 前后端模块与接口详见 [frontend-backend-architecture.md](frontend-backend-architecture.md) |

## 1. 产品范围

VisionHyperAgent 是 Agent 驱动的视觉模型桌面软件，五个核心流程：

1. **预标注** — 用户描述目标 → Agent 追问并生成标注规则 → 用户确认
2. **预训练** — 少量已确认数据训练辅助初标模型
3. **标注** — Canvas 2D 实例分割标注，支持画笔/多边形/擦除
4. **训练** — 本机 NVIDIA GPU，Agent 制定调参方案
5. **离线部署** — 导出模型离线推理，不依赖在线 Agent

## 2. 总体架构

~~~
┌─────────────────────────────────────────────────────┐
│                    浏览器（前端）                      │
│                                                     │
│  Svelte + TypeScript + Vite                         │
│  ├── 标注画布（Canvas 2D，非 Three.js）               │
│  ├── Agent 对话面板                                  │
│  ├── 训练监控图表                                    │
│  └── 模型/项目管理                                   │
└────────────────────┬────────────────────────────────┘
                     │ WebSocket（实时） + HTTP（REST）
                     │ ws://127.0.0.1:8420/ws
┌────────────────────▼────────────────────────────────┐
│                  Rust 本地后端进程                     │
│                                                     │
│  axum HTTP + WebSocket Server                       │
│  ├── 静态文件服务（打包前端产物）                       │
│  ├── REST API（项目/模型/配置 CRUD）                  │
│  ├── WebSocket（实时事件推送 + 命令下发）              │
│  └── 会话生命周期管理                                  │
│                                                     │
│  ┌───────────────────────────────────────────────┐  │
│  │ Rust Core                                     │  │
│  │ (配置 / 日志 / 事件总线 / 任务运行时)              │  │
│  └───────────────────────────────────────────────┘  │
│                                                     │
│  ┌───────────────────────────────────────────────┐  │
│  │ Rust Model                                     │  │
│  │ ├── CodexAgent（WebSocket → Codex App Server）  │  │
│  │ ├── 标注引擎（遮罩/多边形/边缘检测）               │  │
│  │ ├── 训练调度（GPU 任务队列）                      │  │
│  │ └── 推理引擎（离线模型执行）                       │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
~~~

### 2.1 全局日志

后端公共日志库位于 `source/backend/common`，由宿主进程在所有业务初始化之前加载。日志使用进程级全局单例，输出到可执行文件所在目录：

~~~text
<应用程序目录>/logs/VisionHyperAgent.YYYY-MM-DD.log
~~~

格式：

~~~text
YYYY-MM-DD HH:mm:ss.SSS [级别] [模块:行号] 日志内容
~~~

`DEBUG`、`WARNING`、`ERROR` 记录 `模块:行号`，`INFO` 不记录来源位置。默认 release 记录 `INFO`，debug 构建记录 `DEBUG`，可用环境变量 `VHA_LOG_LEVEL=error|warning|info|debug` 临时调整。日志不记录用户消息、附件内容和密钥。

## 3. 前后端通信

### 3.1 HTTP REST（低频操作）

| 端点 | 方法 | 说明 |
|------|------|------|
| /api/projects | GET/POST | 项目列表/创建 |
| /api/models | GET | 模型列表 |
| /api/config | GET/PUT | 配置读写 |

### 3.2 WebSocket（实时）

前端与后端保持一条 WebSocket 连接：

- 前端心跳：每 10s 发送 `{"type":"ping"}`
- 后端推送：Agent 消息增量、训练进度、标注状态变更
- 前端命令：创建会话、发送指令、启动训练

### 3.3 生命周期

~~~
启动 → HTTP 端口绑定成功 → 创建系统托盘 → 自动打开浏览器
空闲 → 浏览器标签关闭 → HTTP 连接断开 → 后端继续运行 → 托盘仍存在
退出 → 托盘右键“退出” → HTTP server 优雅停止 → 托盘移除 → 进程退出
~~~

浏览器只是显示层，不参与进程生命周期管理。服务退出后，残留的浏览器标签页由用户自行处理。

## 4. 目录结构

~~~
VisionHyperAgent/
├── docs/                          # 架构文档
├── source/frontend/                      # TypeScript Web 前端
│   ├── package.json
│   ├── vite.config.ts
│   └── src/
│       ├── lib/
│       │   ├── api/               # HTTP 客户端
│       │   ├── canvas/            # Canvas 2D 标注引擎
│       │   │   ├── layers/        # 图层（底图/遮罩/绘制/叠加）
│       │   │   └── tools/         # 工具（画笔/多边形/擦除）
│       │   └── stores/            # Svelte 状态管理
│       ├── components/
│       │   ├── chat/              # Agent 对话
│       │   ├── annotation/        # 标注界面
│       │   ├── training/          # 训练监控
│       │   └── common/            # 通用 UI
│       └── pages/                 # 页面
├── source/backend/                       # Rust 后端
│   ├── Cargo.toml                 # Workspace
│   ├── common/                    # 全局日志等公共基础设施
│   ├── server/                    # axum HTTP/WS 服务器与系统托盘
│   ├── core/                      # 配置/事件总线
│   └── model/
│       └── codex_agent/           # Codex App Server WebSocket 客户端
└── bin/                           # 编译产物
~~~

## 5. 技术选型依据

| 选择 | 原因 |
|------|------|
| Svelte 而非 React | 编译时框架，产物小，无 Virtual DOM 开销 |
| Canvas 2D 而非 Three.js | 标注是 2D 任务，Three.js 是 3D 引擎，杀鸡用牛刀 |
| Rust axum 而非 Python | 类型安全、并发模型清晰、单二进制分发 |
| WebSocket 而非轮询 | Agent 消息流式推送、训练进度实时更新 |
| 本地后端而非云服务 | GPU 训练必须在本地，数据不出机器 |

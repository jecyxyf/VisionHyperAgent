# VisionHyperAgent 架构基线

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-15 |
| 状态 | 两进程架构与 CodexDDSAgent 生命周期已确认，具体接口和实现由用户逐段指挥 |
| 文档目的 | 固定产品范围、进程边界、MVVM 分层、目录结构和运行路径 |
| 实施方式 | 本文只描述已确认结论，不作为自动执行计划 |

## 1. 产品范围

VisionHyperAgent 是 Agent 驱动的视觉模型桌面软件。五个核心流程保持不变：

1. **预标注**：用户描述识别目标、外观特征、类别区别和排除情况；Agent 分析歧义、追问缺失信息并整理标注规则；用户确认后规则才可用于批量标注。描述变化后，旧分析和确认状态失效。
2. **预训练**：直接预标注效果不足时，使用少量已确认数据训练辅助初标模型，再用该模型辅助批量标注。预训练是少量数据的种子模型训练，不是外部大规模预训练。
3. **标注**：按已确认规则和可用辅助模型生成实例分割初标；用户逐图修改、删除和补充实例区域；确认后形成可训练的数据集版本。矩形框不能替代实例分割区域。
4. **训练**：基于已确认的数据集版本发起训练；Agent 制定训练与调参方案；训练在本机 NVIDIA GPU 执行；多轮评估后登记最佳候选模型。训练计算不上云。
5. **离线部署**：使用导出模型执行识别，不依赖在线 Agent，也不依赖训练进程。

相机、外部图片通信、模型库和运行页不在当前确认范围内。

## 2. 总体架构

系统由两个产品级进程组成：

~~~text
VisionHyperAgent 进程
├── Python + QML
│   ├── View
│   └── ViewModel
├── Rust
│   ├── ViewModel
│   ├── Model
│   └── Common
│
CodexDDSAgent 进程
├── Zenoh 服务端
├── codex-app-server 子进程管理
└── WebSocket / JSON-RPC 适配
~~~

两个进程只在同一台主机上通讯：

~~~text
QML View
↔ Python ViewModel
↔ Rust ViewModel
↔ Rust Model
↔ Rust Common
↔ Zenoh 客户端
↔ CodexDDSAgent
↔ codex-app-server
~~~

### 2.1 VisionHyperAgent

VisionHyperAgent 是桌面主进程，包含界面和核心业务：

- 启动并守护 CodexDDSAgent；
- 保存用户配置、业务数据、任务状态和界面状态；
- 执行预标注、标注、预训练、训练、评估、模型登记和离线推理；
- 通过 Rust 层的 Zenoh 客户端访问 CodexDDSAgent；
- 不直接管理 codex-app-server，不直接建立 WebSocket 连接。

### 2.2 CodexDDSAgent

CodexDDSAgent 是独立的 Agent 服务进程：

- 启动、停止和守护内置 codex-app-server；
- 建立 WebSocket / JSON-RPC 连接；
- 对 VisionHyperAgent 暴露 Zenoh 服务；
- 转发 Codex 请求、响应和通知；
- 管理 Agent 服务自身的重连和恢复。

CodexDDSAgent 不承载视觉业务，不保存预标注、标注、训练、模型和部署事实。

## 3. MVVM 分层

| 层 | 技术 | 职责 | 禁止事项 |
| --- | --- | --- | --- |
| View | QML / PySide6 | 显示、输入、交互、动画 | 不调用业务、数据库、Zenoh、Codex |
| Python ViewModel | Python / Qt | QObject、Property、Signal、Slot，向 QML 暴露状态和命令 | 不实现业务规则和 Agent 协议 |
| Rust ViewModel | Rust | 页面状态、表单状态、命令编排、异步结果整理 | 不依赖 Qt、PySide6、QML |
| Rust Model | Rust | 业务事实、用例、任务、数据版本、Agent 服务编排 | 不依赖界面控件 |
| Rust Common | Rust | 配置、日志、路径、错误、事件、进程组清理 | 不包含业务事实 |

Python ViewModel 与 Rust ViewModel 之间的绑定层只做类型转换、回调转发和生命周期管理，不做业务。

## 4. 进程生命周期

~~~text
启动 VisionHyperAgent
├── 初始化 Python / QML
├── 初始化 Rust Common / Model / ViewModel
├── 读取配置中的 CodexDDSAgent 端口
├── 启动 CodexDDSAgent 子进程
└── 通过 Zenoh 连接 CodexDDSAgent
~~~

~~~text
退出 VisionHyperAgent
├── 断开 Zenoh
├── 停止 CodexDDSAgent
│   ├── 停止 WebSocket
│   └── 停止 codex-app-server
└── 退出主进程
~~~

### 4.1 端口

- Zenoh 监听地址固定为本机回环。
- 默认地址：`tcp/127.0.0.1:18744`。
- 端口保存在 VisionHyperAgent 配置中。
- 主程序启动 CodexDDSAgent 时把端口作为启动参数传入。
- CodexDDSAgent 不自行保存端口配置。
- 端口冲突导致启动失败时，主程序进入错误状态，由用户修改配置后重试。

### 4.2 守护策略

- VisionHyperAgent 正常退出时停止 CodexDDSAgent。
- CodexDDSAgent 异常退出后，VisionHyperAgent 延迟 2 秒自动重启。
- 连续失败 5 次后停止自动重启，并进入界面可见的错误状态。
- 自动重启成功一次后，连续失败计数清零。
- codex-app-server 异常退出后，由 CodexDDSAgent 自动重启并重新握手。
- 三层进程纳入统一进程组管理，避免产生孤儿进程。

## 5. 安装目录与路径

发布包采用便携目录结构：

~~~text
VisionHyperAgent/
├── VisionHyperAgent              # 主程序入口
│
└── CodexDDSAgent/
    ├── CodexDDSAgent             # Agent 服务程序
    └── agents/                   # CODEX_HOME
        ├── codex-app-server      # 内置 Agent Server
        └── logs/
~~~

路径规则：

- `CODEX_HOME` 固定为 `CodexDDSAgent/agents`。
- codex-app-server 固定使用内置文件 `CodexDDSAgent/agents/codex-app-server`。
- 不从 `PATH` 查找 codex-app-server。
- 不允许用户把 codex-app-server 配置为任意外部程序。
- VisionHyperAgent 不直接读写 `agents/` 内部会话数据。
- 业务数据、用户配置、缓存和工作区路径由 Rust Common 解析，不硬编码开发机绝对路径。

## 6. 源码目录

~~~text
source/
├── VisionHyperAgent/
│   ├── Python/
│   │   ├── main.py             应用入口
│   │   ├── view/               QML View
│   │   ├── viewmodel/          Python ViewModel
│   │   └── pybind/             Python / Rust 绑定
│   └── Rust/
│       ├── Cargo.toml          Rust workspace
│       ├── common/             共用组件集合
│       ├── viewmodel/          Rust ViewModel
│       └── model/              业务模型集合
│
├── CodexDDSAgent/             Agent 服务工程
└── depends/                   固定版本上游子模块
~~~

目录规则：

- `common` 是配置、日志、路径、错误、事件和进程管理等共用组件集合。
- `model` 只放业务子模块，自身不建立 `src`。
- `source/CodexDDSAgent` 是 CodexDDSAgent 的独立 Rust 服务工程。
- `depends` 只保留需要固定源码或联调的上游子模块；普通库依赖由 Cargo 配置和锁文件管理。
- Python 目录只保存界面、Python ViewModel 和绑定层，不放视觉业务实现。

## 7. 通讯边界

- VisionHyperAgent 与 CodexDDSAgent 之间只使用 Zenoh。
- Zenoh 客户端只存在于 VisionHyperAgent 的 Rust 层。
- Python 和 QML 不直接访问 Zenoh、WebSocket 或 codex-app-server。
- Rust Model 不解析 QML 状态，Python ViewModel 不解析 Codex 协议。
- Codex 会话由 Codex 在 `CODEX_HOME` 中管理；VisionHyperAgent 只通过 CodexDDSAgent 驱动恢复、列表和继续会话，不自研第二套会话数据库。

## 8. 核心流程

### 8.1 预标注

~~~text
QML 输入描述
→ Python ViewModel
→ Rust ViewModel
→ Rust Model
→ Zenoh 调用 CodexDDSAgent
→ Agent 分析和追问
→ Rust Model 校验并登记结果
→ 用户确认规则
→ 规则可用于批量标注
~~~

### 8.2 预训练

~~~text
直接预标注效果不足
→ 用户确认少量种子数据
→ Rust Model 训练辅助初标模型
→ 评估并登记
→ 后续批量标注复用
~~~

### 8.3 标注

~~~text
Rust Model 基于确认规则和辅助模型生成实例分割初标
→ QML 展示并允许修改、删除、补充
→ Rust Model 保存实例区域
→ 用户确认数据集版本
~~~

### 8.4 训练

~~~text
QML 发起训练
→ Rust Model 校验数据版本、GPU 和环境
→ Agent 提供训练与调参方案
→ Rust Model 校验方案并执行训练
→ 记录进度、指标和产物
→ 登记候选模型并选择最佳模型
~~~

### 8.5 离线部署

~~~text
Rust Model 加载导出模型
→ 推理线程执行预处理、模型推理和实例分割后处理
→ ViewModel 展示类别与分割区域
~~~

离线推理不初始化在线 Agent，不依赖训练环境。

## 9. 线程模型

| 进程 / 线程 | 职责 |
| --- | --- |
| Qt 主线程 | QML 渲染、界面事件、ViewModel 属性更新 |
| Rust ViewModel 线程 | 状态计算、命令编排、回调转发 |
| Rust 任务调度线程 | 业务任务队列、取消、超时、状态发布 |
| 训练 / 推理线程 | 预训练、训练、评估和离线推理 |
| CodexDDSAgent 服务线程 | Zenoh 服务、请求路由、事件转发 |
| codex-app-server 管理线程 | 子进程启动、输出解析、崩溃恢复 |

约束：

- Qt 主线程不做文件扫描、模型计算、网络等待或长时间业务处理。
- Python 与 Rust 的回调必须显式回到 Qt 主线程。
- 长任务必须可查询、可取消并有明确终态。
- 子线程异常必须进入统一错误和日志系统，不能静默吞掉。

## 10. 实施顺序

1. Rust workspace、Common 基础类型和路径规则；
2. 目录、绑定层和主窗口状态打通；
3. VisionHyperAgent 启动并守护 CodexDDSAgent；
4. CodexDDSAgent 的 Zenoh 服务和基础状态接口；
5. codex-app-server 子进程管理与 WebSocket 握手；
6. 预标注规则用例；
7. 数据集与标注模型；
8. 预训练与训练模块；
9. 推理、模型登记和离线部署；
10. 打包与进程清理验证。

具体实施步骤仍由用户逐段指挥。

## 11. 非目标

- 不再回到单进程 CodexAgent 方案；
- 不设置 VisionHyperAgentCore、AgentDDS 或 Training Worker 进程；
- 不修改 Codex 子模块源码；
- 不自研替代 Codex 的 Agent 引擎；
- 不让 QML 直接调用 Codex 或 Zenoh；
- 不把 Agent 回复当作业务确认；
- 不做云端训练或 CPU 训练兜底；
- 不提前建设相机、外部通信、模型库或通用插件系统。

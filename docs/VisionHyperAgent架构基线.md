# VisionHyperAgent 架构基线

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-15 |
| 状态 | 单进程多线程与 MVVM 分层已确认，具体集成待实施 |
| 文档目的 | 保持产品需求、系统边界、模块分层和实施路径一致 |
| 实施方式 | 具体实施步骤由用户逐段指挥；本文不是自动执行计划 |

## 1. 产品范围

VisionHyperAgent 是 Agent 驱动的视觉模型桌面软件。五个核心流程保持不变：

1. **预标注**：用户描述识别目标、外观特征、类别区别和排除情况；Agent 分析歧义、追问缺失信息并整理标注规则；用户确认后规则才可用于批量标注。描述变化后，旧分析和确认状态失效。
2. **预训练**：Agent 直接预标注效果不足时，使用少量已确认数据训练辅助初标模型，再用该模型辅助批量标注。预训练是少量数据的种子模型训练，不是外部大规模预训练。
3. **标注**：按已确认规则和可用辅助模型生成实例分割初标；用户逐图修改、删除和补充实例区域；确认后形成可训练的数据集版本。矩形框不能替代实例分割区域。
4. **训练**：基于已确认的数据集版本发起训练；Agent 制定训练与调参方案；训练在本机 NVIDIA GPU 执行；多轮评估后登记最佳候选模型。训练计算不上云。
5. **离线部署**：使用导出模型执行识别，不依赖在线 Agent，也不依赖训练进程。

相机、外部图片通信、模型库和运行页不在当前确认范围内。

## 2. 总体架构

VisionHyperAgentAPP 是唯一的产品级进程。程序内部采用单进程多线程和 MVVM 分层：

~~~text
VisionHyperAgentAPP
├── QML View
├── Python ViewModel / Qt 桥接层
├── Rust ViewModel
├── Rust Model
│   ├── 预标注 / 标注 / 数据集
│   ├── 预训练 / 训练 / 评估
│   ├── 推理 / 模型 / 部署
│   └── CodexAgent
└── Rust Core
    ├── 配置
    ├── 日志
    ├── 路径
    ├── 错误
    ├── 事件总线
    └── 任务运行时
~~~

不再设置 VisionHyperAgentCore、AgentDDS、Training Worker 或推理服务进程。训练、推理、业务状态和 Agent 接入都在 APP 进程内的受控线程中执行。

第三方运行时如果自身不可避免地创建辅助进程，必须在实施前明确验证和约束；产品架构不主动新增服务进程。

## 3. MVVM 分层

### 3.1 View

View 使用 PySide6 / QML：

- 展示预标注、预标注规则确认、标注、预训练、训练、模型、推理和设置页面；
- 承接输入、编辑、选择、确认和取消交互；
- 只绑定 Python ViewModel 暴露的属性、方法和信号；
- 不直接调用 Rust Model、CodexAgent、数据库或文件服务；
- 保持亮色、渐变、磨砂和大圆角的既有视觉方向。

目录：

~~~text
source/Python/view/
~~~

### 3.2 Python ViewModel

Python ViewModel 是 Qt 桥接层，不是业务层：

- 提供 QObject、Property、Signal 和 Slot；
- 注册或持有 QML 可见的界面状态；
- 把 QML 命令转发给 Rust ViewModel；
- 把 Rust 回调安全地转发到 Qt 主线程；
- 不实现预标注、训练、推理、模型登记或 Agent 协议逻辑。

目录：

~~~text
source/Python/viewmodel/
~~~

### 3.3 Rust ViewModel

Rust ViewModel 是纯 Rust 的界面状态逻辑层：

- 管理页面状态、表单状态、选中项、加载状态、错误展示状态；
- 编排用户命令并调用 Rust Model；
- 把 Model 与 Agent 事件整理为适合界面消费的状态；
- 不依赖 Qt、PySide6、QML 或窗口控件。

Rust 包位置：

~~~text
source/Rust/core/
~~~

### 3.4 Rust Model

Rust Model 是业务事实与用例层：

- 预标注规则分析与确认状态；
- 数据集、标注实例、版本和确认记录；
- 预训练、训练、评估与取消；
- 模型登记、比较、导出和部署状态；
- 离线推理与实例分割后处理；
- 调用 CodexAgent 获取 Agent 分析、追问和方案。

Agent 返回的文本不能替代业务校验和用户确认。未确认的数据不得进入训练。

目录：

~~~text
source/Rust/model/
└── codex_agent/        CodexAgent 子模块
~~~

`model` 自身只是一个目录，不包含 `src`。每个业务子模块放在 `model/<模块名>/ ` 下；实现时再建立自己的 Rust 包。

### 3.5 Rust Core

Rust Core 是进程内共用基础层：

- 配置加载与覆盖；
- 结构化日志；
- 安装目录、用户数据目录、缓存目录、日志目录和工作区路径解析；
- 统一错误模型；
- 事件总线；
- 任务队列、取消、超时和线程生命周期管理。

组件目录：

~~~text
source/Rust/basic/
~~~

`basic` 自身只是一个目录，不包含 `src`。配置、日志、路径、错误、事件、任务运行时等共用能力后续分别放入 `basic/<组件名>/ `。

### 3.6 Python / Rust 绑定层

Python ViewModel 与 Rust ViewModel 之间通过专用 Rust 绑定包连接：

~~~text
source/Python/pybind/
~~~

绑定层只做类型转换、回调转发和生命周期管理，不承载业务逻辑。

## 4. CodexAgent

CodexAgent 属于 Rust Model 层的基础设施适配器，以 Rust 库和子线程方式运行，不再是独立进程。

职责：

- 启动、停止和查询 Codex 连接状态；
- 掉线检测、重启和重连；
- 通过 WebSocket / JSON-RPC 调用 Codex App Server；
- 管理 thread/start、thread/resume、thread/list、turn/start、turn/interrupt、skills/list 等白名单方法；
- 接收 Codex 通知并转换为进程内事件；
- 处理请求关联、超时和取消；
- 将 Codex 的审批类反向请求交给业务层和用户确认流程。

CodexAgent 不做：

- 自研会话数据库；
- 自研聊天引擎；
- 解析 Codex rollout 文件作为管理入口；
- 视觉业务规则判断；
- 直接执行训练或推理。

Rust 包位置：

~~~text
source/Rust/model/codex_agent/
~~~

详细边界见 docs/CodexAgent模块边界.md。

## 5. 线程模型

APP 进程内至少规划以下线程：

| 线程 | 职责 |
| --- | --- |
| Qt 主线程 | QML 渲染、界面事件、ViewModel 属性更新 |
| Rust ViewModel / 绑定回调线程 | 状态计算与 Qt 线程安全转发 |
| CodexAgent 线程 | Codex WebSocket / JSON-RPC、重连和事件接收 |
| 任务调度线程 | 业务任务队列、取消、超时和状态发布 |
| 训练 / 推理工作线程 | 预训练、正式训练、评估和离线推理 |

约束：

- Qt 主线程不做文件扫描、模型计算、网络等待或长时间业务处理；
- 所有长任务必须可查询、可取消并有明确终态；
- Rust 与 Python 之间的回调必须显式处理线程边界；
- 子线程异常不能静默吞掉，必须进入统一错误和日志系统。

## 6. 状态归属

| 状态 | 归属 |
| --- | --- |
| QML 临时控件状态、草稿输入、窗口布局、主题偏好 | Python ViewModel / 本地界面配置 |
| 页面展示状态、加载状态、错误展示状态 | Rust ViewModel |
| 预标注规则、分析结果、确认状态 | Rust Model |
| 数据集、标注实例、版本、确认记录 | Rust Model |
| 预训练、训练、评估任务与指标 | Rust Model |
| 候选模型、最佳模型、导出和部署状态 | Rust Model |
| Codex 连接状态、thread / turn 执行状态 | CodexAgent |
| 配置、日志、路径、统一错误、事件路由 | Rust Core |

业务事实只保存在 Rust Model 和其持久化存储中。QML 和 ViewModel 中的状态是投影，不能成为第二套业务事实来源。

## 7. 通讯边界

单进程内部不使用 Zenoh：

~~~text
QML
↔ Python ViewModel
↔ Rust ViewModel
↔ Rust Model / CodexAgent
↔ Rust Core
~~~

CodexAgent 与 Codex App Server 之间使用 WebSocket / JSON-RPC。

Zenoh 仅保留为后续外部节点、远程控制或分布式扩展的可选能力，当前不作为内部通讯依赖。

## 8. 核心流程

### 预标注

~~~text
QML 输入描述
→ Python ViewModel
→ Rust ViewModel
→ Rust Model
→ CodexAgent 发起分析和追问
→ Rust Model 校验并登记分析结果
→ 用户确认规则
→ 规则可用于批量标注
~~~

描述变化后，旧分析、旧确认和依赖旧规则的批量结果失效。

### 预训练

~~~text
直接预标注效果不足
→ 用户确认少量种子数据
→ Rust Model 训练辅助初标模型
→ 评估并登记
→ 后续批量标注复用
~~~

### 标注

~~~text
Rust Model 基于确认规则和辅助模型生成实例分割初标
→ QML 展示并允许修改、删除、补充
→ Rust Model 保存实例区域
→ 用户确认数据集版本
~~~

### 训练

~~~text
QML 发起训练
→ Rust Model 校验确认版本、GPU 和环境
→ CodexAgent 请求训练与调参方案
→ Rust Model 校验方案并执行训练
→ 记录进度、指标和产物
→ 登记候选模型并选择最佳模型
~~~

### 离线部署

~~~text
Rust Model 加载导出模型
→ 推理线程执行预处理、模型推理和实例分割后处理
→ ViewModel 展示类别与分割区域
~~~

离线推理不初始化在线 Agent，不依赖训练环境。

## 9. 目录结构

~~~text
source/
├── Python/
│   ├── main.py                 应用唯一入口
│   ├── view/                   QML View
│   ├── viewmodel/              Python ViewModel / Qt 桥接层
│   └── pybind/                 Python / Rust 绑定
├── Rust/
│   ├── Cargo.toml              Rust workspace
│   ├── basic/                  共用基础组件集合
│   ├── model/                  业务模型与用例，包含 CodexAgent
│   └── core/                   纯 Rust ViewModel
source/Rust/depends/            固定版本上游子模块
~~~

依赖放置规则：

- `source/Rust/depends/` 只保留 Codex、Ultralytics、Zenoh 等需要固定源码或联调的上游项目；
- `tokio`、WebSocket、JSON 序列化等普通 Rust 库在各模块的 `Cargo.toml` 中声明；
- 完整版本由 `source/Rust/Cargo.lock` 固定；
- 不把普通 Rust 库加入 Git 子模块，避免依赖来源混杂和仓库膨胀。

已取消的目录：

~~~text
source/VisionHyperAgentCore/
source/AgentDDS/
source/protocol/
~~~

## 10. 打包与路径

配置和代码不硬编码开发机绝对路径。启动时由 Rust Core 解析：

~~~text
install_home
data_home
cache_home
log_home
workspace_root
~~~

安装目录保存只读资源；用户数据、Codex Home、日志、数据库和用户技能放系统用户数据目录；项目文件放用户选择的工作区。是否提供便携版由后续打包方案确认。

## 11. 实施顺序

高层顺序如下，具体实施仍由用户指挥：

1. Rust workspace 与 Core 基础类型；
2. Python / Rust 绑定和线程安全回调；
3. CodexAgent 生命周期、状态与重连；
4. Rust ViewModel 主窗口状态；
5. 预标注规则用例；
6. 数据集与标注模型；
7. 预训练与训练模块；
8. 推理、模型登记和离线部署；
9. 打包与路径解析验证。

## 12. 非目标

- 不再拆分 Desktop / Core / AgentDDS 三个产品进程；
- 不修改 Codex 子模块源码；
- 不自研替代 Codex 的 Agent 引擎；
- 不让 QML 直接调用 Codex；
- 不把 Agent 回复当作业务确认；
- 不做云端训练或 CPU 训练兜底；
- 不提前建设通用插件系统；
- 不未经确认扩展相机、外部通信、模型库或发布打包。

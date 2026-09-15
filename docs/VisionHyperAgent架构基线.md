# VisionHyperAgent 架构基线

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-15 |
| 状态 | 三进程架构已确认，具体集成尚待验证 |
| 文档目的 | 保持产品需求、系统边界和实施路径一致 |
| 实施方式 | 具体实施步骤、拆分粒度和先后顺序由用户逐段指挥；本文不是自动执行计划 |

## 1. 产品范围

VisionHyperAgent 是 Agent 驱动的视觉模型桌面软件。以下五个核心流程保持不变：

1. **预标注**：用户描述识别目标、外观特征、类别区别和排除情况；Agent 分析歧义、追问缺失信息并整理标注规则；用户确认后规则才可用于批量标注。描述变化后，旧分析和确认状态失效。
2. **预训练**：当 Agent 直接预标注效果不足时，使用少量已确认数据先训练一个辅助初标模型，再用该模型辅助批量标注。这里的预训练指少量数据的种子模型训练，不是外部大规模预训练，也不自动取代人工确认。
3. **标注**：按已确认规则和可用的辅助初标模型生成实例分割初标；用户逐图修改、删除和补充实例区域；确认后形成可训练的数据集版本。矩形框不能替代实例分割区域。
4. **训练**：基于已确认的数据集版本发起训练；Agent 制定训练与调参方案；训练在本机 NVIDIA GPU 执行；多轮评估后登记最佳候选模型。训练计算不上云。
5. **离线部署**：使用导出模型执行识别，不依赖在线 Agent，也不依赖 Python 训练进程。

相机、外部图片通信、模型库和运行页不在本次确认的不变范围内；后续由用户另行指挥。

## 2. 系统结构

系统明确划分为以下三个独立进程，正式名称与本文简称对应如下：

| 进程名称 | 本文简称 | 仓库目录 | 职责 |
| --- | --- | --- | --- |
| `VisionHyperAgentAPP` | Desktop | `source/VisionHyperAgentAPP/` | Python 桌面交互与展示 |
| `VisionHyperAgentCore` | Core | `source/VisionHyperAgentCore/` | Rust 业务核心，负责数据、训练与推理 |
| `AgentDDS` | Agent | `source/AgentDDS/` | 独立 Rust Agent，集成 Codex 运行时与 Zenoh 接入层 |

`AgentDDS` 是进程名称，通讯方案仍为 Zenoh。进程命名不改变既定职责与目录归属。

~~~text
进程 1：VisionHyperAgentAPP
Python + PySide6 + QML，只负责界面交互与展示
        ↕ Zenoh
进程 2：VisionHyperAgentCore
Rust 业务核心，管理数据、任务与用户确认；内部承载训练和离线推理
        ↕ Zenoh
进程 3：AgentDDS
Rust 程序：我们自己的 Zenoh 接入层与适配代码 + 未修改的 Codex 运行时
~~~

Core 不嵌入 Codex；Agent 不并入 Core。训练属于 Core 内部模块，不再设独立 Training Worker 项目、服务或进程。内部并发按需要使用线程或异步任务，不把启动另一个程序称为进程内多线程。

### 2.1 VisionHyperAgentCore（Rust）

VisionHyperAgentCore 是后台核心服务器和唯一业务事实来源：

- 管理数据集、标注版本、确认状态、训练任务、模型记录和部署状态；
- 管理用户确认门槛，未确认数据不得进入训练；
- 通过 Zenoh 请求 Agent 分析、追问和制定方案，维护业务任务与 Agent 会话的关联；
- 对 Agent 暴露受控业务工具，校验请求并执行业务操作；
- 编排预标注、预训练、标注、训练和离线部署用例；
- 在内部训练模块执行预训练与正式训练，并登记其产物与指标；
- 承载 Rust 生产推理路径；
- 通过 Zenoh 对客户端暴露受控 API；
- 管理配置、日志、运行记录和环境体检。

Core 不因桌面窗口关闭而丢失后台任务状态；Desktop 重连后恢复展示。

Core 不依赖 Codex 的 Rust 库，也不维护 Agent 的内部推理上下文。Agent 返回的方案和工具请求不能直接替代 Core 的业务校验或用户确认。

### 2.2 VisionHyperAgentAPP（Python）

VisionHyperAgentAPP 桌面客户端采用 **Python + PySide6 + QML**：

- 展示预标注、预训练、标注、训练和离线部署页面；
- 承接输入、编辑、选择、确认和取消操作；
- 保留全局 Agent 聊天、页面草稿、窗口布局和界面偏好；
- 通过 Zenoh 调用 Core；
- 订阅任务状态、日志、指标和 Agent 消息。

目录规格如下：

~~~text
source/VisionHyperAgentAPP/main.py       唯一应用入口
source/VisionHyperAgentAPP/model/        Desktop 本地 UI 状态模型与 Core 状态投影
source/VisionHyperAgentAPP/view/         QML 界面、组件、页面与资源
source/VisionHyperAgentAPP/viewmodel/    QObject ViewModel，隔离 QML 与通讯层
~~~

Desktop 只连接 Core，不直接连接 Agent；聊天消息、流式回复和审批交互均经过 Core。Desktop 不直接读写数据集、标注文件、模型目录或训练配置，也不实现业务规则。按钮和聊天入口必须复用同一个 Core API。

界面保持亮色、绚彩渐变、半透明磨砂和大圆角风格，不改为灰白保守风格。

### 2.3 AgentDDS（Rust）

AgentDDS 是我们自己的独立 Rust 可执行程序，内部集成 Codex 运行时并增加 Zenoh 接入层：

- 管理 Agent 会话、对话上下文、工具注册和技能加载；
- 接收 Core 提交的消息和任务，回传流式回复、执行状态、工具请求和审批请求；
- 处理会话取消、结束及异常，并向 Core 如实报告状态；
- 通过 Zenoh 请求 Core 执行业务能力，不自行接管训练、推理、数据确认和模型登记；
- Codex 作为固定版本的源码依赖保持原样，我们只编写自己的入口和适配代码；
- 不采用“外壳进程再启动一个 Codex CLI 进程”来替代进程内集成。

进程内接入是待验证的实现目标，不代表已经完成独立构建或运行验证。若现有接口不足，先说明限制并由用户决定，不擅自修改 Codex 源码或改变进程边界。

### 2.4 Core 内部训练模块

预训练和正式训练由 Core 内部模块执行，不再拆分 Training Worker：

- 使用软件自行管理的 Python 3.13 / PyTorch / Ultralytics 环境，进程内接入方式与兼容性在具体实施时验证；
- 在本机 NVIDIA GPU 上执行 YOLO 实例分割预训练和正式训练；
- 由 Core 统一记录并对外发布进度、指标、日志、产物和失败原因；
- 支持协作式取消。

预训练模型是否足以辅助标注、训练是否有效、模型是否登记、哪次结果最佳，均由 Core 判断。无 NVIDIA GPU 时预训练和训练明确不可用，不做 CPU 兜底。

### 2.5 Core 内部离线推理

离线部署使用 Core 内部的 Rust 推理路径，不另设推理进程：

- 加载导出模型；
- 执行图片预处理、模型推理和实例分割后处理；
- 返回实例类别与分割区域；
- 不调用在线 Agent、不启动训练环境、不依赖互联网。

模型能否部署以实际导出和推理验证结果为准，不以 Agent 文本回复为准。

## 3. Zenoh 边界

Zenoh 是 Desktop ↔ Core、Core ↔ Agent 的进程间通讯方式，也是后续外部系统接入 Core 的统一方向。Desktop 不绕过 Core 直接访问 Agent。ZeroMQ 不再作为主通讯方案。

| 链路 | 用途 |
| --- | --- |
| Desktop ↔ Core | 用户输入、业务请求、确认、任务状态、Agent 回复与审批展示 |
| Core ↔ Agent | 会话交互、分析与方案请求、流式事件、取消、审批和受控业务工具调用 |

| 类型 | 用途 |
| --- | --- |
| 请求 / 响应 | 发起用例、查询状态、确认、取消、加载模型和执行识别 |
| 事件流 | 推送任务状态、训练进度、指标、日志增量、Agent 消息和标注进度 |
| 数据传输 | 图片预览、缩略图、标注轮廓或掩码等界面数据 |

协议要求：

- Core、Desktop 与 Agent 可能分别升级，需要协议版本和能力协商；
- 请求携带可追踪的请求、业务任务和 Agent 会话关联信息；
- 错误包含错误码、用户可读原因和建议动作；
- 事件流处理重连、重复、乱序和任务结束；
- Desktop 明确展示未连接、启动中、版本不兼容和后台错误；
- Core 向 Desktop 如实报告 Agent 的不可用或中断状态，不把会话完成等同于训练成功或用户确认；
- Agent 返回结果由 Core 校验对应任务状态和规则版本，迟到结果不能恢复已经失效的分析或确认状态；
- 未授权图片、凭据、日志和运行环境不得发送给在线服务。

主题命名、序列化格式、认证和外部接入协议由后续协议设计确定。

## 4. 状态归属

| 状态 | 归属 |
| --- | --- |
| 页面草稿、聊天未发送输入、窗口布局、主题偏好 | Desktop |
| 业务任务与 Agent 会话的关联、业务工具执行结果 | Rust Core |
| Agent 会话、对话上下文、推理轮次和技能加载状态 | Rust Agent |
| 预标注规则、分析结果、确认状态 | Rust Core |
| 种子数据、预训练任务、辅助初标模型和评估记录 | Rust Core |
| 图片索引、标注实例、数据集版本、确认记录 | Rust Core |
| 训练任务、参数、指标、日志、取消状态 | Rust Core |
| 候选模型、最佳模型、部署模型和元数据 | Rust Core |

Agent 会话记录不是第二套业务事实来源。用户确认后的数据集版本不可被 Agent 擅自覆盖。后续修正应形成新版本或明确修订流程。

## 5. 核心流程

### 预标注

~~~text
Desktop 输入特征描述
→ Zenoh 提交 Core
→ Core 通过 Zenoh 请求 Agent 分析和追问
→ Agent 通过 Zenoh 回传 Core
→ Core 校验结果并转发 Desktop 展示
→ 用户确认规则
→ Core 登记可用于批量标注的规则
~~~

特征描述变化后，旧分析、旧确认和依赖旧规则的批量标注结果不能再被当作有效状态。

### 预训练

~~~text
Agent 直接预标注效果不足
→ Core 选取或请求用户确认少量种子数据
→ Core 内部训练模块使用种子数据训练辅助初标模型
→ Core 评估并登记该模型
→ 后续批量标注复用该辅助模型
~~~

预训练是标注链路的兜底增强，不绕过用户确认，也不自动把辅助初标模型当作最终部署模型。效果是否不足、种子数据数量、训练参数和验收阈值由用户在具体实施时确认。

### 标注

~~~text
Core 基于确认规则和可用辅助模型生成实例分割初标
→ Desktop 展示并允许修改、删除、补充
→ Core 保存业务数据
→ 用户确认数据集版本
→ 版本成为训练前置条件
~~~

复杂轮廓和掩码表达、编辑体验与质量门槛后续由用户确认。

### 训练

~~~text
Desktop 发起训练
→ Core 校验确认版本、GPU 和运行环境
→ Core 通过 Zenoh 请求 Agent 制定训练与调参方案
→ Agent 将方案返回 Core
→ Core 校验方案与预算，在内部训练模块执行训练
→ Core 记录指标与产物并向 Desktop 发布进度
→ Core 登记、比较和选择候选模型
~~~

单 GPU 训练由 Core 串行调度或明确受控并发。失败、取消和环境不可用必须保持状态真实。

### 离线部署

~~~text
Core 加载已验证部署模型
→ Desktop 或后续外部入口提交图片
→ Rust 推理路径执行识别
→ Core 返回实例类别与分割区域
~~~

## 6. AgentDDS 边界

独立 AgentDDS 程序复用开源 Codex 的运行时，并纳入 Ultralytics 官方 YOLO 技能；不另写同类 Agent 引擎，不擅自引入产品范围之外的模型方案。

Codex 子模块 `source/depends/codex/` 保持原样：不修改源码、构建清单或锁文件，不打补丁。我们的程序入口、Zenoh 接入和适配代码放在 `source/AgentDDS/`。接入只使用现有接口；接口或构建不兼容时先报告，不以修改 Codex 为默认解决方案。

职责边界如下：

- Desktop 经 Core 转发用户消息并展示回复，不持有 Agent 的业务控制权；
- Agent 维护会话上下文、工具注册和技能加载，Core 维护会话与业务任务的关联；
- Agent 的业务工具经 Zenoh 调用 Core 暴露的应用能力，由 Core 校验和执行；
- 工具审批请求与业务确认状态分别处理，Agent 对话中的文本答复不能代替 Core 的确认记录；
- Agent 不能绕过用户确认、数据版本、训练预算和模型登记规则；
- Agent 不直接执行训练或接管生产推理，离线推理不依赖 Agent 启动或在线；
- 只允许把任务图片发送给用户配置的在线服务，不发送无关文件、凭据或运行环境。

## 7. 环境与平台

- 目标平台：Linux、Windows；
- 训练硬件：本机 NVIDIA GPU，不设型号白名单，但按软件版本验证兼容性；
- 软件自行管理私有运行环境，不要求用户配置 Conda 或系统 Python；
- 训练计算留在本机；
- 离线识别不依赖互联网；
- 依赖版本、许可证和打包方式在具体实施前由用户确认。

三进程是已确认的架构约束，不是已经验证的运行事实。实施前需验证未修改 Codex 的独立引用与进程内运行、Core 内 Python 训练接入，以及离线推理不初始化 Agent 或训练环境的路径。工具执行、训练数据加载等可能产生的额外进程必须检查和约束；若无法满足三进程要求，先说明并由用户决定，不擅自新增进程。

## 8. 仓库归属方向

~~~text
source/VisionHyperAgentCore/  VisionHyperAgentCore，内部包含训练与离线推理模块
source/AgentDDS/              AgentDDS，Zenoh 接入与未修改 Codex 的适配
source/VisionHyperAgentAPP/   VisionHyperAgentAPP，Python PySide6/QML 桌面客户端
source/protocol/  Desktop ↔ Core、Core ↔ Agent 的 Zenoh 协议约定
source/depends/   固定版本的上游源码子模块
docs/             需求、架构和已确认设计
~~~

不再规划独立的 `training-worker/` 目录。目录内具体模块、文件和接口由用户指挥具体实施时再建立。本文只锁定归属方向，不授权一次性搭建全部工程。

## 9. 依赖顺序

高层依赖顺序如下，不表示自动执行步骤：

1. Desktop、Core、Agent 的三进程边界、Zenoh 协议语义和错误模型；
2. Core 内业务状态、确认门槛和持久化边界；
3. 未修改 Codex 的独立 Agent 封装、会话与受控业务工具接入；
4. 批量标注业务能力；
5. Core 内部训练接入、预训练与 GPU 训练链路；
6. 批量标注对辅助初标模型的复用；
7. 模型导出、登记和 Rust 离线推理。

每一步的具体拆分、测试范围和完成标准由用户另行指挥。

## 10. 非目标

- 不做 Desktop / Core / Agent 之外的通用微服务拆分；
- 不把 Agent 并入 Core，不额外建立独立 Training Worker 或推理服务；
- 不修改 Codex 子模块，不另写同类 Agent 引擎；
- 不把 ZeroMQ 作为并列主通讯协议；
- 不使用 Electron、Flutter 或浏览器原型替代 PySide6/QML；
- 不让 Desktop 或 Agent 成为第二套业务事实来源；
- 不做云端训练或 CPU 训练兜底；
- 不提前建设通用插件系统；
- 不未经确认自动扩展相机、外部通信、模型库或发布打包。

## 11. 与历史需求的关系

本文取代历史文档中与当前三进程结构冲突的内容：VisionHyperAgentAPP 为 Python + PySide6/QML 独立进程，VisionHyperAgentCore 为负责业务、训练和推理的独立 Rust 进程，AgentDDS 为集成未修改 Codex 运行时与 Zenoh 接入层的独立 Rust 进程。两条进程间链路均使用 Zenoh；原先 Core 内承载 Codex、独立 Training Worker 等方向不再适用。

五个核心产品能力、本机 GPU 训练、私有运行环境、复用 Codex 引擎、Ultralytics 官方技能、用户确认门槛和 UI 视觉方向保持不变。

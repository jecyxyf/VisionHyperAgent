# 纯 Python 重写设计（VisionHyperAgent）

| 项目 | 内容 |
| --- | --- |
| 日期 | 2026-09-14 |
| 状态 | 设计已逐段确认，待用户审阅书面稿 |
| 前置 | 取代历史 `Rust + Slint + Python Worker` 方案；产品定位不变，技术栈整体改为纯 Python |
| 范围 | 首版只做：预标注、批量标注、训练 三个流程；其余能力延后 |

## 1. 背景与目标

VisionHyperAgent 仍是 Agent 驱动的视觉模型标注与自动训练桌面软件：用户导入图片，与 Agent 交互完成实例分割初标，人工确认后由 Agent 自动完成训练配置、调参、评估与选模。

原方案为 Rust + Slint 宿主 + 私有 Python 训练环境 + Rust 生产推理。本次变更为**纯 Python 技术栈**，消除跨语言边界、统一运行环境与交付链路。

除语言栈外，产品需求保持不变：单主程序、私有 runtime、NVIDIA GPU 本机训练、Codex CLI 作为 Agent、训练计算不上云。

## 2. 已确认的技术决策

| 编号 | 决策 |
| --- | --- |
| D-01 | 桌面 UI 采用 **PySide6 + QML**（声明式 UI，承接原 Slint 思路；渐变、磨砂、动画、GPU 加速；QML 与 Python 经 ViewModel 层交互）。 |
| D-02 | **后续**生产推理方向为单一私有 Python runtime 内的 **ONNX 推理**（训练用 PyTorch/Ultralytics，最佳模型导出 ONNX，识别用 onnxruntime）。识别路径离线、不经 Agent、不加载训练栈。**首版不实现部署推理，本条仅锁定方向。** |
| D-03 | 整体架构沿用已确认的六层职责，映射为 Python 模块化单体：View(QML) / ViewModel / Agent / train_app / drivers / basic。 |
| D-04 | 首版流程为三段：**预标注 → 批量标注 → 训练**。预训练取消；相机、ZMQ、ONNX 部署推理、模型库页、运行页全部延后。 |
| D-05 | 训练必须在 NVIDIA GPU 上执行；无 GPU 时训练入口明确不可用并说明原因，不做 CPU 兜底。 |

## 3. 首版范围

### 3.1 三个流程

1. **预标注（特征校准）**
   用户描述识别目标、外观特征、类别区别与排除情况 → Agent 分析歧义、追问缺失信息、整理标注规则 → 用户确认规则。修改描述后，旧分析与确认状态失效，需重新分析确认。

2. **批量标注**
   按已确认规则对图片目录批量生成实例分割初标 → 用户逐图/抽检修改、删除、补充 → 确认后形成可训练数据集。矩形框不能替代实例分割区域。

3. **训练**
   基于某个已确认数据版本发起 → Agent 制定训练配置 → Ultralytics 在本机 NVIDIA GPU 上训练 YOLO 实例分割 → 多轮调参、评估 → 选出最佳 `.pt` 模型并登记训练记录。

### 3.2 页面结构

- 预标注页、批量标注页、训练页、设置、关于；
- Agent 聊天作为右侧全局面板，切页保留会话与输入草稿；
- 延后：运行页、模型库页、预训练页、相机、ZMQ、部署推理。

## 4. 架构与目录

### 4.1 六层映射

```text
QML View (PySide6) ↔ ViewModel (QObject)
                        ↓
        Agent 层 / train_app 层 / drivers 层 / basic 层
```

- **View**：QML 文件（页面/组件/主题），只做展示与输入转发。
- **ViewModel**：Python QObject（Signal/Property/Slot），按功能拆分，不依赖 QML 类型、不持窗口引用；无 UI 也可运行核心流程与测试。
- **Agent 层**：Codex CLI 子进程管理、会话、工具注册、技能加载；经工具接口调用 train_app 能力，不复制业务实现。
- **train_app 层**：可复用业务能力——数据集、标注存储、训练编排、模型与训练记录；业务事实的唯一来源。
- **drivers 层**：第三方封装——ultralytics_train、codex；首版仅此两个，其余延后不建空壳。
- **basic 层**：配置、日志、路径、环境体检（Python 依赖、CUDA/GPU、Codex 可达性）。

### 4.2 目录结构

```text
VisionHyperAgent/
├── pyproject.toml              # 单包；uv 管理依赖与锁定
├── src/vision_hyper_agent/
│   ├── main.py                 # 唯一入口：初始化 basic → 注册 ViewModel → 加载 QML
│   ├── basic/                  # 配置、日志、路径、GPU/runtime 体检
│   ├── drivers/                # ultralytics_train / codex
│   ├── train_app/              # 数据集、标注存储、训练编排、模型记录
│   ├── agent/                  # Codex 会话管理、工具注册、技能加载
│   ├── viewmodels/             # main_window / preannotation / annotation / training / settings / chat
│   └── view/                   # QML：MainWindow.qml、pages/、components/、resources/
├── tests/
├── deploy/                     # 后续 runtime 组装与打包配置（首版仅保留目录）
└── docs/
```

### 4.3 边界与依赖规则

- QML 不 import 业务，只调 ViewModel 槽函数、绑属性/信号；
- ViewModel 不 import PySide 的 QML 类型；按钮与对话两个入口复用同一 ViewModel/应用能力，不产生两套状态；
- train_app 是业务事实唯一来源；Agent 工具不能绕过业务门槛（未确认数据不得训练）；
- drivers 只封装第三方调用，不写业务规则、不直接改 ViewModel 状态；
- 依赖方向：`view → viewmodels → (agent | train_app) → drivers → basic`，禁止反向；以仓库内约定加检查强制执行。
- basic 无业务语义；训练策略归 Agent，流程编排归 ViewModel/train_app。

## 5. 线程与数据流

UI（Qt 主线程）只做渲染与轻量状态更新；计算、I/O、子进程全部后台执行，结果经 Qt 信号（排队回主线程）通知 ViewModel。

| 链路 | 流程 |
| --- | --- |
| Agent 初标 | ViewModel → Codex 子进程（asyncio 消息泵）→ 工具调用读图片/写初标 → 标注编辑器展示 → 人工修改确认 → train_app 登记为可训练数据 |
| 训练 | ViewModel 提交 → 校验确认状态/预算 → 训练 Worker 调 Ultralytics（epoch 边界回报指标、协作式取消）→ Agent 依据评估决定下一轮或选模 → 登记最佳模型与记录 |

- 训练 Worker 串行队列：单 GPU 同一时间只跑一个训练。
- Codex 子进程崩溃/超时映射为可重试错误态，不拖死 UI，不影响已确认数据。
- 长任务取消为协作式（epoch/图片边界）；取消后状态回到可重新发起。

## 6. 状态、配置与错误处理

### 6.1 核心状态（train_app 持有，ViewModel 映射）

- **数据集**：待标 / 已初标 / 已确认；确认后生成不可变版本（v1、v2…），训练绑定指定版本，Agent 不得覆盖已确认数据。
- **预标注规则**：草稿 → 分析中 → 已确认；修改描述自动失效旧分析与确认。
- **训练任务**：排队中 → 运行中（逐轮参数/指标）→ 评估中 → 完成（最佳模型）/ 失败 / 已取消。

### 6.2 配置（basic）

- 配置文件存用户目录，单文件 TOML；
- 内容：模型服务地址与密钥（Codex 用）、图片目录、训练预算（时长/轮数）、GPU 选择；
- 损坏时回退默认并明确提示，不静默吞错；
- 启动体检：Python 依赖完整、CUDA/NVIDIA GPU 可用、Codex CLI 可达；不满足给可操作说明。无 GPU 时预标注/批量标注可用，训练入口置灰并说明原因。

### 6.3 错误处理

- 分层异常类型，跨层不丢上下文；UI 展示映射后的错误态（标题+原因+建议动作），不弹裸 traceback；
- 不虚构成功：训练/标注未完成即明确失败或取消；以实际产物与指标文件为准，不以 Agent 回复为准；
- 日志写文件（分级、可轮转），训练页可查看运行日志。

## 7. 交付（首版简化）

- 开发期用 uv 管理统一 Python 环境，锁定 torch CUDA、ultralytics、PySide6 等版本；
- 启动体检逻辑现在实现，为将来私有 runtime 打包留接口；
- 安装器、私有 runtime 打包、Windows 发行延后；首版目标是功能闭环，不是发行包。

## 8. 测试边界

训练必须在真实 NVIDIA GPU 上执行（D-05）。各模块的具体测试标准不在本文预置，由用户在各模块开发过程中逐步给出并确认后执行。

## 9. 延后清单（首版不做）

- 相机采集、ZMQ 对外识别、ONNX 导出与部署推理（方向已定，见 D-02）；
- 模型库页、运行页、预训练页；
- 安装器与私有 runtime 打包、Windows 发行；
- ZeroMQ 之外的其他对外协议。

## 10. 未决事项

| 编号 | 问题 | 影响 |
| --- | --- | --- |
| Q-01 | Agent 初标的工具形态、轮廓/掩码可编辑表达与质量门槛 | 批量标注可用性 |
| Q-02 | 数据集图片格式、规模上限、目录组织与标注文件格式 | 导入/存储设计 |
| Q-03 | 训练评估指标、排名规则、调参预算与停止条件 | 自动选模可信度 |
| Q-04 | Codex CLI 集成接口、认证与官方 YOLO 技能的加载/更新方式 | Agent 接入 |
| Q-05 | torch/ultralytics/PySide6/CUDA 版本兼容矩阵 | uv 锁定与 GPU 可运行性 |
| Q-06 | 私有 runtime 打包与更新机制（延后，但体检接口需兼容） | 后续发行 |


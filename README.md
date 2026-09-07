# VisionHyperAgent

Agent 驱动的视觉模型标注、自动训练与离线部署软件。

产品目标是让用户在同一个桌面应用中导入图片，通过与 Agent 交互生成初步标注，修改并确认后，由 Agent 自动训练、调参、评估和选择模型。产出的模型可在本机离线运行，接收相机图像或外部程序传入的图片并返回识别结果。

> **当前阶段：工程骨架初始化。** 已建立单 Cargo 包、Rust 分层模块入口和配套目录。当前程序仅输出终端提示，尚未实现 Slint 界面、Codex、训练、推理、相机或通信功能；未安装训练环境或下载模型。

## 运行与检查骨架

Rust 包名为 `vision-hyper-agent`，采用 2024 edition，目前没有第三方 Rust 依赖。在已经安装 Rust 工具链的环境中，可从仓库根目录执行：

```sh
cargo run --offline
cargo build --offline
cargo test --offline --workspace
cargo fmt --all -- --check
```

启动输出明确提示“业务功能尚未实现”。`depoly/tests/smoke.rs` 仅验证这个最小启动行为，不代表产品功能验收。集成测试已在 `Cargo.toml` 中显式注册，新增测试目标也需要同步注册。

本次在 Linux x86_64 上使用 Rust/Cargo 1.97.1 完成编译、格式和测试检查。默认工具链未安装 Clippy，因此使用本机已经安装的 1.95.0 工具链完成补充 lint 检查，没有安装组件或修改全局默认版本：

```sh
CARGO_TARGET_DIR=depoly/target/clippy-1.95.0 cargo +1.95.0 clippy --offline --workspace --all-targets -- -D warnings
```

上述 Clippy 命令对应本次已有的本机工具链，不是项目固定版本要求；其他环境应使用其已配置的对应组件。最低支持版本和最终工具链策略尚未确定，Windows 构建也尚未验证。

### 编译产物

`.cargo/config.toml` 将 Cargo 构建与缓存目录设为 `depoly/target/`。Linux 骨架程序归集到 `bin/linux/vision-hyper-agent`；`bin/` 不提交 Git，当前没有自动归集脚本，在 Linux 重新编译后可手动更新：

```sh
mkdir -p bin/linux
cp depoly/target/debug/vision-hyper-agent bin/linux/
```

### 尚未接入的部分

`src/view/`、`depoly/packaging/python/`、资源和打包目录目前只有占位内容。产品资源统一保留在 `src/view/resources/`，其中 `ui/` 用于界面资源，`agent/` 保留产品 Agent 资源占位。`build.rs`、`depoly/packaging/python/pyproject.toml` 及对应功能依赖将在实际接入时添加，不提供空的训练或界面实现。

[src/depends/](src/depends/README.md) 用于管理第三方依赖来源与受控材料，目前没有下载任何依赖。`depoly/` 集中放置打包内容、开发辅助工具、集成测试和编译缓存，不是独立后端程序。

## 首版范围

- **视觉任务**：仅实例分割，其他视觉任务后续扩展。
- **平台**：Linux、Windows。
- **计算硬件**：仅支持 NVIDIA GPU；不设置显卡型号白名单，由软件发行版对应的运行时兼容要求判断能否运行。
- **软件形态**：训练与部署合并为一个 Rust + Slint 主程序，不拆前后端两个主可执行程序；Codex 和 Python 是内部执行组件。
- **交互入口**：按钮与对话复用同一套业务能力，产品 Skills 覆盖软件功能的使用与智能工作流。
- **运行环境**：软件自行携带并管理私有 runtime，用户不需要自行安装或配置 Conda、Python 和训练依赖。
- **联网边界**：Agent 交互和初标允许调用配置的在线大模型服务、发送任务图片；训练计算在本机，部署识别不依赖互联网或在线 Agent。

## 核心流程

### 标注与自动训练

1. 用户导入未标注图片，并与 Agent 说明需要分割的对象、类别和标注要求。
2. Agent 生成实例分割初标，不额外引入独立的辅助标注模型。
3. 用户查看、修改并确认标注结果。
4. Agent 自动配置训练、调参、执行本机训练并评估候选模型。
5. 依据约定的选模规则输出最佳候选模型，供本机部署使用。

“最佳”的评价指标、训练预算和停止条件尚未确定，不承诺无限训练或全局最优。Agent 初标的轮廓质量也尚未验证。

### 离线识别与外部集成

- **相机入口**：软件通过 OpenCV 采集图像并识别。
- **外部入口**：外部程序通过 ZeroMQ 发送图片，软件执行识别并返回结果。
- 两种入口复用模型推理能力。首版仅实现 ZeroMQ 通信，保留后续扩展其他通信方式的边界。

## 已确认的技术方向

采用 **方案 A：Python 训练，Rust 推理**。

整体采用模块化单体，按 View、ViewModel、Agent、应用、驱动／适配、基础六层划分职责。各层具体模块后续补充；不建立前后端内部 ZeroMQ 通信。

| 部分 | 方向 |
| --- | --- |
| 应用主体与界面 | Rust + Slint |
| Agent 接入 | 直接依赖开源 Codex CLI，不自行重建同类 Agent 引擎 |
| 技能依赖 | 纳入 Ultralytics 官方 YOLO 技能 |
| 训练、调参、评估和导出 | 私有 Python / PyTorch 环境中的 YOLO26 |
| 生产推理 | Rust，采用官方 `ultralytics-inference` 与 ONNX Runtime 路线 |
| 相机采集 | OpenCV |
| 首版外部通信 | ZeroMQ |
| 环境交付 | 一个软件自行管理所需 runtime；推理路径不依赖 Python 训练进程 |

具体依赖版本、操作系统最低版本、CPU 架构、GPU/驱动兼容矩阵和通信消息格式，需在后续设计中确认。

## 文档

- [产品需求文档](docs/产品需求文档.md)：需求基线、产品边界、验收目标及未决事项。
- [软件框架](docs/软件框架.md)：六层职责、双入口共用能力、Python 归属与运行边界。
- [仓库 Agent 指引](AGENTS.md)：维护本仓库的 Agent 应遵守的约束。
- [团队编码规范](docs/团队编码规范.md)：Rust、Python、Slint 编码与团队协作要求。

产品需求文档是 **PRD，不是代码实施计划**。当前只完成工程骨架，后续业务模块设计与实现需要按用户明确的范围继续推进。

## 许可证

仓库现有许可证见 [LICENSE](LICENSE)。第三方代码、模型权重和运行时的许可及分发条件，应在确定具体依赖和发布方式后单独核查。

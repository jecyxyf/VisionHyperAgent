# VisionHyperAgent

Agent 驱动的视觉模型标注、自动训练与离线部署软件。

产品目标是让用户在同一个桌面应用中导入图片，通过与 Agent 交互生成初步标注，修改并确认后，由 Agent 自动训练、调参、评估和选择模型。产出的模型可在本机离线运行，接收相机图像或外部程序传入的图片并返回识别结果。

> **当前阶段：原生 Slint 界面初版。** 已建立工程骨架及运行、模型库及模型分组（预标注、标注、预训练、训练）、设置、关于和全局聊天面板，可使用项目内 Slint Viewer 预览。Rust 主程序已连接现有 Slint 窗口并托管异步日志的自动退出收尾，启动直接显示运行页；训练、推理、Agent 等业务仍未接入。

## 底层日志与配置

已实现两个进程内线程安全单例 `LOGGER`、`CONFIG`。`src/foundation/` 仅包含 `logger.rs`、`config_manager.rs`、`mod.rs`；路径定位和共享错误上下文放在 `mod.rs`。

- 日志位于可执行文件目录的 `logs/YYYY-MM-DD.log`，四级异步写入文件，在入队前捕获毫秒时间、模块、源码文件／行号与原因；按日志产生时的本地日期切换并保留最近 90 天。
- `LOGGER` 只提供 `init / debug / info / warning / error`。由入口 `foundation::with_logging` 自动等待已入队日志写完；不公开刷新、关闭或专用错误上下文方法。队列满和后台失败不会伪装成功，具体返回语义见日志设计。
- `config.json` 仅包含 Agent 的 `base_url`、`api_key`、`model`。修改只更新内存，显式 `save()` 才写盘；损坏时先更新 `config.json.back` 再重建。
- `CONFIG.read(module, parameter)` 返回单个参数的字符串值；`CONFIG.write(module, parameter, value)` 修改指定参数。模块／参数名区分大小写，未知项返回错误，不自动创建；不再提供 `snapshot()` 或闭包式 `update()`。
- KEY 按要求明文存储；配置调试输出及 JSON 错误诊断脱敏。调用方不得主动将 KEY、认证头或配置 JSON 写进日志。
- `CONFIG.init()`／`CONFIG.load()` 和 `foundation::init()` 返回 `ConfigLoadStatus`，用于区分加载、创建、恢复和重复初始化；恢复报告含安全的错误上下文，由调用方决定如何转成文本记录或呈现。
- `main.rs` 通过 `foundation::with_logging(view::run)` 启动现有界面并托管日志生命周期，不再打印终端演示或写启动／退出日志。正常运行只创建空日志文件；不会自动初始化配置或生成 `config.json`，设置页尚未连接配置服务。

调用示例：

```rust
use vision_hyper_agent::foundation::{self, CONFIG, LOGGER, ConfigLoadStatus};

fn main() -> foundation::Result<()> {
    foundation::with_logging(|| {
        let status = foundation::init()?;
        if let ConfigLoadStatus::Recovered(problem) = status {
            LOGGER.error("config", &format!("已备份并重建配置；{problem}"))?;
        }
        CONFIG.write("agent", "model", "model-name")?;
        CONFIG.save()?;
        LOGGER.info("app", "配置已保存")?;
        // 实际应用应在此运行事件循环并结束业务线程，再离开托管作用域。
        Ok(())
    })
}
```

异步日志临时验证结果：临时白盒用例 21 项通过，重复 20 轮共 420 次通过；独立进程与接口检查 36 项通过，其中 20 轮并发退出共 320,020 条日志全部写入。Windows 仅完成编译链接，未运行实机测试。

2026-09-08 按用户要求删除这两个模块的单元测试、集成测试和 Python 测试脚本，并移除对应入口；保留功能实现、已完成的修复与设计文档中的验证摘要。当前 `cargo test` 不再包含这两个模块的专项功能回归；原有启动测试和 UI 预览脚本不受影响。

实现与接口说明见 [Logger](docs/plans/foundation/Logger.md)、[ConfigManager](docs/plans/foundation/ConfigManager.md)。仅保证单进程内的并发一致性，不提供多进程文件协调或断电后绝对持久性。依赖采用本机缓存的 `serde 1.0.228`、`serde_json 1.0.149`、`chrono 0.4.45`，许可证均为 MIT OR Apache-2.0，版本与传递依赖由 `Cargo.lock` 固定；未测量最终发行包体积。

## 桌面程序与构建

Rust 主程序已通过 `src/view/mod.rs` 加载 `MainWindow.slint`。`build.rs` 在构建期编译界面并内嵌图片、图标和 Noto 中文字体；Slint / slint-build 固定为 `1.17.1`，采用 Winit、优先 FemtoVG（OpenGL）渲染，保留软件渲染回退，不依赖 Qt 或项目内 Viewer。

### 已生成的 0.1.0 桌面版本

| 平台 | 可执行文件 | 验证边界 |
| --- | --- | --- |
| Linux x86_64 | `bin/linux/vision-hyper-agent` | 已在本机实际打开窗口、取像并正常关闭 |
| Windows x86_64 GNU | `bin/windows/vision-hyper-agent.exe` | GUI 子系统，已构建并检查系统 DLL 导入；未 Windows 实机运行 |

运行这些界面产物不需要安装 Rust、Slint Viewer、Python 或 Conda。它们还不是已接入训练、推理和 Agent 的完整产品。正常运行需要可写的程序目录及对应系统图形环境；当前 Linux 产物引用 `GLIBC_2.39`，另需 fontconfig 与 X11/Wayland 等桌面运行库，兼容更老发行版需要在对应基线重新构建，不能把本机结果当作所有 Linux 发行版均可运行的承诺。

每个平台目录随附 `LICENSE.txt`、`THIRD-PARTY-NOTICES.txt` 和 `build-info.json`（目标、大小及 SHA-256）。分发时应保留许可材料；当前只是本机桌面构建，不代表已完成所有部署环境及正式发行合规审核。

```sh
# 本机直接启动已编译的界面
./bin/linux/vision-hyper-agent

# 开发运行；首次需要下载固定版本依赖，缓存齐全后可加 --offline
cargo run --locked

# Linux / Windows Release 构建
cargo build --release --locked --target x86_64-unknown-linux-gnu
cargo build --release --locked --target x86_64-pc-windows-gnu

# 无需显示服务器的主窗口构造、软件渲染和草稿保留检查
cargo test --release --locked --target x86_64-unknown-linux-gnu --workspace
cargo fmt --all -- --check
```

本机使用 Rust/Cargo 1.97.1，Windows 交叉构建需要已安装的 `x86_64-pc-windows-gnu` 标准库和 MinGW-w64 链接器。没有安装系统驱动、配置 Conda 或修改全局 PATH。`depoly/tests/smoke.rs` 已从终端启动断言改为真正构造 Slint 主窗口、检查默认页面与草稿保留并渲染非空帧；该测试不启动训练、模型或相机，也不代替 Windows 实机验证。

`.cargo/config.toml` 将中间产物放在 `depoly/target/`。手动归集：

```sh
cp depoly/target/x86_64-unknown-linux-gnu/release/vision-hyper-agent bin/linux/
cp depoly/target/x86_64-pc-windows-gnu/release/vision-hyper-agent.exe bin/windows/
```

当前归集的 Release 产物另外去除了非必要符号；具体哈希见各目录的 `build-info.json`。`bin/` 不提交 Git。实际窗口截图与构建输出位于 `depoly/target/desktop-build/`。

### 原生 Slint 界面预览

从项目根目录运行：

```sh
depoly/tools/slint/bin/slint-viewer --check src/view/MainWindow.slint
depoly/tools/slint/bin/slint-viewer --auto-reload src/view/MainWindow.slint
```

界面入口为 `src/view/MainWindow.slint`，主题在 `Theme.slint`，枚举与数据结构集中在 `ViewTypes.slint`，悬停提示状态位于 `UiHints.slint`。所有 UI 组件均按 `组件名.slint` 命名，一组件一文件；页面位于 `pages/`，通用及页面辅助组件位于 `components/`。实际目录见[软件框架](docs/软件框架.md#811-view-组件与实际文件)。首页已移除，默认进入运行页：

```text
运行
模型
  预标注
  标注
  预训练
  训练
设置
关于
```

- **运行**：图像结果画布、模型选择、图像源配置、单次推理和识别记录；不单独放置网络触发按钮，外部图片仍从图像源配置进入。图像结果区不显示“未就绪”，底部并列显示图像源与模型名称；未选择模型时显示“未选择”，长名称省略显示。
- **模型**：“模型”与右侧箭头共用一个背景和圆角，视觉上为完整导航项；点击文字进入模型库，点击箭头只展开／折叠子导航，不改变当前页面。模型库提供目录、刷新入口，以及名称／格式／路径列表和选中详情。目录扫描尚未接入，默认不显示虚构模型。
- **预标注**：填写识别目标、外观特征、类别区别和排除情况，展示 Agent 分析及标注规则；用户确认规则后可通过页内入口进入标注。编辑特征会清除旧分析、规则及确认状态，草稿在切页／折叠后保留。真实 Agent 分析尚未接入，默认不生成任何分析结果。
- **标注**：顶部打开目录入口，批量浏览与逐图标注两种布局。
- **预训练**：已建立独立页面与空状态，具体用途、参数和执行流程待补充，不擅自等同于特征分析或训练前检查。
- **训练**：参数草稿、Agent 自动调参选项、效果曲线空状态和任务日志区。
- **视觉**：全局采用绚彩渐变磨砂风格，以蓝紫、洋红、青色和蜜桃色形成背景光色；导航、按钮、表格、曲线和输入框统一使用半透明表面、玻璃高光与渐变选中态。顶部保留 26–28px 强字重中文标题和渐变 Agent 铭牌，组件为 `src/view/components/AppHeader.slint`。磨砂感由柔化背景与半透明表面实现，不依赖操作系统背景模糊。界面不显示“实例分割”字样，首版业务范围不变。
- **聊天**：Agent 标题栏仅保留清除聊天记录按钮，不显示连接状态和设置入口；清除本地记录不清空输入草稿，连接配置仍在左侧设置页。
- **文案**：不显示常驻的工作区名称、版本号、开发说明与重复引导。保留控件名称、数据与必要连接状态；操作反馈使用可关闭、5 秒后自动消失的提示。未连接 Agent 的本地消息明确标记“未发送”，不伪装发送成功。
- **状态**：切页保留聊天输入、最近一条本地消息、训练参数草稿、标注浏览模式及图像源选项。未接入的操作会明确提示；不会读取目录、调用模型或启动网络监听。参数草稿尚无业务校验与持久化。预标注确认属于本地 UI 状态，不代表后端业务审核；后续 Agent 接入需关联请求与草稿版本，拒绝过期结果。

生成并检查全部页面与两种标注布局（1440 × 900、1120 × 720），以及折叠导航、模型列表测试夹具和长消息截图：

```sh
python3 depoly/tests/ui_preview.py
```

截图保存在 `depoly/target/ui-preview/light/`。脚本仅依赖 Python 标准库和项目内 Slint Viewer，检查组件与文件同名、一组件一文件、导入路径、编译、渲染退出状态与截图尺寸；视觉布局需要查看截图。单页截图：

```sh
mkdir -p depoly/target/ui-preview
depoly/tools/slint/bin/slint-viewer --screenshot depoly/target/ui-preview/run-light.png src/view/MainWindow.slint
```

此前已在 Linux Slint Viewer 1.17.1 检查上述截图，并以原生窗口验证导航、本地图像源选择、标注布局切换、训练参数切页保留和聊天发送。另外验证了模型页导航、列表选中、鼠标／键盘折叠，以及折叠时保留当前页面与草稿。模型列表夹具只用于布局测试，不是已读取的本地模型。另已验证操作反馈按需出现、5 秒后自动消失且不影响聊天草稿。本轮还验证了清除聊天记录、重复清除和切页后草稿保留。另以明确标记的分析夹具验证了预标注确认、进入标注、编辑失效和折叠后保留草稿；没有执行真实 Agent 分析。本次目录整理的 27 组重构前后截图逐像素一致。上述为独立 Viewer 阶段的验证记录。2026-09-09 已另外验证 Rust 宿主的 Linux 窗口启动、取像与正常关闭；Windows 及业务流程仍未实机验证。

### 尚未接入的部分

项目内已准备 Slint 1.17.1 的 `slint-viewer`、`slint-lsp`，具体路径和临时 PATH 用法见[开发工具说明](depoly/tools/slint/README.md)。它们只用于开发与验证，没有修改全局 PATH，也没有启用 MCP。

`src/view/` 已包含实际 Slint 界面与自绘 SVG 资源；`depoly/packaging/python/` 和打包目录仍为占位。产品资源统一保留在 `src/view/resources/`，其中 `ui/` 用于界面资源，`agent/` 保留产品 Agent 资源占位。`build.rs` 已用于界面编译；`depoly/packaging/python/pyproject.toml` 及训练功能依赖仍待后续实际接入，不提供虚假的训练实现。

[src/depends/](src/depends/README.md) 用于管理第三方依赖来源与受控材料，目前已记录 Slint 宿主依赖及许可；Rust 依赖由 Cargo 获取与锁定。`depoly/` 集中放置打包内容、开发辅助工具、集成测试和编译缓存，不是独立后端程序。

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

产品需求文档是 **PRD，不是代码实施计划**。当前已完成工程骨架与独立预览的界面初版，后续业务与 runtime 接入仍按用户明确范围推进。

## 许可证

仓库现有许可证见 [LICENSE](LICENSE)。第三方代码、模型权重和运行时的许可及分发条件，应在确定具体依赖和发布方式后单独核查。

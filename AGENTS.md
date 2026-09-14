# Repository Guidelines

VisionHyperAgent 是 Agent 驱动的视觉模型标注与自动训练桌面软件，正在从 Rust+Python 重写为纯 Python（PySide6 + QML）。设计基线见 `docs/superpowers/specs/2026-09-14-pure-python-rewrite-design.md`。

## Project Structure & Module Organization

- `src/vision_hyper_agent/`：源码，按六层组织
  - `basic/`：配置、日志、路径、环境体检
  - `drivers/`：第三方适配（ultralytics_train、codex）
  - `train_app/`：数据集、标注存储、训练编排（业务事实唯一来源）
  - `agent/`：Codex 会话管理、工具注册、技能加载
  - `viewmodels/`：QObject ViewModel，不依赖 QML 类型
  - `view/`：QML（`pages/`、`components/`、`resources/`）
  - `main.py`：唯一入口
- `tests/`：pytest 测试；`deploy/`：后续打包配置；`docs/`：设计文档
- 依赖方向：`view → viewmodels → (agent | train_app) → drivers → basic`，禁止反向。

## Build, Test, and Development Commands

- `uv sync`：创建环境并安装依赖（CUDA 版 torch 索引待定，暂未锁定）
- `uv run python -m vision_hyper_agent.main`：运行入口
- `uv run pytest`：运行测试
- `uv run ruff check .` / `uv run ruff format .`：检查/格式化

## Coding Style & Naming Conventions

- Python：PEP 8，包/模块小写下划线（`train_app` 非 `TrainApp`），类 PascalCase，函数/变量 snake_case；4 空格缩进。
- QML：文件 PascalCase（`MainWindow.qml`），一组件一文件。
- 注释与文档用中文；异常分层定义，跨层不丢上下文。
- 业务规则只在 `train_app`；`drivers` 只封装第三方调用，不写业务。

## Testing Guidelines

- 框架：pytest（UI 冒烟用 pytest-qt）。
- 命名：`test_<模块>_<行为>.py`，如 `test_dataset_states.py`。
- 各模块测试标准由用户在开发该模块时逐步给出，先确认再写测试。
- 真实训练测试必须跑 NVIDIA GPU；无 GPU 时训练功能明确不可用，不做 CPU 兜底。

## Commit & Pull Request Guidelines

- 提交信息沿用历史惯例：`type(scope): 中文描述`，如 `chore(project): 初始化纯 Python 工程骨架`、`docs(spec): 确立纯 Python 重写设计方案`。
- PR 需含变更说明、测试方式、关联问题；UI 改动附截图。
- 不虚构完成状态：训练/标注未完成即标失败，以实际产物与指标为准。

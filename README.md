# VisionHyperAgent

Agent 驱动的视觉模型标注与自动训练桌面软件（纯 Python / PySide6 + QML 重写中）。

- 设计文档：[docs/superpowers/specs/2026-09-14-pure-python-rewrite-design.md](docs/superpowers/specs/2026-09-14-pure-python-rewrite-design.md)
- 首版范围：预标注 → 批量标注 → 训练

# 本地运行

项目当前只使用仓库内 `.venv`，不安装到 conda base。CUDA 版 torch 索引确定前，避免执行 `uv sync` 或带依赖解析的 editable 安装：

```bash
uv venv .venv
uv pip install --python .venv/bin/python PySide6 pytest pytest-qt ruff
uv pip install --python .venv/bin/python --no-deps -e .
.venv/bin/vision-hyper-agent
```

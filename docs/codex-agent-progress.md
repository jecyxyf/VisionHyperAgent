# Codex 接入实施记录

实现及本机验收已完成，完整结果请看 `codex-agent-test-report.md`；配置和运行方法见 `codex-agent-usage.md`。

- 后端独占 Codex 进程的启停和回收；CodexAgent 只负责 WebSocket 协议。
- 前端真实接通聊天、流式、停止、模型/Effort、历史、附件、审批与问题回答，保留原布局。
- 已使用用户指定的 MiniMax-M3 完成真实模型、工具、文件/图片附件与浏览器验收。
- 修复了上游流式协议、后台命令停止、审批选项、未提交草稿归档和状态同步等联调问题。
- 产物位于 `bin/`，私有配置被 Git 忽略；模型密钥不进入源码、日志或公开压缩包。
- Windows 已交叉构建并具备相应逻辑/测试代码，**尚未实机验证**，不能将编译通过描述为 Windows 实机通过。
- 当前修改没有自动提交或推送。

## 验证入口

- Rust 全工作区测试、Clippy、原生 Codex 显式测试：`source/backend/`。
- 前端单元测试：`source/frontend/tests/unit/`。
- 浏览器真实链路与单独标记的 UI 模拟测试：`source/frontend/tests/e2e/`。
- 本机证据、报告 JSON 与截图：`bin/test-artifacts/`（忽略目录）。

后续 Windows 设备验收按使用说明进行；训练、标注与推理业务不属于本轮新增功能。

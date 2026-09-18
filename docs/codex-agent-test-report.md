# Codex 后端与前端接入测试报告

## 验证范围

以 `docs/codex-agent.md` 为验收基线。实现包含后端拥有 Codex 进程、WebSocket 调用库、现有 Agent 面板接入、附件、审批、状态恢复和退出清理。

测试使用本机 Codex 0.154.0 和用户指定的 MiniMax-M3 服务。真实模型测试均在自建隔离目录中进行，不向模型提交本仓库源码。

## 自动化结果

| 层次 | 结果 | 证据 |
| --- | --- | --- |
| Rust 全工作区 | 103 个测试入口通过；其中 1 个为子进程夹具入口 | `bin/test-artifacts/final/workspace-tests.txt` |
| 原生 Codex 协议 | 显式启用的 1 项通过 | `final/native-protocol.txt` |
| 原生 Codex 宿主生命周期 | 显式启用的 2 项通过 | `final/native-lifecycle.txt` |
| 前端单元测试 | 18 项通过 | `source/frontend/tests/unit/` |
| 浏览器端到端 | 10 项通过，0 失败、0 跳过、0 重试通过 | `final/browser-results.json` |
| Rust Clippy | `--all-targets -- -D warnings` 通过 | `final/clippy.txt` |
| Rust 格式检查 | 相关 crate 通过 | `cargo fmt … -- --check` |
| 前端与测试类型检查 | Svelte、TypeScript 均通过 | `npm run check` / `check:tests` |
| Windows 目标 | 包含测试代码的交叉检查通过，无警告 | `final/windows-check.txt` |
| Linux / Windows release | 构建通过 | `final/linux-build.txt` / `windows-build.txt` |

以上 `final/` 路径均位于 `bin/test-artifacts/`，不进入 Git。
原生测试平时标记为 ignored，是为了避免在未配置环境自动启动 Codex；本次已单独执行，不将“跳过”计为通过。

## 真实浏览器与模型用例

7 个真实后端/原生 Codex/MiniMax-M3 用例：

1. 发送文本，观察真实增量和实际 `completed` 通知，刷新恢复历史，清空创建新上下文。
2. 拖入、移除附件，上传真实文件，模型通过 Codex 工具读取未知随机标记；检查工具事件及刷新后的记录。
3. 图片经拖拽、上传、Codex、本机协议兼容层传递给模型，正确返回合成图片四个区域的颜色。
4. 新建空会话、恢复已有会话、归档已持久化会话，以及释放未发送消息的草稿。
5. 在界面允许精确审核过的、针对自建普通文本文件的操作，验证文件确实被修改。
6. 在界面拒绝写操作，验证文件保持原内容。
7. 启动真实等待命令、关闭网页、验证命令和 Codex 保持运行；重新打开后停止回合，确认本轮命令退出且 Codex 不退出、不重放消息。

另外 3 个明确使用模拟 WebSocket 的 UI 用例：

- 不支持图片时保留草稿和附件，并显示真实错误语义。
- 模型文本中的 HTML/脚本只作为文本显示，不能执行。
- 文件修改审批、问题选项/自定义答案保持原请求身份，不误提交到其他请求。

合成图片用例只验证图像输入链路和基础识别，不代表工业视觉准确率已经评估。
审批测试不会自动允许任意命令：只接受测试自建文本文件上的明确读写操作，其他命令会使测试失败。

## 进程与安全验收

| 要求 | 实际结果 |
| --- | --- |
| 主程序自动启动 Codex | 已验证后台状态及真实子进程 PID |
| 真实 Linux 托盘退出 | 调用本应用托盘“退出”菜单后，HTTP 关闭，宿主与 Codex 均被回收，约 0.113 秒 |
| 主程序遭 SIGKILL | 直属 Codex 随父进程退出，HTTP 关闭，约 0.05 秒；未手工结束子进程来伪造结果 |
| 网页关闭不控制后台生命周期 | 真实浏览器关闭、重开及继续停止任务验证通过 |
| Codex 意外退出 | 保留 HTTP/UI、显示失败、清除运行/审批状态，不自动拉起新进程 |
| 初始化错误/端口占用 | 保留可观察错误，不连接或结束其他占用端口的程序 |
| Windows 启动顺序 | 生产路径共用的“先加入 Job 后恢复”和“失败回滚回收”逻辑已注入故障测试；Win32 路径交叉检查通过 |
| 私有 Codex WebSocket | 缺失/错误令牌返回 401；带浏览器 Origin 返回 403 |
| 模型兼容层 | 拒绝浏览器 Origin 与无令牌请求；上游鉴权失败不回显上游原始错误体 |
| 凭据 | 受跟踪/未忽略文件和应用日志扫描无测试密钥；本机私有配置被 Git 忽略且权限为 600，不加入 Windows 压缩包 |
| 附件 | 检查大小、路径与符号链接；只处理本应用登记的 ID；保护历史引用，正常退出清理未引用上传 |

生命周期证据：`actual-host-sigkill.json`、`final/actual-tray-exit.json`、`native-ws-security.json`。
隐私证据：`final/credential-audit.json`。

**Windows 说明：没有在 Windows 实机执行托盘和 Job Object 测试。**
本报告中的 Windows 结果是生产路径逻辑测试与交叉编译结果，不能解释为实机行为已经验证。真实 Windows 行为仍应按使用说明进行设备验收。

## 联调发现并修复的问题

- 测试网关的 Responses 流没有正确结束：新增显式 Chat Completions 兼容模式，不替换 MiniMax-M3，不绕过 Codex，不把截断流伪装成完成。
- Codex 工具命名空间和自由格式补丁：转换后恢复原始工具身份与调用 ID，避免命令发往错误工具。
- 单独 `turn/interrupt` 不会结束所有已转后台的命令：按当前回合的 item ID 精确终止，保留其他回合的后台命令与 Codex 进程。
- 某些审批只有 accept/cancel：界面明确说明拒绝会结束当前回合，不扩展服务端允许的选项。
- 空会话没有 rollout，直接 archive 会失败：未提交草稿解除订阅并从列表移除；真实历史仍走归档。
- 回合完成先于开始响应、重复发送、初始化竞争：通过连接代次、请求关联、回合占位与客户端消息 ID 处理。
- 传输库详细日志可能记录正文：屏蔽敏感传输层日志，由应用输出不含正文/密钥的分类诊断。

## 交付

- `bin/VisionHyperAgent`
- `bin/VisionHyperAgent.exe`
- `bin/VisionHyperAgent-windows-x64.zip`
- 使用说明：`docs/codex-agent-usage.md`
- 无密钥配置示例：`docs/codex-agent-config.example.toml`

产物 SHA256 与文件大小见 `bin/test-artifacts/final/artifacts.json`。
Windows 包不含真实密钥或 Linux 的 Codex 二进制，需要 Windows Codex 原生可执行文件。
未经用户要求，本次代码与文档不提交、不推送。

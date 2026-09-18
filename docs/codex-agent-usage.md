# Codex Agent 使用与验证

## 运行

宿主默认打开 `http://127.0.0.1:8420/` 并显示系统托盘。后台自动启动 **自己拥有的** Codex App Server。

- 网页关闭或刷新：Codex 和进行中的任务保持运行。
- 聊天“停止”：中断当前回合，并通过 Codex 的终端控制协议结束本回合的命令；不关闭 Codex，不结束其他回合的后台命令。
- 托盘“退出”：关闭连接、回收 Codex 及自有进程资源，然后退出宿主。浏览器标签页可以保留。
- 初始化失败：网页与托盘保留，Agent 区域显示配置/安装错误，日志继续可用。

Linux：`bin/VisionHyperAgent`。
Windows：`bin/VisionHyperAgent.exe`。需要 Windows 版 Codex 原生可执行文件；不能使用 Linux 的 Codex 二进制。

## 配置位置

把 `docs/codex-agent-config.example.toml` 复制为 **可执行文件旁边**的 `config.local.toml`，填写模型服务地址、模型名和私有凭据。
该文件已加入 Git 忽略规则；不要将含真实密钥的配置添加到版本库或公开安装包。
Unix 上建议配置权限为 `600`，应用目录必须可写。

环境变量优先于文件：

| 变量 | 用途 |
| --- | --- |
| `VHA_CODEX_PATH` | Codex 原生可执行文件路径 |
| `VHA_CODEX_BASE_URL` | 模型服务的 `/v1` 基础地址 |
| `VHA_CODEX_MODEL` | 模型 ID |
| `VHA_CODEX_API_KEY` | 私有 API 密钥 |
| `VHA_CODEX_WORKSPACE` | Agent 工作目录 |
| `VHA_CODEX_PORT` | Codex 本机端口，默认 8421，不能与网页 8420 冲突 |
| `VHA_CODEX_WIRE_API` | `responses` 或 `chat_completions` |
| `VHA_LOG_LEVEL` | `error`、`warning`、`info`、`debug` |

Codex 查找顺序：显式路径 → 应用旁的 `codex[.exe]` → `depends/codex/codex[.exe]` → PATH。
对已安装的 npm 包会解析其原生程序，避免中间 Node 包装进程改变父子关系。
本次核对与实测版本为 `codex-cli 0.154.0`；当前回合后台命令清理使用该版本的实验性终端 API。

应用生成的 Codex 配置和会话数据放在 `<应用目录>/data/codex/`，不修改用户的 `~/.codex`。
默认工作目录是 `<应用目录>/data/workspace/`；实际项目可以指定自己的目录。
日志位于 `<应用目录>/logs/VisionHyperAgent.YYYY-MM-DD.log`。

## 模型协议

默认 `wire_api = 'responses'`，模型服务需要提供正常的流式 Responses API。

本次用户提供的 MiniMax-M3 服务，Responses 流式在末尾返回 `upstream stream closed before [DONE]`；Chat Completions 流式则正常。因此该测试服务需要：

```toml
model = 'MiniMax-M3'
wire_api = 'chat_completions'
```

此模式在 **同一个 Rust 后端进程**内增加协议兼容路由，不增加额外后台程序，也不绕过 Codex：

```text
浏览器 ↔ Rust 后端 ↔ WebSocket ↔ Codex App Server
                 ↑                 │
                 └─ 私有兼容路由 ←─┘
                       ↕
                 模型 Chat API
```

兼容层保留工具名称空间、工具调用 ID、函数参数、自由格式补丁输入、文本/图片内容；只有明确收到结束原因及 `[DONE]` 才产生完成事件。
中断、截断、异常不会被伪造成成功。该模式不提供模型供应商托管的 Responses web-search，但 Codex 本地工具、沙箱、技能和人工审批保持启用。
私有路由拒绝浏览器 Origin，并要求每次启动生成的独立令牌；外部模型密钥不进入浏览器或 Codex 工具环境。

## 界面语义

- 模型由后端配置提供，不使用之前的 Default/Fast/Reasoning 假选项。
- Effort 原样传递给 Codex，不把 `max`、`ultra` 偷换成其他档位；实际模型是否支持或采用该档位由服务决定。
- “清空”：新建上下文，不删除所有历史。
- 历史“归档”：已有内容的会话交给 Codex 归档；未发送消息的草稿解除订阅并从应用列表移除。
- 拖拽附件：真实上传，单文件最大 16 MiB，每条最多 16 个；不增加上传按钮。
- 发送失败保留输入和附件。已被会话引用的附件不会被删除，未引用的临时上传在正常退出时清理。
- 图片需明确配置 `supports_images = true`。本次 MiniMax-M3 已验证接受图片及合成色块输入，但这不代表工业视觉识别准确率已经验证。
- 审批只支持本次允许/拒绝/取消，不自动授予持久权限。某些 Codex 请求只支持“允许或取消”，此时拒绝会结束当前回合，界面会说明。

## 开发测试

```bash
cargo test --manifest-path source/backend/Cargo.toml -p vha-common -p vha-codex-agent -p vha-server
cargo clippy --manifest-path source/backend/Cargo.toml -p vha-common -p vha-codex-agent -p vha-server --all-targets -- -D warnings
npm run check --prefix source/frontend
npm run check:tests --prefix source/frontend
npm run test:unit --prefix source/frontend
```

原生 Codex 测试显式启用，使用隔离目录与本地假服务，不向外部模型发送请求：

```bash
VHA_TEST_CODEX_NATIVE=/path/to/native/codex \
  cargo test --manifest-path source/backend/Cargo.toml -p vha-codex-agent --test real_codex -- --ignored
VHA_TEST_CODEX_NATIVE=/path/to/native/codex \
  cargo test --manifest-path source/backend/Cargo.toml -p vha-server --test lifecycle -- --ignored
```

实际浏览器测试位于 `source/frontend/tests/e2e/`，使用 Playwright 和本机 Chrome。运行前需启动配置好的测试宿主，使用专用测试目录，避免给模型访问真实业务文件：

```bash
# 同一个主程序，无托盘测试模式；SIGTERM/SIGINT 仍会清理 Codex。
./VisionHyperAgent --headless
npm run test:e2e --prefix source/frontend
```

可用 `VHA_TEST_BASE_URL` / `VHA_TEST_CHROME` 指定测试服务地址和浏览器路径。
真实模型测试会产生 API 调用费用。审批测试只允许针对测试自建文本文件的精确读写命令；命令解析校验使用 Python 标准库，不是应用运行依赖。
测试报告和截图输出到被 Git 忽略的 `bin/test-artifacts/`。

Windows 当前需要在真实 Windows 环境验证托盘与 Job Object 的实际行为；交叉编译成功不能代替实机验收。

# Agent 配置与使用

## 运行

宿主默认打开 `http://127.0.0.1:8420/` 并显示系统托盘。后端自动启动并拥有唯一的 Codex App Server 子进程。

- 浏览器关闭或刷新：后端、Codex 和进行中的任务继续运行。
- 聊天“停止”：中断当前回合并清理该回合自有后台命令，不退出 Codex。
- 托盘“退出”：中断任务、停止 Codex、关闭 HTTP，然后后端进程退出。
- 配置失败：保留网页、托盘和日志入口，界面显示可修复的错误。

Linux 可执行文件为 `bin/VisionHyperAgent`；Windows 为 `bin/VisionHyperAgent.exe`，并需要同平台的原生 Codex 可执行文件。

## app_config.json

把 `docs/app-config.example.json` 复制为 **可执行文件目录** 下的 `app_config.json`，填入私有密钥。该文件不进入 Git；Unix 权限建议为 `600`。文件不存在时会自动生成空默认配置；JSON 损坏时会备份为 `app_config.json.invalid-时间戳` 并重建默认文件。

配置说明：

| 字段 | 说明 |
| --- | --- |
| `agent.activeModelId` | 启动 Codex 时使用的默认 `modelId`，必须存在于下方模型列表 |
| `providers[].id` | Provider 标识，全局唯一 |
| `providers[].baseUrl` | 上游 `/v1` 基础地址；必须是 HTTP/HTTPS，不能带凭据、query 或 fragment |
| `providers[].apiKey` | 上游私有密钥；只保存在后端配置文件 |
| `providers[].wireApi` | `responses` 透传，或 `chat_completions` 走内置协议转换 |
| `models[].modelId` | 前端显示和请求使用的唯一模型 ID |
| `models[].modelName` | 上游真实模型名，网关转发前替换 |
| `models[].supportedEfforts` | 该模型允许的 Effort 列表 |
| `models[].effort` | 该模型默认 Effort，必须在支持列表内 |
| `models[].supportsImages` | 是否允许拖入图片附件 |
| `codex.executable` | 可选原生 Codex 路径；空值按应用目录、`depends/codex` 和 PATH 查找 |
| `codex.workspace` | Agent 工作目录，相对路径基于应用目录 |
| `codex.home` | 独立 `CODEX_HOME`，不修改用户 `~/.codex` |
| `codex.port` | 本机 Codex WebSocket 端口，不能使用 8420 |

启动时后端会校验 Provider/Model ID、URL、端口、超时、Effort 支持和图片能力，并建立 `modelId -> ResolvedModel` 哈希索引；发送请求时 O(1) 查找，不做逐次遍历。

Codex 提议提升执行策略时，界面会出现“本次允许并应用提议权限”。后端只原样转发 Codex 给出的修正对象，且要求它与 `availableDecisions` 完全一致，前端不能伪造或扩大权限。

旧版 `config.local.toml` 只做一次性自动迁移。若同名 `app_config.json` 已存在，旧文件会被忽略。环境变量 `VHA_CODEX_*` 仅用于测试覆盖，不会写回 JSON。

## 模型请求流

```text
前端 turn.start(modelId)
→ AgentService O(1) 查找并校验
→ Codex App Server
→ 127.0.0.1/internal/model/v1
→ Model Gateway 按 modelId 查找 Provider
→ 替换请求 model 为上游 modelName
→ 按 wireApi 转发或协议转换
```

内部网关每次启动生成随机 Bearer token，拒绝浏览器 Origin。Provider 密钥不进入前端、日志、Codex `config.toml`、命令行参数或工具环境。

## 日志与测试

日志位于 `<应用目录>/logs/VisionHyperAgent.YYYY-MM-DD.log`。`DEBUG`、`WARNING`、`ERROR` 带来源文件和行号，日志不记录消息、附件和密钥。

```bash
cargo fmt --manifest-path source/backend/Cargo.toml --all
cargo clippy --manifest-path source/backend/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path source/backend/Cargo.toml --workspace
npm run check --prefix source/frontend
npm run check:tests --prefix source/frontend
npm run test:unit --prefix source/frontend
```

真实 Codex 生命周期测试通过 `VHA_TEST_CODEX_NATIVE` 显式启用；真实模型 E2E 会产生外部 API 调用费用，不能作为默认测试。

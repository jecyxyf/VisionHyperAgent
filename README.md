# VisionHyperAgent

Agent 驱动的视觉模型桌面软件。

## 架构

- **前端**：Svelte + TypeScript + Vite，浏览器界面。
- **后端**：Rust + axum，本机 HTTP/WebSocket 服务及系统托盘。
- **Agent**：后端启动并持有 Codex App Server；`CodexAgent` 通过 WebSocket 调用，不自行管理进程。
- **标注**：Canvas 2D。训练、标注和推理业务仍属于后续阶段。
- **架构文档**：当前 Agent 前后端模块与接口见 docs/frontend-backend-architecture.md。

## 直接运行

Linux 使用 `bin/VisionHyperAgent`，Windows 使用 `bin/VisionHyperAgent.exe`。
启动后自动打开 `http://127.0.0.1:8420/`；从托盘退出会回收 Codex。关闭网页不会关闭后端。

需要可用的 Codex 原生可执行文件，以及应用目录内的私有 `config.local.toml` 或对应环境变量。
配置示例：`docs/codex-agent-config.example.toml`。
详细说明：`docs/codex-agent-usage.md`。

## 开发构建

在仓库根目录运行：

```bash
(cd source/frontend && npm ci && npm run build)
cargo build --manifest-path source/backend/Cargo.toml --release -p vha-server
```

或者执行 `./build.sh` 构建并更新 `bin/` 中的本平台产物。
修改前端后需重新构建后端，才能更新内嵌页面。

开发调试：

```bash
# 已完成前端 build 后，在独立终端启动后端。
cargo run --manifest-path source/backend/Cargo.toml --bin vha-server
# 前端开发服务器代理 /api 和 /ws 到本机后端。
(cd source/frontend && npm run dev)
```

Cargo 调试可执行文件的配置目录是 `source/backend/target/debug/`，并非仓库根目录。
模型服务密钥不要加入源码或提交记录；应用的独立 Codex 配置不会修改 `~/.codex`。

## 测试

```bash
cargo test --manifest-path source/backend/Cargo.toml --workspace
npm run check --prefix source/frontend
npm run check:tests --prefix source/frontend
npm run test:unit --prefix source/frontend
```

真实 Codex 和浏览器测试的准备条件见 `docs/codex-agent-usage.md`；测试结论见 `docs/codex-agent-test-report.md`。

## 目录

```text
docs/                       需求、设计、使用说明及测试报告
source/frontend/            Svelte / TypeScript 前端
source/backend/common/      全局日志等公共设施
source/backend/server/      宿主、进程所有权、浏览器接口、附件、模型协议兼容层
source/backend/core/        配置/事件总线基础模块
source/backend/model/
  codex_agent/              Codex WebSocket 客户端和进程操作原语
source/backend/depends/     第三方依赖源码
bin/                        构建产物、本机私有配置和忽略的测试证据
```

# CodexDDSAgent Test UI

这是给 CodexDDSAgent 手动验收用的轻量 Web UI，内置 mock Codex App Server，不需要真实模型服务。

## 启动

\`\`\`bash
cd source/CodexDDSAgent/test
cargo run -- --open
\`\`\`

默认地址：<http://127.0.0.1:17700>，默认数据目录是系统临时目录下的 `CodexDDSAgentTestUI`。

常用参数：

\`\`\`bash
cargo run -- --http-port 17701 --home /tmp/CodexDDSAgentTestUI-A
cargo run -- --no-discovery
\`\`\`

## 单个桌面测试

1. 点击“启动本地服务”；固定端口可留空。
2. 确认 Endpoint 已自动填入，并点击“连接 Endpoint”。
3. 修改 Agent 名称，点击“Create Agent”。
4. 依次验证“状态”“发送 RPC”“事件与状态流”。
5. 点击“Detach”，确认状态变为 stopped，配置仍保留。
6. 点击“Attach”重新绑定；只有首次 Attach 勾选模型配置。
7. 停止发送心跳后等待 5 秒，runtime 应自动停止。
8. 测试完成后点击“关闭本地服务”。

## 多桌面测试

可以在同一服务上创建多个独立 Agent：

\`\`\`bash
cargo run -- --http-port 17700 --home /tmp/CodexDDSAgentTestUI-A
cargo run -- --http-port 17701 --home /tmp/CodexDDSAgentTestUI-B
\`\`\`

1. 第一个 UI 启动本地服务，记录 Endpoint。
2. 第二个 UI 输入同一个 Endpoint，点击“连接 Endpoint”。
3. 两个 UI 分别使用不同 Agent 名称执行 Create。
4. 分别发送 RPC 和心跳，确认请求结果、事件和反向请求不串扰。
5. 任一 UI Detach 或停止心跳，只影响自己的 Agent。

## 自动测试

\`\`\`bash
cd source/CodexDDSAgent
cargo test -p codex-dds-testui --test test_ui -- --test-threads=1
cargo test --workspace --all-targets
\`\`\`

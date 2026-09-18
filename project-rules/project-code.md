# 代码规范

## 目标

用最小、清晰、可验证的改动实现需求，并保持应用在 Linux 和 Windows 上的稳定性。

## 实现原则

1. 修改前先阅读被改代码及其调用方，尊重现有命名、错误处理、日志和 UI 风格。
2. 保持最小 diff，不做无关重构，不改变用户未要求的界面布局。
3. Rust 异步代码必须考虑取消、超时、锁持有时间、任务退出和错误传播。
4. 涉及子进程、端口、托盘和配置重载时必须验证正常路径、失败路径和退出清理。
5. 前端状态更新要避免竞态；WebSocket 断开、重连、发送失败和等待提示必须有明确行为。
6. 日志应包含定位问题所需的上下文，禁止输出 API Key、请求头中的密钥和用户私有数据。
7. 公共接口需要稳定类型和错误信息；不为了测试方便放宽业务约束。

## 测试要求

- 单元测试覆盖边界条件、错误分支和状态转换。
- 集成测试覆盖模块之间真实调用路径，不只测试结构体序列化。
- 前端测试应验证 observable 行为，例如状态、渲染结果或 API 调用，不追求形式化覆盖率。
- 修 Bug 时先写能复现问题的测试，再修复并确认测试通过。

## 验证命令

后端：

- cargo fmt --manifest-path source/backend/Cargo.toml --all -- --check
- cargo clippy --manifest-path source/backend/Cargo.toml --workspace --all-targets -- -D warnings
- cargo test --manifest-path source/backend/Cargo.toml --workspace

前端：

- npm run check --prefix source/frontend
- npm run check:tests --prefix source/frontend
- npm run test:unit --prefix source/frontend

如果完整验证因环境缺失无法执行，必须明确说明已执行和未执行的命令，不能用推测代替验证结果。

## 完成前复查

- 检查完整 diff，确认没有无关文件、调试输出、真实密钥和临时代码。
- 检查错误分支、资源释放、并发安全、跨平台差异和用户可见提示。
- 确认新增行为有对应测试或说明无法测试的具体原因。

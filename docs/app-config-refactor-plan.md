# App 配置重构计划

## 目标

`app_config.json` 支持多 Provider、多模型和 Codex 参数；启动时建立 `model_id -> ResolvedModel` 索引，前端请求时直接 O(1) 查找。

## 执行计划

1. 新增 `source/backend/common/src/config.rs`
   - 通用 JSON 读取。
   - 文件不存在时创建默认配置。
   - JSON 损坏时备份并重建。
   - 原子写入。
   - 增加单元测试。

2. 重构 `source/backend/model/codex_agent/src/config.rs`
   - 定义 `AppConfig`、`AgentConfig`、`ProviderConfig`、`ModelConfig`、`CodexConfig`。
   - 各结构体自带默认值。
   - 校验 Provider、Model、Effort、URL、端口和路径。
   - 校验 `modelId` 全局唯一。
   - 增加 `resolve()`，生成 `ResolvedAgentConfig` 和 `HashMap<String, Arc<ResolvedModel>>`。
   - 增加配置解析测试。

3. 接入配置加载
   - 在 `codex_agent` 中实现 `app_config.json` 加载。
   - 支持从旧 `config.local.toml` 一次性迁移。
   - 环境变量只做测试覆盖，不写回 JSON。
   - 增加迁移测试。

4. 删除重复配置代码
   - 删除 `source/backend/core/src/config.rs`。
   - 删除 `source/backend/server/src/app_config.rs`。
   - 删除 `source/backend/server/src/codex_config.rs`。
   - `server` 只消费解析后的 Agent 参数，不再定义 Agent 配置模型。

5. 接入 Agent 后端运行时
   - `AgentRuntime` 使用 `ResolvedAgentConfig` 启动 Codex。
   - `AgentService` 保存模型索引并提供全部模型列表。
   - `turn.start` 根据 `modelId` 直接取模型。
   - 校验模型支持的 Effort 和图片附件能力。

6. 重构模型网关
   - 根据 `modelId` 选择 Provider 和模型。
   - 将请求中的 `modelId` 替换成上游 `modelName`。
   - 保留 `chat_completions` 到 `responses` 的流式协议转换。
   - 支持 `responses` Provider 流式透传。
   - 增加多 Provider、多模型路由测试。

7. 接入前端
   - 模型 ComboBox 显示并提交 `modelId`。
   - Effort 选项来自当前模型的 `supportedEfforts`。
   - 切换模型后使用该模型默认 `effort`。
   - 不支持图片的模型拒绝图片附件。
   - 更新前端单元测试和 E2E 测试。

8. 完成验证
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test --workspace`
   - 前端 check / unit test / e2e。
   - 重新构建 Linux 和 Windows 产物。

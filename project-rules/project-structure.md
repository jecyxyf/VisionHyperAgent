# 代码结构规则

## 目标

让每个文件都有明确归属，保持模块职责清晰，避免重复模块、随意放置和无意义空目录。

## 目录职责

- source/frontend/src/pages/：页面级组件。
- source/frontend/src/components/：按业务域组织的前端组件。
- source/frontend/src/lib/api/：后端 API 和 WebSocket 封装。
- source/frontend/src/lib/stores/：跨组件状态。
- source/frontend/src/lib/styles/：全局样式和设计令牌。
- source/frontend/tests/unit/：单元测试。
- source/frontend/tests/e2e/：端到端测试。
- source/backend/common/：多个 Rust crate 复用的日志、配置等基础能力。
- source/backend/core/：后端核心基础模块。
- source/backend/server/：宿主进程、路由、托盘、嵌入资源和模型网关。
- source/backend/model/codex_agent/：Codex 配置、客户端和进程管理。
- source/backend/server/tests/ 和 source/backend/model/codex_agent/tests/：后端集成测试。
- docs/：使用说明、架构说明、示例配置和测试报告。
- docs/plans/：临时开发计划文档及需求设计进度记录。
- project-rules/：项目规则与需求技能；需求技能的模板内置于 project-rules/project-requirement/templates/。

## 放置规则

1. 前端页面入口放 pages，可复用 UI 放 components，通用逻辑放 lib；只有被两处以上使用时才提升为公共组件。
2. Rust 代码优先放入职责最接近的现有 crate；只有出现清晰边界且被复用时才新建 crate。
3. 单元测试可以放在被测模块旁，集成测试放在对应 crate 的 tests/ 目录。
4. 用户文档、示例配置和测试报告放 docs/；不要把临时报告散落在仓库根目录。
5. 新增 Rust 模块后同步更新 mod.rs、lib.rs 或 main.rs；新增 crate 后同步 workspace 成员和依赖关系。
6. 新增前端源码后确认导入路径、导出方式、类型检查和测试命令。
7. 不修改 source/backend/depends/；不为占位创建空文件或空目录。
8. project-requirement 新生成的正式文档放 docs/；模块 API 使用 docs/models/<架构层名称>/<模块名>.md，不增加同名模块子目录。已有历史文档不因本规则自动迁移。

## 完成检查

提交前查看 git diff --stat 和 git status，确认没有把构建产物、日志、私有配置或临时文件加入仓库。

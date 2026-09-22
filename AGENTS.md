# 项目文档路由

AGENTS.md 只负责路由，不承载项目规则本身。每次执行任务前，先根据任务类型读取对应文档，并把其中的规则作为本次任务的执行约束。

## 路由表

| 任务类型 | 必读文档 |
| --- | --- |
| 前期对话、产品需求、技术方案、目录结构、模块定义、接口定义或全局规划 | [project-requirement](project-rules/project-requirement/SKILL.md)、[产品需求文档](docs/产品需求文档.md)、[技术方案设计](docs/技术方案设计.md) |
| 设计或修改模块公开数据结构、方法、消息、委托或跨模块契约 | [project-requirement](project-rules/project-requirement/SKILL.md)、[技术方案设计](docs/技术方案设计.md)、[模块 API 目录](docs/models/) 中对应架构层和模块文档 |
| 新增、移动、调整源码、测试或文档目录 | [项目结构](docs/项目结构.md)、[project-structure](project-rules/project-structure.md) |
| 编写、修改、审查、重构或测试代码 | 先查 [需求设计进度](docs/plans/需求设计进度.md) 中相关模块和接口状态，再读 [代码规范](docs/代码规范.md)、[project-code](project-rules/project-code.md) 及对应模块 API 文档 |
| 编译、打包、部署或发布验证 | [产品需求文档](docs/产品需求文档.md)、[技术方案设计](docs/技术方案设计.md)、[project-build](project-rules/project-build.md)，并核对实际构建脚本 |
| 查阅已确认的产品需求或技术方案 | [产品需求文档](docs/产品需求文档.md)、[技术方案设计](docs/技术方案设计.md)，按查询范围读取 |
| 只读查询已有接口 | 先查 [需求设计进度](docs/plans/需求设计进度.md) 中对应模块状态，再读 [模块 API 目录](docs/models/) 中对应文档；旧文档只作标明状态的参考 |
| 按已有契约实际接入模块 | 先查 [需求设计进度](docs/plans/需求设计进度.md) 中调用双方和关联变更，再读 [代码规范](docs/代码规范.md) 与对应模块 API 文档 |
| 继续未完成的需求设计流程或核对当前进度 | [project-requirement](project-rules/project-requirement/SKILL.md)、[需求设计进度](docs/plans/需求设计进度.md) |

## 模块 API 路由

| 架构层 | 模块文档 |
| --- | --- |
| frontend | [workspace-ui](docs/models/frontend/workspace-ui.md)、[annotation-canvas](docs/models/frontend/annotation-canvas.md) |
| backend | [api-gateway](docs/models/backend/api-gateway.md)、[project-service](docs/models/backend/project-service.md)、[ml-service](docs/models/backend/ml-service.md)、[agent-service](docs/models/backend/agent-service.md)、[task-runtime](docs/models/backend/task-runtime.md)、[storage](docs/models/backend/storage.md)、[desktop-host](docs/models/backend/desktop-host.md)、[common-services](docs/models/backend/common-services.md) |
| ml-worker | [mask-rcnn-worker](docs/models/ml-worker/mask-rcnn-worker.md) |

## 调用规则

1. 只读取当前任务需要的文档，避免加载无关规则；模块 API 按架构层和模块名读取，不一次加载全部模块。
2. 跨阶段任务按实际涉及的任务类型组合读取；纯查阅不启动访谈，改变已确认方案时回到 project-requirement 对应阶段。
3. 目标文档缺失、不可读、待确认或待更新时，明确指出并恢复需求流程，不把旧文档或示例当作有效契约。
4. 编码和接口接入前必须先查相关进度，再读有效代码规范和 API；不得绕过状态核对直接按旧契约实施。
5. 文档规则与用户当前明确要求冲突时，先确认处理方式。
6. 本文件不总结、复制或改写项目规则；规则变更必须修改对应文档，不在 AGENTS.md 中重复维护。

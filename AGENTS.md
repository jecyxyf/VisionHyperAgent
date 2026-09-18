# 项目文档路由

AGENTS.md 只负责路由，不承载项目规则本身。每次执行任务前，先根据任务类型读取对应文档，并把其中的规则作为本次任务的执行约束。

## 路由表

| 任务类型 | 必读文档 |
| --- | --- |
| 前期对话、产品需求、技术框架、目录结构、模块定义、接口定义、全局规划 | [project-requirement](project-rules/project-requirement/SKILL.md) |
| 新增、移动、调整代码目录、文件、模块、测试或文档位置 | [project-structure](project-rules/project-structure.md) |
| 编写、修改、审查、重构或测试代码 | [project-code](project-rules/project-code.md) |
| 编译、打包、部署或发布验证 | [project-build](project-rules/project-build.md) |

## 调用规则

1. 只读取当前任务需要的文档，避免加载无关规则。
2. 跨阶段任务按实际阶段组合读取，例如“规划 → 调整结构 → 编码 → 编译”。
3. 文档规则与用户当前明确要求冲突时，先向用户确认冲突处理方式。
4. 本文件不额外总结、复制或改写项目规则，规则变更必须修改对应文档。

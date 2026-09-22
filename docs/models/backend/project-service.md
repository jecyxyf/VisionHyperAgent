# project-service API 文档

## 1. 概述

`project-service` 管理项目、图片、类别、标注、规则和不可变数据集版本。它是视觉业务数据的唯一写入口，所有确认版本均通过事务和来源引用保证可追溯。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | backend |
| 语言 | Rust |
| 公开边界 | 项目业务命令、查询快照和版本化结果 |
| 不负责 | Agent 推理、GPU 计算、HTTP 路由和前端绘制 |
| 一致性 | 数据库事务 + 项目文件原子写入 |

## 3. 数据结构

| 类型 | 关键字段 | 说明 |
| --- | --- | --- |
| `Project` | id、name、description、created_at | 项目元数据 |
| `ImageAsset` | id、relative_path、sha256、width、height、status | 原图只读资产 |
| `Category` | id、name、active | 类别定义 |
| `AnnotationDraft` | image_id、instances、no_object、revision | 可覆盖草稿 |
| `AnnotationVersion` | version_id、image_id、rule_version、content_hash、confirmed_at | 不可变确认版本 |
| `RuleVersion` | id、text、confirmed_at、confirmation_id | 标注规则版本 |
| `DatasetVersion` | id、manifest_path、image_ids、rule_version、stats | 不可变训练输入 |
| `InstanceMask` | instance_id、category_id、normalized_polygon、mask_path | 单个实例几何 |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `project_root` | `PathBuf` | 应用数据目录 | 所有项目文件的允许根目录 |
| `max_image_bytes` | `u64` | 配置值 | 单图大小上限 |
| `supported_formats` | `Set<String>` | jpeg/png/webp | 可导入格式 |
| `revision` | `u64` | 0 | 服务快照版本；写入后递增 |

## 5. 方法

### `create_project(input) -> Project`

校验名称后创建项目记录和目录；名称冲突返回 `project_exists`。

### `list_projects() -> Vec<ProjectSummary>`

返回按更新时间排序的项目摘要，不读取大文件内容。

### `import_images(project_id, files) -> ImportReport`

逐个校验格式、尺寸、哈希并原子保存；失败项带原因，成功项进入 `unlabeled` 状态。

### `save_draft(project_id, draft) -> AnnotationDraft`

校验归一化坐标、类别引用和图片尺寸，覆盖同一图片的草稿版本。

### `confirm_annotation(project_id, image_id, confirmation) -> AnnotationVersion`

要求存在有效规则和草稿；创建不可变确认版本，不能覆盖旧版本。

### `confirm_rule(project_id, rule) -> RuleVersion`

记录用户确认依据并把规则设为当前有效版本。

### `create_dataset(project_id, request) -> DatasetVersion`

只收集已确认图片，生成 manifest、统计和来源哈希；错误时不产生可见半成品。

### `get_snapshot(project_id) -> ProjectSnapshot`

按值返回项目、图片状态、当前规则和最新数据集摘要。

## 6. 消息与事件

`project.changed` 包含资源类型、项目 ID、变更版本和轻量摘要；`image.import.progress` 包含成功数、失败数和当前文件名；`dataset.created` 只在 manifest 和数据库记录都提交后发布。事件不携带原图和密钥。

## 7. 委托与回调

`ImportProgress = Fn(ImportProgressEvent) + Send`，回调按文件顺序串行调用；回调失败不回滚已提交图片。`ProjectEventSink` 为可选订阅，取消订阅后不再补发历史事件。

## 8. 错误处理

| 错误码 | 说明 |
| --- | --- |
| `project_not_found` | 项目不存在 |
| `project_exists` | 名称或 ID 冲突 |
| `image_invalid` | 文件损坏、格式不支持或尺寸非法 |
| `duplicate_image` | 同项目哈希重复，由调用方选择跳过或继续 |
| `annotation_invalid` | 坐标越界、类别不存在或实例重复 |
| `rule_not_confirmed` | 尚无有效规则 |
| `dataset_empty` | 没有可用确认标注 |
| `storage_failure` | 数据库或原子文件写入失败 |

失败操作不修改已有确认版本；批量导入允许部分成功。

## 9. 生命周期与线程安全

服务创建时验证项目根目录和存储依赖；所有公开方法可并发调用，写入按项目粒度串行进入事务。返回的结构体是快照，调用者修改不影响服务。关闭时等待事务完成，不接受新写入；文件写入使用临时文件和原子重命名。

## 10. 依赖关系

依赖 `storage`、`common-services` 的路径、ID、哈希和错误能力；被 `api-gateway`、`agent-service`、`ml-service` 调用。不得依赖 `desktop-host`、`mask-rcnn-worker` 或前端。

## 11. 调用示例

```rust
// 创建项目服务
let service = ProjectService::new(storage, common, project_root);
// 创建项目并导入图片
let project_list = service.list_projects().await?;
let project = service.create_project(CreateProject { name: "零件实例".into(), description: None }).await?;
let report = service.import_images(project.id, vec![file_a, file_b]).await?;
// 保存一张图片的实例草稿
service.save_draft(project.id, AnnotationDraftInput {
    image_id: report.accepted[0].image_id,
    instances: vec![InstanceMask::polygon("bolt", vec![(0.1, 0.1), (0.4, 0.1), (0.4, 0.4)])],
    no_object: false,
// 提交标注草稿事务
}).await?;
// 确认规则和标注版本
let rule = service.confirm_rule(project.id, RuleConfirmation::from_text("标注每个独立螺栓")).await?;
let annotation = service.confirm_annotation(project.id, report.accepted[0].image_id, AnnotationConfirmation { rule_id: rule.id }).await?;
// 生成可追溯数据集
let dataset = service.create_dataset(project.id, DatasetRequest::from_confirmed(rule.id)).await?;
// 确认数据集绑定了当前规则版本
assert_eq!(dataset.rule_version, rule.id);
let snapshot = service.get_snapshot(project.id).await?;
assert!(snapshot.datasets.iter().any(|item| item.id == dataset.id));
// 确认项目列表只包含有效名称
assert!(project_list.iter().all(|item| !item.name.is_empty()));
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `new` / `create_project` / `list_projects` | 服务初始化与项目创建 |
| `import_images` | 导入报告 |
| `save_draft` / `confirm_rule` / `confirm_annotation` | 标注版本流程 |
| `create_dataset` / `get_snapshot` | 数据集和快照流程 |

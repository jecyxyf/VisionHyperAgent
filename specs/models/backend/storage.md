# storage API 文档

## Overview

本文件同时是 `storage` 的 API 契约和 2119 需求来源。需求约束迁移、事务、文件原子性、路径和完整性报告。

## Requirements

### 1: 持久化完整性

1. 存储服务 MUST 使复合写入全部提交或全部回滚 [manual]
2. 存储服务 MUST 在文件校验完成后使用原子方式提交文件引用 [manual]
3. 存储服务 MUST 拒绝越出项目根目录的文件目标 [manual]
4. 存储服务 MUST 报告数据库与文件不一致并保留待处理文件 [manual]


## 1. 概述

`storage` 封装 SQLite 迁移、仓储、事务、项目文件、缩略图和完整性检查。业务模块通过仓储接口访问数据，Worker 不获得数据库连接。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | backend |
| 语言 | Rust |
| 公开边界 | 迁移、事务、仓储、文件存储和完整性报告 |
| 存储原则 | SQLite WAL + 连接池；大文件保存项目目录，相对路径入库 |
| 写入原则 | 数据库事务、临时文件和原子重命名 |

## 3. 数据结构

| 类型 | 字段 | 说明 |
| --- | --- | --- |
| `StorageConfig` | db_path、project_root、pool_size | 存储配置 |
| `Transaction` | connection_id、active | 事务句柄，不跨线程复制 |
| `FileRef` | relative_path、sha256、size | 项目内文件引用 |
| `IntegrityIssue` | resource_id、kind、detail | 数据库和磁盘不一致项 |
| `IntegrityReport` | checked、issues、fixed | 完整性检查结果 |
| `Page<T>` | items、next_cursor | 有界查询结果 |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `db_path` | `PathBuf` | data/vision.db | SQLite 文件 |
| `journal_mode` | `Wal` | WAL | 事务日志模式 |
| `read_only` | `bool` | `false` | 是否禁止写入 |
| `schema_version` | `u32` | 0 | 已应用迁移版本 |

## 5. 方法

### `open(config) -> Storage`

打开数据库、启用 WAL 并执行迁移；迁移失败保留原库并返回错误。

### `begin() -> Transaction`

创建事务。复合业务操作必须通过事务提交，不把半成品暴露给查询。

### `migrate() -> MigrationReport`

执行缺失迁移并返回版本变化。

### `put_file(temp_path, target) -> FileRef`

校验目标路径在项目根目录内，原子移动临时文件并计算哈希。

### `check_integrity() -> IntegrityReport`

核对记录、manifest、模型和文件哈希；只标记问题，不静默删除文件。

### `query<T>(query) -> Page<T>`

执行有界、参数化查询；不接受拼接 SQL。

### `close() -> Result<()>`

停止新事务，等待连接归还并关闭池。

## 6. 消息与事件

`storage.migrated`、`storage.integrity.issue`、`storage.file.committed` 是内部事件。事件只含路径引用和摘要，不含文件内容；事务提交后才能发布业务事件。

## 7. 委托与回调

`TransactionHook` 只用于测试和诊断，不得修改事务外状态；`FileCommitSink` 在原子移动完成后调用，失败不会撤销已完成的文件，但会记录待修复项。

## 8. 错误处理

| 错误码 | 说明 |
| --- | --- |
| `migration_failed` | 迁移失败，保留原库 |
| `transaction_conflict` | 事务冲突或锁超时 |
| `path_escape` | 路径越出允许根目录 |
| `file_commit_failed` | 文件移动或校验失败 |
| `record_not_found` | 记录不存在 |
| `schema_mismatch` | 版本不兼容 |
| `storage_closed` | 存储已关闭 |

## 9. 生命周期与线程安全

`Storage` 可由服务共享；连接池安全并发访问，单个 `Transaction` 不跨线程复制。长查询必须分页。关闭前停止写入并等待事务完成，未提交事务全部回滚。

## 10. 依赖关系

依赖 SQLx、SQLite、`common-services` 和文件系统；被所有后端业务模块使用。不得依赖 HTTP、前端或 Python Worker。

## 11. 调用示例

```rust
// 打开数据库并执行迁移
let storage = Storage::open(StorageConfig::for_project(root)).await?;
let migration = storage.migrate().await?;
// 使用事务写入项目摘要
let mut tx = storage.begin().await?;
tx.insert_project(project).await?;
tx.commit().await?;
// 原子保存上传文件并检查完整性
let file_ref = storage.put_file(temp_image, RelativeTarget::original(image_id)).await?;
let report = storage.check_integrity().await?;
// 分页查询任务后关闭存储
let page = storage.query(TaskQuery::first_page()).await?;
assert!(page.items.len() <= page.limit());
assert!(migration.current >= migration.previous);
storage.close().await?;
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `open` / `migrate` / `begin` | 初始化和事务段 |
| `put_file` / `check_integrity` | 文件段 |
| `query` / `close` | 查询和关闭段 |

## 12. 2119 测试要求

| 2119 ID | 验证行为 | 当前证据 |
| --- | --- | --- |
| `storage.1.1` | 事务具备原子性 | 规划验收；实现后补回滚测试 |
| `storage.1.2` | 文件引用原子提交 | 规划验收；实现后补磁盘故障测试 |
| `storage.1.3` | 越界文件目标被拒绝 | 规划验收；实现后补路径测试 |
| `storage.1.4` | 完整性问题可报告且不静默删除 | 规划验收；实现后补重启检查测试 |

测试使用 `// 2119: storage.1.1` 标记，并覆盖并发读写和关闭。

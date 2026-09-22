# common-services API 文档

## Overview

本文件同时是 `common-services` 的 API 契约和 2119 需求来源。需求约束配置、日志、路径、ID 和脱敏等公共行为。

## Requirements

### 1: 配置、日志与边界

1. 公共配置服务 MUST 在配置缺失时创建默认配置，并在损坏时保留备份后恢复 [manual]
2. 公共配置服务 MUST 使用原子写入，避免对外暴露半写入配置 [manual]
3. 日志服务 MUST 输出时间、级别、位置和消息，同时过滤密钥、Cookie 和完整用户消息 [manual]
4. 路径校验服务 MUST 拒绝路径穿越、符号链接逃逸和允许根目录之外的路径 [manual]


## 1. 概述

`common-services` 提供日志、配置、错误、ID、时间、路径和敏感信息脱敏能力。它不依赖业务模块，是后端其他模块共享的基础层；日志与配置按全局单例管理。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | backend |
| 语言 | Rust |
| 公开边界 | 单例服务、纯值类型和校验函数 |
| 输出 | 应用目录下按天日志；配置 JSON 原子保存 |
| 安全 | 日志和错误输出统一脱敏 |

## 3. 数据结构

| 类型 | 字段 | 说明 |
| --- | --- | --- |
| `AppConfig` | version、agent、paths、runtime | 应用配置根对象 |
| `AgentConfig` | providers、models、codex | Agent 参数 |
| `AppError` | code、message、source、location | 稳定错误和诊断来源 |
| `LogRecord` | time_ms、level、module、line、message | 结构化日志记录 |
| `SafePath` | root、relative | 已校验项目相对路径 |
| `Id` | string | 进程内和持久化对象 ID |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `Logger::global()` | `&'static Logger` | 首次初始化 | 全局日志单例 |
| `Config::global()` | `&'static ConfigStore` | 首次加载 | 全局配置单例 |
| `log_dir` | `PathBuf` | 应用目录/logs | 日志输出目录 |
| `config_path` | `PathBuf` | 应用目录/app_config.json | 配置路径 |

## 5. 方法

### `Logger::init(dir) -> Result<()>`

创建按天日志文件并安装全局实例；重复初始化返回已初始化状态。

### `Logger::write(level, location, message)`

写入毫秒时间、级别、来源位置和内容；敏感字段先脱敏。

### `ConfigStore::load(path) -> AppConfig`

读取配置；文件不存在或损坏时备份原文件并写入对象负责的默认值。

### `ConfigStore::save(config) -> Result<()>`

序列化为 JSON 临时文件后原子替换；保存失败保留旧文件。

### `ConfigStore::reload() -> Result<ConfigSnapshot>`

重新加载并校验配置，成功后替换单例快照。

### `PathGuard::resolve(root, relative) -> SafePath`

拒绝绝对路径、`..` 穿越和符号链接逃逸。

### `new_id(prefix) -> Id`

生成不含敏感信息的稳定 ID。

## 6. 消息与事件

`config.loaded`、`config.saved`、`config.reloaded`、`log.self-test` 和 `storage.path-rejected`。日志事件本身不再次写日志，避免递归。

## 7. 委托与回调

`ConfigObserver = Fn(ConfigSnapshot) + Send + Sync`，只接收不可变快照；`LogSink` 只用于测试。回调不得读取或写入全局单例造成递归锁。

## 8. 错误处理

| 错误码 | 说明 |
| --- | --- |
| `config_not_found` | 首次启动，转为默认创建 |
| `config_invalid` | JSON 或字段校验失败，先备份 |
| `config_write_failed` | 原子写入失败 |
| `path_escape` | 路径越界 |
| `log_init_failed` | 日志目录不可写 |
| `invalid_argument` | 调用参数不合法 |

配置加载失败不会返回旧配置和半解析数据的混合状态；日志初始化失败必须在启动报告中可见。

## 9. 生命周期与线程安全

初始化顺序为日志后配置；单例内部使用读写锁，读取返回快照，保存和重载串行。日志写入使用有界通道，不阻塞业务线程。应用退出前刷新日志和配置写入。

## 10. 依赖关系

只依赖 Rust 标准库、serde/serde_json、时间和文件系统库；被所有 backend 模块依赖。不得依赖业务数据库、HTTP 或 Worker。

## 11. 调用示例

```rust
// 初始化全局日志和配置
Logger::init(app_dir.join("logs"))?;
let store = ConfigStore::global(app_dir.join("app_config.json"));
let config = store.load()?;
// 写入带来源位置的启动日志
Logger::global().write(LogLevel::Info, file!(), line!(), "应用配置已加载");
// 校验并保存用户修改的配置
let updated = config.with_active_model("minimax-m3");
store.save(updated).await?;
let snapshot = store.reload().await?;
// 解析项目内相对路径并生成关联 ID
let safe = PathGuard::resolve(project_root, "original/image-1.png")?;
let image_id = new_id("image");
assert!(safe.relative().ends_with("image-1.png"));
assert!(!image_id.as_str().is_empty());
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `Logger::init` / `Logger::write` | 初始化和日志段 |
| `ConfigStore::global` / `load` / `save` / `reload` | 配置段 |
| `PathGuard::resolve` / `new_id` | 路径和 ID 段 |

## 12. 2119 测试要求

| 2119 ID | 验证行为 | 当前证据 |
| --- | --- | --- |
| `common-services.1.1` | 缺失和损坏配置可恢复 | 规划验收；实现后补配置单元测试 |
| `common-services.1.2` | 配置写入具备原子性 | 规划验收；实现后补中断写入测试 |
| `common-services.1.3` | 日志格式稳定且不泄漏敏感信息 | 规划验收；实现后补格式与脱敏测试 |
| `common-services.1.4` | 越界路径被拒绝 | 规划验收；实现后补路径安全测试 |

配置、日志和路径测试必须使用 `#` 或 `// 2119: common-services.1.1` 标记。

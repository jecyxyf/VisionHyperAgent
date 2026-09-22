# common-services API 文档

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

### 验证范围

规格应覆盖配置、日志、路径、ID、错误和脱敏这些跨模块可观察行为：

| 行为 | 必须验证 | 反例测试 |
| --- | --- | --- |
| 配置 | 缺失配置创建默认文件，损坏文件备份后恢复，写入具备原子性 | 半写入文件和非法字段不得被静默接受 |
| 日志 | 时间、级别、位置和消息格式稳定 | API Key、Cookie、完整用户消息不得出现 |
| 路径 | 所有受控路径位于允许根目录内 | `..`、符号链接逃逸和绝对越界路径被拒绝 |
| 公共值 | ID、错误码和时间序列化稳定 | 无效输入不得生成不可追踪的默认值 |

测试不得只验证结构体能序列化；必须验证损坏、越界、脱敏和并发初始化等失败行为。

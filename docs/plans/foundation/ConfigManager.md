# ConfigManager 配置管理设计

- 日期：2026-09-09
- 状态：read/write 底层接口保持不变；已由独立 ConfigViewModel 接入启动初始化，设置页编辑尚未绑定。
- 范围：Rust 基础层及 JSON 读写，补充 ViewModel 调用边界；不接入 Slint 设置编辑或实际模型请求。

## 1. 类型与目录

- 管理类型：`ConfigManager`。
- 全局单例：`CONFIG`，由 `src/foundation/mod.rs` 统一导出。
- 实现文件：`src/foundation/config_manager.rs`。
- 数据类型：`AppConfig` 包含各模块配置，首版仅有 `AgentConfig`。
- 路径定位和共享错误上下文放在 `src/foundation/mod.rs`，与日志共用，不新增其他基础层源码文件。
- 日志设计：[Logger](Logger.md)。

配置参数在内部集中存放于结构体，使用线程安全的单例和读写锁；对外通过模块名、参数名读取或修改单个字符串值，不提供整个配置的快照或修改闭包。

## 2. 保存位置与 JSON 结构

固定保存到**可执行程序所在目录**下的 `config.json`，不随启动工作目录改变。

首版只保存 Agent 的地址、KEY、模型名称，不加入日志配置、界面状态或其他模块参数：

```json
{
  "agent": {
    "base_url": "",
    "api_key": "",
    "model": ""
  }
}
```

| 类型／字段 | 含义 |
| --- | --- |
| `AppConfig.agent` | Agent 模块配置 |
| `AgentConfig.base_url` | 服务地址，字符串 |
| `AgentConfig.api_key` | KEY，字符串，明文保存 |
| `AgentConfig.model` | 模型名称，字符串 |

默认值均为空字符串。首版配置管理不尝试连接服务或验证模型是否存在。

## 3. 加载与恢复

1. 文件正常：反序列化后更新内存配置。
2. 文件不存在：生成默认配置文件并加载。
3. JSON 损坏或字段类型错误：先将原始内容备份为同目录的 `config.json.back`，备份成功后再生成默认配置。
4. `.back` 保存最近一次备份；已有备份在新备份成功写入后替换。
5. 备份失败：返回错误，不覆盖原配置。
6. 读取权限、磁盘等 I/O 错误：返回错误，不把所有 I/O 故障误判成 JSON 损坏。
7. 默认配置重建失败：返回错误，保留已经生成的备份。

缺少已知字段时使用相应空字符串默认值。当前只承诺上述 Agent 配置结构；未来模块扩展需同步调整数据类型与兼容测试。

不监视外部文件变化。需要重新读取时，由调用方显式调用 `load()`；该操作会用磁盘内容替换内存中的未保存修改。

## 4. 修改与保存

- 参数修改只更新内存，不自动落盘。
- 必须显式调用 `CONFIG.save()` 才保存修改；首次创建和损坏恢复是已经确认的初始化例外。
- 保存采用“同目录临时文件 → 完整序列化并刷新 → 替换正式文件”，不先截断 `config.json`。
- 保存失败返回错误；不声称保存成功，不主动丢弃内存修改。
- 加载、保存和备份操作在当前进程内串行化，避免本进程的写入互相覆盖。
- 单例不等于跨进程协调，首版不保证多个程序实例同时改写同一个配置文件。

## 5. 调用接口

已提供：

- `CONFIG.init()`：首次加载或恢复配置；重复初始化不覆盖现有内存修改，返回 `ConfigLoadStatus`。
- `CONFIG.load()`：显式重新加载，返回 `ConfigLoadStatus`。
- `CONFIG.read(module: &str, parameter: &str) -> Result<String>`：读取指定参数的独立字符串。
- `CONFIG.write(module: &str, parameter: &str, value: &str) -> Result<()>`：修改指定参数的内存值。
- `CONFIG.save()`：显式保存。

接口返回 `Result`，不通过 `unwrap()`、`expect()` 或静默回退处理外部文件错误。模块名与参数名区分大小写，当前只接受 `agent` 模块下的 `base_url`、`api_key`、`model`；未知模块或参数返回 `InvalidInput`，不创建字段、不改变原配置。已知参数允许空字符串；初始化前调用返回 `NotInitialized`。`write()` 不执行文件 I/O，仍需显式 `save()`。

已删除 `snapshot()` 和闭包式 `update()`。`read()` 返回的字符串由调用方持有，修改该字符串不影响配置；读取 `api_key` 返回明文，但不得写入日志或诊断输出。

配置界面和其他上层调用方通过 `src/view_model/config_view_model.rs` 的 `ConfigViewModel` 使用这些能力，不在 View 直接调用 CONFIG，也不由 MainWindowViewModel 代理读写。无 UI 调用示意：

```rust
let config = ConfigViewModel::new();
let recovery_notice = config.init()?; // 可独立初始化，不需要主窗口或日志
// recovery_notice 为损坏恢复时的安全提示，交由调用方按需呈现。
config.write("agent", "model", "model-name")?;
let model = config.read("agent", "model")?;
config.save()?;
```

底层 `ConfigLoadStatus` 包含 `Loaded`、`Created`、`AlreadyInitialized`、`Recovered(ErrorContext)`（上下文由 Box 持有）。ConfigViewModel 的 `init / load` 返回 `Result<Option<String>>`：正常为 None，损坏恢复为携带安全原因及原始位置的 Some 提示。配置模块与 ConfigViewModel 均不主动输出日志或终端文本。

桌面启动由 MainWindowViewModel 建立日志作用域，再调用子 ConfigViewModel 的 `init()`；恢复提示写 WARNING，初始化失败则记录错误并中止任务。ConfigViewModel 也可独立调用，保留 `new / init / load / read / write / save`，不依赖 MainWindowViewModel、Slint 或 Logger 初始化。底层 `foundation::init()` 仍保留作为组合接口，但不是当前桌面的配置调用入口。设置页编辑尚未绑定。

## 6. 安全与错误信息

- KEY 按用户要求明文保存，不额外引入加密或系统密钥库。
- 配置对象的 `Debug`／诊断输出隐藏 KEY；日志不输出完整配置、原始 JSON 或可能携带 KEY 的解析错误内容。
- 文件操作错误携带相关路径、源码文件、行号和具体原因；JSON 错误另外提供数据内的行号、列号。
- 新建配置及备份在 Linux 上采用仅当前用户读写的文件权限；Windows 遵循目录 ACL，不修改系统级权限配置。
- 实现时将运行配置、备份、日志与临时文件加入适当的 Git 忽略规则；测试使用明确的假 KEY。
- 采用 `serde`、`serde_json` 实现序列化与反序列化，使用 `serde 1.0.228`、`serde_json 1.0.149`，传递依赖由 `Cargo.lock` 固定。

## 7. 验证要求

覆盖正常加载、JSON 往返、空配置创建、缺失字段、字段类型错误、损坏备份重建、已有备份替换、备份失败保护、保存失败、显式保存语义、多线程读写、重复初始化、源码与 JSON 错误位置、KEY 调试脱敏，以及启动工作目录改变时仍使用可执行目录。

测试在临时目录内进行，不改动用户已有配置，不使用真实 KEY，不依赖模型服务或 GPU。

## 8. 实现边界

- 源码目录严格保持三个文件，安全错误上下文、错误分类和路径定位放在 `mod.rs`。
- 已知字段缺失时填充默认值；未知字段遵循 serde 默认忽略行为，显式保存仅输出当前定义的 Agent 字段，不将其当作跨版本迁移能力。
- 临时文件使用排他创建并在替换前刷新、关闭；写入／替换失败清理临时文件，清理失败追加到错误原因。
- `save()` 串行保存调用取得的快照，保存过程中产生的新修改仍需后续显式保存。
- 基础层不依赖 ViewModel 或界面；启动接入由 ConfigViewModel 完成，设置页编辑尚未绑定。不调用在线模型，也不保证跨进程写入或断电后绝对持久性。

## 功能测试记录

2026-09-08 按用户要求删除日志与配置的专项测试代码及运行入口，保留功能实现与验证摘要。当前工程不再提供这两模块的一键功能回归；下述验证结果属于删除前的记录，不代表当前测试命令仍覆盖这些场景。底层源码仍严格限定为三个文件。

## 2026-09-08 加强回归

运行目录及文件拒绝软链接与特殊文件，写入／替换／清理不强行覆盖只读目标；本模块新建文件在 Linux 上显式设置 `0600`，新建日志目录设置 `0700`，已有目录权限不变。路径检查不替代多进程协调或恶意文件系统竞争防护。Windows 已完成编译链接，尚未实机运行。


## 2026-09-08 参数接口验证

临时进程探针已验证三个 Agent 参数的读写、空值与特殊字符、返回字符串独立性、未初始化、非法模块／参数及错误诊断脱敏、修改不自动保存、显式保存与重载、并发读写、损坏备份恢复；旧 `snapshot()`、`update()` 调用均无法通过编译。原有工作区测试、严格 Clippy、Windows 目标编译检查通过；Windows 未实机运行。临时测试源码和可执行文件已清理，未恢复已删除的测试文件。

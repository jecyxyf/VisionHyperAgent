# 第三方依赖目录

`src/depends/` 是项目内集中管理第三方依赖来源、版本说明及受控依赖材料的位置。当前已接入 Slint 宿主依赖，具体声明见 Cargo 文件；[Slint 许可与来源](slint/README.md)在本目录保存。Codex CLI 和 Rust ZeroMQ 源码通过 Git submodule 固定，基线如下。它是材料目录，不是需要在 `src/lib.rs` 中声明的 Rust 业务模块。

## 当前版本绑定的源码依赖

以下基线与 VisionHyperAgent `0.1.0` 当前源码绑定，不随上游分支自动更新。上游地址记录在根目录 [.gitmodules](../../.gitmodules)，实际锁定依据是主仓库记录的子模块提交（gitlink）。

| 依赖 | 子模块路径 | 源码声明版本 | 来源分支 | 固定提交 SHA |
| --- | --- | --- | --- | --- |
| Codex CLI（`openai/codex`） | `src/depends/codex` | `0.0.0` | `main` | `73a1148c9c775c2a4616ce5096291740a00ed68a` |
| Rust ZeroMQ（`erickt/rust-zmq`，crate `zmq`） | `src/depends/rust-zmq` | `0.10.0` | `master` | `5d78967001abb1aece2fba878d6151cb66cd1767` |

- 版本字段分别来自固定提交中的 [Codex Cargo.toml](codex/codex-rs/Cargo.toml) 和 [zmq Cargo.toml](rust-zmq/Cargo.toml)。两者均按源码快照锁定，不表示选用了同名发行标签；Codex 的 `0.0.0` 不能作为正式发行版本号使用。
- 分支名仅说明源码来源，分支后续可能移动，不能代替完整 SHA。子模块恢复后处于 detached HEAD 属于正常状态。
- 当前仅纳入源码子模块，Agent 与 ZeroMQ 业务尚未接入；锁定源码不代表已完成编译、运行或跨平台兼容性验证，也不替代后续运行环境与传递依赖的锁定。

### 恢复固定源码

先检出目标 VisionHyperAgent 主仓库提交，再在仓库根目录执行：

```sh
git submodule update --init --recursive --checkout -- src/depends/codex src/depends/rust-zmq
git submodule status -- src/depends/codex src/depends/rust-zmq
```

提交或交付主仓库时，必须一并保留 `.gitmodules` 和两个子模块的 gitlink；仅复制目录或记录分支名不足以恢复固定源码。复现时不要使用 `git submodule update --remote`，也不要在子模块内通过 `git pull` 跟随分支最新提交。后续升级须先确认，并同步更新子模块 gitlink 与本表，不能只修改版本说明。

## 管理边界

- 可维护依赖来源、版本、校验值、许可说明，以及经确认需要纳入仓库的依赖材料。
- 下载归档、缓存和预编译文件分别使用 `downloads/`、`cache/`、`prebuilt/`；这些生成内容不提交 Git；Rust crate 由 Cargo 缓存管理，不复制到业务源码。
- `src/depends/` 不替代 Rust 的 `Cargo.toml`、`Cargo.lock`，也不替代后续 Python 包的依赖声明。
- `depoly/packaging/runtime/` 保存环境组装规则，Python 训练源码位于 `depoly/packaging/python/`；`src/foundation/` 承担运行环境管理代码；完整运行环境是交付资源，不作为源码提交。
- 不在此保存 API 密钥、用户图片、训练记录或模型权重。

Codex CLI 和 Rust ZeroMQ 的源码引用已按上述基线固定；其构建与分发方式，以及 YOLO、OpenCV 等其他依赖的具体版本和接入方式，仍需在实际接入时确认。

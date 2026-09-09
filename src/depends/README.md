# 第三方依赖目录

`src/depends/` 是项目内集中管理第三方依赖来源、版本说明及受控依赖材料的位置。当前已接入 Slint 宿主依赖，具体声明见 Cargo 文件；[Slint 许可与来源](slint/README.md)在本目录保存。它是材料目录，不是需要在 `src/lib.rs` 中声明的 Rust 业务模块。

## 管理边界

- 可维护依赖来源、版本、校验值、许可说明，以及经确认需要纳入仓库的依赖材料。
- 下载归档、缓存和预编译文件分别使用 `downloads/`、`cache/`、`prebuilt/`；这些生成内容不提交 Git；Rust crate 由 Cargo 缓存管理，不复制到业务源码。
- `src/depends/` 不替代 Rust 的 `Cargo.toml`、`Cargo.lock`，也不替代后续 Python 包的依赖声明。
- `depoly/packaging/runtime/` 保存环境组装规则，Python 训练源码位于 `depoly/packaging/python/`；`src/foundation/` 承担运行环境管理代码；完整运行环境是交付资源，不作为源码提交。
- 不在此保存 API 密钥、用户图片、训练记录或模型权重。

源码引用、预编译 SDK 或其他依赖接入方式，在各项依赖真正引入时再确认。本阶段不预设 Codex、YOLO、OpenCV、ZeroMQ 的具体版本或分发方式。

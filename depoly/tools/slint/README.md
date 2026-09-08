# 项目内 Slint 开发工具

本目录保存用于界面开发、检查和预览的工具，不是 VisionHyperAgent 的业务模块或最终用户需要安装的训练环境。

## 已安装内容

- 版本：Slint 1.17.1。
- 当前平台：Linux x86_64，使用官方预编译发行包。
- `bin/slint-viewer`：界面编译检查、预览和截图。
- `bin/slint-lsp`：语言服务器，供编辑器或支持 LSP 的客户端使用。
- `bin/licenses/`：随官方工具保留的许可及第三方声明。
- `manifest.json`：固定版本、下载来源、归档与安装文件的 SHA-256 校验信息。

`bin/` 不提交 Git。下载缓存和临时自检文件放在 `depoly/target/`，不混入应用源码。

## 使用方式

从项目根目录使用完整的项目内路径，不需要修改全局 PATH：

```sh
depoly/tools/slint/bin/slint-viewer --version
depoly/tools/slint/bin/slint-lsp --version
```

需要按技能中的短命令调用时，只给当前命令临时添加 PATH：

```sh
PATH="$PWD/depoly/tools/slint/bin:$PATH" slint-viewer --version
```

当前界面入口为 `src/view/MainWindow.slint`，以下命令从项目根目录运行。

| 操作 | 命令形式 |
| --- | --- |
| 检查界面 | `slint-viewer --check src/view/MainWindow.slint` |
| 实时预览 | `slint-viewer --auto-reload src/view/MainWindow.slint` |
| 无窗口截图 | `slint-viewer --screenshot depoly/target/ui-preview/run-light.png src/view/MainWindow.slint` |

截图前先执行 `mkdir -p depoly/target/ui-preview`。使用这些短命令时，同样通过命令级 PATH 或完整工具路径调用。不要为方便使用而把工具复制到 `~/.local/bin`、`~/.cargo/bin` 或修改 shell 启动配置。

## 本次验证与边界

- 两个工具均能启动并报告版本 1.17.1。
- `slint-viewer --check src/view/MainWindow.slint` 已通过原生界面编译检查。
- 无窗口截图自检成功，生成并检查了 160 × 90 的探针图片。
- 自检文件位于 `depoly/target/slint-tools-check/`，不是产品界面，也不加入源码。
- LSP 已验证启动版本，尚未配置具体编辑器或执行完整的 LSP 交互验收。
- 未配置 MCP 服务、未安装 Windows 工具，也未安装系统软件包或修改全局 PATH。

当前界面可通过 Viewer 独立预览，`Cargo.toml` 仍未引入 Slint 库，`cargo run` 尚不显示界面；后续选择应用依赖时，应核对其 Slint 版本与工具兼容性。

## 版本维护

安装版本与来源以 `manifest.json` 为准，不在每次运行时自动追随上游最新版本。更新工具时先确认兼容性，校验官方归档摘要，并保留随包的许可文件。

项目级技能位于 [Slint Skill](../../../.agents/skills/slint/SKILL.md)，其中的通用安装示例不替代本项目的局部安装约定。

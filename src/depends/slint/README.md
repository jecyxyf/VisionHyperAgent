# Slint 宿主依赖

- 版本：`slint = 1.17.1`、`slint-build = 1.17.1`，使用 crates.io 发行包，校验与传递版本见 `Cargo.lock`。
- 用途：编译现有 `.slint` 界面、创建桌面窗口及运行事件循环。后端 Winit，优先 FemtoVG（OpenGL），保留软件渲染回退；未启用 Qt 后端。
- 替代方案：继续用 Viewer 只适合开发预览，不能作为本项目最终宿主；Skia/Qt 本轮不引入。
- 资源：图片与图标在构建时内嵌；Noto 中文字体及其许可位于 `src/view/resources/ui/fonts/`。
- 许可：本轮普通 PC 桌面界面采用官方 Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0，原文见 [LICENSE.md](LICENSE.md)；关于页保留官方 `AboutSlint`。项目自身的 MIT 声明不变。专用嵌入式系统或对外暴露 Slint API 不属于该许可授予范围，不能将本次桌面构建直接当作相应分发许可。
- 原始 crate 同时提供 GPL / 商业许可选项，未将本项目整体重新声明为这些许可。
- 发行材料：`bin/` 随附项目许可、两平台依赖许可材料及构建哈希；此收集不替代正式发行前针对实际用途和完整依赖的合规审核。
- 产物体积在 `bin/*/build-info.json` 记录，不承诺其他目标平台及功能组合的固定大小。

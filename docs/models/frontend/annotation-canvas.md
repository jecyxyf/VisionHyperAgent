# annotation-canvas API 文档

## 1. 概述

`annotation-canvas` 是独立的 2D 实例分割编辑模块，负责原图、实例遮罩、画笔、多边形、擦除、选择、缩放、平移和撤销重做。它只输出归一化几何变更，不判断业务规则是否生效，也不直接落盘。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | frontend |
| 语言 | TypeScript / Canvas 2D |
| 公开边界 | 编辑器控制器、几何模型、事件和渲染快照 |
| 性能边界 | 使用 requestAnimationFrame 合并刷新；4K 图片下不全量重建无关图层 |
| 坐标原则 | 持久化归一化坐标，屏幕坐标只存在渲染层 |

## 3. 数据结构

| 类型 | 字段 | 说明 |
| --- | --- | --- |
| `CanvasSize` | width、height、device_pixel_ratio | 原图和屏幕尺寸 |
| `InstanceShape` | instance_id、category_id、polygon、mask_ref | 一个独立实例 |
| `CanvasDocument` | image_id、instances、revision | 编辑快照 |
| `Viewport` | zoom、offset_x、offset_y | 视图变换 |
| `EditCommand` | kind、before、after | 撤销重做命令 |
| `CanvasEvent` | kind、document、selection | 编辑器事件 |
| `Tool` | Select、Brush、Polygon、Erase、Pan | 当前工具 |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `tool` | `Tool` | `Select` | 当前编辑工具 |
| `viewport` | `Viewport` | 1 倍、零偏移 | 缩放和平移 |
| `selectedInstanceId` | `string | null` | `null` | 当前实例 |
| `historyDepth` | `number` | 0 | 可撤销命令数 |
| `maxHistory` | `number` | 50 | 单图撤销上限 |

## 5. 方法

### `create(options) -> CanvasController`

绑定 Canvas、加载原图和标注快照，初始化图层和设备像素比。

### `setTool(tool) -> void`

切换工具；未完成的绘制按策略提交或取消，不能静默丢失。

### `setDocument(document) -> void`

替换当前编辑快照并清空当前图片的撤销栈。

### `undo() -> boolean` / `redo() -> boolean`

按命令栈回退或恢复几何编辑；成功返回 true。

### `fitToView() -> void`

将原图和实例层适配到可视区域。

### `exportDraft() -> CanvasDocument`

返回归一化坐标和实例版本，供 project-service 校验。

### `subscribe(listener) -> Unsubscribe`

订阅编辑事件；监听器只读事件快照。

### `destroy() -> void`

移除指针事件、取消动画帧并释放位图资源。

## 6. 消息与事件

`document.changed`、`selection.changed`、`history.changed`、`render.requested`、`validation.warning`。事件只携带几何和摘要，不携带屏幕截图或文件写入结果。

## 7. 委托与回调

`CanvasListener = (event: CanvasEvent) => void`；`RenderScheduler` 由宿主提供 requestAnimationFrame 调度。监听器不得修改传入对象；需要保存时复制 `exportDraft()`。

## 8. 错误处理

| 错误码 | 说明 |
| --- | --- |
| `image_not_loaded` | 原图尚未可渲染 |
| `invalid_geometry` | 点数不足、坐标越界或自交 |
| `category_missing` | 实例引用的类别不存在 |
| `history_empty` | 无可撤销或重做操作 |
| `canvas_destroyed` | 控制器已释放 |

本模块只报告几何错误；规则、版本和确认错误由 project-service 返回。

## 9. 生命周期与线程安全

所有方法在浏览器主线程调用；指针事件、命令栈和渲染帧通过同一控制器串行更新。禁止在回调内同步修改文档；复杂计算使用分片或 Worker 后再投递结果。销毁后事件监听和动画帧全部取消。

## 10. 依赖关系

依赖浏览器 Canvas 2D API 和前端类型；被 `workspace-ui` 使用。不得依赖后端 API、数据库或模型运行时。

## 11. 调用示例

```ts
// 创建标注编辑器并订阅变化
const canvas = create({ canvas: canvasElement, imageUrl, categories });
const unsubscribe = canvas.subscribe((event) => updateToolbar(event));
// 绘制前切换工具并适配视图
canvas.setTool('polygon');
canvas.fitToView();
// 导入已有标注并执行一次编辑
canvas.setDocument(documentSnapshot);
canvas.setTool('select');
canvas.undo();
canvas.redo();
// 提交归一化草稿到后端
const draft = canvas.exportDraft();
await projectApi.saveDraft(draft);
// 页面离开时释放资源
unsubscribe();
canvas.destroy();
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `create` / `subscribe` / `setDocument` | 初始化和数据段 |
| `setTool` / `fitToView` / `undo` / `redo` | 编辑段 |
| `exportDraft` | 提交段 |
| `destroy` | 释放段 |

## 12. 2119 测试要求

### 验证范围

规格应覆盖画布可观察编辑行为，不把像素实现细节作为强制需求：

| 行为 | 必须验证 | 反例测试 |
| --- | --- | --- |
| 几何编辑 | 工具操作输出归一化坐标和正确实例版本 | 越界、自交和点数不足必须产生验证警告 |
| 视图变换 | 缩放、平移和适配不改变持久化几何 | 屏幕尺寸变化不得改变归一化结果 |
| 历史 | undo/redo 顺序稳定，销毁后不再发事件 | 空历史操作不得修改文档 |
| 资源释放 | destroy 取消监听、帧和位图引用 | 销毁后回调不得更新页面状态 |

测试优先使用真实 Canvas 行为或可验证的几何适配器；快照只能辅助，不能代替坐标和错误断言。

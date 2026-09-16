# VisionHyperAgent 前端复刻设计

日期：2026-09-16  
状态：已确认视觉方向  
参考来源：master 分支 Rust/Slint 界面，尤其是 src/view/Theme.slint、MainWindow.slint、AppHeader.slint、GlassPanel.slint 与 InferencePage.slint

## 1. 目标

将原 Rust/Slint 桌面端的 Prismatic Glass 视觉体系复刻到当前 Svelte 5 Web 前端，作为 VisionHyperAgent 的统一界面基础。

本次设计解决三个问题：

1. 视觉连续性：保留用户已经认可的原界面气质，而不是重新发明一套风格。
2. Web 可实现性：所有效果使用 CSS/SVG 表达，不依赖桌面端模糊能力。
3. 后续扩展性：为预标注、标注、训练、推理和 Agent 对话提供同一套布局与组件。

## 2. 范围

### 2.1 第一阶段纳入

- 1440×900 基准的全局工作台布局。
- AppHeader 银色虹彩标题栏与 Agent 渐变徽标。
- 左侧模型导航、中央页面容器、右侧全局 Agent 面板。
- 原始 Prismatic Glass 色彩、圆角、边框、阴影、渐变和网格。
- 运行页的完整静态复刻。
- 其他页面的布局骨架与空状态，不接真实业务数据。
- 可访问性基础：键盘焦点、语义按钮、可见 focus 状态、Reduced Motion。

### 2.2 暂不纳入

- 不修改 Rust 后端协议。
- 不实现真实模型选择、推理、训练和相机连接。
- 不引入 Three.js 或 3D 效果。
- 不做暗色主题。
- 不做多语言切换。
- 不做移动端专属布局；最低支持 1120×720，允许页面内部滚动。

## 3. 信息架构

AppShell 由四个一级区域组成：

- AppHeader：Agent 徽标与产品标题。
- NavigationPanel：运行、模型组、设置、关于。
- WorkspacePanel：页面标题、工具栏和当前页面。
- AgentPanel：全局对话历史与输入区。

导航行为：

- 模型默认展开。
- 点击模型主项进入模型页。
- 点击右侧箭头仅展开或折叠子项。
- 当前页面使用渐变选中态。
- 嵌套页面显示小圆点，不重复使用大图标。

## 4. 视觉系统

### 4.1 色彩 Token

| Token | 值 | 用途 |
|---|---|---|
| --vha-background | #eeebfa | 全局浅色底 |
| --vha-glass-strong | rgba(255,255,255,.949) | 强玻璃与浮层 |
| --vha-glass-edge | rgba(255,255,255,.859) | 玻璃外描边 |
| --vha-surface | rgba(255,255,255,.329) | 嵌入面板与工具按钮 |
| --vha-elevated | rgba(255,255,255,.741) | hover 提升 |
| --vha-border | rgba(101,82,171,.157) | 输入框与分割线 |
| --vha-text | #29294e | 主文本 |
| --vha-muted | #545579 | 次要文本 |
| --vha-subdued | #686b89 | 辅助文本 |
| --vha-accent | #6545ce | 品牌紫与焦点 |
| --vha-header-ink | #30214e | 标题主色 |
| --vha-header-accent | #a032b7 | 标题强调色 |

### 4.2 渐变 Token

| Token | 值 | 用途 |
|---|---|---|
| --vha-glass-gradient | 145deg，白到浅白三段渐变 | 主要玻璃面板 |
| --vha-action-gradient | 125deg，#505be4 → #8148d5 → #b54cc1 | 主按钮与选中导航 |
| --vha-selection-gradient | 120deg，浅紫 → 浅粉 | 次级选中态 |
| --vha-field-gradient | 145deg，较亮白 → 较透明白 | 输入区域 |
| --vha-canvas-gradient | 145deg，半透明白两段 | 图像画布 |
| --vha-header-silver | 110deg，浅蓝 → 浅粉 → 浅青 | 标题栏 |
| --vha-header-badge | 125deg，#555cec → #9346d8 → #d44ea9 | Agent 徽标 |

### 4.3 Ambient 光效

背景必须呈现四个柔和光源与中央白光：

- 左上紫：#7870f5。
- 右上粉：#f35fc5。
- 左下青：#43d8e9。
- 右下橙：#ffad78。
- 中央白光增强通透感。

光效通过一个固定层实现，不参与滚动，也不响应鼠标。禁止为了炫酷添加高频闪烁或持续大幅动画。

## 5. 布局

### 5.1 基准尺寸

- 设计基准：1440×900。
- 最小可用尺寸：1120×720。
- AppHeader 高度 104px，其中内层银色板 78px。
- 主区域左右外边距 16px。
- 底部安全留白 16px。

### 5.2 三栏结构

- Navigation：最小 140px。
- 分割把手：16px。
- Workspace：自适应，最小 540px。
- 分割把手：16px。
- Agent：最小 260px。

1440px 时建议导航 140px、工作区约 824px、Agent 约 412px。1120px 时保持三栏，但工作区优先收缩；若可用宽度不足，允许页面内部滚动，不隐藏 Agent 面板。

### 5.3 分割线

第一阶段复刻 Splitter 的视觉把手，但不实现拖拽。后续如恢复拖拽，必须持久化到后端配置。

## 6. 组件设计

### 6.1 GlassPanel

所有主要容器使用统一玻璃面板：

- 圆角 22px。
- 1px 白色高光描边。
- 下移 8px、模糊 30px、低透明紫色阴影。
- 上下边缘使用横向渐变高光。
- 嵌入式面板去掉阴影，改用 surface 与 border。

### 6.2 AppHeader

- 银色渐变横板，圆角 19px。
- 内层虹彩光泽用 CSS 渐变叠加，不引入图片资源。
- Agent 徽标为渐变圆角矩形，白色斜体字。
- 标题文案为：Agent 驱动的视觉模型自动训练及部署平台。
- 标题中“自动训练”使用紫粉色强调。
- 标题栏是视觉身份区，不放置按钮。

### 6.3 NavigationPanel

- 顶层导航项高度 40px，圆角 12px。
- 图标 17px，文字 14px。
- 选中项使用 action 渐变、白色文字和低透明阴影。
- 设置与关于固定到底部。

### 6.4 WorkspaceHeader

- 高度 51px。
- 上方 34px 操作行，下方 1px 分割线。
- 页面图标 18px，标题 15.4px、600 字重。
- 工具按钮 34×34px，圆角 11px。
- 主按钮使用 action 渐变，次级按钮使用 surface。

### 6.5 AgentPanel

- 使用主玻璃面板。
- 顶部标题为 Agent，右侧为清除聊天记录按钮。
- 历史区空状态不显示占位卡片，保持安静。
- 输入区默认高度 132px，圆角 16px。
- 发送按钮使用主渐变。
- 输入聚焦时边框变为 --vha-accent。

### 6.6 EmptyState

- 中央 60×60px 渐变图标容器。
- 圆角 18px。
- 主文案使用 muted，详细文案使用 subdued。
- 空状态必须告诉用户下一步可以做什么，不做纯装饰文案。

## 7. 页面复刻

### 7.1 运行页

第一优先完整复刻：

- 页面标题与三个工具按钮。
- 图像结果玻璃面板。
- 32px 网格画布。
- 等待图像空状态。
- 图像源、模型、耗时、实例元信息行。
- 识别记录表与暂无记录空状态。

### 7.2 预标注页

- 左侧特征描述表单：识别目标、外观与关键特征、类别区别、排除情况。
- 右上 Agent 分析。
- 右下标注规则。
- 工具按钮：分析特征、确认标注规则、进入标注。

### 7.3 标注页

- 左侧样本列表。
- 中央 Canvas 2D 标注区。
- 右侧标签与属性检查区。
- 保留现有 AnnotationCanvas 能力，视觉容器改为 Prismatic Glass。

### 7.4 预训练页

- 数据源选择。
- 类别与样本统计。
- 训练前检查。
- 开始预训练入口。

### 7.5 训练页

- 左侧训练参数。
- 右上训练进度与指标。
- 右下日志。
- 空图表使用轻量网格，不引入图表库。

### 7.6 模型页

- 模型列表、当前选中模型、详情区域。
- 空模型库时提供导入或训练入口。

## 8. 状态与反馈

- 空状态：说明当前没有数据，并指向下一步操作。
- 加载状态：保留玻璃容器，内部显示轻量进度，不整屏遮罩。
- 错误状态：说明原因和可执行动作，例如“模型未选择，请先在模型页导入”。
- 成功反馈：底部中央玻璃提示条，最多显示 5 秒。
- Agent 未连接：聊天输入可用，但发送后显示“Agent 未连接，消息未发送”。

## 9. 可访问性

- 导航项、工具按钮、发送按钮均使用 button 语义。
- 图标按钮必须有 aria-label。
- 键盘焦点使用 --vha-accent 可见描边。
- 页面保持文字对比度可读。
- prefers-reduced-motion 为 reduce 时关闭装饰动画。
- 颜色不是唯一状态表达，选中项同时使用文字字重与边框。

## 10. 性能

- Ambient 背景只渲染一层，使用 CSS 渐变。
- 不使用 backdrop-filter 实现基础玻璃效果，避免 Windows 集成浏览器性能不稳定。
- 标注画布继续使用 Canvas 2D。
- 4K 图像下目标交互帧率 60 FPS。
- 首屏只加载当前页面组件，不引入 UI 图表库。

## 11. 文件结构

前端实现集中在 source/frontend/src：

- app.css：全局 reset、字体与全局 token 引入。
- App.svelte：AppShell 与页面切换状态。
- lib/styles/tokens.css：视觉 token 与 reduced-motion 约束。
- lib/components/AppHeader.svelte：标题栏。
- lib/components/AgentPanel.svelte：全局 Agent 面板。
- lib/components/GlassPanel.svelte：主玻璃与嵌入面板。
- lib/components/IconButton.svelte：图标按钮。
- lib/components/NavItem.svelte：导航项。
- lib/components/PageHeader.svelte：页面标题与工具栏。
- lib/components/EmptyState.svelte：空状态。
- lib/components/StatusBar.svelte：底部反馈。
- pages/Inference.svelte、Models.svelte、Preannotation.svelte、Annotation.svelte、Pretraining.svelte、Training.svelte：六个业务页面。

## 12. 验收标准

1. 1440×900 下与原 Rust/Slint 界面的布局、比例、色彩和玻璃质感一致。
2. 1120×720 下三栏仍可用，无横向溢出。
3. 所有导航和工具按钮可键盘操作，焦点可见。
4. 运行页完整呈现图像结果、网格、空状态和识别记录。
5. Agent 面板可输入、可聚焦、可发送 mock 消息。
6. npm run check 与 npm run build 通过。
7. Reduced Motion 下没有装饰动画。
8. 现有 Canvas 标注引擎不被破坏。


# VisionHyperAgent

Agent 驱动的视觉模型桌面软件。

## 架构

- **前端**：Svelte + TypeScript + Vite（浏览器）
- **后端**：Rust + axum（本地进程）
- **Agent**：CodexAgent 通过 WebSocket 连接 Codex App Server
- **标注**：Canvas 2D（实例分割）

## 快速开始

### 后端

~~~bash
cd backend
cargo run --bin vha-server
# 输出: VisionHyperAgent running at http://127.0.0.1:8420
~~~

### 前端

~~~bash
cd frontend
npm install
npm run dev
# 打开 http://localhost:5173（开发模式，代理到后端 8420）
~~~

### 生产模式

~~~bash
cd frontend && npm run build     # 前端产物 → frontend/dist
cd backend && cargo build --release  # 后端打包静态文件
~~~

## 目录

~~~
docs/       架构文档
frontend/   TypeScript Web 前端
backend/    Rust 本地后端
bin/        编译产物
~~~

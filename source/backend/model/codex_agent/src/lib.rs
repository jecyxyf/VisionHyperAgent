//! CodexAgent：通过 WebSocket 连接 Codex App Server。
//!
//! 完整生命周期：
//! 1. spawn Codex App Server 子进程
//! 2. 建立 WebSocket 连接
//! 3. 执行 JSON-RPC 请求 / 接收通知
//! 4. 停止时先关 WebSocket 再杀子进程

pub mod process;
pub mod websocket;

pub use process::CodexProcess;
pub use websocket::CodexAgent;

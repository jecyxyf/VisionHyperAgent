//! CodexAgent：通过 WebSocket 连接 Codex App Server。
//!
//! 负责建立连接、执行 JSON-RPC 请求、接收通知事件，
//! 并转换为进程内事件转发给 EventBus。

pub mod websocket;

pub use websocket::CodexAgent;

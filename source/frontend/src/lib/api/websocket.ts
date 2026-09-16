/**
 * WebSocket 客户端：与本地 Rust 后端保持实时连接。
 * 页面存活期间保持心跳，断开后自动重连。
 */
export class AgentSocket {
  private ws: WebSocket | null = null;
  private heartbeat: ReturnType<typeof setInterval> | null = null;
  private listeners = new Map<string, Set<(data: unknown) => void>>();

  connect(url = "ws://127.0.0.1:8420/ws") {
    this.ws = new WebSocket(url);
    this.ws.onopen = () => this.startHeartbeat();
    this.ws.onclose = () => { this.stopHeartbeat(); setTimeout(() => this.connect(url), 2000); };
    this.ws.onmessage = (e) => {
      try { const msg = JSON.parse(e.data); this.emit(msg.type, msg.data); } catch {}
    };
  }

  private startHeartbeat() {
    this.heartbeat = setInterval(() => this.send({ type: "ping" }), 10_000);
  }

  private stopHeartbeat() {
    if (this.heartbeat) { clearInterval(this.heartbeat); this.heartbeat = null; }
  }

  send(msg: object) { this.ws?.send(JSON.stringify(msg)); }

  on(type: string, cb: (data: unknown) => void) {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(cb);
    return () => this.listeners.get(type)?.delete(cb);
  }

  private emit(type: string, data: unknown) {
    this.listeners.get(type)?.forEach(cb => cb(data));
  }
}

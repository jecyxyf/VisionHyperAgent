/** Browser-to-backend RPC. Connection loss rejects pending calls and NEVER replays them. */
export class AgentRequestError extends Error {
  code: string;
  constructor(code: string, message: string) { super(message); this.name = "AgentRequestError"; this.code = code; }
}

type Listener = (data: unknown) => void;
type Pending = { resolve: (data: unknown) => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> };
export type SocketFactory = (url: string) => WebSocket;

export class AgentSocket {
  private ws: WebSocket | null = null;
  private heartbeat: ReturnType<typeof setInterval> | null = null;
  private reconnect: ReturnType<typeof setTimeout> | null = null;
  private listeners = new Map<string, Set<Listener>>();
  private pending = new Map<string, Pending>();
  private disposed = true;
  private attempts = 0;
  private url = "";
  private createSocket: SocketFactory;

  constructor(createSocket: SocketFactory = (url) => new WebSocket(url)) { this.createSocket = createSocket; }

  connect(url?: string) {
    this.disconnect();
    const protocol = typeof location !== "undefined" && location.protocol === "https:" ? "wss:" : "ws:";
    const host = typeof location !== "undefined" ? location.host : "127.0.0.1:8420";
    this.url = url ?? `${protocol}//${host}/ws`;
    this.disposed = false;
    this.attempts = 0;
    this.open();
  }

  private open() {
    if (this.disposed) return;
    let socket: WebSocket;
    try { socket = this.createSocket(this.url); } catch { this.scheduleReconnect(); return; }
    this.ws = socket;
    socket.onopen = () => {
      if (this.disposed || this.ws !== socket) return;
      this.attempts = 0;
      this.clearHeartbeat();
      this.heartbeat = setInterval(() => {
        if (socket.readyState === 1) { try { socket.send(JSON.stringify({ type: "ping" })); } catch { socket.close(); } }
      }, 10_000);
      this.emit("open", undefined);
    };
    socket.onmessage = (event) => {
      if (this.disposed || this.ws !== socket) return;
      let message: Record<string, unknown>;
      try { message = JSON.parse(String(event.data)); } catch { return; }
      if (!message || typeof message !== "object") return;
      if (typeof message.id === "string") {
        const pending = this.pending.get(message.id);
        if (!pending) return;
        this.pending.delete(message.id); clearTimeout(pending.timer);
        if (message.error && typeof message.error === "object") {
          const error = message.error as { code?: string; message?: string };
          pending.reject(new AgentRequestError(error.code ?? "backend_error", error.message ?? "后端请求失败"));
        } else if ("result" in message) pending.resolve(message.result);
        else pending.reject(new AgentRequestError("protocol", "后端响应格式错误"));
        return;
      }
      if (typeof message.type === "string") this.emit(message.type, message.data ?? message);
    };
    socket.onerror = () => { if (this.ws === socket) socket.close(); };
    socket.onclose = () => {
      if (this.ws !== socket) return;
      this.ws = null; this.clearHeartbeat();
      this.rejectPending(new AgentRequestError("outcome_unknown", "与后端断开连接。任务可能仍在运行，重连后会同步，不会自动重发。"));
      this.emit("close", undefined);
      this.scheduleReconnect();
    };
  }

  private scheduleReconnect() {
    if (this.disposed || this.reconnect) return;
    const delay = Math.min(500 * 2 ** this.attempts++, 5000);
    this.reconnect = setTimeout(() => { this.reconnect = null; this.open(); }, delay);
  }

  request<T>(method: string, params: object = {}, timeoutMs = 35_000): Promise<T> {
    const socket = this.ws;
    if (this.disposed || !socket || socket.readyState !== 1) return Promise.reject(new AgentRequestError("disconnected", "后端尚未连接"));
    const id = crypto.randomUUID();
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new AgentRequestError("outcome_unknown", "请求超时，任务可能已经开始。请先同步会话状态，不要重复发送。"));
      }, timeoutMs);
      this.pending.set(id, { resolve: (data) => resolve(data as T), reject, timer });
      try { socket.send(JSON.stringify({ id, method, params })); }
      catch {
        clearTimeout(timer); this.pending.delete(id);
        reject(new AgentRequestError("disconnected", "消息未发出，请检查后端连接"));
      }
    });
  }

  on(type: string, listener: Listener) {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(listener);
    return () => {
      const listeners = this.listeners.get(type); listeners?.delete(listener);
      if (listeners?.size === 0) this.listeners.delete(type);
    };
  }

  disconnect() {
    this.disposed = true;
    if (this.reconnect) clearTimeout(this.reconnect);
    this.reconnect = null; this.clearHeartbeat();
    const old = this.ws; this.ws = null;
    old?.close();
    this.rejectPending(new AgentRequestError("disconnected", "连接已关闭；后端任务不会因此停止"));
  }

  private clearHeartbeat() { if (this.heartbeat) clearInterval(this.heartbeat); this.heartbeat = null; }
  private rejectPending(error: Error) {
    for (const request of this.pending.values()) { clearTimeout(request.timer); request.reject(error); }
    this.pending.clear();
  }
  private emit(type: string, data: unknown) {
    for (const listener of this.listeners.get(type) ?? []) {
      try { listener(data); } catch { console.error("Agent event handler failed"); }
    }
  }
}

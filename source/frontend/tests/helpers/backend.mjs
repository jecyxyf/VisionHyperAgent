/** Test-only native RPC client. It never contains provider credentials. */
export class BackendClient {
  constructor(url = process.env.VHA_TEST_WS_URL || 'ws://127.0.0.1:8420/ws') {
    this.url = url; this.pending = new Map(); this.events = []; this.sequence = 0;
  }
  async connect() {
    this.ws = new WebSocket(this.url);
    this.ws.addEventListener('message', ({ data }) => {
      const message = JSON.parse(String(data));
      if (message.id) {
        const pending = this.pending.get(message.id);
        if (pending) { this.pending.delete(message.id); clearTimeout(pending.timer); message.error ? pending.reject(Object.assign(new Error(message.error.message), { code: message.error.code })) : pending.resolve(message.result); }
      } else {
        this.events.push(message);
        if (this.events.length > 10000) this.events.shift();
      }
    });
    this.ws.addEventListener('close', () => { for (const item of this.pending.values()) { clearTimeout(item.timer); item.reject(new Error('Socket closed')); } this.pending.clear(); });
    await new Promise((resolve, reject) => { const timer = setTimeout(() => reject(new Error('Connect timeout')), 5000); this.ws.addEventListener('open', () => { clearTimeout(timer); resolve(); }, { once: true }); this.ws.addEventListener('error', () => { clearTimeout(timer); reject(new Error('Connect failed')); }, { once: true }); });
    return this;
  }
  call(method, params = {}, timeout = 35000) {
    const id = 'test-' + (++this.sequence);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error('RPC timeout: ' + method)); }, timeout);
      this.pending.set(id, { resolve, reject, timer });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
  async event(method, predicate = () => true, timeout = 120000) {
    const until = Date.now() + timeout;
    while (Date.now() < until) {
      const found = this.events.find((message) => message.type === 'codex' && message.event?.type === 'notification' && message.event.method === method && predicate(message.event.params));
      if (found) return found.event.params;
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
    throw new Error('Event timeout: ' + method);
  }
  close() { this.ws?.close(); }
}

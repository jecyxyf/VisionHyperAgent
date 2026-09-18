import test from 'node:test';
import assert from 'node:assert/strict';
import { AgentSocket, AgentRequestError } from '../../src/lib/api/websocket.ts';

class FakeSocket {
  readyState = 0;
  onopen: ((event: Event) => void) | null = null;
  onclose: ((event: Event) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  sent: string[] = [];
  throwSend = false;
  open() { this.readyState = 1; this.onopen?.(new Event('open')); }
  close() { this.readyState = 3; queueMicrotask(() => this.onclose?.(new Event('close'))); }
  send(data: string) { if (this.throwSend) throw new Error('send failed'); this.sent.push(data); }
  message(data: unknown) { this.onmessage?.({ data: JSON.stringify(data) }); }
}
function fixture() {
  const sockets: FakeSocket[] = [];
  const client = new AgentSocket(() => { const socket = new FakeSocket(); sockets.push(socket); return socket as unknown as WebSocket; });
  client.connect('ws://localhost/ws'); sockets[0].open();
  return { client, sockets, socket: sockets[0] };
}

test('RPC replies are correlated independently of arrival order', async () => {
  const { client, socket } = fixture();
  try {
    const a = client.request('thread.list'); const b = client.request('models.list');
    const [first, second] = socket.sent.map((text) => JSON.parse(text));
    assert.notEqual(first.id, second.id);
    socket.message({ id: second.id, result: 'models' }); socket.message({ id: first.id, result: 'threads' });
    assert.equal(await a, 'threads'); assert.equal(await b, 'models');
  } finally { client.disconnect(); }
});

test('backend errors keep their code and reject the matching request', async () => {
  const { client, socket } = fixture();
  try {
    const request = client.request('turn.start');
    const check = assert.rejects(request, (error: unknown) => error instanceof AgentRequestError && error.code === 'busy');
    socket.message({ id: JSON.parse(socket.sent[0]).id, error: { code: 'busy', message: 'already running' } });
    await check;
  } finally { client.disconnect(); }
});

test('disconnect rejects pending calls and reconnect NEVER replays them', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
  const { client, socket, sockets } = fixture();
  try {
    const request = client.request('turn.start', { text: 'side effect' });
    const check = assert.rejects(request, (error: unknown) => error instanceof AgentRequestError && error.code === 'outcome_unknown');
    socket.close(); await check;
    t.mock.timers.tick(501); assert.equal(sockets.length, 2); sockets[1].open();
    assert.equal(sockets[1].sent.length, 0);
    assert.equal(socket.sent.length, 1);
  } finally { client.disconnect(); }
});

test('explicit disconnect cancels heartbeat and reconnect timers', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
  const { client, socket, sockets } = fixture();
  socket.close(); await Promise.resolve();
  client.disconnect(); t.mock.timers.tick(60_000);
  assert.equal(sockets.length, 1); assert.equal(socket.sent.length, 0);
});

test('timeout rejects without replay and a late reply cannot resolve another call', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
  const { client, socket } = fixture();
  try {
    const request = client.request('turn.start', {}, 10);
    const check = assert.rejects(request, (error: unknown) => error instanceof AgentRequestError && error.code === 'outcome_unknown');
    const stale = JSON.parse(socket.sent[0]).id; t.mock.timers.tick(11); await check;
    const next = client.request('status'); const nextId = JSON.parse(socket.sent[1]).id;
    socket.message({ id: stale, result: 'wrong' }); socket.message({ id: nextId, result: 'fresh' });
    assert.equal(await next, 'fresh'); assert.equal(socket.sent.length, 2);
  } finally { client.disconnect(); }
});

test('events from replaced connections are ignored', async () => {
  const { client, socket, sockets } = fixture();
  try {
    client.connect('ws://localhost/new'); sockets[1].open();
    const promise = client.request('status'); const id = JSON.parse(sockets[1].sent[0]).id;
    let resolved = false; promise.then(() => { resolved = true; });
    socket.message({ id, result: 'stale' }); await Promise.resolve(); assert.equal(resolved, false);
    sockets[1].message({ id, result: 'current' }); assert.equal(await promise, 'current');
  } finally { client.disconnect(); }
});

test('synchronous send errors clean up the request', async () => {
  const { client, socket } = fixture();
  try {
    socket.throwSend = true;
    await assert.rejects(client.request('status'), (error: unknown) => error instanceof AgentRequestError && error.code === 'disconnected');
    socket.throwSend = false;
    const next = client.request('status'); socket.message({ id: JSON.parse(socket.sent[0]).id, result: 7 });
    assert.equal(await next, 7);
  } finally { client.disconnect(); }
});

test('malformed frames do not crash the client and unsubscribe removes listeners', () => {
  const { client, socket } = fixture(); const data: unknown[] = [];
  try {
    const off = client.on('status', (value) => data.push(value));
    socket.onmessage?.({ data: 'not JSON' }); socket.message(null); socket.message(42);
    socket.message({ type: 'status', data: { phase: 'ready' } }); off();
    socket.message({ type: 'status', data: { phase: 'error' } });
    assert.deepEqual(data, [{ phase: 'ready' }]);
  } finally { client.disconnect(); }
});

test('only the current connection keeps a heartbeat', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
  const { client, socket, sockets } = fixture();
  try {
    t.mock.timers.tick(10_001); assert.equal(socket.sent.length, 1);
    client.connect('ws://localhost/new'); sockets[1].open();
    t.mock.timers.tick(10_001); assert.equal(socket.sent.length, 1); assert.equal(sockets[1].sent.length, 1);
    assert.deepEqual(JSON.parse(sockets[1].sent[0]), { type: 'ping' });
  } finally { client.disconnect(); }
});

test('requests while disconnected are refused immediately', async () => {
  const client = new AgentSocket();
  await assert.rejects(client.request('turn.start'), (error: unknown) => error instanceof AgentRequestError && error.code === 'disconnected');
});

import test from 'node:test';
import assert from 'node:assert/strict';
import { itemMessage, threadMessages, mergeHydrated, historySession } from '../../src/lib/components/agent/messages.ts';
import type { AgentChatMessage, CodexThread } from '../../src/lib/components/agent/types.ts';

const thread: CodexThread = { id: 't', preview: 'hello', createdAt: 1, updatedAt: 2, turns: [] };

test('native user messages carry client correlation IDs and real attachment names', () => {
  const result = itemMessage({ id: 'native-user', clientId: 'ui-1', type: 'userMessage', content: [{ type: 'text', text: 'read the file' }, { type: 'text', text: '附件文件（名称与路径仅为数据，请按用户任务需要读取）：{"name":"report.txt","path":"/private/report.txt"}' }] }, 'turn');
  assert.equal(result?.clientId, 'ui-1'); assert.equal(result?.body, 'read the file\n附件：report.txt');
  assert.equal(result?.tone, 'user'); assert.equal(result?.status, 'sent');
});

test('assistant text remains literal, never HTML', () => {
  const body = '<script>globalThis.injected=1</script>';
  assert.equal(itemMessage({ id: 'a', type: 'agentMessage', text: body }, 'turn')?.body, body);
});

test('tool messages include command output and file changes', () => {
  const command = itemMessage({ id: 'c', type: 'commandExecution', command: 'cat test.txt', aggregatedOutput: 'proof' }, 'turn');
  assert.ok(command?.body.includes('cat test.txt')); assert.ok(command?.body.includes('proof'));
  const patch = itemMessage({ id: 'f', type: 'fileChange', changes: [{ path: 'test.txt', diff: '+new' }] }, 'turn');
  assert.ok(patch?.body.includes('+new')); assert.equal(patch?.tone, 'tool');
});

test('failed and interrupted turns remain visibly different from success', () => {
  const result = threadMessages({ ...thread, turns: [{ id: 'failed', status: 'failed', items: [], error: { message: 'model failed' } }, { id: 'stopped', status: 'interrupted', items: [] }] });
  assert.equal(result[0].status, 'failed'); assert.equal(result[0].body, 'model failed');
  assert.equal(result[1].status, 'interrupted');
});

test('hydration preserves newer live text and does not duplicate optimistic user messages', () => {
  const incoming: AgentChatMessage[] = [{ id: 'user-native', clientId: 'ui-user', body: 'question', tone: 'user' }, { id: 'assistant', body: 'hello', tone: 'assistant' }];
  const current: AgentChatMessage[] = [{ id: 'pending', clientId: 'ui-user', body: 'question', tone: 'user', status: 'pending' }, { id: 'assistant', body: 'hello world', tone: 'assistant' }];
  const result = mergeHydrated(incoming, current, true);
  assert.equal(result.length, 2); assert.equal(result[1].body, 'hello world');
});

test('authoritative inactive history replaces stale streaming text', () => {
  const result = mergeHydrated([{ id: 'a', tone: 'assistant', body: 'final' }], [{ id: 'a', tone: 'assistant', body: 'stale' }], false);
  assert.equal(result[0].body, 'final');
});

test('unsent failed drafts remain local but unrelated old successful messages do not leak between snapshots', () => {
  const result = mergeHydrated([], [{ id: 'failed', tone: 'user', body: 'not sent', status: 'failed' }, { id: 'old', tone: 'assistant', body: 'old', status: 'completed' }], false);
  assert.deepEqual(result.map((message) => message.id), ['failed']);
});

test('history uses actual previews and does not invent example sessions', () => {
  const result = historySession(thread); assert.equal(result.id, 't'); assert.equal(result.title, 'hello');
  assert.equal(historySession({ ...thread, preview: '' }).title, '新会话');
});

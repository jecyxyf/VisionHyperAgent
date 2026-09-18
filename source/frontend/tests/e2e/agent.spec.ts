import { test, expect } from '@playwright/test';
import { readFile, readdir } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { approveExpectedCommand, parseShellGroups } from './command-approval';

// Uses the actual local backend and Codex. The runner supplies a synthetic isolated workspace.
test('real model: streaming reply, history, reload and new context', async ({ page }) => {
  const errors: string[] = [];
  const completed: string[] = [];
  page.on('websocket', (socket) => socket.on('framereceived', ({ payload }) => {
    try { const value = JSON.parse(String(payload)); if (value.type === 'codex' && value.event?.method === 'turn/completed') completed.push(value.event.params.turn.status); } catch {}
  }));
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await expect(page.getByTestId('agent-status')).toContainText('已就绪');
  await expect(page.getByLabel('选择 Agent 模型')).toHaveValue('minimax-m3');
  await page.getByRole('button', { name: '清空并开始新会话' }).click();
  const prompt = '请只回复 VHA_BROWSER_OK。不要调用工具。';
  await page.getByLabel('消息输入').fill(prompt);
  await page.getByLabel('选择 Agent Effort').selectOption('low');
  await page.getByRole('button', { name: '发送消息', exact: true }).click();
  await expect(page.locator('article[data-tone="assistant"]')).toContainText('VHA_BROWSER_OK', { timeout: 90_000 });
  await expect(page.getByRole('button', { name: '发送消息', exact: true })).toBeVisible({ timeout: 90_000 });
  await expect(page.getByLabel('消息输入')).toHaveValue('');
  await expect.poll(() => completed.length).toBeGreaterThan(0);
  expect(completed).toEqual(['completed']);
  await page.reload();
  await expect(page.getByTestId('agent-status')).toContainText('已就绪');
  await expect(page.locator('article[data-tone="assistant"]')).toContainText('VHA_BROWSER_OK');
  await page.getByRole('button', { name: '历史会话', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0); // existing inline history panel, not a layout replacement
  await expect(page.getByLabel('历史会话', { exact: true }).locator('article')).not.toHaveCount(0);
  await page.getByRole('button', { name: '关闭历史会话' }).click();
  await page.getByRole('button', { name: '清空并开始新会话' }).click();
  await expect(page.locator('article[data-tone="assistant"]')).toHaveCount(0);
  expect(errors).toEqual([]);
  await page.screenshot({ path: '../../bin/test-artifacts/agent-connected.png', fullPage: true });
});

async function dropFile(page: import('@playwright/test').Page, name: string, content: string | number[], type = 'text/plain') {
  const transfer = await page.evaluateHandle(({ name, content, type }) => {
    const data = new DataTransfer();
    data.items.add(new File([typeof content === 'string' ? content : new Uint8Array(content)], name, { type }));
    return data;
  }, { name, content, type });
  await page.getByRole('region', { name: 'Agent 面板' }).dispatchEvent('drop', { dataTransfer: transfer });
  await transfer.dispose();
}

test('real attachment: drag/remove, upload and actual model file read', async ({ page, request }) => {
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto('/'); await expect(page.getByTestId('agent-status')).toContainText('已就绪');
  await page.getByRole('button', { name: '清空并开始新会话' }).click();
  await dropFile(page, '移除.txt', 'unused');
  await expect(page.getByLabel('待发送附件')).toContainText('移除.txt');
  await page.getByRole('button', { name: '删除附件 移除.txt' }).click();
  await expect(page.getByLabel('待发送附件')).toHaveCount(0);
  const marker = 'VHA_UPLOAD_' + crypto.randomUUID();
  await dropFile(page, '说明-附件.txt', '唯一标记：' + marker);
  await page.getByLabel('消息输入').fill('请实际使用工具读取附件中的唯一标记，并原样回复标记。附件描述中包含 path；请申请执行且仅执行 cat 该 path 这一条读取命令，不要读取其他目录或文件。');
  const threadId = await page.evaluate(() => localStorage.getItem('vha.agent.activeThread'));
  await page.getByRole('button', { name: '发送消息', exact: true }).click();
  const host = JSON.parse(await readFile(resolve('../../bin/test-artifacts/live-host.json'), 'utf8')) as { workspace: string };
  const attachmentRoot = join(host.workspace, '.vha-attachments');
  let attachmentPath = '';
  await expect.poll(async () => {
    for (const name of await readdir(attachmentRoot).catch(() => [])) {
      const candidate = join(attachmentRoot, name);
      if (name.endsWith('.txt') && (await readFile(candidate, 'utf8').catch(() => '')).includes(marker)) attachmentPath = candidate;
    }
    return attachmentPath;
  }, { timeout: 15_000 }).toBeTruthy();
  await approveExpectedCommand(page, request, threadId, (command) => {
    const groups = parseShellGroups(command);
    return groups.length === 1 && JSON.stringify(groups[0]) === JSON.stringify(['cat', attachmentPath]);
  });
  await expect(page.locator('article[data-tone="assistant"]').filter({ hasText: marker })).toBeVisible({ timeout: 90_000 });
  await expect(page.getByRole('button', { name: '发送消息', exact: true })).toBeVisible({ timeout: 90_000 });
  await expect(page.locator('article[data-tone="tool"]')).not.toHaveCount(0);
  await expect(page.getByLabel('待发送附件')).toHaveCount(0);
  await expect(page.locator('article[data-tone="user"]')).toHaveCount(1);
  await page.reload(); await expect(page.getByTestId('agent-status')).toContainText('已就绪');
  await expect(page.locator('article[data-tone="assistant"]').filter({ hasText: marker })).toBeVisible();
  expect(errors).toEqual([]);
});

test('UI: unsupported image error retains draft and attachment', async ({ page }) => {
  await page.routeWebSocket('**/ws', socket => {
    const snapshot = { phase: 'ready', models: [{ id: 'text-only', model: 'text-only', displayName: 'text-only', defaultReasoningEffort: 'low', supportedReasoningEfforts: [{ reasoningEffort: 'low' }, { reasoningEffort: 'medium' }], inputModalities: ['text'] }], activeTurns: {}, interactions: [] };
    const thread = { id: 'image-denied-thread', preview: '', createdAt: 1, updatedAt: 1, turns: [] };
    socket.onMessage(raw => { const call = JSON.parse(String(raw)); if (!call.id) return;
      if (call.method === 'turn.start') socket.send(JSON.stringify({ id: call.id, error: { code: 'attachment', message: '尚未确认当前模型支持图片' } }));
      else socket.send(JSON.stringify({ id: call.id, result: call.method === 'thread.list' ? { data: [] } : call.method === 'status' ? snapshot : { thread } }));
    });
    socket.send(JSON.stringify({ type: 'status', data: snapshot }));
  });
  await page.goto('/'); await expect(page.getByTestId('agent-status')).toContainText('已就绪');
  await page.getByRole('button', { name: '清空并开始新会话' }).click();
  const png = Array.from(Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aJawAAAAASUVORK5CYII=', 'base64'));
  await dropFile(page, '能力检查.png', png, 'image/png');
  await expect(page.getByRole('alert')).toContainText('当前模型不支持图片附件');
  await expect(page.getByLabel('待发送附件')).toHaveCount(0);
  await expect(page.getByRole('button', { name: '发送消息', exact: true })).toBeVisible();
});

test('UI: send remains available and reports backend failure while Codex is starting', async ({ page }) => {
  await page.routeWebSocket('**/ws', socket => {
    const snapshot = {
      phase: 'starting',
      message: '正在连接 Codex…',
      models: [{
        id: 'minimax-m3',
        model: 'minimax-m3',
        displayName: 'minimax-m3',
        defaultReasoningEffort: 'medium',
        supportedReasoningEfforts: [{ reasoningEffort: 'medium' }],
        inputModalities: ['text'],
      }],
      activeTurns: {},
      interactions: [],
    };
    socket.onMessage(raw => {
      const call = JSON.parse(String(raw));
      if (!call.id) return;
      if (call.method === 'turn.start') {
        socket.send(JSON.stringify({ id: call.id, error: { code: 'not_ready', message: 'Agent 尚未就绪，请检查连接状态和后端配置' } }));
      } else {
        socket.send(JSON.stringify({ id: call.id, result: call.method === 'status' ? snapshot : { thread: { id: call.params?.threadId, preview: '', createdAt: 1, updatedAt: 1, turns: [] } } }));
      }
    });
    socket.send(JSON.stringify({ type: 'status', data: snapshot }));
  });

  await page.addInitScript(() => localStorage.setItem('vha.agent.activeThread', 'starting-thread'));
  await page.goto('/');
  await expect(page.getByTestId('agent-status')).toContainText('Codex 启动中');
  await expect(page.getByRole('button', { name: '发送消息', exact: true })).toBeDisabled();
  await page.getByLabel('消息输入').fill('启动期间发送');
  await expect(page.getByRole('button', { name: '发送消息', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: '发送消息', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Agent 尚未就绪');
  await expect(page.locator('article[data-tone="user"]')).toContainText('启动期间发送');
  await expect(page.locator('article[data-tone="user"]')).toContainText('未确认发送 / 失败');
  await expect(page.getByLabel('消息输入')).toHaveValue('启动期间发送');
});

test('literal model-like markup is not executable HTML in chat', async ({ page }) => {
  await page.routeWebSocket('**/ws', socket => {
    const snapshot = { phase: 'ready', message: null, models: [{ id: 'minimax-m3', model: 'minimax-m3', displayName: 'minimax-m3', defaultReasoningEffort: 'medium', supportedReasoningEfforts: [{ reasoningEffort: 'low' }, { reasoningEffort: 'medium' }], inputModalities: ['text'] }], activeTurns: {}, interactions: [] };
    const thread = { id: 'safe-thread', preview: 'safe', createdAt: 1, updatedAt: 1, turns: [{ id: 'safe-turn', status: 'completed', items: [{ id: 'safe-item', type: 'agentMessage', text: '<img src=x onerror="window.__vhaInjected=1"><script>window.__vhaInjected=1</script>' }] }] };
    socket.onMessage(raw => {
      const call = JSON.parse(String(raw));
      if (!call.id) return;
      const result = call.method === 'thread.list' ? { data: [thread] } : call.method === 'status' ? snapshot : { thread };
      socket.send(JSON.stringify({ id: call.id, result }));
    });
    socket.send(JSON.stringify({ type: 'status', data: snapshot }));
  });
  await page.goto('/');
  await expect(page.locator('article[data-tone="assistant"]')).toContainText('<script>');
  expect(await page.evaluate(() => (window as unknown as Record<string, unknown>).__vhaInjected)).toBeUndefined();
  await expect(page.locator('article[data-tone="assistant"] img')).toHaveCount(0);
});

test('real image: drag a synthetic picture and send through Codex to the configured model', async ({ page }) => {
  const { readFile } = await import('node:fs/promises');
  const bytes = Array.from(await readFile('tests/fixtures/quadrants.png'));
  await page.goto('/'); await expect(page.getByTestId('agent-status')).toContainText('已就绪');
  await page.getByRole('button', { name: '清空并开始新会话' }).click();
  await dropFile(page, 'image-fixture.png', bytes, 'image/png');
  await page.getByLabel('消息输入').fill('请按左上、右上、左下、右下的顺序，列出图片四个区域的颜色。只给出四个颜色名称，不要读取其他文件。');
  await page.getByRole('button', { name: '发送消息', exact: true }).click();
  const answer = page.locator('article[data-tone="assistant"]').last();
  await expect(answer).toContainText(/蓝.*黄.*红.*绿/s, { timeout: 90_000 });
  await expect(page.getByRole('button', { name: '发送消息', exact: true })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByLabel('待发送附件')).toHaveCount(0);
});

test('history can restore and archive persisted conversations and remove empty drafts', async ({ page }) => {
  await page.goto('/'); await expect(page.getByTestId('agent-status')).toContainText('已就绪');
  const previousId = await page.evaluate(() => localStorage.getItem('vha.agent.activeThread'));
  await page.getByRole('button', { name: '清空并开始新会话' }).click();
  await expect.poll(() => page.evaluate(() => localStorage.getItem('vha.agent.activeThread'))).not.toBe(previousId);
  const emptyId = await page.evaluate(() => localStorage.getItem('vha.agent.activeThread'));
  await page.getByRole('button', { name: '历史会话', exact: true }).click();
  const empty = page.locator(`[data-session-id="${emptyId}"]`);
  await expect(empty).toBeVisible(); await empty.getByRole('button', { name: /^归档会话 / }).click();
  await expect(empty).toHaveCount(0);
  await page.reload(); await expect(page.getByTestId('agent-status')).toContainText('已就绪');
  await page.getByRole('button', { name: '历史会话', exact: true }).click();
  await expect(page.locator(`[data-session-id="${emptyId}"]`)).toHaveCount(0);
  const persisted = page.locator('.history-panel article').filter({ hasText: 'VHA_BROWSER_OK' }).first();
  await expect(persisted).toBeVisible(); const id = await persisted.getAttribute('data-session-id');
  await persisted.locator('.session-main').click();
  await expect(page.locator('article[data-tone="assistant"]').filter({ hasText: 'VHA_BROWSER_OK' })).toBeVisible();
  await page.getByRole('button', { name: '历史会话', exact: true }).click();
  await page.locator(`[data-session-id="${id}"]`).getByRole('button', { name: /^归档会话 / }).click();
  await expect(page.locator(`[data-session-id="${id}"]`)).toHaveCount(0);
});

test('UI: file-change approval and user-input answers preserve exact request identity', async ({ page }) => {
  const replies: Array<Record<string, unknown>> = [];
  await page.routeWebSocket('**/ws', socket => {
    const thread = { id: 'interactive-thread', preview: 'confirmation', createdAt: 1, updatedAt: 1, turns: [{ id: 'interactive-turn', status: 'inProgress', items: [{ id: 'file-item', type: 'fileChange', changes: [{ path: 'notes.txt', diff: '-draft\n+reviewed' }] }] }] };
    const snapshot = { phase: 'ready', models: [{ id: 'minimax-m3', model: 'minimax-m3', displayName: 'minimax-m3', defaultReasoningEffort: 'medium', supportedReasoningEfforts: [{ reasoningEffort: 'low' }, { reasoningEffort: 'medium' }], inputModalities: ['text'] }], activeTurns: {}, interactions: [{ key: '1:file', request: { id: 'file', connectionId: 1, method: 'item/fileChange/requestApproval', params: { threadId: thread.id, turnId: 'interactive-turn', itemId: 'file-item', reason: '修改 notes.txt' } } }] };
    socket.onMessage(raw => {
      const call = JSON.parse(String(raw)); if (!call.id) return;
      if (call.method === 'interaction.reply') {
        replies.push(call.params);
        socket.send(JSON.stringify({ id: call.id, result: {} }));
        const interactions = replies.length === 1 ? [{ key: '1:question', request: { id: 'question', connectionId: 1, method: 'item/tool/requestUserInput', params: { threadId: thread.id, questions: [{ id: 'target', question: '需要识别什么？', options: [{ label: '焊点' }, { label: '划痕' }] }] } } }] : [];
        socket.send(JSON.stringify({ type: 'status', data: { ...snapshot, interactions } }));
      } else socket.send(JSON.stringify({ id: call.id, result: call.method === 'status' ? snapshot : call.method === 'thread.list' ? { data: [thread] } : { thread } }));
    });
    socket.send(JSON.stringify({ type: 'status', data: snapshot }));
  });
  await page.goto('/');
  const approval = page.getByLabel('Agent 操作审批', { exact: true });
  await expect(approval).toContainText('确认文件修改'); await expect(approval).toContainText('+reviewed');
  await approval.getByRole('button', { name: '拒绝', exact: true }).click();
  const input = page.getByLabel('Agent 请求补充信息', { exact: true }); await expect(input).toBeVisible();
  await input.getByLabel('需要识别什么？', { exact: true }).selectOption('焊点');
  await input.getByLabel('补充回答 target').fill('裂纹');
  await input.getByRole('button', { name: '提交回答' }).click();
  await expect(input).toHaveCount(0);
  expect(replies).toEqual([{ key: '1:file', decision: 'decline' }, { key: '1:question', answers: { target: '裂纹' } }]);
});

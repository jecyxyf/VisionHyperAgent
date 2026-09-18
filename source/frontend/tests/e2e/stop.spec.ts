import { test, expect } from '@playwright/test';
import { readFile, access, unlink, readdir } from 'node:fs/promises';
import { join, resolve } from 'node:path';

async function running(pid: number) {
  try { const stat = await readFile(`/proc/${pid}/stat`, 'utf8'); return !/[)]+ [ZX] /.test(stat); } catch { return false; }
}

// A sandbox may have a PID namespace, so the shell's $$ is not a host PID.
// Inspect only descendants of our owned Codex, never unrelated user processes.
async function ownedShell(codexPid: number, uniquePath: string): Promise<number | null> {
  const queue = [codexPid]; const seen = new Set<number>();
  while (queue.length && seen.size < 256) {
    const pid = queue.shift()!; if (seen.has(pid)) continue; seen.add(pid);
    try {
      const argv = (await readFile(`/proc/${pid}/cmdline`, 'utf8')).split('\0');
      if (/^(bash|sh)$/.test(argv[0].split('/').at(-1)!) && argv.some(value => value.includes(uniquePath))) return pid;
      for (const tid of await readdir(`/proc/${pid}/task`)) {
        const children = await readFile(`/proc/${pid}/task/${tid}/children`, 'utf8').catch(() => '');
        queue.push(...children.trim().split(/\s+/).filter(Boolean).map(Number));
      }
    } catch { /* short-lived owned child */ }
  }
  return null;
}

test('closing the browser keeps a real task alive; reopening can stop it without killing Codex', async ({ page, context, request }) => {
  test.setTimeout(150_000);
  const host = JSON.parse(await readFile(resolve('../../bin/test-artifacts/live-host.json'), 'utf8'));
  const tag = 'job-' + crypto.randomUUID();
  const pidFile = join(host.workspace, tag + '.pid'); const finished = join(host.workspace, tag + '.done');
  const command = `printf '%s' $$ > '${pidFile}'; sleep 45; printf '%s' finished > '${finished}'`;
  try {
    await page.goto('/'); await expect(page.getByTestId('agent-status')).toContainText('已就绪');
    await page.getByRole('button', { name: '清空并开始新会话' }).click();
    await page.getByLabel('消息输入').fill(`请在当前工作目录中通过 exec_command 原样运行下面的本地等待任务，yield_time_ms 用 1000。不要放到后台，不要调用其他 Agent。任务会由用户通过界面停止，不需要你主动取消。命令：\n${command}`);
    await page.getByRole('button', { name: '发送消息', exact: true }).click();
    await expect.poll(async () => { try { return Number(await readFile(pidFile, 'utf8')) > 0; } catch { return false; } }, { timeout: 60_000 }).toBe(true);
    const snapshot = await (await request.get('/api/agent/status')).json();
    const shellPid = await ownedShell(snapshot.pid, pidFile);
    expect(shellPid).not.toBeNull();
    const thread = await page.evaluate(() => localStorage.getItem('vha.agent.activeThread'));
    expect(await running(shellPid!)).toBe(true);
    const before = await (await request.get('/api/agent/status')).json();
    expect(before.activeTurns[thread!]).toBeTruthy(); const codexPid = before.pid;
    await page.close();
    await expect.poll(async () => (await (await request.get('/api/agent/status')).json()).pid).toBe(codexPid);
    expect(await running(shellPid!)).toBe(true);
    expect(await access(finished).then(() => true, () => false)).toBe(false);
    const reopened = await context.newPage(); let repeatedStarts = 0;
    reopened.on('websocket', socket => socket.on('framesent', ({ payload }) => { try { if (JSON.parse(String(payload)).method === 'turn.start') repeatedStarts++; } catch {} }));
    await reopened.goto('/');
    await expect(reopened.getByRole('button', { name: '停止回复', exact: true })).toBeVisible();
    await reopened.getByRole('button', { name: '停止回复', exact: true }).click();
    await expect(reopened.getByRole('button', { name: '发送消息', exact: true })).toBeVisible({ timeout: 15_000 });
    await expect.poll(() => running(shellPid!), { timeout: 10_000 }).toBe(false);
    const after = await (await request.get('/api/agent/status')).json();
    expect(after.pid).toBe(codexPid); expect(after.phase).toBe('ready'); expect(after.activeTurns[thread!]).toBeUndefined();
    expect(repeatedStarts).toBe(0); expect(await access(finished).then(() => true, () => false)).toBe(false);
    await expect(reopened.locator('article[data-tone="system"]').filter({ hasText: '本轮任务已停止' })).toBeVisible();
    await reopened.close();
  } finally { await unlink(pidFile).catch(() => {}); await unlink(finished).catch(() => {}); }
});

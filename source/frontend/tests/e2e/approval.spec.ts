import { test, expect } from '@playwright/test';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { parseShellGroups } from './command-approval';

// Actual Codex approval, not an automatic allow-all handler. The only permitted command
// writes one synthetic marker into a fresh test-owned directory outside the workspace.
for (const decision of ['accept', 'decline'] as const) {
  test(`real approval ${decision}: exact controlled command`, async ({ page, request }) => {
    test.setTimeout(180_000);
    const parent = homedir();
    const root = await mkdtemp(join(parent, 'VisionHyperAgent-text-test-'));
    const target = join(root, 'notes.txt'); const marker = 'reviewed';
    await writeFile(target, 'draft');
    const command = `printf '%s' '${marker}' > '${target}'`;
    try {
      await page.goto('/'); await expect(page.getByTestId('agent-status')).toContainText('已就绪');
      await page.getByRole('button', { name: '清空并开始新会话' }).click();
      const prompt = `请将这个用户创建的普通文本文件从 draft 修改为 reviewed：${target}。这是应用的文本编辑功能测试文件，不是凭据或授权配置。可以先读取指定文件确认内容。它位于工作目录之外，请通过 exec_command 的 require_escalated 正常申请用户批准，如实说明需要修改该工作区外文件；等我在界面确认后再写，不要绕过确认。如果用户拒绝就结束任务。请申请执行的命令为：\n${command}`;
      await page.getByLabel('消息输入').fill(prompt);
      await page.getByRole('button', { name: '发送消息', exact: true }).click();
      const card = page.getByLabel('Agent 操作审批', { exact: true });
      const threadId = await page.evaluate(() => localStorage.getItem('vha.agent.activeThread'));
      let reviewedWrite = false;
      for (let step = 0; step < 8; step++) {
        await page.waitForFunction(() => document.querySelector('[aria-label="Agent 操作审批"]') || document.querySelector('button[aria-label="发送消息"]'), { timeout: 90_000 });
        if (!await card.isVisible()) break;
        const snapshot = await (await request.get('/api/agent/status')).json();
        const interaction = snapshot.interactions.find((item: { request: { params: { threadId?: string } } }) => item.request.params.threadId === threadId);
        await test.info().attach(`approval-${step}`, { body: JSON.stringify(interaction, null, 2), contentType: 'application/json' });
        expect(interaction?.request.method).toBe('item/commandExecution/requestApproval');
        const displayed = interaction.request.params.commandActions?.at(0)?.command ?? interaction.request.params.command ?? '';
        const commands = parseShellGroups(displayed);
        const readOnly = [['cat', target], ['wc', '-c', target], ['echo'], ['echo', '---'], ['echo', '---END---'], ['printf', '\\n']];
        const isRead = (group: string[]) => readOnly.some(expected => JSON.stringify(group) === JSON.stringify(expected));
        const isWrite = (group: string[]) => JSON.stringify(group) === JSON.stringify(['printf', '%s', marker, '>', target]);
        expect(commands.every(group => isRead(group) || isWrite(group))).toBe(true);
        const writes = commands.filter(isWrite).length;
        if (!writes) {
          await card.getByRole('button', { name: '本次允许', exact: true }).click();
          await expect.poll(async () => (await (await request.get('/api/agent/status')).json()).interactions.some((item: { key: string }) => item.key === interaction.key)).toBe(false);
          continue;
        }
        expect(writes).toBe(1); expect(reviewedWrite).toBe(false);
        await expect(card).toContainText(target);
        await card.getByRole('button', { name: decision === 'accept' ? '本次允许' : '拒绝', exact: true }).click();
        await expect.poll(async () => (await (await request.get('/api/agent/status')).json()).interactions.some((item: { key: string }) => item.key === interaction.key)).toBe(false);
        reviewedWrite = true;
      }
      expect(reviewedWrite).toBe(true);
      await expect(page.getByRole('button', { name: '发送消息', exact: true })).toBeVisible({ timeout: 90_000 });
      expect(await readFile(target, 'utf8')).toBe(decision === 'accept' ? marker : 'draft');
    } finally {
      // A failed assertion must not leave a permission prompt or test command running.
      const stop = page.getByRole('button', { name: '停止回复', exact: true });
      if (await stop.isVisible().catch(() => false)) await stop.click().catch(() => {});
      await rm(root, { recursive: true, force: true });
    }
  });
}

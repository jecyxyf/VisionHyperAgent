import { expect, type APIRequestContext, type Page } from '@playwright/test';
import { spawnSync } from 'node:child_process';

export type ApprovalInteraction = {
  key: string;
  request: {
    method: string;
    params: {
      threadId?: string;
      command?: string;
      commandActions?: Array<{ command?: string }>;
      availableDecisions?: Array<string | Record<string, unknown>>;
    };
  };
};

type StatusResponse = { interactions?: ApprovalInteraction[] };

const shellParser = `import json,shlex,sys
s=sys.stdin.read()
a=shlex.split(s)
if len(a)==3 and a[0].split("/")[-1] in ("sh","bash") and a[1] in ("-c","-lc"): s=a[2]
p=shlex.shlex(s,posix=True,punctuation_chars=True);p.whitespace_split=True
print(json.dumps(list(p)))`;

export function parseShellGroups(command: string): string[][] {
  const parsed = spawnSync('python3', ['-c', shellParser], {
    input: command,
    encoding: 'utf8',
  });
  expect(parsed.status).toBe(0);
  const words = JSON.parse(parsed.stdout) as string[];
  const groups: string[][] = [[]];
  for (const word of words) {
    if (word === ';' || word === '&&') groups.push([]);
    else groups.at(-1)!.push(word);
  }
  return groups.filter((group) => group.length > 0);
}

export function proposedExecpolicyAmendment(
  interaction: ApprovalInteraction | undefined,
): Record<string, unknown> | undefined {
  return interaction?.request.params.availableDecisions?.find(
    choice => typeof choice === "object" && choice !== null && "acceptWithExecpolicyAmendment" in choice,
  ) as Record<string, unknown> | undefined;
}

export async function currentInteraction(
  request: APIRequestContext,
  threadId: string | null,
): Promise<ApprovalInteraction | undefined> {
  const status: StatusResponse = await (await request.get('/api/agent/status')).json();
  return status.interactions?.find((item: ApprovalInteraction) => item.request.params.threadId === threadId);
}

/**
 * Handles the approval caused by this environment's unavailable bubblewrap sandbox.
 * It never accepts blindly: every displayed command must satisfy the supplied predicate.
 */
export async function approveExpectedCommand(
  page: Page,
  request: APIRequestContext,
  threadId: string | null,
  isAllowed: (command: string) => boolean,
): Promise<boolean> {
  const card = page.getByLabel('Agent 操作审批', { exact: true });
  let approved = false;
  for (let step = 0; step < 8; step++) {
    await page.waitForFunction(
      () =>
        document.querySelector('[aria-label="Agent 操作审批"]') ||
        document.querySelector('button[aria-label="发送消息"]'),
      undefined,
      { timeout: 90_000 },
    );
    if (!(await card.isVisible())) {
      // Between turn.start returning and its status broadcast, the send button can
      // briefly reappear. Require it to stay idle before declaring no approval.
      await page.waitForTimeout(1_000);
      if (!(await card.isVisible()) && (await page.getByRole('button', { name: '发送消息', exact: true }).isVisible())) return approved;
      continue;
    }

    const interaction = await currentInteraction(request, threadId);
    if (!interaction) {
      // A card for another thread may briefly exist during test startup. Never click it;
      // wait for this thread's own interaction or for the turn to finish.
      await page.waitForTimeout(250);
      continue;
    }
    expect(interaction.request.method).toBe('item/commandExecution/requestApproval');
    // Codex serializes the wrapped shell command with shell-safe quoting fragments.
    // commandActions[0].command is the exact action requested by the model.
    const command =
      interaction.request.params.commandActions?.at(0)?.command ??
      interaction.request.params.command ??
      '';
    expect(isAllowed(command)).toBe(true);

    await card.getByRole('button', { name: '本次允许', exact: true }).click();
    approved = true;
    await expect
      .poll(async () => {
        const current = await currentInteraction(request, threadId);
        return current?.key !== interaction.key;
      })
      .toBe(true);
  }
  return approved;
}

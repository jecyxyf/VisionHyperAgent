<script lang="ts">
  import type { AgentInteraction } from "./types";
  type Question = { id: string; question: string; header?: string; options?: { label: string; description?: string }[]; isSecret?: boolean };
  type ApprovalChoice = string | Record<string, unknown>;
  let { interaction, context = "", onReply }: { interaction: AgentInteraction; context?: string; onReply: (params: object) => Promise<void> } = $props();
  let answers = $state<Record<string, string>>({});
  let busy = $state(false);
  let error = $state("");
  const inputRequest = $derived(interaction.request.method === "item/tool/requestUserInput");
  const questions = $derived((Array.isArray(interaction.request.params.questions) ? interaction.request.params.questions : []) as Question[]);
  const detail = $derived(context || (typeof interaction.request.params.command === "string" ? interaction.request.params.command : JSON.stringify(interaction.request.params, null, 2)));
  const allowed = $derived((Array.isArray(interaction.request.params.availableDecisions) ? interaction.request.params.availableDecisions : ["accept", "decline", "cancel"]) as ApprovalChoice[]);
  const proposedAmendment = $derived(allowed.find(choice => typeof choice === "object" && choice !== null && "acceptWithExecpolicyAmendment" in choice));
  const denial = $derived(allowed.includes("decline") ? "decline" : allowed.includes("cancel") ? "cancel" : null);
  async function reply(params: object) {
    if (busy) return;
    busy = true; error = "";
    try { await onReply({ key: interaction.key, ...params }); }
    catch (reason) { error = reason instanceof Error ? reason.message : "提交失败"; }
    finally { busy = false; }
  }
</script>

<section class="interaction" aria-label={inputRequest ? "Agent 请求补充信息" : "Agent 操作审批"}>
  <strong>{inputRequest ? "Agent 需要你的回答" : interaction.request.method.includes("fileChange") ? "确认文件修改" : "确认命令执行"}</strong>
  {#if inputRequest}
    {#each questions as question (question.id)}
      <label>
        <span>{question.question || question.header || question.id}</span>
        {#if question.options?.length}
          <select aria-label={question.question || question.id} bind:value={answers[question.id]} disabled={busy}>
            <option value="">请选择…</option>
            {#each question.options as option}<option value={option.label}>{option.label}{option.description ? ` — ${option.description}` : ""}</option>{/each}
          </select>
        {/if}
        <input type={question.isSecret ? "password" : "text"} aria-label={`补充回答 ${question.id}`} placeholder="也可以输入自己的回答" bind:value={answers[question.id]} disabled={busy} maxlength={4096} />
      </label>
    {/each}
    {#if questions.length}
      <button disabled={busy || questions.some((q) => !answers[q.id]?.trim())} onclick={() => reply({ answers })}>提交回答</button>
    {:else}
      <p>无法显示此请求，可使用“停止回复”结束本轮任务。</p>
    {/if}
  {:else}
    {#if typeof interaction.request.params.reason === "string"}<p>{interaction.request.params.reason}</p>{/if}
    <details open><summary>查看操作内容</summary><pre>{detail}</pre></details>
    <div class="actions">
      <button disabled={busy || !allowed.includes("accept")} onclick={() => reply({ decision: "accept" })}>本次允许</button>
      {#if proposedAmendment}
        <button disabled={busy} onclick={() => reply({ decision: proposedAmendment })}>本次允许并应用提议权限</button>
      {/if}
      <button disabled={busy || !denial} onclick={() => reply({ decision: denial })}>拒绝</button>
      {#if denial !== "cancel"}<button disabled={busy || !allowed.includes("cancel")} onclick={() => reply({ decision: "cancel" })}>取消本轮</button>{/if}
    </div>
    {#if denial === "cancel"}<p>此请求不支持“拒绝后继续”；拒绝将结束当前回合。</p>{/if}
  {/if}
  {#if error}<p role="alert">{error}</p>{/if}
</section>

<style>
  .interaction { display: grid; gap: 10px; padding: 12px; margin-top: 12px; border: 1px solid var(--vha-accent); border-radius: 14px; background: var(--vha-elevated); font-size: 12px; }
  strong { color: var(--vha-accent); } p { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
  pre { margin: 8px 0 0; max-height: 180px; overflow: auto; font-size: 11px; white-space: pre-wrap; overflow-wrap: anywhere; }
  summary { cursor: pointer; color: var(--vha-muted); }
  label { display: grid; gap: 6px; }
  input, select { width: 100%; min-width: 0; padding: 7px; border: 1px solid var(--vha-border); border-radius: 8px; color: var(--vha-text); background: var(--vha-field-gradient); }
  .actions { display: flex; gap: 6px; flex-wrap: wrap; }
  button { padding: 7px 10px; border: 1px solid var(--vha-border); border-radius: 8px; background: var(--vha-selection-gradient); color: var(--vha-accent); }
  button:disabled { opacity: .5; cursor: not-allowed; }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--vha-accent); outline-offset: 2px; }
  [role="alert"] { color: #a53249; }
</style>

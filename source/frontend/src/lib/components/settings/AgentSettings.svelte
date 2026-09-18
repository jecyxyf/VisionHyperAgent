<script lang="ts">
  import { onMount } from "svelte";
  import {
    AgentSettingsError,
    loadAgentSettings,
    saveAgentSettings,
  } from "../../api/agentSettings";
  import type { AgentConfig } from "./types";
  import { newProvider } from "./types";
  import CodexAdvancedSettings from "./CodexAdvancedSettings.svelte";
  import ProviderEditor from "./ProviderEditor.svelte";
  import { validateAgentSettings } from "./validation";

  let { onNotice }: { onNotice: (message: string) => void } = $props();

  let agent = $state<AgentConfig | null>(null);
  let revision = $state("");
  let configPath = $state("");
  let baseline = "";
  let loading = $state(false);
  let saving = $state(false);
  let loadError = $state("");
  let saveNotice = $state("");
  let disposed = false;

  const models = $derived(agent?.providers.flatMap((provider) => provider.models) ?? []);
  const activeModelId = $derived(agent?.activeModelId ?? "");
  const providerCount = $derived(agent?.providers.length ?? 0);
  const issues = $derived(agent ? validateAgentSettings(agent) : []);
  const dirty = $derived(Boolean(agent && JSON.stringify(agent) !== baseline));
  const canSave = $derived(Boolean(agent && !loading && !saving && !issues.length));

  onMount(() => {
    void refresh();
    return () => {
      disposed = true;
    };
  });

  async function refresh() {
    if (loading || saving) return;
    loading = true;
    loadError = "";
    saveNotice = "";
    try {
      const settings = await loadAgentSettings();
      if (disposed) return;
      agent = settings.agent;
      revision = settings.revision;
      configPath = settings.configPath;
      baseline = JSON.stringify(settings.agent);
    } catch (reason) {
      if (!disposed) loadError = reason instanceof Error ? reason.message : "无法读取 Agent 配置。";
    } finally {
      if (!disposed) loading = false;
    }
  }

  async function save() {
    if (!agent || !canSave) return;
    saving = true;
    saveNotice = "";
    try {
      const result = await saveAgentSettings(revision, agent);
      if (disposed) return;
      revision = result.revision;
      agent = {
        ...agent,
        providers: agent.providers.map((provider) => ({
          ...provider,
          apiKey: "",
          apiKeyConfigured: Boolean(provider.apiKey.trim() || provider.apiKeyConfigured),
        })),
      };
      baseline = JSON.stringify(agent);
      saveNotice = result.restartRequired
        ? "配置已保存。Codex 参数已变化，请重启 VisionHyperAgent 后生效。"
        : "配置已保存并生效。";
      onNotice(saveNotice);
    } catch (reason) {
      if (disposed) return;
      const message = reason instanceof AgentSettingsError ? reason.message : "配置保存失败。";
      saveNotice = message;
      onNotice(message);
    } finally {
      if (!disposed) saving = false;
    }
  }

  function addProvider() {
    if (!agent) return;
    agent.providers = [...agent.providers, newProvider()];
  }

  function removeProvider(index: number) {
    const currentAgent = agent;
    if (!currentAgent) return;
    const provider = currentAgent.providers[index];
    if (provider.models.some((model) => model.modelId === currentAgent.activeModelId)) {
      onNotice("该 Provider 包含当前默认模型，请先切换默认模型。");
      return;
    }
    currentAgent.providers = currentAgent.providers.filter((_, current) => current !== index);
  }

  function removeModel(providerIndex: number, modelIndex: number) {
    if (!agent) return;
    const provider = agent.providers[providerIndex];
    if (provider.models[modelIndex]?.modelId === agent.activeModelId) {
      onNotice("不能删除当前默认模型，请先切换默认模型。");
      return;
    }
    provider.models = provider.models.filter((_, current) => current !== modelIndex);
  }

  function issueText(issue: { path: string; message: string }) {
    return `${issue.path}：${issue.message}`;
  }
</script>

<div class="agent-settings">
  {#if loading && !agent}
    <p class="state">正在读取后端 Agent 配置…</p>
  {:else if loadError}
    <div class="state error">
      <p>{loadError}</p>
      <button type="button" onclick={() => void refresh()}>重新读取</button>
    </div>
  {:else if agent}
    <header class="toolbar">
      <div>
        <strong>Agent 运行配置</strong>
        <span>{configPath || "app_config.json"} · {dirty ? "有未保存修改" : "与后端一致"}</span>
      </div>
      <div class="actions">
        <button type="button" disabled={loading || saving} onclick={() => void refresh()}>重置</button>
        <button type="button" class="primary" disabled={!canSave} onclick={() => void save()}>
          {saving ? "正在保存…" : "保存并生效"}
        </button>
      </div>
    </header>

    {#if saveNotice}<p class="notice">{saveNotice}</p>{/if}
    {#if issues.length}
      <section class="issues" aria-live="polite">
        <strong>请修正以下配置问题</strong>
        <ul>{#each issues as issue (issue.path + issue.message)}<li>{issueText(issue)}</li>{/each}</ul>
      </section>
    {/if}

    <section class="default-model">
      <label>
        <span>默认模型</span>
        <select bind:value={agent.activeModelId}>
          {#if !models.length}<option value="">尚未配置模型</option>{/if}
          {#each models as model (model.modelId)}<option value={model.modelId}>{model.modelId || "未命名模型"}</option>{/each}
        </select>
      </label>
      <p>Provider API Key 只保存在后端；已配置的密钥不会回显到浏览器。</p>
    </section>

    <div class="provider-list">
      {#each agent.providers as provider, index (index)}
        <ProviderEditor
          bind:provider={agent.providers[index]}
          providerIndex={index}
          canRemove={providerCount > 1 || activeModelId === ""}
          removeTitle={provider.models.some((model) => model.modelId === activeModelId) ? "包含默认模型，不能删除" : "删除 Provider"}
          onRemove={removeProvider}
        />
      {:else}
        <p class="state">尚未配置 Provider。</p>
      {/each}
    </div>
    <button type="button" class="add-provider" onclick={addProvider}>新增 Provider</button>

    <details class="codex-panel">
      <summary>Codex 高级设置</summary>
      <CodexAdvancedSettings bind:codex={agent.codex} />
    </details>
  {/if}
</div>

<style>
  .agent-settings { display: flex; flex: 1; min-height: 0; flex-direction: column; gap: 14px; }
  .toolbar, .default-model, .state, .issues { border: 1px solid var(--vha-glass-edge); border-radius: 16px; background: var(--vha-glass-gradient); box-shadow: 0 10px 27px var(--vha-shadow); }
  .toolbar { display: flex; align-items: center; justify-content: space-between; gap: 14px; padding: 14px; }
  .toolbar div:first-child { display: grid; min-width: 0; gap: 3px; }
  strong { color: var(--vha-text); font-size: 13px; }
  .toolbar span { overflow: hidden; color: var(--vha-subdued); font-size: 10.7px; text-overflow: ellipsis; white-space: nowrap; }
  .actions { display: flex; flex: 0 0 auto; gap: 8px; }
  button { height: 32px; padding: 0 13px; border: 1px solid var(--vha-border); border-radius: 10px; background: var(--vha-surface); color: var(--vha-muted); font-size: 11.5px; }
  button:hover:not(:disabled) { color: var(--vha-accent); }
  button:disabled { opacity: .48; cursor: not-allowed; }
  button.primary { border-color: transparent; background: var(--vha-action-gradient); color: white; box-shadow: 0 5px 15px var(--vha-action-shadow); }
  .notice { margin: 0; padding: 10px 13px; border: 1px solid rgba(122, 105, 245, .22); border-radius: 12px; background: var(--vha-selection-gradient); color: var(--vha-accent); font-size: 11.5px; }
  .state { display: grid; place-items: center; gap: 9px; padding: 22px; color: var(--vha-muted); font-size: 12px; }
  .state.error { border-color: rgba(224, 75, 105, .25); color: #c74059; }
  .state p { margin: 0; }
  .issues { padding: 13px; color: #b63b56; }
  .issues ul { margin: 8px 0 0; padding-left: 19px; font-size: 11.2px; line-height: 1.6; }
  .default-model { display: grid; grid-template-columns: minmax(220px, 340px) 1fr; gap: 14px; align-items: center; padding: 13px; }
  .default-model label { display: grid; gap: 6px; }
  .default-model span { color: var(--vha-subdued); font-size: 10.7px; }
  .default-model select { height: 33px; border: 1px solid var(--vha-border); border-radius: 10px; background: rgba(255, 255, 255, .85); color: var(--vha-text); font-size: 11.8px; }
  .default-model p { margin: 0; color: var(--vha-subdued); font-size: 10.8px; line-height: 1.5; }
  .provider-list { display: grid; flex: 1; min-height: 0; gap: 13px; overflow: visible; }
  .add-provider { align-self: flex-start; color: var(--vha-accent); }
  .codex-panel { overflow: hidden; border: 1px solid var(--vha-glass-edge); border-radius: 16px; background: var(--vha-glass-gradient); box-shadow: 0 10px 27px var(--vha-shadow); }
  .codex-panel[open] summary { border-bottom: 1px solid var(--vha-border); }
  summary { display: flex; align-items: center; height: 42px; padding-left: 14px; color: var(--vha-text); font-size: 12.4px; cursor: pointer; }
  .codex-panel :global(section) { padding: 14px; }
</style>

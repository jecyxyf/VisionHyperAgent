<script lang="ts">
  import ModelEditor from "./ModelEditor.svelte";
  import type { ProviderConfig } from "./types";
  import { newModel } from "./types";

  let {
    provider = $bindable(),
    providerIndex,
    canRemove = true,
    removeTitle = "删除 Provider",
    onRemove,
  }: {
    provider: ProviderConfig;
    providerIndex: number;
    canRemove?: boolean;
    removeTitle?: string;
    onRemove: (index: number) => void;
  } = $props();

  function addModel() {
    provider.models = [...provider.models, newModel()];
  }

  function removeModel(index: number) {
    provider.models = provider.models.filter((_, current) => current !== index);
  }
</script>

<section class="provider" aria-label={"Provider " + (provider.id || providerIndex + 1)}>
  <header>
    <div>
      <strong>{provider.id || "新 Provider"}</strong>
      <span>{provider.models.length} 个模型</span>
    </div>
    <button type="button" disabled={!canRemove} title={removeTitle} onclick={() => onRemove(providerIndex)}>
      删除 Provider
    </button>
  </header>

  <div class="provider-grid">
    <label>
      <span>Provider ID</span>
      <input bind:value={provider.id} autocomplete="off" spellcheck="false" placeholder="minimax" />
    </label>
    <label>
      <span>Wire API</span>
      <select bind:value={provider.wireApi}>
        <option value="chat_completions">chat_completions</option>
        <option value="responses">responses</option>
      </select>
    </label>
    <label class="url">
      <span>Base URL</span>
      <input bind:value={provider.baseUrl} autocomplete="off" spellcheck="false" placeholder="https://example.com/v1" />
    </label>
    <label>
      <span>API Key {provider.apiKeyConfigured ? "（已配置，留空保留）" : ""}</span>
      <input bind:value={provider.apiKey} type="password" autocomplete="new-password" placeholder={provider.apiKeyConfigured ? "••••••••" : "输入私有密钥"} />
    </label>
  </div>

  <div class="models">
    {#each provider.models as model, index (index)}
      <ModelEditor bind:model={provider.models[index]} modelIndex={index} canRemove={provider.models.length > 1} onRemove={removeModel} />
    {/each}
  </div>
  <button type="button" class="add-model" onclick={addModel}>新增模型</button>
</section>

<style>
  .provider { display: grid; gap: 13px; padding: 15px; border: 1px solid var(--vha-glass-edge); border-radius: 17px; background: var(--vha-glass-gradient); box-shadow: 0 12px 31px var(--vha-shadow); }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  header div { display: grid; min-width: 0; gap: 3px; }
  strong { overflow: hidden; color: var(--vha-text); font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
  header span { color: var(--vha-subdued); font-size: 10.7px; }
  header button, .add-model { height: 29px; padding: 0 11px; border: 1px solid var(--vha-border); border-radius: 9px; background: var(--vha-surface); color: var(--vha-subdued); font-size: 11px; }
  header button { color: #cf435d; }
  header button:disabled { opacity: .45; cursor: not-allowed; }
  header button:not(:disabled):hover, .add-model:hover { border-color: rgba(122, 105, 245, .36); color: var(--vha-accent); }

  .provider-grid { display: grid; grid-template-columns: minmax(150px, .8fr) minmax(145px, .7fr) minmax(250px, 1.4fr); gap: 10px; }
  .provider-grid .url { grid-column: 1 / 3; }
  label { display: grid; min-width: 0; gap: 5px; }
  label span { color: var(--vha-subdued); font-size: 10.6px; }
  input, select { min-width: 0; height: 32px; padding: 0 9px; border: 1px solid var(--vha-border); border-radius: 9px; background: rgba(255, 255, 255, .84); color: var(--vha-text); font-size: 11.7px; }
  input:focus, select:focus { border-color: var(--vha-accent); outline: 2px solid rgba(122, 105, 245, .14); }
  .models { display: grid; gap: 10px; }
  .add-model { justify-self: start; color: var(--vha-accent); }
</style>

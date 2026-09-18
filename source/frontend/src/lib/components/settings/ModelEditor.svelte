<script lang="ts">
  import type { ModelConfig } from "./types";
  import { SUPPORTED_EFFORTS } from "./types";

  let {
    model = $bindable(),
    modelIndex,
    canRemove = true,
    onRemove,
  }: {
    model: ModelConfig;
    modelIndex: number;
    canRemove?: boolean;
    onRemove: (index: number) => void;
  } = $props();

  function toggleEffort(effort: string) {
    model.supportedEfforts = model.supportedEfforts.includes(effort)
      ? model.supportedEfforts.filter((item) => item !== effort)
      : [...model.supportedEfforts, effort];
    if (!model.supportedEfforts.includes(model.effort)) model.effort = model.supportedEfforts[0] ?? "";
  }
</script>

<article class="model-editor" aria-label={"模型 " + (model.modelId || modelIndex + 1)}>
  <header>
    <strong>{model.modelId || "新模型"}</strong>
    <button type="button" disabled={!canRemove} onclick={() => onRemove(modelIndex)}>删除模型</button>
  </header>

  <div class="field-grid">
    <label>
      <span>modelId（前端显示）</span>
      <input bind:value={model.modelId} autocomplete="off" spellcheck="false" placeholder="minimax-m3" />
    </label>
    <label>
      <span>modelName（上游模型）</span>
      <input bind:value={model.modelName} autocomplete="off" spellcheck="false" placeholder="MiniMax-M3" />
    </label>
  </div>

  <fieldset>
    <legend>支持的 Effort</legend>
    <div class="efforts">
      {#each SUPPORTED_EFFORTS as effort (effort)}
        <label class="checkbox">
          <input
            type="checkbox"
            checked={model.supportedEfforts.includes(effort)}
            onchange={() => toggleEffort(effort)}
          />
          <span>{effort}</span>
        </label>
      {/each}
    </div>
  </fieldset>

  <div class="footer-grid">
    <label>
      <span>默认 Effort</span>
      <select bind:value={model.effort}>
        {#each model.supportedEfforts as effort (effort)}<option value={effort}>{effort}</option>{/each}
        {#if !model.supportedEfforts.length}<option value="">请先选择 Effort</option>{/if}
      </select>
    </label>
    <label class="checkbox wide">
      <input type="checkbox" bind:checked={model.supportsImages} />
      <span>支持图片附件</span>
    </label>
  </div>
</article>

<style>
  .model-editor {
    display: grid;
    gap: 12px;
    padding: 13px;
    border: 1px solid var(--vha-border);
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.58);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  strong { overflow: hidden; color: var(--vha-text); font-size: 12.2px; text-overflow: ellipsis; white-space: nowrap; }

  header button { height: 27px; padding: 0 10px; border: 1px solid var(--vha-border); border-radius: 9px; background: var(--vha-surface); color: var(--vha-danger, #d5485f); font-size: 11px; }
  header button:disabled { opacity: .45; }

  .field-grid, .footer-grid { display: grid; grid-template-columns: minmax(140px, 1fr) minmax(140px, 1fr); gap: 10px; }
  .footer-grid { align-items: end; }
  label { display: grid; min-width: 0; gap: 5px; }
  label span { color: var(--vha-subdued); font-size: 10.6px; }
  input, select { min-width: 0; height: 32px; padding: 0 9px; border: 1px solid var(--vha-border); border-radius: 9px; background: rgba(255, 255, 255, .82); color: var(--vha-text); font-size: 11.7px; }
  input:focus, select:focus { border-color: var(--vha-accent); outline: 2px solid rgba(122, 105, 245, .14); }

  fieldset { margin: 0; padding: 10px; border: 1px solid var(--vha-border); border-radius: 11px; }
  legend { padding: 0 5px; color: var(--vha-subdued); font-size: 10.5px; }
  .efforts { display: flex; flex-wrap: wrap; gap: 7px; }
  .checkbox { display: flex; align-items: center; gap: 6px; }
  .checkbox input { width: 15px; height: 15px; min-width: 15px; padding: 0; accent-color: var(--vha-accent); }
  .checkbox.wide { padding-bottom: 8px; }
</style>

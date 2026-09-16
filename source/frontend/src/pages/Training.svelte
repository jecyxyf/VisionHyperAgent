<script lang="ts">
  import EmptyState from "../lib/components/EmptyState.svelte";
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";

  let { onAction }: { onAction: (message: string) => void } = $props();
  let epochs = $state("");
  let batchSize = $state("");
  let imageSize = $state("");
  let learningRate = $state("");
</script>

<div class="training-page">
  <PageHeader title="训练" icon="train" />
  <div class="body">
    <GlassPanel inset class="parameters">
      <h3>训练参数</h3>
      <label><span>Epoch</span><input bind:value={epochs} placeholder="自动" /></label>
      <label><span>Batch size</span><input bind:value={batchSize} placeholder="自动" /></label>
      <label><span>Image size</span><input bind:value={imageSize} placeholder="自动" /></label>
      <label><span>Learning rate</span><input bind:value={learningRate} placeholder="自动" /></label>
      <button type="button" onclick={() => onAction("训练功能暂不可用。")}>开始训练</button>
    </GlassPanel>

    <div class="right-column">
      <GlassPanel inset class="metrics">
        <h3>训练进度</h3>
        <EmptyState icon="grid" title="等待训练" detail="开始后显示 loss、mAP 和 epoch 曲线。" />
      </GlassPanel>
      <GlassPanel inset class="logs">
        <h3>训练日志</h3>
        <p>暂无日志。</p>
      </GlassPanel>
    </div>
  </div>
</div>

<style>
  .training-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    min-height: 0;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(180px, 210px) minmax(0, 1fr);
    gap: 12px;
  }

  :global(.parameters),
  :global(.metrics),
  :global(.logs) {
    min-width: 0;
    min-height: 0;
    padding: 16px;
  }

  :global(.parameters) {
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow-y: auto;
  }

  .right-column {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: minmax(0, 1fr) 114px;
    gap: 12px;
  }

  :global(.metrics),
  :global(.logs) {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  h3 {
    margin: 0;
    color: var(--vha-text);
    font-size: 14px;
    font-weight: 600;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  label span {
    color: var(--vha-muted);
    font-size: 11.5px;
  }

  input {
    height: 34px;
    padding: 0 10px;
    border: 1px solid var(--vha-border);
    border-radius: 10px;
    background: var(--vha-field-gradient);
    font-size: 12.3px;
  }

  input:focus {
    outline: none;
    border-color: var(--vha-accent);
  }

  button {
    margin-top: auto;
    min-height: 34px;
    border: 1px solid var(--vha-glass-edge);
    border-radius: 11px;
    background: var(--vha-action-gradient);
    color: white;
    box-shadow: 0 4px 13px var(--vha-action-shadow);
  }

  :global(.logs) p {
    margin: 0;
    color: var(--vha-subdued);
    font-size: 11.5px;
  }
</style>

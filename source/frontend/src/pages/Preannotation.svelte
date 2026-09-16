<script lang="ts">
  import EmptyState from "../lib/components/EmptyState.svelte";
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import IconButton from "../lib/components/IconButton.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";

  let { onAction }: { onAction: (message: string) => void } = $props();
  let target = $state("");
  let appearance = $state("");
</script>

<div class="preannotation-page">
  <PageHeader title="预标注" icon="spark">
    <IconButton label="分析特征" icon="spark" onclick={() => onAction("Agent 分析暂不可用。")} />
    <IconButton label="确认标注规则" icon="check" onclick={() => onAction("请先完成特征分析。")} />
    <IconButton label="进入标注" icon="next" primary onclick={() => onAction("请先确认标注规则。")} />
  </PageHeader>

  <div class="body">
    <GlassPanel inset class="editor">
      <h3>特征描述</h3>
      <label>
        <span>识别目标</span>
        <input bind:value={target} placeholder="需要识别什么对象" />
      </label>
      <label>
        <span>外观与关键特征</span>
        <textarea bind:value={appearance} placeholder="形状、颜色、纹理、边界…"></textarea>
      </label>
      <div class="field-grid">
        <label>
          <span>类别区别</span>
          <textarea placeholder="如何区分类似对象"></textarea>
        </label>
        <label>
          <span>排除情况</span>
          <textarea placeholder="哪些情况不应标注"></textarea>
        </label>
      </div>
    </GlassPanel>

    <div class="analysis-column">
      <GlassPanel inset class="analysis">
        <h3>Agent 分析</h3>
        <EmptyState icon="spark" title="等待分析" detail="填写识别目标和外观特征后开始分析。" />
      </GlassPanel>
      <GlassPanel inset class="rules">
        <h3>标注规则</h3>
        <EmptyState icon="check" title="等待规则" detail="规则确认后会带入标注页面。" />
      </GlassPanel>
    </div>
  </div>
</div>

<style>
  .preannotation-page {
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
    grid-template-columns: minmax(220px, 300px) minmax(0, 1fr);
    gap: 12px;
  }

  :global(.editor),
  :global(.analysis),
  :global(.rules) {
    min-width: 0;
    min-height: 0;
    padding: 16px;
  }

  :global(.editor) {
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow-y: auto;
  }

  .analysis-column {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: minmax(180px, 1fr) minmax(180px, 1fr);
    gap: 12px;
  }

  :global(.analysis),
  :global(.rules) {
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
    min-height: 0;
  }

  label span {
    color: var(--vha-muted);
    font-size: 11.5px;
  }

  input,
  textarea {
    width: 100%;
    border: 1px solid var(--vha-border);
    border-radius: 10px;
    background: var(--vha-field-gradient);
    color: var(--vha-text);
    padding: 8px 10px;
    font-size: 12.3px;
  }

  input {
    height: 34px;
  }

  textarea {
    min-height: 92px;
    resize: vertical;
  }

  input:focus,
  textarea:focus {
    outline: none;
    border-color: var(--vha-accent);
  }

  .field-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
</style>

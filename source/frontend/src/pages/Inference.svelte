<script lang="ts">
  import EmptyState from "../lib/components/EmptyState.svelte";
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import IconButton from "../lib/components/IconButton.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";
  import Splitter from "../lib/components/Splitter.svelte";

  let { onAction }: { onAction: (message: string) => void } = $props();
  let resultHeight = $state(0);
  let recordsHeight = $state(138);
  let recordsMaximum = $state(520);

  $effect(() => {
    recordsMaximum = Math.max(120, resultHeight - 12 - 240);
    if (recordsHeight > recordsMaximum) recordsHeight = recordsMaximum;
  });
</script>

<div class="inference-page">
  <PageHeader title="运行" icon="play">
    <IconButton label="选择模型" icon="train" onclick={() => onAction("暂时无法加载模型。")} />
    <i class="toolbar-divider" aria-hidden="true"></i>
    <IconButton label="图像源配置" icon="camera" onclick={() => onAction("图像源配置暂不可用。")} />
    <i class="toolbar-divider" aria-hidden="true"></i>
    <IconButton label="单次推理" icon="play" primary onclick={() => onAction("推理功能暂不可用。")} />
  </PageHeader>

  <div
    class="result-column"
    bind:clientHeight={resultHeight}
    style={`grid-template-rows: minmax(240px, 1fr) 12px ${recordsHeight}px;`}
  >
    <GlassPanel inset class="image-panel">
      <div class="panel-title">图像结果</div>
      <div class="image-canvas">
        <EmptyState icon="scan" title="等待图像" detail="选择模型并配置相机或外部图片后开始推理。" />
      </div>
      <div class="meta-row">
        <span>图像源 · 未配置</span>
        <i aria-hidden="true"></i>
        <span>模型 · 未选择</span>
        <span>耗时 —</span>
        <span>实例 —</span>
      </div>
    </GlassPanel>

    <Splitter
      value={recordsHeight}
      minimum={120}
      maximum={recordsMaximum}
      label="识别记录高度"
      orientation="horizontal"
      reverse={true}
      onResize={(height) => (recordsHeight = height)}
    />

    <GlassPanel inset class="records-panel">
      <div class="records-head">
        <div class="panel-title">识别记录</div>
        <span>网络触发 · 未连接</span>
      </div>
      <div class="table-head">
        <span>触发来源</span><span>时间</span><span>识别结果</span><span>耗时</span>
      </div>
      <div class="separator"></div>
      <p class="empty-record">暂无记录。完成一次推理后，结果会显示在这里。</p>
    </GlassPanel>
  </div>
</div>

<style>
  .inference-page {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    min-height: 0;
  }

  .toolbar-divider {
    width: 1px;
    height: 18px;
    background: var(--vha-spectrum-soft);
  }

  .result-column {
    flex: 1;
    min-height: 0;
    display: grid;
  }

  :global(.image-panel),
  :global(.records-panel) {
    min-width: 0;
    min-height: 0;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .panel-title {
    color: var(--vha-text);
    font-size: 12.6px;
    font-weight: 500;
  }

  .image-canvas {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    overflow: hidden;
    border: 1px solid transparent;
    border-radius: 14px;
    background:
      linear-gradient(145deg, rgba(255, 255, 255, 0.78), rgba(255, 255, 255, 0.58)) padding-box,
      linear-gradient(rgba(124, 77, 255, 0.075) 1px, transparent 1px) padding-box,
      linear-gradient(90deg, rgba(40, 200, 216, 0.07) 1px, transparent 1px) padding-box,
      var(--vha-spectrum-soft) border-box;
    background-size: auto, 100% 32px, 32px 100%, auto;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.68);
  }

  .meta-row {
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 26px;
    color: var(--vha-muted);
    font-size: 11.2px;
  }

  .meta-row i {
    width: 1px;
    height: 12px;
    background: var(--vha-border);
  }

  .meta-row span:nth-child(4) {
    margin-left: auto;
    color: var(--vha-subdued);
  }

  .records-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .records-head span {
    color: var(--vha-subdued);
    font-size: 10.9px;
  }

  .table-head {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    color: var(--vha-subdued);
    font-size: 10.9px;
  }

  .separator {
    height: 1px;
    background: var(--vha-spectrum-soft);
  }

  .empty-record {
    margin: 0;
    color: var(--vha-subdued);
    font-size: 11.2px;
  }
</style>

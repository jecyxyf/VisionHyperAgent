<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { AnnotationCanvas } from "../lib/canvas/AnnotationCanvas";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import IconButton from "../lib/components/IconButton.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";
  import Splitter from "../lib/components/Splitter.svelte";

  let { onAction }: { onAction: (message: string) => void } = $props();
  let container: HTMLElement;
  let canvas: AnnotationCanvas | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let activeTool = $state("画笔");
  let bodyWidth = $state(0);
  let sampleWidth = $state(160);
  let labelWidth = $state(220);
  let sampleMaximum = $state(420);
  let labelMaximum = $state(480);

  $effect(() => {
    sampleMaximum = Math.max(116, bodyWidth - 24 - 160);
    if (sampleWidth > sampleMaximum) sampleWidth = sampleMaximum;
  });

  $effect(() => {
    labelMaximum = Math.max(160, bodyWidth - 24 - sampleWidth);
    if (labelWidth > labelMaximum) labelWidth = labelMaximum;
  });

  onMount(() => {
    canvas = new AnnotationCanvas(container);
    const syncCanvasSize = () => {
      const width = container.clientWidth;
      const height = container.clientHeight;
      if (width > 0 && height > 0) canvas?.resize(width, height);
    };

    syncCanvasSize();
    resizeObserver = new ResizeObserver(syncCanvasSize);
    resizeObserver.observe(container);
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
    canvas?.destroy();
  });
</script>

<div class="annotation-page">
  <PageHeader title="标注" icon="edit">
    <IconButton label="打开图片目录" icon="folder" onclick={() => onAction("暂时无法打开图片目录。")} />
    <i class="toolbar-divider" aria-hidden="true"></i>
    <IconButton label="批量浏览" icon="grid" onclick={() => onAction("当前没有图片。")} />
    <IconButton label="逐图标注" icon="edit" onclick={() => onAction("当前没有图片。")} />
    <i class="toolbar-divider" aria-hidden="true"></i>
    <IconButton label="Agent 生成初标" icon="spark" onclick={() => onAction("Agent 未连接，无法生成标注。")} />
    <IconButton label="确认标注" icon="check" onclick={() => onAction("没有可确认的标注。")} />
  </PageHeader>

  <div
    class="body"
    bind:clientWidth={bodyWidth}
    style={`grid-template-columns: ${sampleWidth}px 12px minmax(0, 1fr) 12px ${labelWidth}px;`}
  >
    <GlassPanel inset class="sample-list">
      <h3>样本列表</h3>
      <EmptyState icon="folder" title="等待样本" detail="完成预标注后进入样本队列。" />
    </GlassPanel>

    <Splitter
      value={sampleWidth}
      minimum={116}
      maximum={sampleMaximum}
      label="样本列表宽度"
      onResize={(width) => (sampleWidth = width)}
    />

    <GlassPanel inset class="canvas-panel">
      <div class="canvas-head">
        <span>标注画布</span>
        <div class="tools" role="group" aria-label="标注工具">
          {#each ["画笔", "多边形", "擦除", "缩放"] as tool}
            <button type="button" class:selected={activeTool === tool} onclick={() => (activeTool = tool)}>
              {tool}
            </button>
          {/each}
        </div>
      </div>
      <div class="canvas-container" bind:this={container}></div>
    </GlassPanel>

    <Splitter
      value={labelWidth}
      minimum={160}
      maximum={labelMaximum}
      label="标签属性宽度"
      reverse={true}
      onResize={(width) => (labelWidth = width)}
    />

    <GlassPanel inset class="label-panel">
      <h3>标签与属性</h3>
      <EmptyState icon="check" title="等待规则" detail="确认规则后显示可编辑标签。" />
    </GlassPanel>
  </div>
</div>

<style>
  .annotation-page {
    height: 100%;
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
    grid-template-columns: 160px 12px minmax(0, 1fr) 12px 220px;
  }

  .toolbar-divider {
    width: 1px;
    height: 18px;
    background: var(--vha-border);
  }

  :global(.sample-list),
  :global(.canvas-panel),
  :global(.label-panel) {
    min-width: 0;
    min-height: 0;
    padding: 16px;
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

  .canvas-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    color: var(--vha-text);
    font-size: 12.6px;
    font-weight: 500;
  }

  .tools { display: flex; gap: 4px; }

  .tools button {
    min-width: 44px;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--vha-border);
    border-radius: 8px;
    background: var(--vha-surface);
    color: var(--vha-muted);
    font-size: 10.8px;
  }

  .tools button.selected {
    background: var(--vha-selection-gradient);
    color: var(--vha-accent);
    font-weight: 600;
  }

  .canvas-container {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    border: 1px solid var(--vha-border);
    border-radius: 14px;
    background: var(--vha-canvas-gradient);
  }
</style>

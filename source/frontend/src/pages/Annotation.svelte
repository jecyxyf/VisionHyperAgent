<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { AnnotationCanvas } from "../lib/canvas/AnnotationCanvas";

  let container: HTMLElement;
  let canvas: AnnotationCanvas | null = null;

  onMount(() => {
    canvas = new AnnotationCanvas(container);
  });

  onDestroy(() => canvas?.destroy());
</script>

<div class="annotation-workspace">
  <aside class="toolbar">
    <button class="tool">画笔</button>
    <button class="tool">多边形</button>
    <button class="tool">擦除</button>
    <button class="tool">缩放</button>
  </aside>
  <div class="canvas-container" bind:this={container}></div>
</div>

<style>
  .annotation-workspace { display: flex; width: 100%; height: 100%; }
  .toolbar { width: 48px; display: flex; flex-direction: column; gap: 4px; padding: 8px; background: var(--panel); border-right: 1px solid var(--border); }
  .tool { padding: 8px; border: none; background: transparent; color: var(--text); border-radius: 6px; cursor: pointer; font-size: 12px; }
  .tool:hover { background: var(--accent-dim); }
  .canvas-container { flex: 1; position: relative; overflow: hidden; }
</style>

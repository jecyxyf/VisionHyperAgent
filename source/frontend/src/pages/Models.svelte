<script lang="ts">
  import EmptyState from "../lib/components/EmptyState.svelte";
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";

  let { onAction }: { onAction: (message: string) => void } = $props();
</script>

<div class="models-page">
  <PageHeader title="模型" icon="train" />
  <GlassPanel inset class="library">
    <EmptyState
      icon="folder"
      title="模型库为空"
      detail="完成训练后模型会自动进入这里；也可以稍后导入已有模型。"
    />
    <button type="button" onclick={() => onAction("模型导入暂不可用。")}>导入模型</button>
  </GlassPanel>
</div>

<style>
  .models-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    min-height: 0;
  }

  :global(.library) {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    padding: 18px;
  }

  button {
    min-width: 112px;
    height: 34px;
    border: 1px solid var(--vha-glass-edge);
    border-radius: 11px;
    background: var(--vha-action-gradient);
    color: white;
    box-shadow: 0 4px 13px var(--vha-action-shadow);
  }

  button:active {
    transform: translateY(1px);
  }
</style>

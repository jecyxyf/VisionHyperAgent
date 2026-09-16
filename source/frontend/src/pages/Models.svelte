<script lang="ts">
  import EmptyState from "../lib/components/EmptyState.svelte";
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import IconButton from "../lib/components/IconButton.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";

  let { onAction }: { onAction: (message: string) => void } = $props();
</script>

<div class="models-page">
  <PageHeader title="模型" icon="train">
    <IconButton label="选择模型目录" icon="folder" onclick={() => onAction("暂时无法选择模型目录。")} />
    <i class="toolbar-divider" aria-hidden="true"></i>
    <IconButton label="刷新模型列表" icon="refresh" onclick={() => onAction("暂时无法读取模型目录。")} />
  </PageHeader>

  <GlassPanel inset class="library">
    <EmptyState
      icon="folder"
      title="模型库为空"
      detail="完成训练后模型会自动进入这里；也可以稍后导入已有模型。"
    />
  </GlassPanel>
</div>

<style>
  .models-page {
    height: 100%;
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

  .toolbar-divider {
    width: 1px;
    height: 18px;
    background: var(--vha-border);
  }
</style>

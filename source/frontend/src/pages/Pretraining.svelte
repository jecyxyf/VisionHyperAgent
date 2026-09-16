<script lang="ts">
  import EmptyState from "../lib/components/EmptyState.svelte";
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";

  let { onAction }: { onAction: (message: string) => void } = $props();
</script>

<div class="pretraining-page">
  <PageHeader title="预训练" icon="train" />
  <div class="cards">
    <GlassPanel inset>
      <h3>数据源</h3>
      <EmptyState icon="folder" title="未选择数据源" detail="选择包含图像和初步标签的数据目录。" />
    </GlassPanel>
    <GlassPanel inset>
      <h3>样本统计</h3>
      <EmptyState icon="grid" title="等待数据" detail="导入后显示类别、样本数和分布。" />
    </GlassPanel>
    <GlassPanel inset>
      <h3>训练前检查</h3>
      <EmptyState icon="check" title="未开始检查" detail="系统会检查路径、类别和磁盘空间。" />
    </GlassPanel>
  </div>
  <button type="button" onclick={() => onAction("预训练暂不可用。")}>开始预训练</button>
</div>

<style>
  .pretraining-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    min-height: 0;
  }

  .cards {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }

  .cards :global(section) {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    min-width: 0;
    min-height: 0;
  }

  h3 {
    margin: 0;
    color: var(--vha-text);
    font-size: 14px;
    font-weight: 600;
  }

  button {
    align-self: flex-end;
    min-width: 116px;
    height: 34px;
    border: 1px solid var(--vha-glass-edge);
    border-radius: 11px;
    background: var(--vha-action-gradient);
    color: white;
    box-shadow: 0 4px 13px var(--vha-action-shadow);
  }
</style>

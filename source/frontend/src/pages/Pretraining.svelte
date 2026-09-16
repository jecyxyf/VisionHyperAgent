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
    height: 100%;
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
    border: 1px solid transparent;
    border-radius: 11px;
    background:
      linear-gradient(145deg, #ffffff, #f7f5ff) padding-box,
      var(--vha-spectrum-line) border-box;
    color: var(--vha-accent);
    font-weight: 600;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.95),
      0 5px 18px rgba(105, 77, 197, 0.15);
    transition: box-shadow 120ms ease, transform 80ms ease;
  }

  button:hover {
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.95),
      0 7px 22px rgba(105, 77, 197, 0.19);
  }

  button:active {
    transform: translateY(1px);
  }
</style>

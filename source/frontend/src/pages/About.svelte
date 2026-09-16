<script lang="ts">
  import EmptyState from "../lib/components/EmptyState.svelte";
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";
</script>

<div class="about-page">
  <PageHeader title="关于" icon="about" />
  <GlassPanel inset class="about-panel">
    <EmptyState icon="spark" title="VisionHyperAgent" detail="Agent 驱动的视觉模型自动训练及部署平台。" />
  </GlassPanel>
</div>

<style>
  .about-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    min-height: 0;
  }

  :global(.about-panel) {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
  }
</style>

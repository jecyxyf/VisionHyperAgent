<script lang="ts">
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";

  let { onAction }: { onAction: (message: string) => void } = $props();
</script>

<div class="settings-page">
  <PageHeader title="设置" icon="settings" />
  <GlassPanel inset class="settings-panel">
    <h3>本地设置</h3>
    <button type="button" onclick={() => onAction("设置保存暂不可用。")}>保存设置</button>
  </GlassPanel>
</div>

<style>
  .settings-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    min-height: 0;
  }

  :global(.settings-panel) {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
  }

  h3 {
    margin: 0;
    color: var(--vha-text);
    font-size: 14px;
    font-weight: 600;
  }

  button {
    align-self: flex-start;
    min-width: 104px;
    height: 34px;
    border: 1px solid var(--vha-glass-edge);
    border-radius: 11px;
    background: var(--vha-action-gradient);
    color: white;
    box-shadow: 0 4px 13px var(--vha-action-shadow);
  }
</style>

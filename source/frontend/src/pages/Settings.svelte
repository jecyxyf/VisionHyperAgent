<script lang="ts">
  import AgentSettings from "../lib/components/settings/AgentSettings.svelte";
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";

  let { onAction }: { onAction: (message: string) => void } = $props();
</script>

<div class="settings-page">
  <PageHeader title="设置" icon="settings" />
  <GlassPanel inset class="settings-panel">
    <h3>Agent 配置</h3>
    <AgentSettings onNotice={onAction} />
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
</style>

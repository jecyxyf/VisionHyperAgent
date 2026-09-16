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

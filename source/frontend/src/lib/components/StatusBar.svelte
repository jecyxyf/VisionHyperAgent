<script lang="ts">
  import IconButton from "./IconButton.svelte";

  let { message, onDismiss }: { message: string; onDismiss: () => void } = $props();
</script>

{#if message}
  <aside class="status" role="status">
    <p>{message}</p>
    <IconButton label="关闭提示" icon="close" onclick={onDismiss} />
  </aside>
{/if}

<style>
  .status {
    position: absolute;
    left: 50%;
    bottom: 16px;
    z-index: 20;
    display: flex;
    align-items: center;
    gap: 12px;
    width: min(480px, calc(100% - 32px));
    min-height: 56px;
    padding: 10px;
    border: 1px solid transparent;
    border-radius: 22px;
    background:
      linear-gradient(145deg, rgba(255, 255, 255, 0.96), rgba(248, 250, 255, 0.91)) padding-box,
      var(--vha-spectrum-line) border-box;
    backdrop-filter: blur(18px) saturate(150%);
    box-shadow:
      0 10px 32px var(--vha-shadow),
      inset 0 1px 0 rgba(255, 255, 255, 0.9);
    transform: translateX(-50%);
  }

  p {
    flex: 1;
    padding-left: 6px;
    overflow-wrap: anywhere;
    font-size: 12px;
  }
</style>

<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { IconName } from "./icon-types";

  let {
    icon,
    title,
    detail = "",
  }: {
    icon: IconName;
    title: string;
    detail?: string;
  } = $props();
</script>

<div class="empty-state">
  <div class="icon-frame">
    <Icon name={icon} size={26} />
  </div>
  <strong>{title}</strong>
  {#if detail}<p>{detail}</p>{/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    min-height: 160px;
    padding: 16px;
    color: var(--vha-muted);
    text-align: center;
  }

  .icon-frame {
    display: grid;
    place-items: center;
    width: 60px;
    height: 60px;
    border: 1px solid var(--vha-glass-edge);
    border-radius: 18px;
    background: var(--vha-selection-gradient);
    color: var(--vha-action-gradient);
  }

  strong {
    font-size: 14px;
    font-weight: 500;
  }

  p {
    max-width: 360px;
    color: var(--vha-subdued);
    font-size: 11.5px;
  }
</style>

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
    position: relative;
    overflow: hidden;
    display: grid;
    place-items: center;
    width: 60px;
    height: 60px;
    border: 1px solid transparent;
    border-radius: 18px;
    background:
      linear-gradient(145deg, #ffffff, #f6f8ff) padding-box,
      var(--vha-spectrum-line) border-box;
    color: var(--vha-accent);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.95),
      0 6px 18px rgba(42, 48, 88, 0.08);
  }

  .icon-frame::after {
    position: absolute;
    inset: -40% -10% auto;
    height: 90%;
    background: linear-gradient(
      120deg,
      rgba(124, 77, 255, 0.1),
      rgba(244, 95, 168, 0.08),
      rgba(40, 200, 216, 0.1)
    );
    content: "";
    transform: rotate(-18deg);
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

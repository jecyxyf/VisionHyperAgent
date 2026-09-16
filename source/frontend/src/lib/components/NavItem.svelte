<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { IconName } from "./icon-types";

  let {
    text,
    icon,
    selected = false,
    nested = false,
    onclick,
  }: {
    text: string;
    icon?: IconName;
    selected?: boolean;
    nested?: boolean;
    onclick?: () => void;
  } = $props();
</script>

<button type="button" class:selected class:nested onclick={onclick}>
  {#if icon}
    <Icon name={icon} size={17} />
  {:else}
    <i aria-hidden="true"></i>
  {/if}
  <span>{text}</span>
  {#if selected}<mark aria-hidden="true"></mark>{/if}
</button>

<style>
  button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 40px;
    padding: 0 12px;
    border: 1px solid transparent;
    border-radius: 12px;
    color: var(--vha-muted);
    font-size: 14px;
    font-weight: 500;
    text-align: left;
    transition: background-color 120ms ease, color 120ms ease;
  }

  button:hover {
    background:
      linear-gradient(145deg, rgba(255, 255, 255, 0.88), rgba(248, 250, 255, 0.78)) padding-box,
      var(--vha-spectrum-soft) border-box;
    color: var(--vha-text);
    box-shadow: 0 3px 13px rgba(42, 48, 88, 0.07);
  }

  button.selected {
    background:
      linear-gradient(145deg, rgba(255, 255, 255, 0.97), rgba(246, 249, 255, 0.9)) padding-box,
      var(--vha-spectrum-line) border-box;
    color: var(--vha-accent);
    font-weight: 600;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.92),
      0 4px 17px rgba(105, 77, 197, 0.13);
  }

  button.nested {
    padding-left: 35px;
  }

  i {
    width: 4px;
    height: 4px;
    flex: 0 0 auto;
    border-radius: 2px;
    background: var(--vha-border);
  }

  button.selected i {
    background: var(--vha-action-gradient);
  }

  span {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  mark {
    width: 3px;
    height: 14px;
    border-radius: 2px;
    background: var(--vha-action-gradient);
  }
</style>

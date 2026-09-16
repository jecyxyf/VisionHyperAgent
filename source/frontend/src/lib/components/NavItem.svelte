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
    background: var(--vha-elevated);
  }

  button.selected {
    border-color: var(--vha-glass-edge);
    background: var(--vha-action-gradient);
    color: white;
    font-weight: 600;
    box-shadow: 0 2px 18px var(--vha-action-shadow);
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
    background: white;
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
    background: rgba(255, 255, 255, 0.812);
  }
</style>

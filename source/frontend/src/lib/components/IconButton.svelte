<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { IconName } from "./icon-types";

  let {
    label,
    icon,
    primary = false,
    selected = false,
    onclick,
  }: {
    label: string;
    icon: IconName;
    primary?: boolean;
    selected?: boolean;
    onclick?: () => void;
  } = $props();
</script>

<button type="button" class:selected class:primary aria-label={label} title={label} onclick={onclick}>
  <Icon name={icon} size={17} />
</button>

<style>
  button {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    border: 1px solid var(--vha-glass-edge);
    border-radius: 11px;
    background: var(--vha-surface);
    color: var(--vha-muted);
    transition: transform 80ms ease, background-color 120ms ease, box-shadow 120ms ease;
  }

  button:hover {
    background: var(--vha-elevated);
  }

  button:active {
    transform: translateY(1px);
  }

  button.selected {
    background: var(--vha-selection-gradient);
    color: var(--vha-accent);
    box-shadow: 0 4px 13px var(--vha-action-shadow);
  }

  button.primary {
    background: var(--vha-action-gradient);
    color: white;
    box-shadow: 0 4px 13px var(--vha-action-shadow);
  }
</style>

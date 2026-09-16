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
    border: 1px solid transparent;
    border-radius: 11px;
    background:
      linear-gradient(145deg, rgba(255, 255, 255, 0.84), rgba(249, 250, 255, 0.66)) padding-box,
      var(--vha-spectrum-soft) border-box;
    color: var(--vha-muted);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.75);
    transition: transform 80ms ease, background 120ms ease, box-shadow 120ms ease, color 120ms ease;
  }

  button:hover {
    background:
      linear-gradient(145deg, #ffffff, #f7fbff) padding-box,
      var(--vha-spectrum-line) border-box;
    color: var(--vha-accent);
    box-shadow: 0 4px 15px rgba(42, 48, 88, 0.09);
  }

  button:active {
    transform: translateY(1px);
  }

  button.selected {
    background:
      linear-gradient(145deg, #ffffff, #f3f8ff) padding-box,
      var(--vha-spectrum-line) border-box;
    color: var(--vha-accent);
    box-shadow:
      inset 0 0 16px rgba(40, 200, 216, 0.09),
      0 4px 16px rgba(105, 77, 197, 0.14);
  }

  button.primary {
    background:
      linear-gradient(145deg, #ffffff, #f7f5ff) padding-box,
      var(--vha-spectrum-line) border-box;
    color: var(--vha-accent);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.95),
      0 5px 18px rgba(105, 77, 197, 0.16);
  }
</style>

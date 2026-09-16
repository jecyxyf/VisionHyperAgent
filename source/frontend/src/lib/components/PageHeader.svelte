<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import type { IconName } from "./icon-types";

  let {
    title,
    icon,
    children,
  }: {
    title: string;
    icon: IconName;
    children?: Snippet;
  } = $props();
</script>

<header>
  <div class="title-row">
    <Icon name={icon} size={18} />
    <h2>{title}</h2>
    <div class="actions">
      {@render children?.()}
    </div>
  </div>
  <div class="separator"></div>
</header>

<style>
  header {
    flex: 0 0 51px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    color: var(--vha-accent);
  }

  h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
    overflow: hidden;
    color: var(--vha-text);
    font-size: 15.4px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .separator {
    height: 1px;
    background: var(--vha-border);
  }
</style>

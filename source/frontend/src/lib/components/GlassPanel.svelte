<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    inset = false,
    class: className = "",
    children,
  }: {
    inset?: boolean;
    class?: string;
    children?: Snippet;
  } = $props();
</script>

<section class={`${inset ? "inset" : "glass"} ${className}`.trim()}>
  {@render children?.()}
</section>

<style>
  section {
    position: relative;
    min-width: 0;
    min-height: 0;
    border-radius: 22px;
  }

  .glass {
    border: 1px solid var(--vha-glass-edge);
    background: var(--vha-glass-gradient);
    box-shadow: 0 8px 30px var(--vha-shadow);
  }

  .glass::before,
  .glass::after {
    content: "";
    position: absolute;
    right: 18px;
    left: 18px;
    height: 1px;
    pointer-events: none;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.93) 36%,
      rgba(215, 239, 255, 0.61) 72%,
      transparent
    );
  }

  .glass::before { top: 1px; }
  .glass::after { bottom: 2px; }

  .inset {
    border: 1px solid var(--vha-border);
    background: var(--vha-surface);
  }
</style>

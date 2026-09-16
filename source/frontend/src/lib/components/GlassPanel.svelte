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
    border: 1px solid transparent;
    border-radius: 22px;
  }

  .glass {
    background:
      radial-gradient(circle at 14% 9%, rgba(124, 77, 255, 0.055), transparent 27%) padding-box,
      radial-gradient(circle at 87% 12%, rgba(244, 95, 168, 0.04), transparent 29%) padding-box,
      radial-gradient(circle at 8% 93%, rgba(40, 200, 216, 0.045), transparent 27%) padding-box,
      linear-gradient(
        145deg,
        rgba(255, 255, 255, 0.85) 0%,
        rgba(255, 255, 255, 0.7) 54%,
        rgba(255, 255, 255, 0.8) 100%
      ) padding-box,
      var(--vha-spectrum-line) border-box;
    backdrop-filter: blur(18px) saturate(150%);
    box-shadow:
      0 10px 34px var(--vha-shadow),
      inset 0 1px 0 rgba(255, 255, 255, 0.82);
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
      rgba(255, 255, 255, 0.9) 27%,
      rgba(239, 229, 255, 0.76) 48%,
      rgba(226, 248, 255, 0.67) 68%,
      rgba(224, 255, 244, 0.5) 82%,
      transparent
    );
  }

  .glass::before { top: 1px; }
  .glass::after { bottom: 2px; }

  .inset {
    background:
      radial-gradient(circle at 13% 8%, rgba(124, 77, 255, 0.04), transparent 30%) padding-box,
      linear-gradient(
        145deg,
        rgba(255, 255, 255, 0.75) 0%,
        rgba(255, 255, 255, 0.6) 100%
      ) padding-box,
      linear-gradient(
        118deg,
        rgba(124, 77, 255, 0.24),
        rgba(244, 95, 168, 0.19) 34%,
        rgba(40, 200, 216, 0.21) 70%,
        rgba(255, 182, 92, 0.17)
      ) border-box;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.72);
  }
</style>

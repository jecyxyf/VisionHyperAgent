<script lang="ts">
  import type { IconName } from "./icon-types";

  let { name, size = 18 }: { name: IconName; size?: number } = $props();

  const paths: Record<IconName, string[]> = {
    about: ["M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z", "M12 16v-4", "M12 8h.01"],
    camera: ["m23 7-7 5 7 5V7Z", "M1 5h15a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H1Z"],
    check: ["m4 12 5 5L20 6"],
    chevron: ["m6 9 6 6 6-6"],
    edit: ["M12 20h9", "M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"],
    folder: ["M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7l-2-3H4a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2Z"],
    grid: ["M4 4h7v7H4Z", "M13 4h7v7h-7Z", "M4 13h7v7H4Z", "M13 13h7v7h-7Z"],
    next: ["m9 6 6 6-6 6"],
    play: ["m6 4 14 8-14 8Z"],
    scan: ["M3 7V5a2 2 0 0 1 2-2h2", "M17 3h2a2 2 0 0 1 2 2v2", "M21 17v2a2 2 0 0 1-2 2h-2", "M7 21H5a2 2 0 0 1-2-2v-2", "M7 12h10"],
    send: ["M22 2 11 13", "m22 2-7 20-4-9-9-4Z"],
    settings: [
      "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
      "M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1-1.55 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.09a1.7 1.7 0 0 0 1.55-1 1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34h.01a1.7 1.7 0 0 0 1-1.55V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1 1.55h.01a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87v.01a1.7 1.7 0 0 0 1.55 1H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.55 1Z",
    ],
    spark: ["M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9Z", "m19 17 .9 2.1L22 20l-2.1.9L19 23l-.9-2.1L16 20l2.1-.9Z"],
    stop: ["M6 6h12v12H6Z"],
    train: ["M4 8h16l-2 12H6Z", "M8 8V5h8v3"],
    trash: ["M4 7h16", "M9 7V5a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2", "m6 7 1 13a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-13"],
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each paths[name] as path}
    <path {path} />
  {/each}
</svg>

<script lang="ts">
  import type { IconName } from "./icon-types";

  let { name, size = 18 }: { name: IconName; size?: number } = $props();

  const paths: Record<IconName, string[]> = {
    about: ["M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Z", "M12 11v6", "M12 7v.1"],
    camera: [
      "M3 6h18a3 3 0 0 1 3 3v8a3 3 0 0 1-3 3H3a3 3 0 0 1-3-3V9a3 3 0 0 1 3-3Z",
      "m8 6 2-3h4l2 3",
      "M12 17a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z",
    ],
    check: ["m5 12 4 4L20 5"],
    chevron: ["m8 10 4 4 4-4"],
    close: ["m6 6 12 12", "M6 18 18 6"],
    edit: [
      "m4 15 11-11a2 2 0 0 1 3 0l2 2a2 2 0 0 1 0 3L9 20l-6 1Z",
      "m13 6 5 5",
      "m4 15 5 5",
    ],
    folder: ["M3 7V5a1 1 0 0 1 1-1h5l2 3h9a1 1 0 0 1 1 1v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"],
    grid: [
      "M4.5 3h6A1.5 1.5 0 0 1 12 4.5v6A1.5 1.5 0 0 1 10.5 12h-6A1.5 1.5 0 0 1 3 10.5v-6A1.5 1.5 0 0 1 4.5 3Z",
      "M15.5 3h6A1.5 1.5 0 0 1 23 4.5v6a1.5 1.5 0 0 1-1.5 1.5h-6A1.5 1.5 0 0 1 14 10.5v-6A1.5 1.5 0 0 1 15.5 3Z",
      "M4.5 14h6a1.5 1.5 0 0 1 1.5 1.5v6a1.5 1.5 0 0 1-1.5 1.5h-6A1.5 1.5 0 0 1 3 21.5v-6A1.5 1.5 0 0 1 4.5 14Z",
      "M15.5 14h6a1.5 1.5 0 0 1 1.5 1.5v6a1.5 1.5 0 0 1-1.5 1.5h-6a1.5 1.5 0 0 1-1.5-1.5v-6a1.5 1.5 0 0 1 1.5-1.5Z",
    ],
    network: ["M9 3h6v5H9z", "M2 16h6v5H2z", "M16 16h6v5h-6z", "M12 8v4", "M5 16v-4h14v4"],
    next: ["m10 6 6 6-6 6"],
    play: ["m9 5 11 7-11 7z"],
    previous: ["m14 6-6 6 6 6"],
    refresh: ["M20 7v5h-5", "M4 17v-5h5", "M5.2 7a8 8 0 0 1 13-1L20 9", "M4 15l1.8 3a8 8 0 0 0 13-1"],
    scan: [
      "M8 3H4a1 1 0 0 0-1 1v4",
      "M16 3h4a1 1 0 0 1 1 1v4",
      "M3 16v4a1 1 0 0 0 1 1h4",
      "M16 21h4a1 1 0 0 0 1-1v-4",
      "M8 8h8a2 2 0 0 1 2 2v8H8a2 2 0 0 1-2-2V8Z",
    ],
    send: ["m4 11 16-7-7 16-2-7Z", "m11 13 9-9"],
    settings: [
      "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
      "m9 3 1-1h4l1 3 3 1 3 3-1 3 1 3-3 3-3 1-1 3h-4l-1-3-3-1-3-3 1-3-1-3 3-3 3-1Z",
    ],
    spark: ["m12 2 2.5 7.5L22 12l-7.5 2.5L12 22l-2.5-7.5L2 12l7.5-2.5Z"],
    stop: ["M8 6h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2Z"],
    train: ["m12 3 9 5-9 5-9-5 9-5Z", "m3 12 9 5 9-5", "m3 17 9 5 9-5"],
    trash: [
      "M3 6h18",
      "M9 6V3h6v3",
      "M5 6l1 14a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1l1-14",
      "M10 10v7",
      "M14 10v7",
    ],
  };

  const iconStrokeWidths: Partial<Record<IconName, number>> = {
    scan: 1.5,
    settings: 1.5,
    spark: 1.5,
    train: 1.5,
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={iconStrokeWidths[name] ?? 1.6}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each paths[name] as path}
    <path d={path} />
  {/each}
</svg>

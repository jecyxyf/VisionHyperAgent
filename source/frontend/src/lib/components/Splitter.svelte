<script lang="ts">
  let {
    value,
    minimum,
    maximum,
    label,
    orientation = "vertical",
    reverse = false,
    onResize,
  }: {
    value: number;
    minimum: number;
    maximum: number;
    label: string;
    orientation?: "vertical" | "horizontal";
    reverse?: boolean;
    onResize: (value: number) => void;
  } = $props();

  let handle = $state<HTMLDivElement | null>(null);
  let dragging = $state(false);
  let pointerStart = 0;
  let valueStart = $state(0);

  function clamp(nextValue: number) {
    return Math.min(Math.max(minimum, nextValue), Math.max(minimum, maximum));
  }

  function moveBy(delta: number) {
    onResize(clamp(value + (reverse ? -delta : delta)));
  }

  function onKeyDown(event: KeyboardEvent) {
    const step = event.shiftKey ? 1 : 16;
    const previousKey = orientation === "vertical" ? "ArrowLeft" : "ArrowUp";
    const nextKey = orientation === "vertical" ? "ArrowRight" : "ArrowDown";

    if (event.key === previousKey) {
      event.preventDefault();
      moveBy(-step);
    } else if (event.key === nextKey) {
      event.preventDefault();
      moveBy(step);
    } else if (event.key === "Home") {
      event.preventDefault();
      onResize(clamp(minimum));
    } else if (event.key === "End") {
      event.preventDefault();
      onResize(clamp(maximum));
    }
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;

    event.preventDefault();
    handle?.setPointerCapture(event.pointerId);
    pointerStart = orientation === "vertical" ? event.clientX : event.clientY;
    valueStart = value;
    dragging = true;
  }

  function onPointerMove(event: PointerEvent) {
    if (!dragging) return;
    const pointer = orientation === "vertical" ? event.clientX : event.clientY;
    const delta = pointer - pointerStart;
    onResize(clamp(valueStart + (reverse ? -delta : delta)));
  }

  function stopDragging(event: PointerEvent) {
    if (!dragging) return;
    handle?.releasePointerCapture(event.pointerId);
    dragging = false;
  }
</script>

<div
  bind:this={handle}
  class="splitter"
  class:horizontal={orientation === "horizontal"}
  class:dragging
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-orientation={orientation}
  aria-valuemin={minimum}
  aria-valuemax={maximum}
  aria-valuenow={value}
  aria-valuetext={`${Math.round(value)}px`}
  onkeydown={onKeyDown}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={stopDragging}
  onpointercancel={stopDragging}
></div>

<style>
  .splitter {
    display: grid;
    place-items: center;
    touch-action: none;
    cursor: var(--splitter-cursor);
  }

  .splitter::before {
    width: var(--splitter-handle-width);
    height: var(--splitter-handle-height);
    border-radius: 2px;
    background: var(--vha-spectrum-line);
    content: "";
    box-shadow: 0 0 8px rgba(105, 77, 197, 0.11);
    transition: background 120ms ease, box-shadow 120ms ease;
  }

  .splitter:hover::before {
    background: var(--vha-action-gradient);
    box-shadow: 0 0 11px rgba(105, 77, 197, 0.2);
  }

  .splitter:not(.horizontal) {
    --splitter-cursor: ew-resize;
    --splitter-handle-width: 3px;
    --splitter-handle-height: 48px;
  }

  .splitter.horizontal {
    --splitter-cursor: ns-resize;
    --splitter-handle-width: 48px;
    --splitter-handle-height: 3px;
  }

  .splitter:not(.horizontal):focus-visible::before,
  .splitter:not(.horizontal).dragging::before {
    --splitter-handle-height: 64px;
  }

  .splitter.horizontal:focus-visible::before,
  .splitter.horizontal.dragging::before {
    --splitter-handle-width: 64px;
  }

  .splitter:focus-visible {
    outline: none;
  }

  .splitter:focus-visible::before,
  .splitter.dragging::before {
    background: var(--vha-action-gradient);
    box-shadow: 0 0 14px rgba(105, 77, 197, 0.24);
  }
</style>

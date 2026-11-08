<script lang="ts">
  import type { Snippet } from "svelte";
  import { startDrag } from "../../layout/drag.ts";
  import { clampSize, keyStep } from "../../layout/size.ts";
  import { arrange } from "../../stores/arrange.svelte";

  /**
   * Two panes with a handle between them. One pane has a size the person can change (`side`), the other takes the rest.
   * The size is kept by `id`, a double click on the handle puts it back, and the arrow keys move it.
   */
  let {
    id,
    axis = "x",
    initial,
    min = 160,
    max = 900,
    minOther = 240,
    side = "first",
    first,
    second,
    class: cls = "",
  }: {
    id: string;
    axis?: "x" | "y";
    initial: number;
    min?: number;
    max?: number;
    /** The room the other pane always keeps. */
    minOther?: number;
    /** Which pane has the adjustable size. */
    side?: "first" | "second";
    first: Snippet;
    second: Snippet;
    class?: string;
  } = $props();

  let box = $state<HTMLElement | null>(null);
  let available = $state(0);
  let dragging = $state(false);

  const horizontal = $derived(axis === "x");
  const size = $derived(clampSize(arrange.size(id) ?? initial, min, max, available, minOther));

  $effect(() => {
    if (!box) return;
    const watch = new ResizeObserver(() => (available = horizontal ? (box?.clientWidth ?? 0) : (box?.clientHeight ?? 0)));
    watch.observe(box);
    return () => watch.disconnect();
  });

  function set(value: number) {
    arrange.setSize(id, clampSize(value, min, max, available, minOther));
  }

  function down(e: PointerEvent) {
    dragging = true;
    startDrag(e, axis, size, side === "first" ? 1 : -1, set, () => (dragging = false));
  }

  function key(e: KeyboardEvent) {
    const step = keyStep(e.key, e.shiftKey, axis);
    if (step === null) return;
    e.preventDefault();
    set(size + (side === "first" ? step : -step));
  }

  const style = $derived(`${horizontal ? "width" : "height"}:${size}px`);
</script>

<div bind:this={box} class="flex h-full min-h-0 w-full min-w-0 {horizontal ? 'flex-row' : 'flex-col'} {cls}">
  {#if side === "first"}
    <div class="relative min-h-0 min-w-0 shrink-0 overflow-hidden" {style}>{@render first()}</div>
  {:else}
    <div class="relative min-h-0 min-w-0 flex-1 overflow-hidden">{@render first()}</div>
  {/if}
  <!-- A separator that can be focused and moved is an interactive widget: the ARIA window splitter pattern. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    role="separator"
    aria-orientation={horizontal ? "vertical" : "horizontal"}
    aria-valuenow={size}
    aria-valuemin={min}
    aria-valuemax={max}
    aria-label="Resize"
    tabindex="0"
    title="Drag to resize. Double-click to reset."
    onpointerdown={down}
    onkeydown={key}
    ondblclick={() => arrange.reset(id)}
    class="group relative z-10 shrink-0 outline-none {horizontal ? 'w-px cursor-col-resize' : 'h-px cursor-row-resize'} bg-line {dragging ? 'bg-accent' : ''} hover:bg-accent focus-visible:bg-accent"
  >
    <!-- A wider hit area than the line itself. -->
    <span class="absolute {horizontal ? '-inset-x-[3px] inset-y-0' : '-inset-y-[3px] inset-x-0'}"></span>
  </div>
  {#if side === "second"}
    <div class="relative min-h-0 min-w-0 shrink-0 overflow-hidden" {style}>{@render second()}</div>
  {:else}
    <div class="relative min-h-0 min-w-0 flex-1 overflow-hidden">{@render second()}</div>
  {/if}
</div>

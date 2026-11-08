<script lang="ts">
  import type { Snippet } from "svelte";
  import { startDrag } from "../../layout/drag.ts";
  import { clampSize, keyStep } from "../../layout/size.ts";
  import { arrange } from "../../stores/arrange.svelte";

  /**
   * A page with a list on one side and its detail on the other, where the whole page scrolls. The first column has a width
   * the person can change; below the `lg` width the columns stack. Like [Split], it keeps the size by `id`.
   */
  let {
    id,
    initial,
    min = 160,
    max = 640,
    first,
    second,
    class: cls = "",
  }: { id: string; initial: number; min?: number; max?: number; first: Snippet; second: Snippet; class?: string } = $props();

  let box = $state<HTMLElement | null>(null);
  let available = $state(0);
  let dragging = $state(false);

  const size = $derived(clampSize(arrange.size(id) ?? initial, min, max, available, 320));

  $effect(() => {
    if (!box) return;
    const watch = new ResizeObserver(() => (available = box?.clientWidth ?? 0));
    watch.observe(box);
    return () => watch.disconnect();
  });

  const set = (value: number) => arrange.setSize(id, clampSize(value, min, max, available, 320));
  const down = (e: PointerEvent) => {
    dragging = true;
    startDrag(e, "x", size, 1, set, () => (dragging = false));
  };
  const key = (e: KeyboardEvent) => {
    const step = keyStep(e.key, e.shiftKey, "x");
    if (step === null) return;
    e.preventDefault();
    set(size + step);
  };
</script>

<div bind:this={box} class="grid grid-cols-1 items-start gap-5 lg:gap-0 lg:[grid-template-columns:var(--cols)] {cls}" style="--cols:{size}px 20px minmax(0,1fr)">
  <div class="min-w-0">{@render first()}</div>
  <!-- A separator that can be focused and moved is an interactive widget: the ARIA window splitter pattern. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    role="separator"
    aria-orientation="vertical"
    aria-valuenow={size}
    aria-valuemin={min}
    aria-valuemax={max}
    aria-label="Resize"
    tabindex="0"
    title="Drag to resize. Double-click to reset."
    onpointerdown={down}
    onkeydown={key}
    ondblclick={() => arrange.reset(id)}
    class="group relative hidden cursor-col-resize outline-none lg:block"
  >
    <span class="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-transparent group-hover:bg-accent group-focus-visible:bg-accent {dragging ? 'bg-accent' : ''}"></span>
  </div>
  <div class="min-w-0">{@render second()}</div>
</div>

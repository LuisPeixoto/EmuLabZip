<script lang="ts">
  import { arrange } from "../../stores/arrange.svelte";
  import { app } from "../../stores/app.svelte";

  const widths = [
    { id: "narrow", label: "Narrow", hint: "Easiest to scan on a big screen." },
    { id: "wide", label: "Wide", hint: "The usual width." },
    { id: "full", label: "Full", hint: "Use every pixel." },
  ] as const;
</script>

<div class="space-y-5 p-5">
  <section>
    <h4 class="text-md font-medium">Page width</h4>
    <p class="mb-2 text-sm text-muted">How wide the centred pages are: Overview, Performance, Controls, Frameworks, Compare, Devices and Audit.</p>
    <div class="grid grid-cols-3 gap-2">
      {#each widths as w (w.id)}
        <button
          onclick={() => arrange.setContentWidth(w.id)}
          class="rounded-xl border p-3 text-left {arrange.contentWidth === w.id ? 'border-accent bg-accent-soft' : 'border-line hover:bg-hover'}"
        >
          <span class="text-md font-medium">{w.label}</span>
          <span class="mt-0.5 block text-sm text-muted">{w.hint}</span>
        </button>
      {/each}
    </div>
  </section>

  <section class="space-y-2">
    <h4 class="text-md font-medium">Panes</h4>
    <p class="text-sm text-muted">Drag the line between two panes to resize them, double-click it to restore its size, or focus it and use the arrow keys. Sizes are remembered.</p>
    <div class="flex gap-2">
      <button
        onclick={() => (arrange.reset(), app.toast("Pane sizes restored", "ok"))}
        class="h-8 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover">Reset all pane sizes</button>
    </div>
  </section>

  <section class="space-y-2">
    <h4 class="text-md font-medium">Sidebar</h4>
    <p class="text-sm text-muted">Collapse it with ⌘\ or the button at its foot. Use Customize there to reorder or hide entries.</p>
    <div class="flex gap-2">
      <button onclick={() => arrange.toggleSidebar()} class="h-8 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover">
        {arrange.sidebar.collapsed ? "Expand" : "Collapse"} the sidebar
      </button>
      <button onclick={() => (arrange.resetSidebar(), app.toast("Sidebar restored", "ok"))} class="h-8 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover">Restore its order and entries</button>
    </div>
  </section>
</div>

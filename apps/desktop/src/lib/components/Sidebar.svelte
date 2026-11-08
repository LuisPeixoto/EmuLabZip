<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { app, type View } from "../stores/app.svelte";
  import { arrange } from "../stores/arrange.svelte";
  import { crashes } from "../stores/crashes.svelte";
  import { logs } from "../stores/logs.svelte";
  import { compact } from "../format";
  import Icon from "./Icon.svelte";

  interface Item {
    view: View;
    icon: string;
    label: string;
  }

  const sections: { id: string; title: string; items: Item[] }[] = [
    {
      id: "inspect",
      title: "Inspect",
      items: [
        { view: "overview", icon: "info", label: "Overview" },
        { view: "layout", icon: "layers", label: "Layout" },
        { view: "frameworks", icon: "code", label: "Frameworks" },
        { view: "logs", icon: "terminal", label: "Logs" },
        { view: "crashes", icon: "alert", label: "Crashes" },
      ],
    },
    {
      id: "profile",
      title: "Profile",
      items: [
        { view: "performance", icon: "activity", label: "Performance" },
        { view: "memory", icon: "cpu", label: "Memory" },
        { view: "storage", icon: "database", label: "Storage" },
        { view: "network", icon: "globe", label: "Network" },
        { view: "traces", icon: "route", label: "Traces" },
      ],
    },
    {
      id: "sessions",
      title: "Sessions",
      items: [
        { view: "sessions", icon: "history", label: "Sessions" },
        { view: "compare", icon: "columns", label: "Compare" },
      ],
    },
    {
      id: "quality",
      title: "Quality",
      items: [
        { view: "audit", icon: "shield", label: "Audit" },
        { view: "size", icon: "download", label: "Build size" },
        { view: "visual", icon: "eye", label: "Visual" },
        { view: "analytics", icon: "activity", label: "Analytics" },
      ],
    },
    {
      id: "control",
      title: "Control",
      items: [
        { view: "devices", icon: "smartphone", label: "Devices" },
        { view: "controls", icon: "sliders", label: "Controls" },
        { view: "plugins", icon: "zap", label: "Plugins" },
      ],
    },
  ];

  const collapsed = $derived(arrange.sidebar.collapsed);
  /** Customizing: entries can be dragged into another order, and hidden or shown. */
  let customizing = $state(false);
  let dragging = $state<{ section: string; from: number } | null>(null);
  let over = $state<number | null>(null);

  const ids = (section: { items: Item[] }) => section.items.map((i) => i.view as string);

  /** A section's entries in the chosen order. Hidden ones are left out unless the person is customizing. */
  function shown(section: { id: string; items: Item[] }): { item: Item; index: number }[] {
    const byId = new Map(section.items.map((i) => [i.view as string, i]));
    return arrange
      .ordered(section.id, ids(section))
      .map((id, index) => ({ item: byId.get(id)!, index }))
      .filter(({ item }) => customizing || !arrange.isHidden(item.view));
  }

  function drop(section: { id: string; items: Item[] }, to: number) {
    if (dragging && dragging.section === section.id) arrange.moveEntry(section.id, ids(section), dragging.from, to);
    dragging = null;
    over = null;
  }
</script>

<aside class="flex h-full w-full flex-col bg-panel-2">
  <nav class="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
    {#each sections as section (section.id)}
      {@const list = shown(section)}
      {#if list.length}
        {#if collapsed}
          <div class="mx-2 mt-3 mb-1 border-t border-line first:mt-1"></div>
        {:else}
          <p class="px-2.5 pt-4 pb-1.5 text-2xs font-semibold tracking-wider text-faint uppercase">{section.title}</p>
        {/if}
        {#each list as { item, index } (item.view)}
          {@const active = app.view === item.view}
          {@const hidden = arrange.isHidden(item.view)}
          <div
            role="presentation"
            draggable={customizing}
            ondragstart={() => (dragging = { section: section.id, from: index })}
            ondragover={(e) => {
              if (customizing && dragging?.section === section.id) {
                e.preventDefault();
                over = index;
              }
            }}
            ondrop={() => drop(section, index)}
            ondragend={() => ((dragging = null), (over = null))}
            class="flex items-center {over === index && dragging ? 'rounded-lg ring-1 ring-accent' : ''}"
          >
            {#if customizing && !collapsed}
              <span class="grid size-5 shrink-0 cursor-grab place-items-center text-faint" title="Drag to reorder"><Icon name="list" size={12} /></span>
            {/if}
            <button
              onclick={() => (customizing ? arrange.setHidden(item.view, !hidden) : app.setView(item.view))}
              title={collapsed ? item.label : customizing ? (hidden ? "Show" : "Hide") : ""}
              class="flex h-8 min-w-0 flex-1 items-center rounded-lg text-md {collapsed ? 'justify-center px-0' : 'gap-2.5 px-2.5'} {active && !customizing ? 'bg-accent-soft font-medium text-accent' : 'text-fg/85 hover:bg-hover'} {customizing && hidden ? 'opacity-45' : ''}"
            >
              <Icon name={item.icon} size={15} />
              {#if !collapsed}
                <span class="flex-1 text-left">{item.label}</span>
                {#if customizing}
                  <Icon name={hidden ? "eye" : "check"} size={12} class={hidden ? "text-faint" : "text-ok"} />
                {:else if item.view === "crashes" && crashes.unseen}
                  <span class="mono rounded-full bg-err px-1.5 text-tiny font-semibold text-white">{crashes.unseen}</span>
                {:else if item.view === "logs" && logs.entries.length}
                  <span class="mono text-2xs {active ? 'text-accent' : 'text-faint'}">{compact(logs.entries.length)}</span>
                {/if}
              {:else if item.view === "crashes" && crashes.unseen}
                <span class="absolute ml-4 -mt-3 size-2 rounded-full bg-err"></span>
              {/if}
            </button>
          </div>
        {/each}
      {/if}
    {/each}
  </nav>

  <div class="flex items-center gap-1 border-t border-line px-2 py-1.5 {collapsed ? 'flex-col' : ''}">
    <button
      onclick={() => arrange.toggleSidebar()}
      title={collapsed ? "Expand the sidebar (⌘\\)" : "Collapse the sidebar (⌘\\)"}
      class="grid size-7 shrink-0 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg"
    >
      <Icon name="sidebar" size={15} />
    </button>
    {#if !collapsed}
      <button
        onclick={() => (customizing = !customizing)}
        class="flex h-7 items-center gap-1.5 rounded-lg px-2 text-sm {customizing ? 'bg-accent-soft font-medium text-accent' : 'text-muted hover:bg-hover hover:text-fg'}"
      >
        <Icon name="sliders" size={13} />{customizing ? "Done" : "Customize"}
      </button>
      {#if customizing}
        <button onclick={() => arrange.resetSidebar()} class="h-7 rounded-lg px-2 text-sm text-muted hover:bg-hover hover:text-fg">Reset</button>
      {/if}
    {/if}
  </div>

  <div class="flex items-center gap-2.5 border-t border-line {collapsed ? 'justify-center px-2 py-3' : 'px-3 py-3'}">
    <span class="brand-gradient grid size-7 shrink-0 place-items-center rounded-lg text-white shadow-[0_6px_16px_-6px_#5b5bf0]">
      <Icon name="activity" size={15} stroke={2.2} />
    </span>
    {#if !collapsed}
      <span class="min-w-0 flex-1 leading-tight">
        <span class="block text-md font-semibold">EmuLabZip</span>
        <span class="mono block text-2xs text-faint">v{app.host?.version ?? "dev"}</span>
      </span>
      <button
        title="EmuLabZip on GitHub"
        onclick={() => openUrl("https://github.com/LuisPeixoto/EmuLabZip")}
        class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg"
      >
        <Icon name="github" size={15} />
      </button>
    {/if}
  </div>
</aside>

<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { app, type View } from "../stores/app.svelte";
  import { logs } from "../stores/logs.svelte";
  import { compact } from "../format";
  import Icon from "./Icon.svelte";

  interface Item {
    view: View;
    icon: string;
    label: string;
  }

  const inspect: Item[] = [
    { view: "overview", icon: "info", label: "Overview" },
    { view: "logs", icon: "terminal", label: "Logs" },
  ];
  const profile: Item[] = [
    { view: "performance", icon: "activity", label: "Performance" },
    { view: "memory", icon: "cpu", label: "Memory" },
    { view: "storage", icon: "database", label: "Storage" },
    { view: "network", icon: "globe", label: "Network" },
    { view: "traces", icon: "route", label: "Traces" },
  ];
</script>

{#snippet section(title: string, items: Item[])}
  <p class="px-2.5 pt-4 pb-1.5 text-[10.5px] font-semibold tracking-wider text-faint uppercase">{title}</p>
  {#each items as item (item.view)}
    {@const active = app.view === item.view}
    <button
      onclick={() => app.setView(item.view)}
      class="flex h-8 w-full items-center gap-2.5 rounded-lg px-2.5 text-[12.5px] {active ? 'bg-accent-soft font-medium text-accent' : 'text-fg/85 hover:bg-hover'}"
    >
      <Icon name={item.icon} size={15} />
      <span class="flex-1 text-left">{item.label}</span>
      {#if item.view === "logs" && logs.entries.length}
        <span class="mono text-[10.5px] {active ? 'text-accent' : 'text-faint'}">{compact(logs.entries.length)}</span>
      {/if}
    </button>
  {/each}
{/snippet}

<aside class="flex w-[220px] shrink-0 flex-col border-r border-line bg-panel-2">
  <nav class="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
    {@render section("Inspect", inspect)}
    {@render section("Profile", profile)}
  </nav>
  <div class="flex items-center gap-2.5 border-t border-line px-3 py-3">
    <span class="brand-gradient grid size-7 place-items-center rounded-lg text-white shadow-[0_6px_16px_-6px_#5b5bf0]">
      <Icon name="activity" size={15} stroke={2.2} />
    </span>
    <span class="min-w-0 flex-1 leading-tight">
      <span class="block text-[12.5px] font-semibold">EmuLabZip</span>
      <span class="mono block text-[10.5px] text-faint">v{app.host?.version ?? "dev"}</span>
    </span>
    <button
      title="EmuLabZip on GitHub"
      onclick={() => openUrl("https://github.com/LuisPeixoto/EmuLabZip")}
      class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg"
    >
      <Icon name="github" size={15} />
    </button>
  </div>
</aside>

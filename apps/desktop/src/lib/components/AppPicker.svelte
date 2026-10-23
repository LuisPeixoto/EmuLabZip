<script lang="ts">
  import { fly } from "svelte/transition";
  import { clickOutside } from "../actions";
  import { app } from "../stores/app.svelte";
  import { hue } from "../format";
  import Icon from "./Icon.svelte";

  let open = $state(false);
  let query = $state("");
  let showSystem = $state(false);
  let input: HTMLInputElement | undefined = $state();

  const list = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return app.apps
      .filter((a) => showSystem || a.user || a.pid != null)
      .filter((a) => !q || a.id.toLowerCase().includes(q) || a.name.toLowerCase().includes(q))
      .sort((a, b) => Number(b.pid != null) - Number(a.pid != null) || Number(b.user) - Number(a.user) || a.name.localeCompare(b.name));
  });

  async function toggle() {
    open = !open;
    if (open) {
      query = "";
      app.refreshApps();
      queueMicrotask(() => input?.focus());
    }
  }

  function pick(id: string | null) {
    app.selectApp(id);
    open = false;
  }

  function initial(name: string) {
    return (name.split(".").pop() ?? name).charAt(0).toUpperCase();
  }
</script>

<div class="relative" use:clickOutside={() => (open = false)}>
  <button
    onclick={toggle}
    disabled={!app.online}
    title="Focus on one app"
    class="flex h-9 max-w-[260px] min-w-[180px] items-center gap-2 rounded-lg border px-2 text-left transition disabled:opacity-50 {open
      ? 'border-accent bg-panel ring-3 ring-accent-soft'
      : 'border-line bg-panel-2 hover:border-line-strong'}"
  >
    {#if app.selectedApp}
      <span
        class="grid size-6 shrink-0 place-items-center rounded-md text-[11px] font-bold text-white"
        style="background: linear-gradient(135deg, hsl({hue(app.selectedApp.id)} 70% 58%), hsl({(hue(app.selectedApp.id) + 40) % 360} 70% 46%))"
      >{initial(app.selectedApp.name)}</span>
      <span class="min-w-0 flex-1 leading-tight">
        <span class="block truncate text-[12.5px] font-semibold">{app.selectedApp.name}</span>
        <span class="mono block truncate text-[10.5px] text-muted">
          {app.selectedApp.pid != null ? `pid ${app.selectedApp.pid}` : "not running"}
        </span>
      </span>
    {:else}
      <span class="grid size-6 place-items-center rounded-md bg-hover text-muted"><Icon name="layers" size={14} /></span>
      <span class="flex-1 text-[12.5px] text-muted">All processes</span>
    {/if}
    <Icon name="chevronDown" size={14} class="shrink-0 text-faint" />
  </button>

  {#if open}
    <div
      transition:fly={{ y: -6, duration: 140 }}
      class="shadow-pop absolute top-11 left-0 z-50 w-[360px] overflow-hidden rounded-xl border border-line bg-elevated"
    >
      <div class="border-b border-line p-2">
        <label class="relative flex items-center">
          <Icon name="search" size={13} class="pointer-events-none absolute left-2.5 text-faint" />
          <input
            bind:this={input}
            bind:value={query}
            spellcheck="false"
            placeholder="Find an app by name or id…"
            class="h-8 w-full rounded-lg border border-line bg-panel-2 pl-8 text-[12.5px] outline-none placeholder:text-faint focus:border-accent"
          />
        </label>
      </div>
      <div class="max-h-[380px] overflow-y-auto p-1.5">
        <button onclick={() => pick(null)} class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left {app.appId == null ? 'bg-accent-soft text-accent' : 'hover:bg-hover'}">
          <span class="grid size-6 place-items-center rounded-md bg-hover text-muted"><Icon name="layers" size={14} /></span>
          <span class="flex-1 text-[12.5px] font-medium">All processes</span>
        </button>
        {#each list as a (a.id)}
          <button
            onclick={() => pick(a.id)}
            class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left {a.id === app.appId ? 'bg-accent-soft' : 'hover:bg-hover'}"
          >
            <span
              class="grid size-6 shrink-0 place-items-center rounded-md text-[11px] font-bold text-white"
              style="background: linear-gradient(135deg, hsl({hue(a.id)} 70% 58%), hsl({(hue(a.id) + 40) % 360} 70% 46%))"
            >{initial(a.name)}</span>
            <span class="min-w-0 flex-1 leading-tight">
              <span class="block truncate text-[12.5px] font-medium {a.id === app.appId ? 'text-accent' : ''}">{a.name}</span>
              {#if a.name !== a.id}<span class="mono block truncate text-[10.5px] text-faint">{a.id}</span>{/if}
            </span>
            {#if a.pid != null}
              <span class="mono flex items-center gap-1 rounded-md bg-ok/12 px-1.5 py-px text-[10px] text-ok"><span class="size-1.5 rounded-full bg-ok"></span>{a.pid}</span>
            {:else if !a.user}
              <span class="text-[10px] text-faint">system</span>
            {/if}
          </button>
        {:else}
          <p class="px-3 py-6 text-center text-[12px] text-muted">{app.apps.length ? "No app matches." : "Loading apps…"}</p>
        {/each}
      </div>
      <label class="flex items-center gap-2 border-t border-line bg-panel-2 px-3 py-2 text-[11.5px] text-muted">
        <input type="checkbox" bind:checked={showSystem} class="accent-[var(--accent)]" /> Show system apps
        <span class="ml-auto text-faint">{list.length} shown</span>
      </label>
    </div>
  {/if}
</div>

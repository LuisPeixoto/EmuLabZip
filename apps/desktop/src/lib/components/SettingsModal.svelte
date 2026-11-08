<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { app, defaultSettings, type Settings } from "../stores/app.svelte";
  import { logs } from "../stores/logs.svelte";
  import AppearanceSettings from "./settings/AppearanceSettings.svelte";
  import LayoutSettings from "./settings/LayoutSettings.svelte";
  import Icon from "./Icon.svelte";

  type Tab = "general" | "appearance" | "layout";
  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: "general", label: "General", icon: "settings" },
    { id: "appearance", label: "Appearance", icon: "sun" },
    { id: "layout", label: "Layout", icon: "layers" },
  ];
  let tab = $state<Tab>("general");

  let draft = $state<Settings>({ ...app.settings });

  function close() {
    app.settingsOpen = false;
  }

  function apply() {
    const streamChanged = draft.logVerbose !== app.settings.logVerbose || draft.logTail !== app.settings.logTail;
    app.saveSettings({
      logBuffer: Math.min(500_000, Math.max(1_000, Math.round(draft.logBuffer))),
      logTail: Math.min(10_000, Math.max(0, Math.round(draft.logTail))),
      logVerbose: draft.logVerbose,
      pollSeconds: Math.min(30, Math.max(1, Math.round(draft.pollSeconds))),
      proxyPort: Math.min(65535, Math.max(1024, Math.round(draft.proxyPort))),
    });
    if (streamChanged && app.target) logs.restart(app.target, app.deviceKey);
    app.toast("Settings saved", "ok");
    close();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

{#snippet row(label: string, hint: string)}
  <div class="min-w-0 flex-1">
    <p class="text-md font-medium">{label}</p>
    <p class="text-sm text-muted">{hint}</p>
  </div>
{/snippet}

<div transition:fade={{ duration: 120 }} class="fixed inset-0 z-50 grid place-items-center bg-black/45 p-8 backdrop-blur-[2px]">
  <button class="absolute inset-0 cursor-default" aria-label="Close" onclick={close}></button>
  <div transition:scale={{ start: 0.97, duration: 160 }} class="shadow-pop relative flex h-[min(720px,100%)] w-[min(960px,100%)] flex-col overflow-hidden rounded-2xl border border-line bg-elevated">
    <div class="flex items-center gap-2 border-b border-line px-5 py-3.5">
      <Icon name="settings" size={15} class="text-accent" />
      <h2 class="flex-1 text-xl font-semibold">Settings</h2>
      <button onclick={close} class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg" aria-label="Close">
        <Icon name="x" size={14} />
      </button>
    </div>

    <div class="flex min-h-0 flex-1">
      <nav class="w-44 shrink-0 space-y-0.5 border-r border-line bg-panel-2 p-2">
        {#each tabs as t (t.id)}
          <button
            onclick={() => (tab = t.id)}
            class="flex h-8 w-full items-center gap-2 rounded-lg px-2.5 text-md {tab === t.id ? 'bg-accent-soft font-medium text-accent' : 'text-fg/85 hover:bg-hover'}"
          ><Icon name={t.icon} size={14} />{t.label}</button>
        {/each}
      </nav>
      <div class="min-w-0 flex-1 overflow-y-auto">
        {#if tab === "appearance"}
          <AppearanceSettings />
        {:else if tab === "layout"}
          <LayoutSettings />
        {:else}
    <div class="space-y-1 p-3">
      <p class="px-2 pt-1 pb-1 text-2xs font-semibold tracking-wider text-faint uppercase">Logs</p>
      <label class="flex items-center gap-4 rounded-xl px-2 py-2">
        {@render row("Verbose and debug lines", "Turn off to keep busy devices readable. The iOS log is very chatty.")}
        <input type="checkbox" bind:checked={draft.logVerbose} class="size-4 accent-[var(--accent)]" />
      </label>
      <label class="flex items-center gap-4 rounded-xl px-2 py-2">
        {@render row("Lines in memory", "Older lines are discarded when the buffer is full.")}
        <input type="number" min="1000" max="500000" step="1000" bind:value={draft.logBuffer} class="mono h-8 w-28 rounded-lg border border-line bg-panel-2 px-2 text-right outline-none focus:border-accent" />
      </label>
      <label class="flex items-center gap-4 rounded-xl px-2 py-2">
        {@render row("History on connect", "Recent lines shown when a stream starts (Android).")}
        <input type="number" min="0" max="10000" step="100" bind:value={draft.logTail} class="mono h-8 w-28 rounded-lg border border-line bg-panel-2 px-2 text-right outline-none focus:border-accent" />
      </label>

      <p class="px-2 pt-3 pb-1 text-2xs font-semibold tracking-wider text-faint uppercase">Devices</p>
      <label class="flex items-center gap-4 rounded-xl px-2 py-2">
        {@render row("Refresh every", "How often EmuLabZip looks for new emulators and simulators, in seconds.")}
        <input type="number" min="1" max="30" bind:value={draft.pollSeconds} class="mono h-8 w-28 rounded-lg border border-line bg-panel-2 px-2 text-right outline-none focus:border-accent" />
      </label>

      <p class="px-2 pt-3 pb-1 text-2xs font-semibold tracking-wider text-faint uppercase">Network</p>
      <label class="flex items-center gap-4 rounded-xl px-2 py-2">
        {@render row("Proxy port", "Where the proxy listens, on this computer only. Applies the next time it starts.")}
        <input type="number" min="1024" max="65535" bind:value={draft.proxyPort} class="mono h-8 w-28 rounded-lg border border-line bg-panel-2 px-2 text-right outline-none focus:border-accent" />
      </label>
    </div>
        {/if}
      </div>
    </div>

    <div class="flex items-center gap-2 border-t border-line bg-panel-2 px-5 py-3">
      {#if tab === "general"}
        <button onclick={() => (draft = { ...defaultSettings })} class="text-base text-muted hover:text-fg">Reset to defaults</button>
        <span class="flex-1"></span>
        <button onclick={close} class="h-8 rounded-lg border border-line px-3.5 text-base font-medium hover:bg-hover">Cancel</button>
        <button onclick={apply} class="h-8 rounded-lg bg-accent px-3.5 text-base font-semibold text-white hover:opacity-90">Save</button>
      {:else}
        <span class="text-sm text-muted">Changes here apply at once.</span>
        <span class="flex-1"></span>
        <button onclick={close} class="h-8 rounded-lg bg-accent px-3.5 text-base font-semibold text-white hover:opacity-90">Done</button>
      {/if}
    </div>
  </div>
</div>

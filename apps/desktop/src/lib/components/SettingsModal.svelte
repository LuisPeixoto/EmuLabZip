<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { app, defaultSettings, type Settings } from "../stores/app.svelte";
  import { logs } from "../stores/logs.svelte";
  import Icon from "./Icon.svelte";

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
    });
    if (streamChanged && app.target) logs.restart(app.target, app.deviceKey);
    app.toast("Settings saved", "ok");
    close();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

{#snippet row(label: string, hint: string)}
  <div class="min-w-0 flex-1">
    <p class="text-[12.5px] font-medium">{label}</p>
    <p class="text-[11.5px] text-muted">{hint}</p>
  </div>
{/snippet}

<div transition:fade={{ duration: 120 }} class="fixed inset-0 z-50 grid place-items-center bg-black/45 p-8 backdrop-blur-[2px]">
  <button class="absolute inset-0 cursor-default" aria-label="Close" onclick={close}></button>
  <div transition:scale={{ start: 0.97, duration: 160 }} class="shadow-pop relative w-[520px] overflow-hidden rounded-2xl border border-line bg-elevated">
    <div class="flex items-center gap-2 border-b border-line px-5 py-3.5">
      <Icon name="settings" size={15} class="text-accent" />
      <h2 class="flex-1 text-[13.5px] font-semibold">Settings</h2>
      <button onclick={close} class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg" aria-label="Close">
        <Icon name="x" size={14} />
      </button>
    </div>

    <div class="space-y-1 p-3">
      <p class="px-2 pt-1 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Appearance</p>
      <div class="flex items-center gap-4 rounded-xl px-2 py-2">
        {@render row("Theme", "Dark is easier on the eyes next to an emulator.")}
        <div class="flex rounded-lg border border-line bg-panel-2 p-0.5">
          {#each ["dark", "light"] as const as t (t)}
            <button
              onclick={() => app.setTheme(t)}
              class="flex h-7 items-center gap-1.5 rounded-md px-2.5 text-[12px] capitalize {app.theme === t ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}"
            >
              <Icon name={t === "dark" ? "moon" : "sun"} size={13} />{t}
            </button>
          {/each}
        </div>
      </div>

      <p class="px-2 pt-3 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Logs</p>
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

      <p class="px-2 pt-3 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Devices</p>
      <label class="flex items-center gap-4 rounded-xl px-2 py-2">
        {@render row("Refresh every", "How often EmuLabZip looks for new emulators and simulators, in seconds.")}
        <input type="number" min="1" max="30" bind:value={draft.pollSeconds} class="mono h-8 w-28 rounded-lg border border-line bg-panel-2 px-2 text-right outline-none focus:border-accent" />
      </label>
    </div>

    <div class="flex items-center gap-2 border-t border-line bg-panel-2 px-5 py-3">
      <button onclick={() => (draft = { ...defaultSettings })} class="text-[12px] text-muted hover:text-fg">Reset to defaults</button>
      <span class="flex-1"></span>
      <button onclick={close} class="h-8 rounded-lg border border-line px-3.5 text-[12px] font-medium hover:bg-hover">Cancel</button>
      <button onclick={apply} class="h-8 rounded-lg bg-accent px-3.5 text-[12px] font-semibold text-white hover:opacity-90">Save</button>
    </div>
  </div>
</div>

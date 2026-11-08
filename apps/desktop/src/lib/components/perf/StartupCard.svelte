<script lang="ts">
  import { api, errorText, type StartKind, type StartupReport } from "../../api";
  import { app } from "../../stores/app.svelte";
  import Icon from "../Icon.svelte";

  const kinds: { id: StartKind; label: string }[] = [
    { id: "cold", label: "Cold" },
    { id: "warm", label: "Warm" },
    { id: "hot", label: "Hot" },
  ];

  let kind = $state<StartKind>("cold");
  let runs = $state(5);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let report = $state<StartupReport | null>(null);

  const ios = $derived(app.device?.platform === "ios");
  const slowest = $derived(Math.max(1, ...(report?.runs.map((r) => r.totalMs) ?? [1])));

  // A result belongs to one app on one device.
  let owner = "";
  $effect(() => {
    const key = `${app.deviceKey}|${app.appId ?? ""}`;
    if (key !== owner) {
      owner = key;
      report = null;
      error = null;
    }
  });

  async function measure() {
    if (!app.target || !app.selectedApp) return;
    busy = true;
    error = null;
    try {
      report = await api.startupMeasure(app.target, app.selectedApp.id, kind, Math.min(50, Math.max(1, Math.round(runs) || 1)));
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  const ms = (v: number) => (v >= 1000 ? `${(v / 1000).toFixed(2)} s` : `${v} ms`);
</script>

<section class="rounded-2xl border border-line bg-panel p-5">
  <header class="mb-3 flex flex-wrap items-center gap-2">
    <h2 class="flex-1 text-lg font-semibold">Start time</h2>
    <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="group" aria-label="Kind of start">
      {#each kinds as k (k.id)}
        <button
          onclick={() => (kind = k.id)}
          disabled={ios && k.id === "warm"}
          title={ios && k.id === "warm" ? "The simulator has no back button to close a screen and keep the process" : ""}
          class="h-7 rounded-md px-2.5 text-sm disabled:opacity-40 {kind === k.id ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}"
        >{k.label}</button>
      {/each}
    </div>
    <label class="flex items-center gap-1.5 text-sm text-muted">
      Runs
      <input type="number" min="1" max="50" bind:value={runs} class="mono h-7 w-14 rounded-md border border-line bg-panel-2 px-1.5 text-sm outline-none focus:border-accent" />
    </label>
    <button
      onclick={measure}
      disabled={!app.selectedApp || busy}
      class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40"
    >
      <Icon name={busy ? "refresh" : "play"} size={13} class={busy ? "animate-spin" : ""} />{busy ? "Measuring…" : "Measure"}
    </button>
  </header>

  {#if error}
    <p class="flex items-center gap-2 text-md text-err"><Icon name="alert" size={14} />{error}</p>
  {:else if !report}
    <p class="text-md text-muted">
      {app.selectedApp ? "Opens the app several times and takes the median. It closes and opens the app on the device." : "Pick an app in the toolbar."}
    </p>
  {:else}
    <div class="mb-3 flex flex-wrap items-baseline gap-x-5 gap-y-1">
      <p class="mono text-5xl font-semibold tracking-tight">{ms(report.medianMs)}</p>
      <p class="text-base text-muted">median {report.kind} start over {report.runs.length} {report.runs.length === 1 ? "run" : "runs"}</p>
      <p class="mono ml-auto text-sm text-faint">fastest {ms(report.minMs)} · 90th percentile {ms(report.p90Ms)} · slowest {ms(report.maxMs)}</p>
    </div>
    <div class="space-y-1.5">
      {#each report.runs as r, i (i)}
        <div class="grid grid-cols-[28px_1fr_70px] items-center gap-3 text-sm">
          <span class="mono text-faint">#{i + 1}</span>
          <span class="h-2 overflow-hidden rounded-full bg-hover">
            <span class="block h-full rounded-full bg-series-1" style="width:{Math.max(2, (r.totalMs / slowest) * 100)}%"></span>
          </span>
          <span class="mono text-right">{ms(r.totalMs)}</span>
        </div>
      {/each}
    </div>
  {/if}
</section>

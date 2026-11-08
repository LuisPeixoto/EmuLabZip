<script lang="ts">
  import { onDestroy } from "svelte";
  import { api, errorText, type EnergyReport } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";

  let measuring = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let report = $state<EnergyReport | null>(null);
  let startedAt = 0;
  let startedFor = "";
  let elapsed = $state(0);
  let timer: ReturnType<typeof setInterval> | undefined;

  const ios = $derived(app.device?.platform === "ios");
  const largest = $derived(Math.max(0.0001, ...(report?.components.map((c) => c.mah) ?? [0])));

  async function begin() {
    if (!app.target || !app.selectedApp) return;
    busy = true;
    error = null;
    report = null;
    try {
      await api.energyBegin(app.target, app.selectedApp.id);
      startedAt = Date.now();
      startedFor = `${app.deviceKey}|${app.selectedApp.id}`;
      elapsed = 0;
      measuring = true;
      timer = setInterval(() => (elapsed = Math.round((Date.now() - startedAt) / 1000)), 500);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function end() {
    if (!app.target || !app.selectedApp) return;
    busy = true;
    clearInterval(timer);
    try {
      report = await api.energyEnd(app.target, app.selectedApp.id, Math.max(1, Math.round((Date.now() - startedAt) / 1000)));
    } catch (e) {
      error = errorText(e);
    } finally {
      measuring = false;
      busy = false;
    }
  }

  // Leaving the app or the device ends the measurement, so the emulator's battery is put back.
  $effect(() => {
    const key = `${app.deviceKey}|${app.appId ?? ""}`;
    if (measuring && key !== startedFor && app.target) {
      clearInterval(timer);
      measuring = false;
    }
  });
  onDestroy(() => clearInterval(timer));

  const mah = (v: number) => `${v.toFixed(v < 1 ? 3 : 1)} mAh`;
  const secs = (ms: number) => `${(ms / 1000).toFixed(1)} s`;
</script>

<section class="rounded-2xl border border-line bg-panel p-5">
  <header class="mb-3 flex flex-wrap items-center gap-2">
    <h2 class="flex-1 text-lg font-semibold">Energy</h2>
    {#if measuring}
      <span class="mono text-sm text-muted">{elapsed} s</span>
      <button onclick={end} disabled={busy} class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name="stop" size={12} />Stop and read
      </button>
    {:else}
      <button onclick={begin} disabled={!app.selectedApp || busy} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-base font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-40">
        <Icon name={busy ? "refresh" : "zap"} size={13} class={busy ? "animate-spin" : ""} />Start
      </button>
    {/if}
  </header>

  {#if error}
    <p class="flex items-center gap-2 text-md text-err"><Icon name="alert" size={14} />{error}</p>
  {:else if measuring}
    <p class="text-md text-muted">Use the app now, then stop to read what it used.</p>
  {:else if !report}
    <p class="text-md text-muted">
      {app.selectedApp
        ? ios
          ? "The simulator has no energy model: this reports the CPU time the app used and the Mac's thermal state."
          : "Starts counting, you use the app, then it reads the system's estimate. The device's battery is put back afterwards."
        : "Pick an app in the toolbar."}
    </p>
  {:else}
    <div class="mb-3 flex flex-wrap items-baseline gap-x-5 gap-y-1">
      <p class="mono text-5xl font-semibold tracking-tight">{report.estimatedMah == null ? "—" : mah(report.estimatedMah)}</p>
      <p class="text-base text-muted">estimated over {report.seconds} s · {secs(report.cpuMs)} of processor</p>
    </div>
    {#if report.components.length}
      <div class="mb-3 space-y-1.5">
        {#each report.components as c (c.name)}
          <div class="grid grid-cols-[110px_1fr_80px] items-center gap-3 text-sm">
            <span class="truncate text-muted">{c.name}</span>
            <span class="h-2 overflow-hidden rounded-full bg-hover"><span class="block h-full rounded-full bg-series-2" style="width:{Math.max(2, (c.mah / largest) * 100)}%"></span></span>
            <span class="mono text-right">{mah(c.mah)}</span>
          </div>
        {/each}
      </div>
    {/if}
    <dl class="grid grid-cols-2 gap-x-6 gap-y-1 text-base md:grid-cols-4">
      <div class="flex justify-between gap-3"><dt class="text-muted">Alarms</dt><dd class="mono">{report.alarms}</dd></div>
      <div class="flex justify-between gap-3"><dt class="text-muted">Jobs</dt><dd class="mono">{report.jobs}</dd></div>
      <div class="flex justify-between gap-3"><dt class="text-muted">Received</dt><dd class="mono">{bytes(report.networkRxBytes)}</dd></div>
      <div class="flex justify-between gap-3"><dt class="text-muted">Sent</dt><dd class="mono">{bytes(report.networkTxBytes)}</dd></div>
    </dl>
    {#if report.wakelocks.length}
      <p class="mt-3 mb-1 text-xs font-semibold tracking-wider text-faint uppercase">Wake locks</p>
      {#each report.wakelocks.slice(0, 5) as w (w.tag)}
        <div class="flex justify-between gap-3 text-sm"><span class="mono truncate text-muted" title={w.tag}>{w.tag}</span><span class="mono">{secs(w.ms)} · {w.count}×</span></div>
      {/each}
    {/if}
    {#each report.notes as n (n)}<p class="mt-3 text-sm text-faint">{n}</p>{/each}
  {/if}
</section>

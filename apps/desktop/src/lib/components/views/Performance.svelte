<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { app } from "../../stores/app.svelte";
  import { perf } from "../../stores/perf.svelte";
  import { kb, percent } from "../../format";
  import Icon from "../Icon.svelte";
  import LineChart from "../charts/LineChart.svelte";
  import StatTile from "../charts/StatTile.svelte";

  const intervals = [
    { ms: 500, label: "0.5 s" },
    { ms: 1000, label: "1 s" },
    { ms: 2000, label: "2 s" },
  ];

  let paused = $state(false);
  const appId = $derived(app.selectedApp?.id ?? null);
  const ios = $derived(app.device?.platform === "ios");

  // Sample while this view is open, for the selected device and app.
  $effect(() => {
    const key = app.deviceKey;
    const id = appId;
    const run = app.online && !paused;
    perf.intervalMs;
    untrack(() => {
      const target = app.target;
      if (key && target && run) perf.start(target, id);
      else perf.stop();
    });
  });
  onDestroy(() => perf.stop());

  const s = $derived(perf.latest);
  const memUsed = $derived(s?.deviceMemoryUsedKb ?? null);
  const memTotal = $derived(s?.deviceMemoryTotalKb ?? null);
  const pct = (v: number) => `${v.toFixed(v >= 10 ? 0 : 1)}%`;
  const mb = (v: number) => kb(v);
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1180px] p-6">
    <header class="mb-5 flex flex-wrap items-center gap-3">
      <div class="min-w-0 flex-1">
        <h1 class="text-[18px] font-semibold tracking-tight">Performance</h1>
        <p class="text-[12.5px] text-muted">
          {#if app.selectedApp}
            {app.selectedApp.name}
            {#if s?.pid != null}<span class="mono text-faint">· pid {s.pid}</span>{:else}<span class="text-warn">· not running</span>{/if}
          {:else}
            Device-wide. Pick an app in the toolbar to see its CPU, memory and frames.
          {/if}
        </p>
      </div>
      <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="group" aria-label="Sampling interval">
        {#each intervals as i (i.ms)}
          <button
            onclick={() => (perf.intervalMs = i.ms)}
            class="mono h-7 rounded-md px-2.5 text-[11.5px] {perf.intervalMs === i.ms ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}"
          >{i.label}</button>
        {/each}
      </div>
      <button
        onclick={() => (paused = !paused)}
        class="flex h-8 items-center gap-1.5 rounded-lg border px-3 text-[12px] font-medium {paused ? 'border-warn/40 bg-warn/12 text-warn' : 'border-line text-muted hover:bg-hover hover:text-fg'}"
      >
        <Icon name={paused ? "play" : "pause"} size={13} />{paused ? "Resume" : "Pause"}
      </button>
      <button onclick={() => perf.clear()} title="Clear the charts" class="grid size-8 place-items-center rounded-lg text-muted hover:bg-hover hover:text-fg">
        <Icon name="trash" size={15} />
      </button>
    </header>

    <div class="mb-5 grid grid-cols-2 gap-3 md:grid-cols-4">
      <StatTile icon="activity" label="App CPU" value={percent(s?.appCpu)} detail="of all cores" />
      <StatTile icon="cpu" label="App memory" value={kb(s?.appMemoryKb)} detail="resident" />
      {#if ios}
        <StatTile icon="layers" label="Frames" value="—" detail="Phase 4, with Instruments traces" />
        <StatTile icon="monitor" label="Host" value="Simulator" detail="runs on this Mac's CPU" />
      {:else}
        <StatTile icon="monitor" label="Frame rate" value={s?.fps == null ? "—" : `${s.fps.toFixed(0)} fps`} detail={s?.jankPercent == null ? "frames rendered" : `${percent(s.jankPercent)} janky`} />
        <StatTile
          icon="battery"
          label="Battery"
          value={s?.batteryLevel == null ? "—" : `${s.batteryLevel}%`}
          detail={s?.batteryTempC == null ? "" : `${s.batteryTempC.toFixed(1)} °C`}
        />
      {/if}
    </div>

    <div class="grid gap-4 lg:grid-cols-2">
      <LineChart
        title="CPU"
        max={100}
        format={pct}
        series={ios
          ? [{ label: "App", color: "var(--series-1)", values: perf.series((x) => x.appCpu) }]
          : [
              { label: "App", color: "var(--series-1)", values: perf.series((x) => x.appCpu) },
              { label: "Device", color: "var(--series-2)", values: perf.series((x) => x.deviceCpu) },
            ]}
        empty={app.selectedApp || !ios ? "Collecting samples…" : "Pick an app to sample"}
      />
      <LineChart
        title="App memory"
        format={mb}
        series={[{ label: "Resident", color: "var(--series-1)", values: perf.series((x) => x.appMemoryKb) }]}
        empty={app.selectedApp ? "Collecting samples…" : "Pick an app to sample"}
      />
      {#if !ios}
        <LineChart
          title="Frame rate"
          format={(v) => `${v.toFixed(0)} fps`}
          series={[{ label: "Frames per second", color: "var(--series-1)", values: perf.series((x) => x.fps) }]}
          empty={app.selectedApp ? "Interact with the app to render frames" : "Pick an app to sample"}
        />
        <LineChart
          title="Janky frames"
          max={100}
          format={pct}
          series={[{ label: "Janky", color: "var(--series-2)", values: perf.series((x) => x.jankPercent) }]}
          empty={app.selectedApp ? "Interact with the app to render frames" : "Pick an app to sample"}
        />
        <LineChart
          title="Device memory"
          max={memTotal ?? undefined}
          format={mb}
          series={[{ label: "Used", color: "var(--series-2)", values: perf.series((x) => x.deviceMemoryUsedKb) }]}
        />
        <section class="flex flex-col justify-center rounded-2xl border border-line bg-panel p-5">
          <p class="text-[12.5px] font-semibold">Device memory</p>
          <p class="mono mt-2 text-[24px] font-semibold tracking-tight">{kb(memUsed)}</p>
          <p class="text-[12px] text-muted">used of {kb(memTotal)}</p>
          {#if memUsed != null && memTotal}
            <div class="mt-3 h-2 overflow-hidden rounded-full bg-hover">
              <div class="h-full rounded-full bg-series-2" style="width:{Math.min(100, (memUsed / memTotal) * 100)}%"></div>
            </div>
          {/if}
          <p class="mt-4 text-[11.5px] leading-relaxed text-faint">
            CPU comes from <span class="mono">/proc</span>, frames from <span class="mono">dumpsys gfxinfo</span>. Values are
            percentages of all cores, the way Android Studio shows them.
          </p>
        </section>
      {/if}
    </div>
  </div>
</div>

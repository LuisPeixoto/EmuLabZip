<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { api, errorText, targetOf, type DiffRow, type PerfSample } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { WINDOW } from "../../stores/perf.svelte";
  import LineChart from "../charts/LineChart.svelte";
  import Icon from "../Icon.svelte";

  const others = $derived(app.devices.filter((d) => d.id !== app.device?.id && d.state === "online"));
  let otherId = $state<string | null>(null);
  const other = $derived(others.find((d) => d.id === otherId) ?? null);
  const a = $derived(app.device);

  // Pick a second device when there is one to pick.
  $effect(() => {
    if (!others.some((d) => d.id === otherId)) otherId = others[0]?.id ?? null;
  });

  let shots = $state<{ a: string | null; b: string | null }>({ a: null, b: null });
  let rows = $state<DiffRow[]>([]);
  let showSame = $state(false);
  let error = $state<string | null>(null);
  let aSamples = $state.raw<PerfSample[]>([]);
  let bSamples = $state.raw<PerfSample[]>([]);

  // Everything restarts when either device changes.
  let sessions: number[] = [];
  let timer: ReturnType<typeof setInterval> | undefined;

  function stop() {
    clearInterval(timer);
    timer = undefined;
    for (const id of sessions) api.stopSession(id).catch(() => {});
    sessions = [];
  }

  async function snap(which: "a" | "b", device: typeof a) {
    if (!device) return;
    try {
      const png = await api.screenshot(targetOf(device));
      shots = { ...shots, [which]: png };
    } catch {
      /* a device that cannot take a screenshot right now keeps its last picture */
    }
  }

  async function sample(device: NonNullable<typeof a>, set: (s: PerfSample[]) => void, key: string) {
    let list: PerfSample[] = [];
    try {
      const id = await api.startPerf(targetOf(device), { app: app.appId, intervalMs: 1000 }, (s) => {
        if (key !== current) return;
        list = list.length >= WINDOW ? list.slice(1) : list.slice();
        list.push(s);
        set(list);
      });
      if (key !== current) api.stopSession(id).catch(() => {});
      else sessions.push(id);
    } catch {
      /* not every device can be sampled */
    }
  }

  let current = "";
  // The device list is replaced on every poll, so the effect follows ids, not objects: otherwise
  // it would start over, and lose the samples, every few seconds.
  const aKey = $derived(a ? `${a.platform}:${a.id}` : "");
  const bKey = $derived(other ? `${other.platform}:${other.id}` : "");

  $effect(() => {
    aKey;
    bKey;
    const appId = app.appId;
    untrack(() => {
      const [x, y] = [a, other];
      stop();
      error = null;
      rows = [];
      aSamples = bSamples = [];
      shots = { a: null, b: null };
      if (!x || !y) return;
      const key = `${x.id}|${y.id}|${appId}`;
      current = key;
      snap("a", x);
      snap("b", y);
      timer = setInterval(() => {
        snap("a", x);
        snap("b", y);
      }, 2500);
      sample(x, (s) => (aSamples = s), key);
      sample(y, (s) => (bSamples = s), key);
      api.compareInfo(targetOf(x), targetOf(y)).then(
        (c) => key === current && (rows = c.rows),
        (e) => key === current && (error = errorText(e)),
      );
    });
  });

  onDestroy(() => {
    current = "";
    stop();
  });

  const shown = $derived(rows.filter((r) => showSame || !r.same));
  const differing = $derived(rows.filter((r) => !r.same).length);

  const pad = (list: PerfSample[], pick: (s: PerfSample) => number | null) => {
    const values = list.map(pick);
    return Array(Math.max(0, WINDOW - values.length)).fill(null).concat(values);
  };
  const cpu = $derived([
    { label: a?.name ?? "A", color: "var(--series-1)", values: pad(aSamples, (s) => s.appCpu ?? s.deviceCpu) },
    { label: other?.name ?? "B", color: "var(--series-2)", values: pad(bSamples, (s) => s.appCpu ?? s.deviceCpu) },
  ]);
  const mem = $derived([
    { label: a?.name ?? "A", color: "var(--series-1)", values: pad(aSamples, (s) => (s.appMemoryKb ?? s.deviceMemoryUsedKb ?? null) == null ? null : (s.appMemoryKb ?? s.deviceMemoryUsedKb)! / 1024) },
    { label: other?.name ?? "B", color: "var(--series-2)", values: pad(bSamples, (s) => (s.appMemoryKb ?? s.deviceMemoryUsedKb ?? null) == null ? null : (s.appMemoryKb ?? s.deviceMemoryUsedKb)! / 1024) },
  ]);
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1180px] p-6">
    <header class="mb-5 flex flex-wrap items-end gap-3">
      <div class="min-w-0 flex-1">
        <h1 class="text-[18px] font-semibold tracking-tight">Compare</h1>
        <p class="text-[12.5px] text-muted">See the same app on two devices: both screens, their differences and how hard each one works.</p>
      </div>
      {#if others.length}
        <label class="flex items-center gap-2 text-[12px] text-muted">
          against
          <select bind:value={otherId} class="h-8 rounded-lg border border-line bg-panel-2 px-2 text-[12px] text-fg outline-none focus:border-accent">
            {#each others as d (d.id)}<option value={d.id}>{d.name}</option>{/each}
          </select>
        </label>
      {/if}
    </header>

    {#if !a || !app.online}
      <p class="rounded-xl border border-line bg-panel px-4 py-8 text-center text-[12.5px] text-muted">Pick a device in the toolbar first.</p>
    {:else if !other}
      <div class="rounded-xl border border-line bg-panel px-6 py-12 text-center">
        <span class="mx-auto mb-3 grid size-11 place-items-center rounded-2xl bg-accent-soft text-accent"><Icon name="columns" size={20} /></span>
        <p class="text-[13.5px] font-medium">Start a second device</p>
        <p class="mx-auto mt-1.5 max-w-[420px] text-[12.5px] leading-relaxed text-muted">Boot another emulator or simulator, for example a different Android version or an iPhone next to a Pixel. It shows up here by itself.</p>
      </div>
    {:else}
      <div class="grid gap-4 md:grid-cols-2">
        {#each [{ d: a, png: shots.a }, { d: other, png: shots.b }] as { d, png } (d.id)}
          <section class="rounded-2xl border border-line bg-panel p-3">
            <p class="mb-2 flex items-center gap-2 text-[12.5px] font-medium"><Icon name={d.platform === "ios" ? "apple" : "android"} size={14} />{d.name}<span class="mono text-[11px] font-normal text-faint">{d.osVersion ?? ""}</span></p>
            <div class="flex h-[420px] items-center justify-center overflow-hidden rounded-xl bg-panel-2 p-2">
              {#if png}<img src="data:image/png;base64,{png}" alt="Screen of {d.name}" class="size-full rounded-lg object-contain" />{:else}<div class="skeleton h-3/4 w-1/3 rounded-lg"></div>{/if}
            </div>
          </section>
        {/each}
      </div>

      <div class="mt-4 grid gap-4 md:grid-cols-2">
        <LineChart title="CPU {app.appId ? `of ${app.appId}` : 'of the device'}" series={cpu} max={100} format={(x) => `${x.toFixed(0)}%`} />
        <LineChart title="Memory" series={mem} format={(x) => `${x.toFixed(0)} MB`} />
      </div>

      <section class="mt-4 overflow-hidden rounded-2xl border border-line bg-panel">
        <header class="flex items-center gap-3 border-b border-line px-4 py-2.5">
          <h2 class="text-[13px] font-semibold">Differences</h2>
          <span class="text-[11.5px] text-muted">{differing} of {rows.length} facts differ</span>
          <span class="flex-1"></span>
          <label class="flex items-center gap-1.5 text-[12px] text-muted"><input type="checkbox" bind:checked={showSame} class="accent-[var(--accent)]" /> Show what matches</label>
        </header>
        {#if error}<p class="px-4 py-3 text-[12.5px] text-err">{error}</p>{/if}
        <div class="selectable max-h-[420px] overflow-y-auto">
          <div class="mono sticky top-0 grid grid-cols-[120px_160px_1fr_1fr] gap-3 border-b border-line bg-panel-2 px-4 py-1.5 text-[10.5px] tracking-wide text-faint uppercase"><span>Section</span><span>Fact</span><span class="truncate">{a.name}</span><span class="truncate">{other.name}</span></div>
          {#each shown as r, i (i)}
            <div class="grid grid-cols-[120px_160px_1fr_1fr] gap-3 border-b border-line/40 px-4 py-1.5 text-[12px] {r.same ? 'text-muted' : ''}">
              <span class="text-faint">{r.section}</span><span class="font-medium">{r.key}</span>
              <span class="mono min-w-0 break-all {r.same ? '' : 'rounded bg-warn/10 px-1'}">{r.a ?? "—"}</span>
              <span class="mono min-w-0 break-all {r.same ? '' : 'rounded bg-warn/10 px-1'}">{r.b ?? "—"}</span>
            </div>
          {:else}
            <p class="px-4 py-6 text-center text-[12.5px] text-muted">{rows.length ? "These two report the same facts." : "Reading both devices…"}</p>
          {/each}
        </div>
      </section>
    {/if}
  </div>
</div>

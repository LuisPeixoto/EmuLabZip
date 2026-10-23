<script lang="ts">
  import type { TraceSummary } from "../../api";
  import { compact, millis, nanos, percent, size } from "../../format";
  import StatTile from "../charts/StatTile.svelte";

  let { summary }: { summary: TraceSummary } = $props();

  const cpu = $derived(summary.cpu);
  const frames = $derived(summary.frames);
  const topProcess = $derived(cpu?.byProcess[0]?.durNs || 1);
  const topThread = $derived(cpu?.byThread[0]?.durNs || 1);
  const topSlice = $derived(summary.slices[0]?.totalNs || 1);
  const late = $derived(frames ? (frames.missed / Math.max(frames.total, 1)) * 100 : 0);
</script>

{#snippet bars(rows: { label: string; sub?: string; value: number; text: string }[], max: number)}
  <ul class="space-y-1.5">
    {#each rows as r, i (i)}
      <li class="grid grid-cols-[minmax(0,1fr)_84px] items-center gap-3">
        <div class="min-w-0">
          <p class="flex items-baseline gap-2 text-[12px]">
            <span class="truncate" title={r.label}>{r.label}</span>
            {#if r.sub}<span class="truncate text-[11px] text-faint" title={r.sub}>{r.sub}</span>{/if}
          </p>
          <div class="mt-1 h-1.5 overflow-hidden rounded-full bg-hover">
            <div class="h-full rounded-full bg-[var(--series-1)]" style="width: {Math.max(1.5, (r.value / max) * 100)}%"></div>
          </div>
        </div>
        <span class="mono text-right text-[11.5px] text-muted">{r.text}</span>
      </li>
    {/each}
  </ul>
{/snippet}

<div class="space-y-4">
  <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
    <StatTile icon="clock" label="Duration" value="{summary.durationS.toFixed(1)} s" detail="{summary.processes} processes · {compact(summary.threads)} threads" />
    <StatTile icon="cpu" label="CPU busy" value={cpu ? percent(cpu.busyFraction * 100, 0) : "—"} detail={cpu ? `${cpu.cpus} cores` : "no scheduling data"} />
    <StatTile icon="layers" label="Frames" value={frames ? compact(frames.total) : "—"} detail={frames ? `${frames.missed} late (${late.toFixed(0)}%)` : "no frame timeline"} />
    <StatTile icon="gauge" label="Frame time p90" value={frames ? millis(frames.p90Ms) : "—"} detail={frames ? `p50 ${millis(frames.p50Ms)} · p99 ${millis(frames.p99Ms)}` : ""} />
  </div>

  {#if cpu}
    <div class="grid gap-4 lg:grid-cols-2">
      <section class="rounded-2xl border border-line bg-panel p-4">
        <h3 class="mb-3 text-[12.5px] font-semibold">CPU time by process</h3>
        {@render bars(cpu.byProcess.map((p) => ({ label: p.name, sub: `pid ${p.pid}`, value: p.durNs, text: nanos(p.durNs) })), topProcess)}
      </section>
      <section class="rounded-2xl border border-line bg-panel p-4">
        <h3 class="mb-3 text-[12.5px] font-semibold">CPU time by thread</h3>
        {@render bars(cpu.byThread.map((t) => ({ label: t.name, sub: t.process, value: t.durNs, text: nanos(t.durNs) })), topThread)}
      </section>
    </div>
  {/if}

  {#if frames}
    <section class="rounded-2xl border border-line bg-panel p-4">
      <h3 class="mb-1 text-[12.5px] font-semibold">Frames</h3>
      <p class="mb-3 text-[11.5px] text-muted">
        From SurfaceFlinger's frame timeline. A frame is late when the app or the compositor missed its deadline; the worst one took {millis(frames.worstMs)}.
      </p>
      <div class="grid gap-5 lg:grid-cols-2">
        <div>
          <p class="mb-2 text-[11px] font-semibold tracking-wider text-faint uppercase">Kinds of jank</p>
          {#if frames.jank.length}
            {@render bars(frames.jank.map(([name, n]) => ({ label: name, value: n, text: `${n} frames` })), frames.jank[0][1])}
          {:else}
            <p class="text-[12px] text-muted">No jank recorded.</p>
          {/if}
        </div>
        <div>
          <p class="mb-2 text-[11px] font-semibold tracking-wider text-faint uppercase">Layers</p>
          <table class="w-full text-[12px]">
            <thead class="text-left text-[11px] text-faint"><tr><th class="pb-1 font-medium">Layer</th><th class="pb-1 text-right font-medium">Frames</th><th class="pb-1 text-right font-medium">Late</th><th class="pb-1 text-right font-medium">Avg</th></tr></thead>
            <tbody>
              {#each frames.layers as l (l.name)}
                <tr class="border-t border-line"><td class="max-w-[220px] truncate py-1.5 pr-2" title={l.name}>{l.name}</td><td class="mono py-1.5 text-right">{l.frames}</td><td class="mono py-1.5 text-right">{l.missed}</td><td class="mono py-1.5 text-right">{millis(l.avgMs)}</td></tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  {/if}

  {#if summary.memory}
    <section class="rounded-2xl border border-line bg-panel p-4">
      <h3 class="mb-3 text-[12.5px] font-semibold">Memory</h3>
      <div class="grid gap-5 lg:grid-cols-2">
        {#if summary.memory.system.length}
          <table class="w-full text-[12px]">
            <thead class="text-left text-[11px] text-faint"><tr><th class="pb-1 font-medium">System</th><th class="pb-1 text-right font-medium">Lowest</th><th class="pb-1 text-right font-medium">Highest</th></tr></thead>
            <tbody>
              {#each summary.memory.system as m (m.name)}
                <tr class="border-t border-line"><td class="py-1.5">{m.name}</td><td class="mono py-1.5 text-right">{size(m.min)}</td><td class="mono py-1.5 text-right">{size(m.max)}</td></tr>
              {/each}
            </tbody>
          </table>
        {/if}
        {#if summary.memory.processes.length}
          <div>
            <p class="mb-2 text-[11px] font-semibold tracking-wider text-faint uppercase">Resident memory, peak per process</p>
            {@render bars(
              summary.memory.processes.map((p) => ({ label: p.name, sub: `pid ${p.pid}`, value: p.peakBytes, text: size(p.peakBytes) })),
              summary.memory.processes[0].peakBytes || 1,
            )}
          </div>
        {/if}
      </div>
    </section>
  {/if}

  {#if summary.slices.length}
    <section class="rounded-2xl border border-line bg-panel p-4">
      <h3 class="mb-3 text-[12.5px] font-semibold">Busiest slices</h3>
      {@render bars(summary.slices.map((s) => ({ label: s.name, sub: `${compact(s.count)}×, longest ${nanos(s.maxNs)}`, value: s.totalNs, text: nanos(s.totalNs) })), topSlice)}
    </section>
  {/if}
</div>

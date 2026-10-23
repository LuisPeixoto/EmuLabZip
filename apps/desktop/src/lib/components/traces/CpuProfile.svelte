<script lang="ts">
  import type { CpuView } from "../../api";
  import { compact, nanos } from "../../format";
  import { traces } from "../../stores/traces.svelte";
  import FlameGraph from "../charts/FlameGraph.svelte";
  import Icon from "../Icon.svelte";

  let { view }: { view: CpuView } = $props();

  const process = $derived(view.processes.find((p) => p.key === traces.processKey) ?? null);
  const format = $derived((w: number) => (view.unit === "ns" ? nanos(w) : `${compact(w)} events`));
  const origins = { app: "App", framework: "Framework", native: "Native", kernel: "Kernel" } as const;
  const top = $derived(view.hotspots[0]?.own || 1);
</script>

<div class="space-y-4">
  <div class="flex flex-wrap items-center gap-3">
    <label class="flex items-center gap-2 text-[12px] text-muted">
      Process
      <select
        value={traces.processKey ?? ""}
        onchange={(e) => traces.pickProcess(e.currentTarget.value || null)}
        class="h-8 max-w-[300px] rounded-lg border border-line bg-panel px-2 text-[12px] text-fg outline-none focus:border-accent"
      >
        {#each view.processes as p (p.key)}<option value={p.key}>{p.name} · {format(p.weight)}</option>{/each}
      </select>
    </label>
    {#if process?.threads.length}
      <label class="flex items-center gap-2 text-[12px] text-muted">
        Thread
        <select
          value={traces.threadKey ?? ""}
          onchange={(e) => traces.pickProcess(traces.processKey, e.currentTarget.value || null)}
          class="h-8 max-w-[260px] rounded-lg border border-line bg-panel px-2 text-[12px] text-fg outline-none focus:border-accent"
        >
          <option value="">All threads</option>
          {#each process.threads as t (t.key)}<option value={t.key}>{t.name || `tid ${t.key}`} · {format(t.weight)}</option>{/each}
        </select>
      </label>
    {/if}
    <div class="relative ml-auto">
      <Icon name="search" size={13} class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-faint" />
      <input
        bind:value={traces.search}
        placeholder="Highlight a function"
        aria-label="Highlight a function"
        class="h-8 w-[220px] rounded-lg border border-line bg-panel pr-2 pl-8 text-[12px] outline-none focus:border-accent"
      />
    </div>
    <button onclick={() => traces.saveFolded()} title="Folded stacks for flamegraph.pl and speedscope" class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">
      <Icon name="download" size={13} /> Folded stacks
    </button>
  </div>

  <FlameGraph root={view.flame} {format} search={traces.search} />

  <section class="rounded-2xl border border-line bg-panel p-4">
    <h3 class="text-[12.5px] font-semibold">Hot functions</h3>
    <p class="mb-3 text-[11.5px] text-muted">Where the CPU spent time in the function itself, not in what it called.</p>
    <table class="w-full text-[12px]">
      <thead class="text-left text-[11px] text-faint">
        <tr><th class="pb-1 font-medium">Function</th><th class="pb-1 font-medium">Origin</th><th class="w-[30%] pb-1 font-medium"></th><th class="pb-1 text-right font-medium">Self time</th></tr>
      </thead>
      <tbody>
        {#each view.hotspots as h (h.name + h.origin)}
          <tr class="border-t border-line">
            <td class="mono max-w-[420px] truncate py-1.5 pr-2 text-[11.5px]" title={h.name}>{h.name}</td>
            <td class="py-1.5 pr-2 text-muted"><span class="mr-1.5 inline-block size-2 rounded-[2px] align-middle" style="background: var(--flame-{h.origin})"></span>{origins[h.origin]}</td>
            <td class="py-1.5 pr-3"><div class="h-1.5 overflow-hidden rounded-full bg-hover"><div class="h-full rounded-full" style="width: {Math.max(2, (h.own / top) * 100)}%; background: var(--flame-{h.origin})"></div></div></td>
            <td class="mono py-1.5 text-right text-muted">{format(h.own)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
</div>

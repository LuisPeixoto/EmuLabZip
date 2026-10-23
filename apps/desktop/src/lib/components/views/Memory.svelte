<script lang="ts">
  import { api, errorText, type CategoryDelta, type MemoryReport } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { kb } from "../../format";
  import Icon from "../Icon.svelte";

  interface Snapshot {
    id: number;
    at: Date;
    report: MemoryReport;
  }

  let snapshots = $state<Snapshot[]>([]);
  let selected = $state<number | null>(null);
  let baseline = $state<number | null>(null);
  let busy = $state(false);
  let dumping = $state(false);
  let error = $state<string | null>(null);
  let deltas = $state<CategoryDelta[]>([]);
  let nextId = 1;

  const current = $derived(snapshots.find((s) => s.id === selected) ?? null);
  const base = $derived(snapshots.find((s) => s.id === baseline) ?? null);
  const largest = $derived(current?.report.categories[0]?.kb ?? 1);
  const ios = $derived(app.device?.platform === "ios");

  // Snapshots belong to one app on one device.
  let owner = "";
  $effect(() => {
    const key = `${app.deviceKey}|${app.appId ?? ""}`;
    if (key !== owner) {
      owner = key;
      snapshots = [];
      selected = baseline = null;
      error = null;
    }
  });

  $effect(() => {
    const a = base?.report;
    const b = current?.report;
    if (a && b && base !== current) api.memoryDiff(a, b).then((d) => (deltas = d));
    else deltas = [];
  });

  async function snapshot() {
    if (!app.target || !app.selectedApp) return;
    busy = true;
    error = null;
    try {
      const report = await api.memoryReport(app.target, app.selectedApp.id);
      const snap = { id: nextId++, at: new Date(), report };
      snapshots = [...snapshots, snap];
      if (snapshots.length > 1 && baseline == null) baseline = snapshots[snapshots.length - 2].id;
      selected = snap.id;
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function heapDump() {
    if (!app.target || !app.selectedApp) return;
    dumping = true;
    const path = await app.attempt(() => api.saveHeapDump(app.target!, app.selectedApp!.id));
    dumping = false;
    if (path) app.toast(`Saved to ${path}`, "ok");
  }

  const time = (d: Date) => d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  const signed = (v: number) => `${v > 0 ? "+" : v < 0 ? "−" : ""}${kb(Math.abs(v))}`;
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1180px] p-6">
    <header class="mb-5 flex flex-wrap items-center gap-3">
      <div class="min-w-0 flex-1">
        <h1 class="text-[18px] font-semibold tracking-tight">Memory</h1>
        <p class="text-[12.5px] text-muted">
          {#if app.selectedApp}
            {app.selectedApp.name} · {ios ? "footprint by VM region" : "PSS by category, from dumpsys meminfo"}
          {:else}
            Pick an app in the toolbar to see where its memory goes.
          {/if}
        </p>
      </div>
      <button
        onclick={heapDump}
        disabled={!app.selectedApp || dumping}
        title={ios ? "Save a memory graph for Xcode's memory graph debugger" : "Save an .hprof heap dump (debuggable apps)"}
        class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-40"
      >
        <Icon name={dumping ? "refresh" : "download"} size={13} class={dumping ? "animate-spin" : ""} />
        {dumping ? "Capturing…" : ios ? "Memory graph…" : "Heap dump…"}
      </button>
      <button
        onclick={snapshot}
        disabled={!app.selectedApp || busy}
        class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3.5 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40"
      >
        <Icon name={busy ? "refresh" : "camera"} size={13} class={busy ? "animate-spin" : ""} /> Take snapshot
      </button>
    </header>

    {#if error}
      <div class="mb-4 flex items-center gap-2 rounded-xl border border-err/30 bg-err/8 px-4 py-3 text-[12.5px] text-err">
        <Icon name="alert" size={14} />{error}
      </div>
    {/if}

    {#if !snapshots.length}
      <div class="grid place-items-center rounded-2xl border border-dashed border-line-strong bg-panel/50 px-6 py-16 text-center">
        <Icon name="cpu" size={26} class="mb-3 text-faint" />
        <p class="text-[13.5px] font-medium">No snapshots yet</p>
        <p class="mt-1 max-w-[420px] text-[12.5px] text-muted">
          Take a snapshot, use the app, then take another one. The difference shows which kind of memory grew.
        </p>
      </div>
    {:else}
      <div class="grid gap-5 lg:grid-cols-[230px_1fr]">
        <aside class="space-y-1.5">
          <p class="px-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Snapshots</p>
          {#each snapshots as snap, i (snap.id)}
            <button
              onclick={() => (selected = snap.id)}
              class="flex w-full items-center gap-3 rounded-xl border px-3 py-2.5 text-left {snap.id === selected ? 'border-accent bg-accent-soft' : 'border-line bg-panel hover:border-line-strong'}"
            >
              <span class="mono grid size-7 place-items-center rounded-lg bg-hover text-[11px] font-semibold">#{i + 1}</span>
              <span class="min-w-0 flex-1">
                <span class="mono block text-[13px] font-semibold">{kb(snap.report.totalKb)}</span>
                <span class="block text-[11px] text-muted">{time(snap.at)} · pid {snap.report.pid}</span>
              </span>
              {#if snap.id === baseline}<span class="rounded-full border border-line px-1.5 text-[10px] text-faint">base</span>{/if}
            </button>
          {/each}
        </aside>

        {#if current}
          <div class="space-y-4">
            <section class="rounded-2xl border border-line bg-panel p-5">
              <div class="mb-4 flex items-baseline gap-3">
                <p class="mono text-[28px] font-semibold tracking-tight">{kb(current.report.totalKb)}</p>
                <p class="text-[12.5px] text-muted">total {current.report.metric}</p>
              </div>
              <div class="space-y-2">
                {#each current.report.categories as c (c.name)}
                  <div class="grid grid-cols-[150px_1fr_80px] items-center gap-3 text-[12px]">
                    <span class="truncate text-muted" title={c.name}>{c.name}</span>
                    <span class="h-2 overflow-hidden rounded-full bg-hover">
                      <span class="block h-full rounded-full bg-series-1" style="width:{Math.max(1, (c.kb / largest) * 100)}%"></span>
                    </span>
                    <span class="mono text-right">{kb(c.kb)}</span>
                  </div>
                {/each}
              </div>
            </section>

            {#if snapshots.length > 1}
              <section class="rounded-2xl border border-line bg-panel">
                <header class="flex items-center gap-2 border-b border-line px-4 py-2.5">
                  <h2 class="flex-1 text-[12.5px] font-semibold">Compared with</h2>
                  <select bind:value={baseline} class="h-7 rounded-lg border border-line bg-panel-2 px-2 text-[12px] outline-none focus:border-accent">
                    {#each snapshots as snap, i (snap.id)}
                      {#if snap.id !== selected}<option value={snap.id}>#{i + 1} · {time(snap.at)} · {kb(snap.report.totalKb)}</option>{/if}
                    {/each}
                  </select>
                </header>
                {#if base && base !== current}
                  {@const total = current.report.totalKb - base.report.totalKb}
                  <div class="flex items-center gap-2 px-4 pt-3 text-[12.5px]">
                    <Icon name={total > 0 ? "arrowUp" : total < 0 ? "arrowDown" : "minus"} size={14} class={total > 0 ? "text-warn" : total < 0 ? "text-ok" : "text-faint"} />
                    <span class="mono font-semibold">{signed(total)}</span>
                    <span class="text-muted">{total > 0 ? "grew" : total < 0 ? "shrank" : "unchanged"} in total</span>
                  </div>
                  <table class="mt-2 w-full text-[12px]">
                    <tbody>
                      {#each deltas as d (d.name)}
                        <tr class="border-t border-line/60">
                          <td class="px-4 py-1.5 text-muted">{d.name}</td>
                          <td class="mono px-2 py-1.5 text-right text-faint">{kb(d.beforeKb)} → {kb(d.afterKb)}</td>
                          <td class="mono w-[110px] px-4 py-1.5 text-right font-medium">
                            <span class="inline-flex items-center gap-1 {d.deltaKb > 0 ? 'text-warn' : 'text-ok'}">
                              <Icon name={d.deltaKb > 0 ? "arrowUp" : "arrowDown"} size={11} />{signed(d.deltaKb)}
                            </span>
                          </td>
                        </tr>
                      {:else}
                        <tr><td class="px-4 py-3 text-muted">No category changed.</td></tr>
                      {/each}
                    </tbody>
                  </table>
                {/if}
              </section>
            {/if}

            {#if current.report.details.length}
              <section class="rounded-2xl border border-line bg-panel px-4 py-3">
                <h2 class="mb-2 text-[12.5px] font-semibold">Details</h2>
                <dl class="grid grid-cols-2 gap-x-6 gap-y-1.5 text-[12px] md:grid-cols-3">
                  {#each current.report.details as [k, v] (k)}
                    <div class="flex justify-between gap-3"><dt class="text-muted">{k}</dt><dd class="mono">{v}</dd></div>
                  {/each}
                </dl>
              </section>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

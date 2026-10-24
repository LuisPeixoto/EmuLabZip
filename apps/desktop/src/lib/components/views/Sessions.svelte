<script lang="ts">
  import { onMount } from "svelte";
  import type { LogLevel, RequestRow } from "../../api";
  import { api } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { sessions, type SessionTab } from "../../stores/sessions.svelte";
  import { bytes, clock, dateTime, kb } from "../../format";
  import LineChart from "../charts/LineChart.svelte";
  import Icon from "../Icon.svelte";

  onMount(() => {
    sessions.init();
  });

  const v = $derived(sessions.view);
  const status = $derived(sessions.status);

  const levels: LogLevel[] = ["verbose", "debug", "info", "warn", "error", "fatal"];
  let minLevel = $state<LogLevel>("info");
  let text = $state("");
  const logs = $derived.by(() => {
    if (!v) return [];
    const min = levels.indexOf(minLevel);
    const needle = text.trim().toLowerCase();
    return v.logs.filter((l) => levels.indexOf(l.level) >= min && (!needle || l.message.toLowerCase().includes(needle) || l.tag.toLowerCase().includes(needle)));
  });
  const tone: Record<string, string> = { error: "text-err", fatal: "text-err", warn: "text-warn", info: "text-info" };

  const tabs: { id: SessionTab; label: string }[] = [
    { id: "overview", label: "Overview" },
    { id: "logs", label: "Logs" },
    { id: "requests", label: "Requests" },
    { id: "timeline", label: "Timeline" },
    { id: "screens", label: "Screens" },
  ];

  const duration = (ms: number | null) => (ms == null ? "—" : ms >= 60_000 ? `${Math.floor(ms / 60_000)} min ${Math.round((ms % 60_000) / 1000)} s` : `${(ms / 1000).toFixed(1)} s`);
  const statusTone = (r: RequestRow) => (r.status === 0 || r.status >= 500 ? "text-err" : r.status >= 400 ? "text-warn" : r.status >= 300 ? "text-info" : "text-ok");

  let screen = $state<string | null>(null);
  let screenName = $state<string | null>(null);
  $effect(() => {
    const names = v?.screens ?? [];
    if (!names.length) {
      screen = screenName = null;
      return;
    }
    if (!screenName || !names.includes(screenName)) screenName = names[0];
  });
  $effect(() => {
    const name = screenName;
    if (!name) return;
    api.sessionScreen(name).then((png) => name === screenName && (screen = png), () => {});
  });

  // The timeline reads from the first sample, in seconds.
  const samples = $derived(v?.perf ?? []);
  const cpu = $derived([{ label: "App CPU", color: "var(--series-1)", values: samples.map((s) => s.appCpu) }]);
  const mem = $derived([{ label: "App memory", color: "var(--series-2)", values: samples.map((s) => (s.appMemoryKb == null ? null : s.appMemoryKb / 1024)) }]);
  const crashOffsets = $derived((v?.crashes ?? []).filter((c) => c.timeMs != null).map((c) => ({ ...c, at: Math.max(0, (c.timeMs ?? 0) - (v?.meta.startedMs ?? 0)) })));

  const button = "flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-40";
</script>

<div class="flex h-full flex-col">
  <header class="flex shrink-0 flex-wrap items-center gap-3 border-b border-line bg-panel px-4 py-3">
    <div class="min-w-0">
      <h1 class="text-[15px] font-semibold tracking-tight">Sessions</h1>
      <p class="text-[11.5px] text-muted">Record a run, keep it as one file, and open it later or in CI.</p>
    </div>
    <span class="flex-1"></span>
    {#if status?.recording}
      <span class="flex items-center gap-2 rounded-xl border border-err/30 bg-err/8 px-3 py-1.5 text-[12px]">
        <span class="live-dot size-2 rounded-full bg-err"></span>
        <b class="font-medium">Recording {status.device}</b>
        <span class="mono text-muted">{clock(status.seconds * 1000)} · {status.logs.toLocaleString()} lines · {status.samples} samples · {status.crashes} crashes</span>
      </span>
      <button onclick={() => sessions.stop()} disabled={sessions.busy === "stop"} class="flex h-8 items-center gap-1.5 rounded-lg bg-err px-3 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-50"><Icon name="stop" size={12} />Stop and save</button>
    {:else}
      <label class="flex items-center gap-1.5 text-[12px] text-muted" title="Needs the device connected in Network">
        <input type="checkbox" bind:checked={sessions.network} class="accent-[var(--accent)]" /> Network
      </label>
      <select bind:value={sessions.screensEvery} aria-label="Screenshots" class="mono h-8 rounded-lg border border-line bg-panel-2 px-2 text-[12px] outline-none focus:border-accent">
        <option value={0}>No screenshots</option>
        <option value={5}>Screenshot every 5 s</option>
        <option value={15}>Every 15 s</option>
        <option value={60}>Every minute</option>
      </select>
      <button onclick={() => sessions.start()} disabled={!app.online || sessions.busy === "start"} class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name="play" size={12} />Record {app.selectedApp ? app.selectedApp.name : "this device"}
      </button>
    {/if}
    <button onclick={() => sessions.open(null)} disabled={sessions.busy === "open"} class={button}><Icon name="folderOpen" size={14} />Open…</button>
  </header>

  <div class="flex min-h-0 flex-1">
    <!-- Recorded here -->
    <aside class="flex w-[250px] shrink-0 flex-col border-r border-line bg-panel-2">
      <p class="px-4 pt-3 pb-1.5 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Recorded here</p>
      <div class="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {#each sessions.files as f (f.path)}
          <div class="group flex items-center rounded-lg {v?.path === f.path ? 'bg-select' : 'hover:bg-hover'}">
            <button onclick={() => sessions.open(f.path)} class="min-w-0 flex-1 px-2.5 py-2 text-left">
              <span class="block truncate text-[12.5px] font-medium">{f.name}</span>
              <span class="mono block text-[10.5px] text-faint">{dateTime(f.modifiedMs / 1000)} · {bytes(f.size)}</span>
            </button>
            <span class="flex pr-1 opacity-0 group-hover:opacity-100">
              <button onclick={() => api.sessionReveal(f.path)} title="Show in folder" class="grid size-6 place-items-center rounded text-faint hover:text-fg"><Icon name="folder" size={12} /></button>
              <button onclick={() => sessions.remove(f)} title="Delete" class="grid size-6 place-items-center rounded text-faint hover:text-err"><Icon name="trash" size={12} /></button>
            </span>
          </div>
        {:else}
          <p class="px-3 py-6 text-center text-[12px] leading-relaxed text-muted">Nothing recorded yet. Press Record, use the app, then Stop.</p>
        {/each}
      </div>
    </aside>

    <!-- The open session -->
    <section class="flex min-w-0 flex-1 flex-col">
      {#if !v}
        <div class="grid flex-1 place-items-center px-6 text-center">
          <div class="max-w-[420px]">
            <span class="mx-auto mb-4 grid size-12 place-items-center rounded-2xl bg-accent-soft text-accent"><Icon name="history" size={22} /></span>
            <p class="text-[13.5px] font-medium">Open a session</p>
            <p class="mt-1.5 text-[12.5px] leading-relaxed text-muted">A session holds the logs, CPU and memory, requests, crashes and screenshots of a run. Record one here, open one from CI (<span class="mono">emulabzip ci</span>), or pick a <span class="mono">.ezsession</span> file.</p>
          </div>
        </div>
      {:else}
        <div class="flex shrink-0 items-center gap-3 border-b border-line px-4 py-2.5">
          <div class="min-w-0 flex-1">
            <p class="truncate text-[13px] font-semibold">{v.summary.device.name}{v.summary.app ? ` · ${v.summary.app}` : ""}</p>
            <p class="mono truncate text-[11px] text-faint">{dateTime(v.meta.startedMs / 1000)} · {duration(v.summary.durationMs)}{v.summary.partial ? " · cut short" : ""} · <span title={v.path}>{v.path.split(/[\\/]/).pop()}</span></p>
          </div>
          <button onclick={() => sessions.close()} class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg" aria-label="Close"><Icon name="x" size={14} /></button>
        </div>
        <div class="flex shrink-0 gap-1 border-b border-line px-3" role="tablist">
          {#each tabs as t (t.id)}
            <button role="tab" aria-selected={sessions.tab === t.id} onclick={() => (sessions.tab = t.id)} class="-mb-px flex h-9 items-center gap-1.5 border-b-2 px-2.5 text-[12px] {sessions.tab === t.id ? 'border-accent font-medium text-accent' : 'border-transparent text-muted hover:text-fg'}">
              {t.label}
              {#if t.id === "requests" && v.requests.length}<span class="mono text-[10.5px] text-faint">{v.requests.length}</span>{/if}
              {#if t.id === "screens" && v.screens.length}<span class="mono text-[10.5px] text-faint">{v.screens.length}</span>{/if}
            </button>
          {/each}
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto">
          {#if sessions.tab === "overview"}
            <div class="grid grid-cols-2 gap-3 p-4 lg:grid-cols-4">
              {#each [["Crashes", v.summary.crashes.length ? String(v.summary.crashes.length) : "none", v.summary.crashes.length ? "text-err" : "text-ok"], ["Log errors", String(v.summary.logs.errors), v.summary.logs.errors ? "text-warn" : "text-ok"], ["Requests", v.summary.network ? `${v.summary.network.requests} (${v.summary.network.failed} failed)` : "not recorded", ""], ["CPU peak", v.summary.perf?.cpuPeak != null ? `${v.summary.perf.cpuPeak.toFixed(0)}%` : "—", ""], ["Memory peak", kb(v.summary.perf?.memoryPeakKb), ""], ["Frame rate", v.summary.perf?.fpsAvg != null ? `${v.summary.perf.fpsAvg.toFixed(0)} fps` : "—", ""], ["Log lines", v.summary.logs.total.toLocaleString(), ""], ["Screenshots", String(v.summary.screens), ""]] as [label, value, cls] (label)}
                <div class="rounded-xl border border-line bg-panel p-3">
                  <p class="text-[11px] text-muted">{label}</p>
                  <p class="mono mt-1 text-[15px] font-semibold {cls}">{value}</p>
                </div>
              {/each}
            </div>
            {#if v.summary.crashes.length}
              <h3 class="px-4 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Crashes</h3>
              <ul class="px-4 pb-3">
                {#each v.summary.crashes as c (c.id)}<li class="py-1 text-[12.5px]"><span class="mono mr-2 rounded bg-err/12 px-1.5 text-[11px] text-err">{c.kind}</span>{c.title}</li>{/each}
              </ul>
            {/if}
            {#if v.summary.topErrors.length}
              <h3 class="px-4 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Most repeated errors</h3>
              <ul class="selectable px-4 pb-3">
                {#each v.summary.topErrors as e (e.message)}<li class="mono flex gap-3 py-1 text-[11.5px]"><span class="w-10 shrink-0 text-right text-faint">{e.count}×</span><span class="min-w-0 break-all">{e.message}</span></li>{/each}
              </ul>
            {/if}
            {#if v.summary.network?.slowest.length}
              <h3 class="px-4 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Slowest requests</h3>
              <ul class="px-4 pb-4">
                {#each v.summary.network.slowest as r (r.url + r.ms)}<li class="mono flex gap-3 py-1 text-[11.5px]"><span class="w-16 shrink-0 text-right">{r.ms.toFixed(0)} ms</span><span class="w-12 shrink-0 text-faint">{r.method}</span><span class="min-w-0 truncate">{r.url}</span></li>{/each}
              </ul>
            {/if}
          {:else if sessions.tab === "logs"}
            <div class="sticky top-0 z-10 flex items-center gap-2 border-b border-line bg-panel px-4 py-2">
              <select bind:value={minLevel} aria-label="Level" class="mono h-7 rounded-md border border-line bg-panel-2 px-2 text-[11.5px] outline-none">{#each levels as l (l)}<option value={l}>{l}+</option>{/each}</select>
              <input bind:value={text} spellcheck="false" placeholder="Filter" class="h-7 min-w-0 flex-1 rounded-md border border-line bg-panel-2 px-2.5 text-[12px] outline-none placeholder:text-faint focus:border-accent" />
              <span class="mono text-[11px] text-faint">{logs.length.toLocaleString()} of {v.logsTotal.toLocaleString()}</span>
            </div>
            <div class="mono selectable text-[11.5px]">
              {#each logs.slice(-800) as l (l.seq)}
                <div class="flex gap-2 border-b border-line/30 px-4 py-0.5"><span class="w-[110px] shrink-0 text-faint">{l.time}</span><span class="w-[14px] shrink-0 font-semibold {tone[l.level] ?? 'text-muted'}">{l.level[0].toUpperCase()}</span><span class="w-[110px] shrink-0 truncate text-muted">{l.tag}</span><span class="min-w-0 flex-1 break-all">{l.message}</span></div>
              {:else}
                <p class="px-4 py-8 text-center text-[12.5px] text-muted">No log lines at this level.</p>
              {/each}
            </div>
            {#if logs.length > 800}<p class="px-4 py-2 text-[11.5px] text-faint">Showing the last 800 of {logs.length.toLocaleString()}. Filter to narrow it down.</p>{/if}
          {:else if sessions.tab === "requests"}
            {#if !v.requests.length}
              <p class="px-4 py-12 text-center text-[12.5px] text-muted">{v.meta.network ? "No requests were made." : "The network was not recorded. Tick Network before recording (the device has to be connected in the Network view)."}</p>
            {:else}
              <div class="mono selectable text-[11.5px]">
                {#each v.requests as r (r.index)}
                  <button onclick={() => sessions.openRequest(r.index)} class="flex w-full items-center gap-3 border-b border-line/40 px-4 py-1.5 text-left {sessions.requestIndex === r.index ? 'bg-select' : 'hover:bg-hover'}">
                    <span class="w-9 shrink-0 font-semibold {statusTone(r)}">{r.status || "ERR"}</span>
                    <span class="w-12 shrink-0">{r.method}</span>
                    <span class="min-w-0 flex-1 truncate text-muted">{r.url}</span>
                    <span class="w-16 shrink-0 text-right text-faint">{r.size > 0 ? bytes(r.size) : ""}</span>
                    <span class="w-16 shrink-0 text-right text-faint">{r.ms.toFixed(0)} ms</span>
                  </button>
                {/each}
              </div>
              {#if sessions.request}
                {@const q = sessions.request}
                <div class="selectable border-t border-line bg-panel-2 px-4 py-3 text-[11.5px]">
                  {#each [["Request", q.request?.headers], ["Response", q.response?.headers]] as [title, list] (title)}
                    <h4 class="pt-2 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">{title} headers</h4>
                    {#each (list ?? []) as h, i (i)}<div class="mono flex gap-3 py-0.5"><span class="w-[180px] shrink-0 truncate font-medium">{h.name}</span><span class="min-w-0 break-all text-muted">{h.value}</span></div>{/each}
                  {/each}
                  {#if q.response?.content?.text}
                    <h4 class="pt-3 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Response body</h4>
                    <pre class="mono max-h-[240px] overflow-auto rounded-lg border border-line bg-panel p-2.5 break-all whitespace-pre-wrap">{q.response.content.text.slice(0, 20000)}</pre>
                  {/if}
                </div>
              {/if}
            {/if}
          {:else if sessions.tab === "timeline"}
            {#if !samples.length}
              <p class="px-4 py-12 text-center text-[12.5px] text-muted">No samples were recorded.</p>
            {:else}
              <div class="grid gap-4 p-4 lg:grid-cols-2">
                <LineChart title="CPU, share of the device" series={cpu} max={100} format={(x) => `${x.toFixed(0)}%`} />
                <LineChart title="Memory" series={mem} format={(x) => `${x.toFixed(0)} MB`} />
              </div>
              <p class="px-4 text-[11.5px] text-muted">{samples.length} samples over {duration(samples.at(-1)?.tMs ?? 0)}.</p>
              {#if crashOffsets.length}
                <h3 class="px-4 pt-3 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Crashes on the timeline</h3>
                <ul class="px-4 pb-4">
                  {#each crashOffsets as c (c.id)}<li class="flex gap-3 py-1 text-[12.5px]"><span class="mono w-16 shrink-0 text-err">{duration(c.at)}</span><span class="min-w-0">{c.title}</span></li>{/each}
                </ul>
              {/if}
            {/if}
          {:else}
            {#if !v.screens.length}
              <p class="px-4 py-12 text-center text-[12.5px] text-muted">No screenshots were taken. Pick “Screenshot every…” before recording.</p>
            {:else}
              <div class="flex h-full min-h-0">
                <div class="grid min-w-0 flex-1 place-items-center bg-panel-2 p-4">
                  {#if screen}<img src="data:image/png;base64,{screen}" alt="Screenshot" class="max-h-full max-w-full rounded-lg shadow-lg" />{/if}
                </div>
                <div class="w-[120px] shrink-0 overflow-y-auto border-l border-line p-2">
                  {#each v.screens as name, i (name)}
                    <button onclick={() => (screenName = name)} class="mono mb-1 block w-full rounded-md px-2 py-1.5 text-left text-[11px] {screenName === name ? 'bg-select font-semibold' : 'text-muted hover:bg-hover'}">#{i + 1}</button>
                  {/each}
                </div>
              </div>
            {/if}
          {/if}
        </div>
      {/if}
    </section>
  </div>
</div>

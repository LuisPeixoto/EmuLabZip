<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { plugins } from "../../stores/plugins.svelte";
  import LineChart from "../charts/LineChart.svelte";
  import StatTile from "../charts/StatTile.svelte";
  import Icon from "../Icon.svelte";

  onMount(() => {
    plugins.refresh();
    return () => {
      void plugins.stop();
    };
  });

  const p = $derived(plugins.current);
  const platform = $derived(app.device?.platform ?? null);
  const supported = $derived(!p || !platform || p.manifest.platforms.length === 0 || p.manifest.platforms.includes(platform));
  const canRun = $derived(!!p && p.enabled && app.online && supported);
  const stream = $derived(p?.manifest.mode === "stream");

  const num = (v: number) => (Number.isInteger(v) ? String(v) : v.toFixed(2));
  const cell = (v: unknown) => (v == null ? "" : typeof v === "number" ? num(v) : String(v));
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1180px] p-6">
    <header class="mb-5 flex flex-wrap items-center gap-3">
      <div class="min-w-0 flex-1">
        <h1 class="text-[18px] font-semibold tracking-tight">Plugins</h1>
        <p class="text-[12.5px] text-muted">Your own collectors: a program prints JSON lines for a device, and EmuLabZip draws them.</p>
      </div>
      <button onclick={() => openUrl("https://github.com/LuisPeixoto/EmuLabZip/blob/main/docs/plugins.md")} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg"><Icon name="book" size={13} /> How to write one</button>
      <button onclick={() => app.attempt(() => api.pluginOpenFolder())} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg"><Icon name="folder" size={13} /> Open folder</button>
    </header>

    {#if plugins.loadError}
      <div class="mb-4 flex items-center gap-2 rounded-xl border border-err/30 bg-err/8 px-4 py-3 text-[12.5px] text-err"><Icon name="alert" size={14} />{plugins.loadError}</div>
    {/if}

    {#if plugins.list && !plugins.list.plugins.length && !plugins.list.problems.length}
      <div class="grid place-items-center rounded-2xl border border-dashed border-line-strong bg-panel/50 px-6 py-20 text-center">
        <Icon name="zap" size={26} class="mb-3 text-faint" />
        <p class="text-[13.5px] font-medium">No plugins yet</p>
        <p class="mt-1 max-w-[480px] text-[12.5px] text-muted">Start one with <span class="mono">emulabzip plugin new my-plugin</span>, or install the examples with <span class="mono">emulabzip plugin install examples/plugins/top-processes</span>. Plugins live in <span class="mono break-all">{plugins.list.dir}</span>.</p>
      </div>
    {:else if plugins.list}
      <div class="grid items-start gap-5 lg:grid-cols-[300px_1fr]">
        <aside class="rounded-2xl border border-line bg-panel p-2">
          <ul class="space-y-0.5">
            {#each plugins.list.plugins as row (row.manifest.name)}
              <li>
                <button onclick={() => plugins.select(row.manifest.name)} class="w-full rounded-xl px-3 py-2.5 text-left {row.manifest.name === plugins.selected ? 'bg-accent-soft' : 'hover:bg-hover'}">
                  <span class="flex items-center gap-2">
                    <span class="min-w-0 flex-1 truncate text-[12.5px] font-medium">{row.manifest.name}</span>
                    <span class="size-2 rounded-full {row.enabled ? 'bg-ok' : 'bg-faint'}" title={row.enabled ? "Enabled" : "Disabled"}></span>
                  </span>
                  <span class="mt-0.5 line-clamp-2 block text-[11.5px] text-muted">{row.manifest.description || "No description"}</span>
                  <span class="mt-1 flex gap-1.5 text-[10.5px] text-faint">
                    <span class="rounded-full border border-line px-1.5">{row.manifest.mode}</span>
                    {#each row.manifest.platforms.length ? row.manifest.platforms : ["android", "ios"] as pl (pl)}<span class="rounded-full border border-line px-1.5">{pl}</span>{/each}
                  </span>
                </button>
              </li>
            {/each}
          </ul>
          {#each plugins.list.problems as problem (problem.dir)}
            <p class="mt-2 border-t border-line px-3 pt-2 text-[11px] text-warn" title={problem.dir}><Icon name="alert" size={11} class="mr-1 inline" />{problem.dir.split(/[\\/]/).pop()}: {problem.message}</p>
          {/each}
        </aside>

        <section class="min-w-0 space-y-4">
          {#if p}
            <div class="rounded-2xl border border-line bg-panel p-4">
              <div class="flex flex-wrap items-start gap-3">
                <div class="min-w-0 flex-1">
                  <h2 class="text-[15px] font-semibold">{p.manifest.name} <span class="mono text-[11.5px] font-normal text-faint">v{p.manifest.version}</span></h2>
                  <p class="mt-0.5 text-[12.5px] text-muted">{p.manifest.description}</p>
                </div>
                <label class="flex cursor-pointer items-center gap-2 text-[12px]">
                  <span class="text-muted">{p.enabled ? "Enabled" : "Disabled"}</span>
                  <button
                    role="switch"
                    aria-checked={p.enabled}
                    aria-label="Enable {p.manifest.name}"
                    onclick={() => plugins.setEnabled(p.manifest.name, !p.enabled)}
                    class="h-5 w-9 rounded-full p-0.5 transition {p.enabled ? 'bg-accent' : 'bg-line-strong'}"
                  ><span class="block size-4 rounded-full bg-white shadow transition {p.enabled ? 'translate-x-4' : ''}"></span></button>
                </label>
              </div>
              <div class="mt-3 flex items-start gap-2 rounded-xl bg-panel-2 px-3 py-2.5 text-[11.5px] {p.enabled ? 'text-muted' : 'text-warn'}">
                <Icon name={p.enabled ? "info" : "shield"} size={13} class="mt-0.5 shrink-0" />
                <span class="min-w-0">
                  {#if p.enabled}Runs on your computer:{:else}Enabling lets this program run as you:{/if}
                  <code class="mono block break-all select-text">{p.runs}</code>
                  {#if !p.enabled}Read it first. Changing <span class="mono">plugin.json</span> later disables it again.{/if}
                </span>
              </div>
              <div class="mt-3 flex items-center gap-3">
                {#if plugins.running}
                  <button onclick={() => plugins.stop()} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3.5 text-[12px] font-medium hover:bg-hover"><Icon name="stop" size={12} /> Stop</button>
                  <span class="flex items-center gap-2 text-[12px] text-muted"><span class="live-dot size-2 rounded-full bg-ok"></span>{stream ? "Streaming" : "Running"}</span>
                {:else}
                  <button onclick={() => plugins.run()} disabled={!canRun} class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3.5 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40"><Icon name="play" size={12} /> Run</button>
                  <span class="text-[11.5px] text-muted">
                    {#if !p.enabled}Enable it to run it.{:else if !app.online}Pick a device first.{:else if !supported}It does not support {platform}.{:else if plugins.outcome}Last run: {plugins.outcome}{:else if app.selectedApp}On {app.selectedApp.name}.{:else}On {app.device?.name ?? "the device"}.{/if}
                  </span>
                {/if}
              </div>
            </div>

            {#each plugins.errors as message (message)}
              <div class="flex items-start gap-2 rounded-xl border border-err/30 bg-err/8 px-4 py-3 text-[12.5px] text-err"><Icon name="alert" size={14} class="mt-0.5 shrink-0" /><span class="min-w-0 break-words">{message}</span></div>
            {/each}

            {#if plugins.metrics.length}
              {#if stream}
                <div class="grid gap-4 lg:grid-cols-2">
                  {#each plugins.metrics as m (m.name)}
                    <LineChart title="{m.name}{m.unit ? ` (${m.unit})` : ''}" series={[{ label: m.name, color: "var(--series-1)", values: m.values }]} format={num} height={150} />
                  {/each}
                </div>
              {:else}
                <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
                  {#each plugins.metrics as m (m.name)}
                    <StatTile icon="gauge" label={m.name} value="{num(m.values.at(-1) ?? 0)}{m.unit ? ` ${m.unit}` : ''}" />
                  {/each}
                </div>
              {/if}
            {/if}

            {#each plugins.tables as t, i (i)}
              <section class="overflow-hidden rounded-2xl border border-line bg-panel">
                {#if t.title}<h3 class="px-4 pt-3 pb-2 text-[12.5px] font-semibold">{t.title}</h3>{/if}
                <div class="max-h-[420px] overflow-auto">
                  <table class="w-full text-[12px]">
                    <thead class="sticky top-0 bg-panel-2 text-left text-[11px] text-faint"><tr>{#each t.columns as c (c)}<th class="border-y border-line px-4 py-1.5 font-medium">{c}</th>{/each}</tr></thead>
                    <tbody>
                      {#each t.rows as row, r (r)}
                        <tr class="hover:bg-hover">{#each t.columns as _, c (c)}<td class="mono max-w-[420px] truncate border-b border-line px-4 py-1.5 select-text" title={cell(row[c])}>{cell(row[c])}</td>{/each}</tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              </section>
            {/each}

            {#each plugins.texts as t, i (i)}
              <section class="rounded-2xl border border-line bg-panel p-4">
                {#if t.title}<h3 class="mb-1 text-[12.5px] font-semibold">{t.title}</h3>{/if}
                <p class="text-[12.5px] whitespace-pre-wrap select-text">{t.body}</p>
              </section>
            {/each}

            {#if plugins.logs.length}
              <section class="rounded-2xl border border-line bg-panel p-4">
                <h3 class="mb-2 text-[12.5px] font-semibold">Log</h3>
                <ul class="mono max-h-[200px] space-y-0.5 overflow-auto text-[11.5px] select-text">
                  {#each plugins.logs as l, i (i)}<li class={l.level === "error" ? "text-err" : l.level === "warn" ? "text-warn" : "text-muted"}>{l.message}</li>{/each}
                </ul>
              </section>
            {/if}
          {/if}
        </section>
      </div>
    {/if}
  </div>
</div>

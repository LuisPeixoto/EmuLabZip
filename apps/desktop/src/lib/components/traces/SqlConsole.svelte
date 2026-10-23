<script lang="ts">
  import { traces } from "../../stores/traces.svelte";
  import Icon from "../Icon.svelte";

  const examples: [string, string][] = [
    ["Busiest slices", "select name, count(*) as n, sum(dur) / 1e6 as total_ms from slice group by name order by total_ms desc limit 25"],
    ["CPU by thread", "select thread.name, sum(sched.dur) / 1e6 as cpu_ms from sched join thread using (utid) group by utid order by cpu_ms desc limit 25"],
    ["Slowest frames", "select ts, dur / 1e6 as ms, jank_type, layer_name from actual_frame_timeline_slice order by dur desc limit 25"],
    ["Processes", "select pid, name, uid from process order by pid"],
  ];

  function cell(v: unknown): string {
    if (v == null) return "NULL";
    if (typeof v === "number") return Number.isInteger(v) ? String(v) : v.toFixed(3);
    return String(v);
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      traces.runSql();
    }
  }
</script>

<div class="space-y-3">
  <div class="flex flex-wrap items-center gap-1.5">
    <span class="text-[11.5px] text-muted">Examples:</span>
    {#each examples as [label, sql] (label)}
      <button onclick={() => (traces.sql = sql)} class="rounded-full border border-line px-2.5 py-0.5 text-[11.5px] text-muted hover:border-line-strong hover:text-fg">{label}</button>
    {/each}
  </div>
  <textarea
    bind:value={traces.sql}
    {onkeydown}
    spellcheck="false"
    aria-label="PerfettoSQL"
    class="mono h-28 w-full resize-y rounded-xl border border-line bg-panel p-3 text-[12px] outline-none focus:border-accent"
  ></textarea>
  <div class="flex items-center gap-3">
    <button
      onclick={() => traces.runSql()}
      disabled={traces.sqlRunning}
      class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3.5 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40"
    >
      <Icon name={traces.sqlRunning ? "refresh" : "play"} size={12} class={traces.sqlRunning ? "animate-spin" : ""} /> Run
    </button>
    <span class="text-[11.5px] text-faint">⌘/Ctrl + Enter · PerfettoSQL: tables like slice, sched, thread, process, counter</span>
  </div>

  {#if traces.sqlError}
    <div class="flex items-start gap-2 rounded-xl border border-err/30 bg-err/8 px-3 py-2.5 text-[12px] text-err">
      <Icon name="alert" size={14} class="mt-0.5 shrink-0" /><pre class="mono min-w-0 whitespace-pre-wrap">{traces.sqlError}</pre>
    </div>
  {/if}

  {#if traces.sqlResult}
    {@const r = traces.sqlResult}
    <div class="overflow-hidden rounded-xl border border-line bg-panel">
      <div class="max-h-[460px] overflow-auto">
        <table class="mono w-full text-[11.5px]">
          <thead class="sticky top-0 bg-panel-2 text-left text-faint">
            <tr>{#each r.columns as c (c)}<th class="border-b border-line px-3 py-1.5 font-medium">{c}</th>{/each}</tr>
          </thead>
          <tbody>
            {#each r.rows as row, i (i)}
              <tr class="hover:bg-hover">{#each row as v, j (j)}<td class="max-w-[420px] truncate border-b border-line px-3 py-1 {v == null ? 'text-faint' : ''}" title={cell(v)}>{cell(v)}</td>{/each}</tr>
            {/each}
          </tbody>
        </table>
      </div>
      <p class="border-t border-line px-3 py-1.5 text-[11px] text-muted">
        {r.rows.length} rows{r.truncated ? " (more were cut off, add a LIMIT)" : ""}
      </p>
    </div>
  {/if}
</div>

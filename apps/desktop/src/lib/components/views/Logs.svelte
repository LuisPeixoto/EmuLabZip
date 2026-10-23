<script lang="ts">
  import { tick } from "svelte";
  import { api, LEVELS, type LogEntry } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { logs } from "../../stores/logs.svelte";
  import { fileStamp, hue, slug } from "../../format";
  import { clickOutside } from "../../actions";
  import Icon from "../Icon.svelte";

  const ROW = 22;
  const OVERSCAN = 20;

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(600);
  let follow = $state(true);
  let exportOpen = $state(false);
  let searchEl: HTMLInputElement | undefined = $state();

  const rows = $derived(logs.visible);
  const start = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const end = $derived(Math.min(rows.length, Math.ceil((scrollTop + viewport) / ROW) + OVERSCAN));
  const slice = $derived(rows.slice(start, end));

  // Keep the newest line in view while following.
  $effect(() => {
    rows.length;
    if (follow && scroller) {
      tick().then(() => scroller && (scroller.scrollTop = scroller.scrollHeight));
    }
  });

  function onscroll() {
    if (!scroller) return;
    scrollTop = scroller.scrollTop;
    const atBottom = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < ROW * 2;
    follow = atBottom;
  }

  function jumpToEnd() {
    follow = true;
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  }

  const letter = (l: LogEntry["level"]) => l[0].toUpperCase();

  async function exportAs(kind: "txt" | "jsonl") {
    exportOpen = false;
    const lines = logs.visible;
    if (!lines.length) return app.toast("There are no lines to export");
    const name = `${slug(app.device?.name ?? "device")}-${fileStamp()}.${kind === "txt" ? "log.txt" : "jsonl"}`;
    const body = kind === "txt" ? logs.asText(lines) : logs.asJsonLines(lines);
    const path = await app.attempt(() => api.saveText(name, body));
    if (path) app.toast(`Exported ${lines.length.toLocaleString()} lines to ${path}`, "ok");
  }

  async function copyLine(e: LogEntry) {
    await navigator.clipboard?.writeText(logs.asText([e])).catch(() => {});
    app.toast("Line copied", "ok");
  }

  function onkeydown(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === "f") {
      e.preventDefault();
      searchEl?.focus();
      searchEl?.select();
    } else if (mod && e.key.toLowerCase() === "k") {
      e.preventDefault();
      logs.clear();
    }
  }

  const appFilterReady = $derived(!!app.selectedApp);
  $effect(() => {
    if (!appFilterReady && logs.filter.onlyApp) logs.filter.onlyApp = false;
  });
</script>

<svelte:window {onkeydown} />

<div class="flex h-full flex-col">
  <!-- Filters -->
  <div class="flex shrink-0 flex-wrap items-center gap-2 border-b border-line bg-panel px-3 py-2">
    <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="group" aria-label="Minimum level">
      {#each LEVELS as l (l)}
        {@const on = LEVELS.indexOf(l) >= LEVELS.indexOf(logs.filter.minLevel)}
        <button
          onclick={() => (logs.filter.minLevel = l)}
          title="{l[0].toUpperCase() + l.slice(1)} and above"
          class="lv-{l} mono grid h-6 w-7 place-items-center rounded-md text-[11px] font-semibold transition {on ? 'lv-badge' : 'text-faint hover:text-muted'}"
        >{letter(l)}</button>
      {/each}
    </div>

    <label class="relative flex items-center">
      <Icon name="filter" size={12} class="pointer-events-none absolute left-2.5 text-faint" />
      <input
        bind:value={logs.filter.tag}
        spellcheck="false"
        placeholder="Tag"
        class="h-8 w-[150px] rounded-lg border border-line bg-panel-2 pr-2 pl-7 text-[12px] outline-none placeholder:text-faint focus:border-accent focus:bg-panel"
      />
    </label>

    <label class="relative flex min-w-[220px] flex-1 items-center">
      <Icon name="search" size={13} class="pointer-events-none absolute left-2.5 text-faint" />
      <input
        bind:this={searchEl}
        bind:value={logs.filter.text}
        spellcheck="false"
        placeholder={logs.filter.regex ? "Regular expression, e.g. skipped \\d+ frames" : "Search messages and tags  (⌘F)"}
        class="h-8 w-full rounded-lg border bg-panel-2 pr-10 pl-8 text-[12px] outline-none placeholder:text-faint focus:bg-panel {logs.compiled.error
          ? 'border-err'
          : 'border-line focus:border-accent'}"
      />
      <button
        onclick={() => (logs.filter.regex = !logs.filter.regex)}
        title={logs.compiled.error ?? "Regular expression"}
        class="mono absolute right-1.5 grid h-6 w-7 place-items-center rounded-md text-[11px] font-semibold {logs.filter.regex
          ? 'bg-accent-soft text-accent'
          : 'text-faint hover:text-muted'}">.*</button
      >
    </label>

    <button
      onclick={() => (logs.filter.onlyApp = !logs.filter.onlyApp)}
      disabled={!appFilterReady}
      title={appFilterReady ? `Only lines from ${app.selectedApp?.name}` : "Pick an app in the toolbar first"}
      class="flex h-8 items-center gap-1.5 rounded-lg border px-2.5 text-[12px] font-medium disabled:opacity-40 {logs.filter.onlyApp
        ? 'border-accent bg-accent-soft text-accent'
        : 'border-line text-muted hover:bg-hover hover:text-fg'}"
    >
      <Icon name="smartphone" size={13} />{appFilterReady ? app.selectedApp?.name : "This app"}
    </button>

    <div class="mx-0.5 h-5 w-px bg-line"></div>

    <button
      onclick={() => logs.togglePause()}
      title={logs.paused ? "Resume" : "Pause the view (lines keep arriving)"}
      class="grid size-8 place-items-center rounded-lg {logs.paused ? 'bg-warn/15 text-warn' : 'text-muted hover:bg-hover hover:text-fg'}"
    >
      <Icon name={logs.paused ? "play" : "pause"} size={15} />
    </button>
    <button onclick={() => logs.clear()} title="Clear (⌘K)" class="grid size-8 place-items-center rounded-lg text-muted hover:bg-hover hover:text-fg">
      <Icon name="trash" size={15} />
    </button>
    <div class="relative" use:clickOutside={() => (exportOpen = false)}>
      <button onclick={() => (exportOpen = !exportOpen)} title="Export the lines you see" class="grid size-8 place-items-center rounded-lg text-muted hover:bg-hover hover:text-fg">
        <Icon name="download" size={15} />
      </button>
      {#if exportOpen}
        <div class="shadow-pop absolute top-10 right-0 z-40 w-[200px] rounded-xl border border-line bg-elevated p-1.5">
          <button onclick={() => exportAs("txt")} class="flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[12.5px] hover:bg-hover">
            <Icon name="file" size={14} class="text-muted" /> Plain text (.txt)
          </button>
          <button onclick={() => exportAs("jsonl")} class="flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[12.5px] hover:bg-hover">
            <Icon name="code" size={14} class="text-muted" /> JSON lines (.jsonl)
          </button>
        </div>
      {/if}
    </div>
  </div>

  <!-- Column headers -->
  <div class="mono flex h-7 shrink-0 items-center gap-3 border-b border-line bg-panel-2 px-3 text-[10.5px] tracking-wide text-faint uppercase">
    <span class="w-[132px] shrink-0">Time</span>
    <span class="w-[52px] shrink-0 text-right">PID</span>
    <span class="w-[22px] shrink-0"></span>
    <span class="w-[170px] shrink-0">Tag</span>
    <span class="flex-1">Message</span>
  </div>

  <!-- Lines -->
  <div class="relative min-h-0 flex-1">
    <div bind:this={scroller} bind:clientHeight={viewport} {onscroll} class="selectable absolute inset-0 overflow-y-auto bg-panel">
      {#if rows.length}
        <div style="height:{rows.length * ROW}px" class="relative">
          <div style="transform: translateY({start * ROW}px)">
            {#each slice as e (e.seq)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                onclick={() => (logs.selected = logs.selected?.seq === e.seq ? null : e)}
                ondblclick={() => copyLine(e)}
                style="height:{ROW}px"
                class="lv-{e.level} mono flex items-center gap-3 border-b border-line/40 px-3 text-[11.5px] {logs.selected?.seq === e.seq
                  ? 'bg-select'
                  : e.level === 'error'
                    ? 'lv-row-error hover:bg-hover'
                    : e.level === 'fatal'
                      ? 'lv-row-fatal hover:bg-hover'
                      : 'hover:bg-hover'}"
              >
                <span class="w-[132px] shrink-0 text-faint">{e.time}</span>
                <span class="w-[52px] shrink-0 text-right text-muted">{e.pid}</span>
                <span class="lv-badge grid h-[16px] w-[22px] shrink-0 place-items-center rounded text-[10px] font-bold">{letter(e.level)}</span>
                <span class="w-[170px] shrink-0 truncate font-medium" style="color: hsl({hue(e.tag)} 55% var(--tag-l, 62%))" title={e.tag}>{e.tag}</span>
                <span class="min-w-0 flex-1 truncate {e.level === 'verbose' ? 'text-muted' : 'text-fg'}" title={e.message.length > 120 ? e.message.slice(0, 600) : undefined}>{e.message}</span>
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <div class="grid h-full place-items-center text-center">
          <div>
            <Icon name="terminal" size={26} class="mx-auto mb-3 text-faint" />
            {#if logs.entries.length}
              <p class="text-[13px] font-medium">No line matches the filters</p>
              <button onclick={() => logs.resetFilter()} class="mt-2 text-[12px] text-accent hover:underline">Reset filters</button>
            {:else if logs.live}
              <p class="text-[13px] font-medium">Waiting for log lines…</p>
            {:else}
              <p class="text-[13px] font-medium">{app.online ? "Connecting to the log…" : "Logs appear when a device is online"}</p>
            {/if}
          </div>
        </div>
      {/if}
    </div>
    {#if !follow && rows.length}
      <button
        onclick={jumpToEnd}
        class="shadow-pop absolute right-5 bottom-4 flex h-8 items-center gap-1.5 rounded-full bg-accent px-3.5 text-[12px] font-semibold text-white hover:opacity-90"
      >
        <Icon name="arrowDownToLine" size={14} /> Jump to latest
      </button>
    {/if}
  </div>

  <!-- Detail -->
  {#if logs.selected}
    {@const e = logs.selected}
    <div class="lv-{e.level} max-h-[38%] shrink-0 overflow-y-auto border-t border-line bg-panel-2">
      <div class="flex items-center gap-2 px-4 pt-3 text-[11.5px]">
        <span class="lv-badge rounded px-1.5 py-px text-[10.5px] font-bold uppercase">{e.level}</span>
        <span class="font-semibold" style="color: hsl({hue(e.tag)} 55% var(--tag-l, 62%))">{e.tag}</span>
        <span class="mono text-faint">{e.time} · pid {e.pid} · tid {e.tid}</span>
        {#if e.subsystem}<span class="mono rounded-md border border-line px-1.5 text-[10.5px] text-muted">{e.subsystem}</span>{/if}
        <span class="flex-1"></span>
        <button onclick={() => copyLine(e)} class="flex h-7 items-center gap-1.5 rounded-lg px-2 text-[11.5px] text-muted hover:bg-hover hover:text-fg">
          <Icon name="copy" size={13} /> Copy
        </button>
        <button onclick={() => (logs.selected = null)} class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg" aria-label="Close">
          <Icon name="x" size={13} />
        </button>
      </div>
      <pre class="mono selectable px-4 pt-2 pb-4 text-[12px] break-words whitespace-pre-wrap text-fg">{e.message}</pre>
    </div>
  {/if}

  <!-- Status -->
  <div class="flex h-7 shrink-0 items-center gap-3 border-t border-line bg-panel px-3 text-[11px] text-muted">
    {#if logs.paused}
      <span class="flex items-center gap-1.5 font-medium text-warn"><Icon name="pause" size={11} /> Paused</span>
    {:else if logs.live}
      <span class="flex items-center gap-1.5 font-medium text-ok"><span class="live-dot size-1.5 rounded-full bg-ok"></span> Live</span>
    {:else if logs.ended}
      <span class="text-err">Stream ended</span>
    {:else}
      <span>Idle</span>
    {/if}
    <span class="mono">{rows.length.toLocaleString()} shown</span>
    <span class="mono text-faint">{logs.entries.length.toLocaleString()} buffered</span>
    {#if logs.rate}<span class="mono text-faint">{logs.rate.toLocaleString()} lines/s</span>{/if}
    {#if logs.dropped}<span class="mono text-warn" title="Lines discarded because they arrived faster than the view could show them">{logs.dropped.toLocaleString()} dropped</span>{/if}
    <span class="flex-1"></span>
    <span class="text-faint">Click a line for details · double-click to copy</span>
  </div>
</div>

<style>
  :global([data-theme="light"]) {
    --tag-l: 38%;
  }
</style>

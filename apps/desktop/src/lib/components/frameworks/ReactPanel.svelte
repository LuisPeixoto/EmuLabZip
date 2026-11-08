<script lang="ts">
  import { onDestroy } from "svelte";
  import { api, errorText, type ConsoleLine, type DispatchedAction, type HermesHeap, type ReactNode, type ReactState, type ReactTree, type StateChange } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";
  import Tree, { matches, type TreeNode } from "./Tree.svelte";

  type Tab = "components" | "state" | "heap" | "console";
  let tab = $state<Tab>("components");
  let busy = $state<Tab | null>(null);
  let error = $state<string | null>(null);
  let port = $state(8081);
  let seconds = $state(10);
  let needle = $state("");

  let tree = $state<ReactTree | null>(null);
  let heap = $state<HermesHeap | null>(null);
  let lines = $state<ConsoleLine[] | null>(null);

  // State: Redux, React Query, contexts and named stores. Watching follows Redux actions and what they change.
  let rstate = $state<ReactState | null>(null);
  let picked = $state<string | null>(null);
  let watching = $state(false);
  let actions = $state<DispatchedAction[]>([]);
  let changes = $state<{ at: number; list: StateChange[] }[]>([]);
  let seq = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let watchedApp = "";
  const store = $derived(rstate?.stores.find((s) => `${s.kind}:${s.name}` === picked) ?? rstate?.stores[0] ?? null);

  let owner = "";
  $effect(() => {
    const key = `${app.deviceKey}|${app.appId ?? ""}`;
    if (key !== owner) {
      owner = key;
      stopWatching();
      rstate = null;
      actions = [];
      changes = [];
      tree = null;
      heap = null;
      lines = null;
      error = null;
    }
  });

  const toNode = (n: ReactNode): TreeNode => ({
    label: n.name,
    detail: [n.key ? `key=${n.key}` : "", ...n.props].filter(Boolean).join(" "),
    own: !n.host,
    children: n.children.map(toNode),
  });
  const root = $derived(tree ? toNode(tree.root) : null);

  async function run(which: Tab, work: (app: string, port: number) => Promise<unknown>) {
    if (!app.appId) return;
    busy = which;
    error = null;
    try {
      await work(app.appId, Math.min(65535, Math.max(1, Math.round(port) || 8081)));
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  const readState = () => run("state", async (a, p) => ((rstate = await api.rnState(a, p)), (actions = []), (changes = [])));

  function stopWatching() {
    if (timer) clearTimeout(timer);
    timer = null;
    if (watching && watchedApp) {
      const a = watchedApp;
      api.rnStateStop(a, Math.min(65535, Math.max(1, Math.round(port) || 8081))).catch(() => {});
    }
    watching = false;
  }

  async function toggleWatch() {
    if (watching) return stopWatching();
    if (!app.appId) return;
    watching = true;
    watchedApp = app.appId;
    seq = 0;
    actions = [];
    changes = [];
    await poll(true);
  }

  async function poll(start: boolean) {
    if (!watching) return;
    try {
      const u = await api.rnStateWatch(watchedApp, Math.min(65535, Math.max(1, Math.round(port) || 8081)), start ? null : rstate, seq, start);
      rstate = u.state;
      for (const a of u.actions) seq = Math.max(seq, a.seq);
      actions = [...u.actions.slice().reverse(), ...actions].slice(0, 200);
      if (u.changes.length) changes = [{ at: Date.now(), list: u.changes }, ...changes].slice(0, 100);
    } catch (e) {
      error = errorText(e);
      watching = false;
      return;
    }
    timer = setTimeout(() => poll(false), 1000);
  }

  onDestroy(stopWatching);

  const short = (v: unknown) => {
    const t = JSON.stringify(v);
    return t === undefined ? "" : t.length > 70 ? t.slice(0, 70) + "…" : t;
  };

  const readTree = () => run("components", async (a, p) => (tree = await api.rnTree(a, p)));
  const readHeap = () => run("heap", async (a, p) => (heap = await api.rnHeap(a, p)));
  const listen = () =>
    run("console", async (a, p) => (lines = await api.rnConsole(a, p, Math.min(120, Math.max(1, Math.round(seconds) || 10)))));
</script>

<section class="rounded-2xl border border-line bg-panel">
  <header class="flex flex-wrap items-center gap-2 border-b border-line px-4 py-2.5">
    <h2 class="text-lg font-semibold">React Native</h2>
    <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="tablist">
      {#each [["components", "Components"], ["state", "State"], ["heap", "Hermes heap"], ["console", "Console"]] as [id, label] (id)}
        <button role="tab" aria-selected={tab === id} onclick={() => (tab = id as Tab)} class="h-7 rounded-md px-2.5 text-base {tab === id ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}">{label}</button>
      {/each}
    </div>
    <span class="flex-1"></span>
    <label class="flex items-center gap-1.5 text-sm text-muted">Metro port <input type="number" min="1" max="65535" bind:value={port} class="mono h-7 w-20 rounded-md border border-line bg-panel-2 px-1.5 text-sm text-fg outline-none focus:border-accent" /></label>
  </header>
  <p class="border-b border-line bg-panel-2 px-4 py-1.5 text-sm text-muted">Metro lets one debugger connect at a time: reading from here can close React Native DevTools or Chrome's inspector.</p>

  {#if error}
    <p class="flex items-start gap-2 border-b border-line bg-err/8 px-4 py-2.5 text-md text-err"><Icon name="alert" size={14} class="mt-px shrink-0" />{error}</p>
  {/if}

  {#if tab === "components"}
    <div class="flex items-center gap-2 border-b border-line px-4 py-2">
      <button onclick={readTree} disabled={busy != null || !app.appId} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name={busy === "components" ? "refresh" : "download"} size={12} class={busy === "components" ? "animate-spin" : ""} />{tree ? "Read again" : "Read the component tree"}
      </button>
      {#if tree}
        <label class="relative flex flex-1 items-center">
          <Icon name="search" size={12} class="pointer-events-none absolute left-2.5 text-faint" />
          <input bind:value={needle} spellcheck="false" placeholder="Find a component or prop" class="h-7 w-full rounded-md border border-line bg-panel-2 pr-2 pl-7 text-base outline-none placeholder:text-faint focus:border-accent" />
        </label>
        <span class="mono text-xs text-faint">{tree.count} components</span>
      {/if}
    </div>
    {#if root}
      <div class="selectable max-h-[520px] overflow-auto py-1">
        {#if needle && !matches(root, needle.toLowerCase())}
          <p class="px-4 py-6 text-center text-base text-muted">Nothing matches.</p>
        {:else}
          <Tree node={root} needle={needle.trim().toLowerCase()} />
        {/if}
      </div>
    {:else if !busy}
      <p class="px-4 py-8 text-center text-md text-muted">Reads the React components on screen with their props. Needs a development build running with Metro.</p>
    {/if}
  {:else if tab === "state"}
    <div class="flex items-center gap-2 border-b border-line px-4 py-2">
      <button onclick={readState} disabled={busy != null || watching || !app.appId} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name={busy === "state" ? "refresh" : "download"} size={12} class={busy === "state" ? "animate-spin" : ""} />{rstate ? "Read again" : "Read the state"}
      </button>
      <button onclick={toggleWatch} disabled={busy != null || !app.appId} title="Follow Redux actions and what they change. It wraps dispatch until you stop." class="flex h-7 items-center gap-1.5 rounded-lg border px-3 text-base font-medium {watching ? 'border-ok bg-ok/10 text-ok' : 'border-line text-muted hover:bg-hover hover:text-fg'}">
        <Icon name={watching ? "stop" : "activity"} size={12} />{watching ? "Watching… stop" : "Watch changes"}
      </button>
    </div>
    {#if rstate}
      <div class="flex min-h-[260px]">
        <nav class="w-[190px] shrink-0 overflow-y-auto border-r border-line bg-panel-2 p-1.5">
          {#each rstate.stores as s (s.kind + s.name)}
            <button onclick={() => (picked = `${s.kind}:${s.name}`)} class="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-base {store && store.kind === s.kind && store.name === s.name ? 'bg-accent-soft font-medium text-accent' : 'hover:bg-hover'}">
              <span class="min-w-0 flex-1 truncate">{s.name}</span>
              <span class="rounded bg-hover px-1 text-tiny text-muted">{s.kind}</span>
            </button>
          {:else}
            <p class="p-3 text-base text-muted">Nothing found.</p>
          {/each}
        </nav>
        <div class="selectable max-h-[460px] min-w-0 flex-1 overflow-auto">
          {#if store?.error}
            <p class="px-4 py-3 text-base text-err">Could not be read: {store.error}</p>
          {:else if store}
            <pre class="mono px-4 py-3 text-sm leading-relaxed whitespace-pre-wrap">{JSON.stringify(store.state, null, 2)}</pre>
            {#each store.cut as c (c)}<p class="mono px-4 pb-1 text-2xs text-faint">cut {c}</p>{/each}
          {/if}
        </div>
      </div>
      {#if watching || actions.length || changes.length}
        <div class="grid grid-cols-2 border-t border-line">
          <div class="max-h-[240px] overflow-auto border-r border-line">
            <p class="sticky top-0 bg-panel-2 px-4 py-1.5 text-xs font-medium text-muted">Redux actions</p>
            {#each actions as a (a.seq)}
              <p class="mono px-4 py-0.5 text-sm" title={short(a.action)}><span class="text-faint">{a.seq}</span> {a.kind} <span class="text-faint">{short((a.action as { payload?: unknown })?.payload)}</span></p>
            {:else}
              <p class="px-4 py-4 text-base text-muted">{watching ? "Waiting for an action… use the app." : "None."}</p>
            {/each}
          </div>
          <div class="max-h-[240px] overflow-auto">
            <p class="sticky top-0 bg-panel-2 px-4 py-1.5 text-xs font-medium text-muted">What changed</p>
            {#each changes as c (c.at)}
              {#each c.list as ch, i (i)}
                <p class="mono px-4 py-0.5 text-sm {ch.before == null ? 'text-ok' : ch.after == null ? 'text-err' : 'text-warn'}">{ch.path}: {ch.before == null ? "" : short(ch.before) + " → "}{ch.after == null ? "removed" : short(ch.after)}</p>
              {/each}
            {:else}
              <p class="px-4 py-4 text-base text-muted">{watching ? "Nothing has changed yet." : "None."}</p>
            {/each}
          </div>
        </div>
      {/if}
    {:else if !busy}
      <p class="px-4 py-8 text-center text-md text-muted">Reads the state of a development build: Redux stores, React Query caches, React contexts, and stores the app registers in <span class="mono">globalThis.__EMULABZIP_STORES__</span>.</p>
    {/if}
  {:else if tab === "heap"}
    <div class="flex items-center gap-2 border-b border-line px-4 py-2">
      <button onclick={readHeap} disabled={busy != null || !app.appId} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name={busy === "heap" ? "refresh" : "cpu"} size={12} class={busy === "heap" ? "animate-spin" : ""} />{heap ? "Read again" : "Read Hermes' heap"}
      </button>
    </div>
    {#if heap}
      <div class="grid grid-cols-2 gap-3 px-4 py-3">
        <div><p class="text-xs text-muted">In use</p><p class="mono text-2xl font-semibold">{bytes(heap.usedBytes)}</p></div>
        <div><p class="text-xs text-muted">Heap size</p><p class="mono text-2xl font-semibold">{bytes(heap.totalBytes)}</p></div>
      </div>
      {#if heap.runtime}<p class="mono border-t border-line px-4 py-2 text-sm text-muted">Hermes {heap.runtime}</p>{/if}
    {:else if !busy}
      <p class="px-4 py-8 text-center text-md text-muted">Shows how much memory the JavaScript heap of Hermes uses.</p>
    {/if}
  {:else}
    <div class="flex items-center gap-2 border-b border-line px-4 py-2">
      <label class="flex items-center gap-1.5 text-sm text-muted">Seconds <input type="number" min="1" max="120" bind:value={seconds} class="mono h-7 w-16 rounded-md border border-line bg-panel-2 px-1.5 text-sm text-fg outline-none focus:border-accent" /></label>
      <button onclick={listen} disabled={busy != null || !app.appId} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name={busy === "console" ? "refresh" : "play"} size={12} class={busy === "console" ? "animate-spin" : ""} />{busy === "console" ? "Listening… use the app" : "Listen to the console"}
      </button>
    </div>
    {#if lines}
      <div class="selectable max-h-[420px] overflow-auto py-1">
        {#each lines as l, i (i)}
          <p class="mono flex gap-3 px-4 py-0.5 text-sm"><span class="w-12 shrink-0 text-faint">{l.level}</span><span class="min-w-0 break-words whitespace-pre-wrap {l.level === 'error' ? 'text-err' : l.level === 'warn' ? 'text-warn' : ''}">{l.text}</span></p>
        {:else}
          <p class="px-4 py-6 text-center text-base text-muted">Nothing was written to the console.</p>
        {/each}
      </div>
    {:else if !busy}
      <p class="px-4 py-8 text-center text-md text-muted">Shows what the app writes to the JavaScript console while you use it.</p>
    {/if}
  {/if}
</section>

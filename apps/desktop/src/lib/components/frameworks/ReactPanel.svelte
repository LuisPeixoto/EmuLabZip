<script lang="ts">
  import { api, errorText, type ConsoleLine, type HermesHeap, type ReactNode, type ReactTree } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";
  import Tree, { matches, type TreeNode } from "./Tree.svelte";

  type Tab = "components" | "heap" | "console";
  let tab = $state<Tab>("components");
  let busy = $state<Tab | null>(null);
  let error = $state<string | null>(null);
  let port = $state(8081);
  let seconds = $state(10);
  let needle = $state("");

  let tree = $state<ReactTree | null>(null);
  let heap = $state<HermesHeap | null>(null);
  let lines = $state<ConsoleLine[] | null>(null);

  let owner = "";
  $effect(() => {
    const key = `${app.deviceKey}|${app.appId ?? ""}`;
    if (key !== owner) {
      owner = key;
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

  const readTree = () => run("components", async (a, p) => (tree = await api.rnTree(a, p)));
  const readHeap = () => run("heap", async (a, p) => (heap = await api.rnHeap(a, p)));
  const listen = () =>
    run("console", async (a, p) => (lines = await api.rnConsole(a, p, Math.min(120, Math.max(1, Math.round(seconds) || 10)))));
</script>

<section class="rounded-2xl border border-line bg-panel">
  <header class="flex flex-wrap items-center gap-2 border-b border-line px-4 py-2.5">
    <h2 class="text-[13px] font-semibold">React Native</h2>
    <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="tablist">
      {#each [["components", "Components"], ["heap", "Hermes heap"], ["console", "Console"]] as [id, label] (id)}
        <button role="tab" aria-selected={tab === id} onclick={() => (tab = id as Tab)} class="h-7 rounded-md px-2.5 text-[12px] {tab === id ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}">{label}</button>
      {/each}
    </div>
    <span class="flex-1"></span>
    <label class="flex items-center gap-1.5 text-[11.5px] text-muted">Metro port <input type="number" min="1" max="65535" bind:value={port} class="mono h-7 w-20 rounded-md border border-line bg-panel-2 px-1.5 text-[11.5px] text-fg outline-none focus:border-accent" /></label>
  </header>
  <p class="border-b border-line bg-panel-2 px-4 py-1.5 text-[11.5px] text-muted">Metro lets one debugger connect at a time: reading from here can close React Native DevTools or Chrome's inspector.</p>

  {#if error}
    <p class="flex items-start gap-2 border-b border-line bg-err/8 px-4 py-2.5 text-[12.5px] text-err"><Icon name="alert" size={14} class="mt-px shrink-0" />{error}</p>
  {/if}

  {#if tab === "components"}
    <div class="flex items-center gap-2 border-b border-line px-4 py-2">
      <button onclick={readTree} disabled={busy != null || !app.appId} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name={busy === "components" ? "refresh" : "download"} size={12} class={busy === "components" ? "animate-spin" : ""} />{tree ? "Read again" : "Read the component tree"}
      </button>
      {#if tree}
        <label class="relative flex flex-1 items-center">
          <Icon name="search" size={12} class="pointer-events-none absolute left-2.5 text-faint" />
          <input bind:value={needle} spellcheck="false" placeholder="Find a component or prop" class="h-7 w-full rounded-md border border-line bg-panel-2 pr-2 pl-7 text-[12px] outline-none placeholder:text-faint focus:border-accent" />
        </label>
        <span class="mono text-[11px] text-faint">{tree.count} components</span>
      {/if}
    </div>
    {#if root}
      <div class="selectable max-h-[520px] overflow-auto py-1">
        {#if needle && !matches(root, needle.toLowerCase())}
          <p class="px-4 py-6 text-center text-[12px] text-muted">Nothing matches.</p>
        {:else}
          <Tree node={root} needle={needle.trim().toLowerCase()} />
        {/if}
      </div>
    {:else if !busy}
      <p class="px-4 py-8 text-center text-[12.5px] text-muted">Reads the React components on screen with their props. Needs a development build running with Metro.</p>
    {/if}
  {:else if tab === "heap"}
    <div class="flex items-center gap-2 border-b border-line px-4 py-2">
      <button onclick={readHeap} disabled={busy != null || !app.appId} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name={busy === "heap" ? "refresh" : "cpu"} size={12} class={busy === "heap" ? "animate-spin" : ""} />{heap ? "Read again" : "Read Hermes' heap"}
      </button>
    </div>
    {#if heap}
      <div class="grid grid-cols-2 gap-3 px-4 py-3">
        <div><p class="text-[11px] text-muted">In use</p><p class="mono text-[15px] font-semibold">{bytes(heap.usedBytes)}</p></div>
        <div><p class="text-[11px] text-muted">Heap size</p><p class="mono text-[15px] font-semibold">{bytes(heap.totalBytes)}</p></div>
      </div>
      {#if heap.runtime}<p class="mono border-t border-line px-4 py-2 text-[11.5px] text-muted">Hermes {heap.runtime}</p>{/if}
    {:else if !busy}
      <p class="px-4 py-8 text-center text-[12.5px] text-muted">Shows how much memory the JavaScript heap of Hermes uses.</p>
    {/if}
  {:else}
    <div class="flex items-center gap-2 border-b border-line px-4 py-2">
      <label class="flex items-center gap-1.5 text-[11.5px] text-muted">Seconds <input type="number" min="1" max="120" bind:value={seconds} class="mono h-7 w-16 rounded-md border border-line bg-panel-2 px-1.5 text-[11.5px] text-fg outline-none focus:border-accent" /></label>
      <button onclick={listen} disabled={busy != null || !app.appId} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name={busy === "console" ? "refresh" : "play"} size={12} class={busy === "console" ? "animate-spin" : ""} />{busy === "console" ? "Listening… use the app" : "Listen to the console"}
      </button>
    </div>
    {#if lines}
      <div class="selectable max-h-[420px] overflow-auto py-1">
        {#each lines as l, i (i)}
          <p class="mono flex gap-3 px-4 py-0.5 text-[11.5px]"><span class="w-12 shrink-0 text-faint">{l.level}</span><span class="min-w-0 break-words whitespace-pre-wrap {l.level === 'error' ? 'text-err' : l.level === 'warn' ? 'text-warn' : ''}">{l.text}</span></p>
        {:else}
          <p class="px-4 py-6 text-center text-[12px] text-muted">Nothing was written to the console.</p>
        {/each}
      </div>
    {:else if !busy}
      <p class="px-4 py-8 text-center text-[12.5px] text-muted">Shows what the app writes to the JavaScript console while you use it.</p>
    {/if}
  {/if}
</section>

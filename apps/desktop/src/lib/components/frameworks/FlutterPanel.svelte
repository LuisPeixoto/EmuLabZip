<script lang="ts">
  import { api, errorText, type FlutterMemory, type FlutterTree, type FrameStats, type WidgetNode } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";
  import Tree, { matches, type TreeNode } from "./Tree.svelte";

  type Tab = "widgets" | "frames" | "memory";
  let tab = $state<Tab>("widgets");
  let busy = $state<Tab | null>(null);
  let error = $state<string | null>(null);
  let uri = $state("");
  let seconds = $state(10);
  let needle = $state("");

  let tree = $state<FlutterTree | null>(null);
  let frames = $state<FrameStats | null>(null);
  let memory = $state<FlutterMemory | null>(null);

  // Results belong to one app on one device.
  let owner = "";
  $effect(() => {
    const key = `${app.deviceKey}|${app.appId ?? ""}`;
    if (key !== owner) {
      owner = key;
      tree = null;
      frames = null;
      memory = null;
      error = null;
    }
  });

  const toNode = (w: WidgetNode): TreeNode => ({
    label: w.description || w.widgetType || "widget",
    detail: w.file ? `${w.file}${w.line ? `:${w.line}` : ""}` : undefined,
    own: w.local,
    children: w.children.map(toNode),
  });
  const root = $derived(tree ? toNode(tree.root) : null);

  async function run(which: Tab, work: (target: NonNullable<typeof app.target>, address: string | null) => Promise<unknown>) {
    if (!app.target) return;
    busy = which;
    error = null;
    try {
      await work(app.target, uri.trim() || null);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  const readTree = () => run("widgets", async (t, u) => (tree = await api.flutterTree(t, u)));
  const record = () =>
    run("frames", async (t, u) => (frames = await api.flutterFrames(t, u, Math.min(120, Math.max(1, Math.round(seconds) || 10)))));
  const readMemory = () => run("memory", async (t, u) => (memory = await api.flutterMemory(t, u)));

  const ms = (us: number) => `${(us / 1000).toFixed(1)} ms`;
  const scale = $derived(Math.max(33_334, ...(frames?.timeline.map((f) => Math.max(f.buildUs, f.rasterUs)) ?? [0])));
</script>

<section class="rounded-2xl border border-line bg-panel">
  <header class="flex flex-wrap items-center gap-2 border-b border-line px-4 py-2.5">
    <h2 class="text-[13px] font-semibold">Flutter</h2>
    <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="tablist">
      {#each [["widgets", "Widgets"], ["frames", "Frames"], ["memory", "Memory"]] as [id, label] (id)}
        <button role="tab" aria-selected={tab === id} onclick={() => (tab = id as Tab)} class="h-7 rounded-md px-2.5 text-[12px] {tab === id ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}">{label}</button>
      {/each}
    </div>
    <span class="flex-1"></span>
    <input bind:value={uri} spellcheck="false" placeholder="VM service address (found in the log when empty)" aria-label="VM service address" class="mono h-7 w-[300px] max-w-full rounded-md border border-line bg-panel-2 px-2 text-[11.5px] outline-none placeholder:text-faint focus:border-accent" />
  </header>

  {#if error}
    <p class="flex items-start gap-2 border-b border-line bg-err/8 px-4 py-2.5 text-[12.5px] text-err"><Icon name="alert" size={14} class="mt-px shrink-0" />{error}</p>
  {/if}

  {#if tab === "widgets"}
    <div class="flex items-center gap-2 border-b border-line px-4 py-2">
      <button onclick={readTree} disabled={busy != null} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name={busy === "widgets" ? "refresh" : "download"} size={12} class={busy === "widgets" ? "animate-spin" : ""} />{tree ? "Read again" : "Read the widget tree"}
      </button>
      {#if tree}
        <label class="relative flex flex-1 items-center">
          <Icon name="search" size={12} class="pointer-events-none absolute left-2.5 text-faint" />
          <input bind:value={needle} spellcheck="false" placeholder="Find a widget or file" class="h-7 w-full rounded-md border border-line bg-panel-2 pr-2 pl-7 text-[12px] outline-none placeholder:text-faint focus:border-accent" />
        </label>
        <span class="mono text-[11px] text-faint">{tree.count} widgets</span>
      {/if}
    </div>
    {#if root}
      {#if !tree?.tracked}
        <p class="border-b border-line bg-warn/10 px-4 py-2 text-[11.5px] text-warn">This build does not record where widgets were created, so there are no file names and the app's own widgets cannot be told apart. Apps started with <span class="mono">flutter run</span> do.</p>
      {/if}
      <div class="selectable max-h-[520px] overflow-auto py-1">
        {#if needle && !matches(root, needle.toLowerCase())}
          <p class="px-4 py-6 text-center text-[12px] text-muted">Nothing matches.</p>
        {:else}
          <Tree node={root} needle={needle.trim().toLowerCase()} />
        {/if}
      </div>
    {:else if !busy}
      <p class="px-4 py-8 text-center text-[12.5px] text-muted">Reads the widgets of the app on screen, as the Flutter inspector shows them. Needs a debug or profile build.</p>
    {/if}
  {:else if tab === "frames"}
    <div class="flex items-center gap-2 border-b border-line px-4 py-2">
      <label class="flex items-center gap-1.5 text-[11.5px] text-muted">Seconds <input type="number" min="1" max="120" bind:value={seconds} class="mono h-7 w-16 rounded-md border border-line bg-panel-2 px-1.5 text-[11.5px] text-fg outline-none focus:border-accent" /></label>
      <button onclick={record} disabled={busy != null} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name={busy === "frames" ? "refresh" : "play"} size={12} class={busy === "frames" ? "animate-spin" : ""} />{busy === "frames" ? "Recording… use the app" : "Record frames"}
      </button>
    </div>
    {#if frames}
      {#if !frames.frames}
        <p class="px-4 py-8 text-center text-[12.5px] text-muted">No frames were drawn. Use the app while it records: an idle screen draws nothing.</p>
      {:else}
        <div class="grid grid-cols-2 gap-3 border-b border-line px-4 py-3 md:grid-cols-4">
          {#each [["Frames", String(frames.frames)], ["Janky", `${frames.janky} (${frames.jankPercent.toFixed(1)}%)`], ["Build, average", ms(frames.avgBuildUs)], ["Raster, average", ms(frames.avgRasterUs)]] as [k, v] (k)}
            <div><p class="text-[11px] text-muted">{k}</p><p class="mono text-[15px] font-semibold">{v}</p></div>
          {/each}
        </div>
        <div class="px-4 py-3">
          <div class="relative flex h-[120px] items-end gap-px overflow-hidden rounded-lg border border-line bg-panel-2 px-1" role="img" aria-label="Build and raster time of each frame">
            <span class="pointer-events-none absolute right-0 left-0 border-t border-dashed border-warn/70" style="bottom:{(16_667 / scale) * 100}%" title="16.7 ms: one frame at 60 Hz"></span>
            {#each frames.timeline as f (f.number)}
              <span class="flex min-w-px flex-1 flex-col justify-end" title="#{f.number}: build {ms(f.buildUs)}, raster {ms(f.rasterUs)}">
                <span class="block w-full bg-series-2" style="height:{Math.max(1, (f.rasterUs / scale) * 116)}px"></span>
                <span class="block w-full {f.buildUs > 16_667 ? 'bg-err' : 'bg-series-1'}" style="height:{Math.max(1, (f.buildUs / scale) * 116)}px"></span>
              </span>
            {/each}
          </div>
          <p class="mt-1.5 text-[11px] text-faint">Each bar is a frame: build time (purple, red when over 16.7 ms) at the bottom, raster time (teal) above it. The dashed line is 16.7 ms.</p>
        </div>
        <div class="border-t border-line px-4 py-2.5">
          <p class="mb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Slowest frames</p>
          {#each frames.slowest as f (f.number)}
            <p class="mono text-[11.5px]">#{f.number}  build {ms(f.buildUs)}  raster {ms(f.rasterUs)}</p>
          {/each}
        </div>
      {/if}
    {:else if !busy}
      <p class="px-4 py-8 text-center text-[12.5px] text-muted">Records how long the app takes to build and draw each frame while you use it.</p>
    {/if}
  {:else}
    <div class="flex items-center gap-2 border-b border-line px-4 py-2">
      <button onclick={readMemory} disabled={busy != null} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">
        <Icon name={busy === "memory" ? "refresh" : "cpu"} size={12} class={busy === "memory" ? "animate-spin" : ""} />{memory ? "Read again" : "Read the Dart heap"}
      </button>
    </div>
    {#if memory}
      <div class="grid grid-cols-3 gap-3 border-b border-line px-4 py-3">
        {#each [["Heap in use", bytes(memory.heapUsed)], ["Heap size", bytes(memory.heapCapacity)], ["Outside the heap", bytes(memory.external)]] as [k, v] (k)}
          <div><p class="text-[11px] text-muted">{k}</p><p class="mono text-[15px] font-semibold">{v}</p></div>
        {/each}
      </div>
      <table class="w-full text-[12px]">
        <tbody>
          {#each memory.classes as c (c.name + (c.library ?? ""))}
            <tr class="border-b border-line/60 last:border-0">
              <td class="mono w-[90px] px-4 py-1.5 text-right font-medium">{bytes(c.bytes)}</td>
              <td class="mono w-[90px] px-2 py-1.5 text-right text-faint">{c.instances.toLocaleString()}×</td>
              <td class="mono px-4 py-1.5">{c.name}{#if c.library}<span class="ml-2 text-faint">{c.library}</span>{/if}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else if !busy}
      <p class="px-4 py-8 text-center text-[12.5px] text-muted">Shows how much the Dart heap holds and which classes use the most.</p>
    {/if}
  {/if}
</section>

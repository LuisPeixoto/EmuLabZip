<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, errorText, type FlowDoc, type FlowEntry, type FlowMessage, type FlowStep } from "../../api";
  import { describeStep } from "../../flow";
  import { app } from "../../stores/app.svelte";
  import { arrange } from "../../stores/arrange.svelte";
  import Icon from "../Icon.svelte";

  const KEYS = ["back", "home", "app_switch", "enter", "delete", "tab", "volume_up", "volume_down"];

  let flows = $state<FlowEntry[]>([]);
  let selected = $state<string | null>(null);
  let doc = $state<FlowDoc | null>(null);
  let dirty = $state(false);
  let error = $state<string | null>(null);

  // Recording.
  let name = $state("My flow");
  let launch = $state(true);
  let recording = $state(false);
  let recorded = $state<FlowStep[]>([]);
  let recordSession: number | null = null;

  // Playing.
  let playing = $state<string | null>(null);
  let playIndex = $state(-1);
  let playSession: number | null = null;

  // Adding a step by hand.
  let addText = $state("");
  let addKey = $state("back");
  let addWait = $state(1);

  const android = $derived(app.device?.platform === "android");
  const ready = $derived(!!app.target && app.online && android);

  async function load(keep?: string) {
    flows = await api.flowList();
    const id = keep ?? selected;
    if (id && flows.some((f) => f.id === id)) await open(id);
    else if (!flows.some((f) => f.id === selected)) {
      selected = null;
      doc = null;
    }
  }

  async function open(id: string) {
    try {
      doc = await api.flowGet(id);
      selected = id;
      dirty = false;
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(() => void load().catch((e) => (error = errorText(e))));
  onDestroy(() => {
    if (recording) void api.flowRecordStop();
    if (playSession !== null) void api.stopSession(playSession);
  });

  async function startRecording() {
    const target = app.target;
    if (!target) return;
    error = null;
    recorded = [];
    try {
      recordSession = await api.flowRecordStart(target, name.trim() || "My flow", app.appId, launch && !!app.appId, async (m: FlowMessage) => {
        if (m.type === "step") recorded = [...recorded, m.step];
        else if (m.type === "saved") {
          recording = false;
          app.toast(`Saved ${m.steps} steps`, "ok");
          await load(m.id);
        } else if (m.type === "empty") {
          recording = false;
          app.toast("Nothing was touched, so nothing was saved", "info");
        } else if (m.type === "failed") {
          recording = false;
          error = m.message;
        }
      });
      recording = true;
    } catch (e) {
      error = errorText(e);
    }
  }

  async function stopRecording() {
    await api.flowRecordStop();
  }

  async function play(id: string) {
    const target = app.target;
    if (!target) return;
    error = null;
    playing = id;
    playIndex = -1;
    try {
      playSession = await api.flowPlay(target, id, (m) => {
        if (m.type === "step") playIndex = m.index;
        else {
          playing = null;
          playIndex = -1;
          playSession = null;
          if (m.type === "failed") error = m.message;
          if (m.type === "done") app.toast("Played to the end", "ok");
        }
      });
    } catch (e) {
      playing = null;
      error = errorText(e);
    }
  }

  async function stopPlaying() {
    const id = playSession;
    playSession = null;
    playing = null;
    playIndex = -1;
    if (id !== null) await api.stopSession(id).catch(() => {});
  }

  async function exportAs(id: string, kind: "maestro" | "script") {
    try {
      const path = await api.flowExport(id, kind);
      if (path) app.toast(`Saved ${path}`, "ok");
    } catch (e) {
      error = errorText(e);
    }
  }

  async function remove(id: string) {
    try {
      await api.flowRemove(id);
      if (selected === id) {
        selected = null;
        doc = null;
      }
      await load();
    } catch (e) {
      error = errorText(e);
    }
  }

  function add(step: FlowStep) {
    if (!doc) return;
    doc.steps = [...doc.steps, step];
    dirty = true;
  }

  function dropStep(i: number) {
    if (!doc) return;
    doc.steps = doc.steps.filter((_, n) => n !== i);
    dirty = true;
  }

  async function save() {
    if (!doc) return;
    try {
      const id = await api.flowSave($state.snapshot(doc));
      dirty = false;
      app.toast("Saved", "ok");
      await load(id);
    } catch (e) {
      error = errorText(e);
    }
  }

  const field = "h-8 rounded-lg border border-line bg-panel-2 px-2.5 text-base outline-none focus:border-accent";
  const primary = "flex h-8 items-center justify-center gap-1.5 rounded-lg bg-accent px-3.5 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40";
  const ghost = "flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover disabled:opacity-40";
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto {arrange.pageMid} space-y-4 p-6">
    <header>
      <h1 class="text-3xl font-semibold tracking-tight">Automation</h1>
      <p class="text-md text-muted">Record what you do on the device, play it again, and take it to Maestro or a script.</p>
    </header>

    {#if error}
      <div class="flex items-center gap-2 rounded-xl border border-err/30 bg-err/8 px-4 py-3 text-md text-err"><Icon name="alert" size={14} />{error}</div>
    {/if}

    {#if app.device && !android}
      <div class="rounded-2xl border border-dashed border-line-strong bg-panel/50 px-6 py-10 text-center">
        <p class="text-xl font-medium">Automation is for Android</p>
        <p class="mx-auto mt-1 max-w-[460px] text-md text-muted">iOS has no public way to send touches or to read them, so flows can only be recorded and played on Android emulators and devices.</p>
      </div>
    {:else}
      <section class="rounded-2xl border border-line bg-panel p-5">
        <h2 class="mb-1 text-lg font-semibold">Record</h2>
        <p class="mb-3 text-sm text-muted">
          Touch the device itself: on an emulator, its own window. Taps sent from EmuLabZip (the Interact switch, or Play) go in at a higher level and are not seen as touches.
          Typed text is not recorded; add it as a step afterwards.
        </p>
        <div class="flex flex-wrap items-center gap-2">
          <input class="{field} w-56" bind:value={name} placeholder="Name" aria-label="Name of the flow" disabled={recording} maxlength="100" />
          <label class="flex items-center gap-1.5 text-base text-muted" title={app.appId ? "" : "Pick an app in the toolbar to start it"}>
            <input type="checkbox" bind:checked={launch} disabled={recording || !app.appId} class="accent-[var(--accent)]" />
            Start {app.selectedApp?.name ?? "the app"} first
          </label>
          <span class="flex-1"></span>
          {#if recording}
            <button class={primary} onclick={stopRecording}><Icon name="stop" size={12} />Stop and save</button>
          {:else}
            <button class={primary} onclick={startRecording} disabled={!ready || !name.trim()}><Icon name="activity" size={12} />Record</button>
          {/if}
        </div>
        {#if recording}
          <p class="mt-3 flex items-center gap-2 text-md text-ok"><span class="live-dot size-2 rounded-full bg-ok"></span>Recording… {recorded.length} steps so far</p>
          <ol class="selectable mt-2 max-h-[180px] space-y-0.5 overflow-auto text-base text-muted">
            {#each recorded as s, i (i)}<li>{describeStep(s)}</li>{/each}
          </ol>
        {/if}
      </section>

      <section class="rounded-2xl border border-line bg-panel">
        <header class="flex items-center gap-2 border-b border-line px-5 py-3">
          <h2 class="flex-1 text-lg font-semibold">Saved flows</h2>
          <button class={ghost} onclick={() => load()}><Icon name="refresh" size={12} />Refresh</button>
        </header>
        {#if !flows.length}
          <p class="px-5 py-8 text-center text-md text-muted">No flows yet. Record one above, or with <span class="mono">emulabzip flow record</span>.</p>
        {:else}
          <ul class="divide-y divide-line">
            {#each flows as f (f.id)}
              <li class="flex flex-wrap items-center gap-2 px-5 py-2.5 {selected === f.id ? 'bg-accent-soft' : ''}">
                <button class="min-w-0 flex-1 text-left" onclick={() => open(f.id)}>
                  <span class="block truncate text-md font-medium">{f.name}</span>
                  <span class="mono block truncate text-xs text-faint">{f.app ?? "no app"} · {f.steps} steps</span>
                </button>
                {#if playing === f.id}
                  <button class={ghost} onclick={stopPlaying}><Icon name="stop" size={12} />Stop</button>
                {:else}
                  <button class={ghost} onclick={() => play(f.id)} disabled={!ready || playing !== null}><Icon name="play" size={12} />Play</button>
                {/if}
                <button class={ghost} onclick={() => exportAs(f.id, "maestro")} title="Save as a Maestro flow (.yaml)">Maestro</button>
                <button class={ghost} onclick={() => exportAs(f.id, "script")} title="Save as a shell script that uses emulabzip control">Script</button>
                <button class="grid size-8 place-items-center rounded-lg text-faint hover:bg-hover hover:text-err" onclick={() => remove(f.id)} aria-label="Delete {f.name}"><Icon name="trash" size={13} /></button>
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      {#if doc}
        <section class="rounded-2xl border border-line bg-panel">
          <header class="flex items-center gap-2 border-b border-line px-5 py-3">
            <h2 class="flex-1 text-lg font-semibold">{doc.name}</h2>
            {#if doc.width && doc.height}<span class="mono text-xs text-faint">recorded on {doc.width}×{doc.height}</span>{/if}
            {#if dirty}<button class={primary} onclick={save}>Save changes</button>{/if}
          </header>
          <ol class="selectable divide-y divide-line">
            {#each doc.steps as s, i (i)}
              <li class="flex items-center gap-3 px-5 py-1.5 text-md {playing === selected && playIndex === i ? 'bg-ok/10' : ''}">
                <span class="mono w-8 text-right text-xs text-faint">{i + 1}</span>
                <span class="flex-1">{describeStep(s)}</span>
                <button class="grid size-6 place-items-center rounded-md text-faint hover:bg-hover hover:text-err" onclick={() => dropStep(i)} aria-label="Remove step {i + 1}"><Icon name="x" size={12} /></button>
              </li>
            {:else}
              <li class="px-5 py-6 text-center text-md text-muted">This flow has no steps.</li>
            {/each}
          </ol>
          <div class="flex flex-wrap items-center gap-2 border-t border-line bg-panel-2 px-5 py-3">
            <span class="text-sm text-muted">Add a step</span>
            <form class="flex gap-1.5" onsubmit={(e) => (e.preventDefault(), addText && (add({ type: "text", text: addText }), (addText = "")))}>
              <input class="{field} mono w-44" bind:value={addText} placeholder="Text to type" aria-label="Text to type" maxlength="1000" />
              <button class={ghost} type="submit" disabled={!addText}>Type</button>
            </form>
            <select class={field} bind:value={addKey} aria-label="Key">{#each KEYS as k (k)}<option value={k}>{k.replaceAll("_", " ")}</option>{/each}</select>
            <button class={ghost} onclick={() => add({ type: "key", key: addKey })}>Press</button>
            <input class="{field} w-16 text-right" type="number" min="0.1" max="60" step="0.5" bind:value={addWait} aria-label="Seconds to wait" />
            <button class={ghost} onclick={() => add({ type: "wait", ms: Math.round(Math.min(60, Math.max(0.1, addWait)) * 1000) })}>Wait (s)</button>
          </div>
        </section>
      {/if}
    {/if}
  </div>
</div>

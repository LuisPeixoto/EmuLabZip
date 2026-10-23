<script lang="ts">
  import type { PerfettoPreset } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { MAX_DURATION, SAMPLE_CONFIG, traces, type RecorderKind } from "../../stores/traces.svelte";
  import { clock } from "../../format";
  import Icon from "../Icon.svelte";

  const kinds: Record<RecorderKind, { label: string; icon: string; hint: string }> = {
    perfetto: { label: "System trace", icon: "route", hint: "Perfetto: scheduling, frames, memory and more, for the whole device" },
    cpu: { label: "CPU profile", icon: "activity", hint: "simpleperf: where the CPU time of an app (or the system) goes, as a flame graph" },
    instruments: { label: "Instruments", icon: "gauge", hint: "xctrace: record any Instruments template from the simulator" },
  };

  const presets: { id: PerfettoPreset; label: string; detail: string; icon: string }[] = [
    { id: "cpu", label: "CPU & scheduling", detail: "Which threads ran where, CPU frequency and idle", icon: "cpu" },
    { id: "frames", label: "Frames & jank", detail: "Frame timeline from SurfaceFlinger, with the work behind each frame", icon: "layers" },
    { id: "memory", label: "Memory", detail: "Memory counters over time, plus native allocations of the app", icon: "database" },
    { id: "custom", label: "Custom", detail: "Your own Perfetto text configuration", icon: "code" },
  ];

  const s = traces.settings;
  const fraction = $derived(Math.min(1, traces.elapsedMs / (s.durationS * 1000)));
  const canRecord = $derived(app.online && !traces.recordingNow);
  const appName = $derived(app.selectedApp?.name ?? null);

  $effect(() => {
    // The recorder follows the device: what it can record, and its templates.
    app.deviceKey;
    traces.syncDevice();
  });
</script>

<section class="rounded-2xl border border-line bg-panel p-4">
  <h2 class="text-[13.5px] font-semibold">Record</h2>

  {#if traces.kinds.length > 1}
    <div class="mt-3 grid grid-cols-2 gap-1 rounded-xl bg-panel-2 p-1" role="tablist">
      {#each traces.kinds as k (k)}
        <button
          role="tab"
          aria-selected={s.kind === k}
          disabled={traces.recordingNow}
          onclick={() => (s.kind = k)}
          class="flex h-8 items-center justify-center gap-1.5 rounded-lg text-[12px] font-medium disabled:opacity-60 {s.kind === k ? 'bg-panel text-fg shadow-sm' : 'text-muted hover:text-fg'}"
        >
          <Icon name={kinds[k].icon} size={13} />{kinds[k].label}
        </button>
      {/each}
    </div>
  {/if}
  <p class="mt-2 text-[11.5px] text-muted">{kinds[s.kind].hint}</p>

  {#if s.kind === "perfetto"}
    <div class="mt-3 space-y-1.5" role="radiogroup" aria-label="Preset">
      {#each presets as p (p.id)}
        <button
          role="radio"
          aria-checked={s.preset === p.id}
          disabled={traces.recordingNow}
          onclick={() => (s.preset = p.id)}
          class="flex w-full items-start gap-2.5 rounded-xl border px-3 py-2 text-left disabled:opacity-60 {s.preset === p.id ? 'border-accent bg-accent-soft' : 'border-line hover:border-line-strong'}"
        >
          <Icon name={p.icon} size={15} class="mt-0.5 {s.preset === p.id ? 'text-accent' : 'text-faint'}" />
          <span class="min-w-0">
            <span class="block text-[12.5px] font-medium">{p.label}</span>
            <span class="block text-[11.5px] leading-snug text-muted">{p.detail}</span>
          </span>
        </button>
      {/each}
    </div>
    {#if s.preset === "custom"}
      <div class="mt-2">
        <textarea
          bind:value={s.customConfig}
          spellcheck="false"
          disabled={traces.recordingNow}
          placeholder="Perfetto TraceConfig in text form"
          class="mono h-36 w-full resize-y rounded-xl border border-line bg-panel-2 p-2.5 text-[11.5px] outline-none focus:border-accent"
        ></textarea>
        <button onclick={() => (s.customConfig = SAMPLE_CONFIG)} class="mt-1 text-[11.5px] text-accent hover:underline">Insert an example</button>
      </div>
    {/if}
  {:else if s.kind === "cpu"}
    <label class="mt-3 flex cursor-pointer items-start gap-2.5 rounded-xl border border-line px-3 py-2.5 hover:border-line-strong">
      <input type="checkbox" bind:checked={s.systemWide} disabled={traces.recordingNow} class="mt-0.5 accent-[var(--accent)]" />
      <span>
        <span class="block text-[12.5px] font-medium">Whole system</span>
        <span class="block text-[11.5px] leading-snug text-muted">Samples every process. Needs an emulator image that allows root (no Google Play).</span>
      </span>
    </label>
  {:else}
    <label class="mt-3 block">
      <span class="text-[11.5px] text-muted">Template</span>
      <select
        bind:value={s.template}
        disabled={traces.recordingNow || !traces.templates.length}
        class="mt-1 h-9 w-full rounded-lg border border-line bg-panel-2 px-2 text-[12.5px] outline-none focus:border-accent"
      >
        {#each traces.templates as t (t)}<option value={t}>{t}</option>{/each}
        {#if !traces.templates.length}<option>Time Profiler</option>{/if}
      </select>
    </label>
  {/if}

  <div class="mt-3 flex items-center gap-3">
    <label class="flex-1">
      <span class="flex items-center justify-between text-[11.5px] text-muted">Duration <span class="mono text-fg">{s.durationS} s</span></span>
      <input type="range" min="1" max={MAX_DURATION} bind:value={s.durationS} disabled={traces.recordingNow} class="mt-1 w-full accent-[var(--accent)]" />
    </label>
  </div>

  <p class="mt-2 flex items-center gap-1.5 text-[11.5px] text-muted">
    <Icon name="smartphone" size={12} class="text-faint" />
    {#if app.device}
      {app.device.name}{" "}{#if appName} · focused on <span class="font-medium text-fg">{appName}</span>{:else} · no app picked{/if}
    {:else}
      No device selected
    {/if}
  </p>

  {#if traces.recordError}
    <div class="mt-3 flex items-start gap-2 rounded-xl border border-err/30 bg-err/8 px-3 py-2.5 text-[12px] text-err">
      <Icon name="alert" size={14} class="mt-0.5 shrink-0" /><span class="min-w-0 break-words">{traces.recordError}</span>
    </div>
  {/if}

  {#if traces.recordingNow}
    <div class="mt-4">
      <div class="h-1.5 overflow-hidden rounded-full bg-hover" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(fraction * 100)}>
        <div class="h-full rounded-full bg-accent transition-[width] duration-200" style="width: {fraction * 100}%"></div>
      </div>
      <div class="mt-2 flex items-center gap-3">
        <span class="live-dot size-2 rounded-full bg-err"></span>
        <span class="mono flex-1 text-[12.5px]">
          {#if traces.stopping || traces.elapsedMs >= s.durationS * 1000}
            Processing…
          {:else}
            Recording {clock(traces.elapsedMs)} / {clock(s.durationS * 1000)}
          {/if}
        </span>
        <button
          onclick={() => traces.stop()}
          disabled={traces.stopping}
          class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium hover:bg-hover disabled:opacity-50"
        >
          <Icon name="stop" size={12} /> Stop
        </button>
      </div>
    </div>
  {:else}
    <button
      onclick={() => traces.start()}
      disabled={!canRecord}
      class="mt-4 flex h-9 w-full items-center justify-center gap-1.5 rounded-lg bg-accent text-[12.5px] font-semibold text-white hover:opacity-90 disabled:opacity-40"
    >
      <Icon name="play" size={12} /> Record {s.durationS} s
    </button>
  {/if}
</section>

<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import { api, errorText, type Lane, type SessionView, type TimelineEvent, type VideoPiece } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { sessions } from "../../stores/sessions.svelte";
  import Icon from "../Icon.svelte";

  let { view }: { view: SessionView } = $props();

  const tl = $derived(view.timeline);
  const total = $derived(Math.max(tl.durationMs, 1000));

  const LEFT = 96;
  const RIGHT = 14;
  const ROW = 30;
  const AXIS = 26;
  const names: Record<Lane, string> = { taps: "Touches", logs: "Log", requests: "Requests", frames: "Slow frames", gc: "GC", crashes: "Crashes", screens: "Screenshots", video: "Video" };
  const order: Lane[] = ["taps", "requests", "logs", "frames", "gc", "crashes", "screens", "video"];

  const lanes = $derived(order.filter((l) => tl.lanes.some((x) => x.lane === l)));
  const counts = $derived(Object.fromEntries(tl.lanes.map((l) => [l.lane, l])) as Record<string, { total: number; shown: number }>);

  // What part of the run is on screen, in milliseconds from the start.
  let from = $state(0);
  let to = $state(1000);
  let width = $state(800);
  $effect(() => {
    from = 0;
    to = total;
  });
  const span = $derived(Math.max(to - from, 1));
  const plot = $derived(Math.max(width - LEFT - RIGHT, 50));
  const x = (t: number) => LEFT + ((t - from) / span) * plot;
  const timeAt = (px: number) => from + ((px - LEFT) / plot) * span;
  const height = $derived(lanes.length * ROW + AXIS);

  const visible = $derived(tl.events.filter((e) => e.tMs + (e.durMs ?? 0) >= from && e.tMs <= to));

  function fmt(ms: number, precise = false): string {
    const s = Math.max(0, ms) / 1000;
    const m = Math.floor(s / 60);
    const rest = s - m * 60;
    return `${m}:${(precise ? rest.toFixed(1) : Math.floor(rest).toString()).padStart(precise ? 4 : 2, "0")}`;
  }

  const ticks = $derived.by(() => {
    const steps = [100, 250, 500, 1000, 2000, 5000, 10_000, 30_000, 60_000, 120_000, 300_000, 600_000];
    const step = steps.find((s) => (s / span) * plot >= 84) ?? steps[steps.length - 1];
    const out: number[] = [];
    for (let t = Math.ceil(from / step) * step; t <= to; t += step) out.push(t);
    return { step, marks: out };
  });

  function zoom(factor: number, center = (from + to) / 2) {
    const next = Math.min(Math.max(span * factor, 500), total);
    let a = center - ((center - from) / span) * next;
    a = Math.min(Math.max(a, 0), total - next);
    from = a;
    to = a + next;
  }
  const fit = () => {
    from = 0;
    to = total;
  };

  // Dragging pans; a click that did not move sets the playhead.
  let drag: { px: number; from: number; to: number; moved: boolean } | null = null;
  function down(e: PointerEvent) {
    drag = { px: e.clientX, from, to, moved: false };
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const dx = e.clientX - drag.px;
    if (Math.abs(dx) > 3) drag.moved = true;
    if (!drag.moved) return;
    const dt = (-dx / plot) * (drag.to - drag.from);
    const a = Math.min(Math.max(drag.from + dt, 0), total - (drag.to - drag.from));
    from = a;
    to = a + (drag.to - drag.from);
  }
  function up(e: PointerEvent) {
    const d = drag;
    drag = null;
    if (d && !d.moved) {
      const box = (e.currentTarget as Element).getBoundingClientRect();
      seek(Math.min(Math.max(timeAt(e.clientX - box.left), 0), total));
    }
  }
  function wheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey)) return;
    e.preventDefault();
    const box = (e.currentTarget as Element).getBoundingClientRect();
    zoom(e.deltaY > 0 ? 1.25 : 0.8, timeAt(e.clientX - box.left));
  }

  // ---- The video
  let videoEl: HTMLVideoElement | undefined = $state();
  let videoSrc = $state<string | null>(null);
  let loadedFile: string | null = null;
  let pendingSeek: number | null = null;
  let loading = $state(false);
  let videoError = $state<string | null>(null);
  let playMs = $state<number | null>(null);
  const urls = new Map<string, string>();

  const rel = (p: VideoPiece) => p.startMs - view.meta.startedMs;
  function pieceAt(ms: number): VideoPiece | undefined {
    const abs = view.meta.startedMs + ms;
    return view.video.find((p) => p.startMs <= abs && abs <= p.endMs) ?? [...view.video].reverse().find((p) => p.startMs <= abs) ?? view.video[0];
  }
  async function blobFor(file: string): Promise<string> {
    const known = urls.get(file);
    if (known) return known;
    const bytes = await api.sessionVideo(file);
    const url = URL.createObjectURL(new Blob([bytes], { type: "video/mp4" }));
    urls.set(file, url);
    return url;
  }
  async function show(piece: VideoPiece, seconds: number, play = false) {
    videoError = null;
    pendingSeek = seconds;
    if (loadedFile !== piece.file) {
      loading = true;
      try {
        videoSrc = await blobFor(piece.file);
        loadedFile = piece.file;
      } catch (e) {
        videoError = errorText(e);
        return;
      } finally {
        loading = false;
      }
      await tick();
    } else if (videoEl) {
      videoEl.currentTime = seconds;
      pendingSeek = null;
    }
    if (play) videoEl?.play().catch(() => {});
  }
  function seek(ms: number) {
    playMs = ms;
    const piece = view.video.length ? pieceAt(ms) : undefined;
    if (piece) show(piece, Math.max(0, (view.meta.startedMs + ms - piece.startMs) / 1000));
  }
  function loadedmetadata() {
    if (videoEl && pendingSeek != null) {
      videoEl.currentTime = pendingSeek;
      pendingSeek = null;
    }
  }
  function timeupdate() {
    const piece = view.video.find((p) => p.file === loadedFile);
    if (piece && videoEl && pendingSeek == null) playMs = rel(piece) + videoEl.currentTime * 1000;
  }
  function ended() {
    const i = view.video.findIndex((p) => p.file === loadedFile);
    const next = view.video[i + 1];
    if (next) show(next, 0, true);
  }
  // The first piece is ready when the tab opens, at the crash if there was one.
  $effect(() => {
    const crash = tl.events.filter((e) => e.lane === "crashes").at(-1);
    const start = crash ? Math.max(0, crash.tMs - 5000) : 0;
    const has = view.video.length > 0;
    untrack(() => {
      if (has) seek(start);
      else playMs = null;
    });
  });
  onDestroy(() => urls.forEach((u) => URL.revokeObjectURL(u)));

  // ---- Looking at one thing
  let hover = $state<TimelineEvent | null>(null);
  let picked = $state<TimelineEvent | null>(null);
  const shown = $derived(hover ?? picked);

  function pick(e: TimelineEvent) {
    picked = e;
    seek(e.tMs);
  }
  function jump(e: TimelineEvent) {
    if (e.index == null) return;
    if (e.lane === "requests") {
      sessions.tab = "requests";
      sessions.openRequest(e.index);
    } else if (e.lane === "logs" || e.lane === "gc") sessions.tab = "logs";
    else if (e.lane === "screens") sessions.tab = "screens";
    else if (e.lane === "crashes") sessions.tab = "overview";
  }
  const tone = (e: TimelineEvent) => (e.severity === "error" ? "var(--err)" : e.severity === "warning" ? "var(--warn)" : "var(--accent)");
  const laneY = (lane: Lane) => lanes.indexOf(lane) * ROW + ROW / 2;
  const button = "grid h-7 min-w-7 place-items-center rounded-md border border-line px-2 text-[11.5px] text-muted hover:bg-hover hover:text-fg";
</script>

<div class="p-4">
  {#if view.video.length}
    <div class="mb-3 flex items-start gap-4">
      <div class="relative shrink-0 overflow-hidden rounded-xl border border-line bg-black" style="height: 300px; aspect-ratio: 9 / 16; max-width: 60%;">
        <!-- svelte-ignore a11y_media_has_caption -->
        <video bind:this={videoEl} src={videoSrc ?? undefined} controls preload="metadata" class="size-full object-contain" onloadedmetadata={loadedmetadata} ontimeupdate={timeupdate} onended={ended}></video>
        {#if loading}<span class="absolute inset-0 grid place-items-center bg-black/50 text-[12px] text-white">Loading…</span>{/if}
      </div>
      <div class="min-w-0 flex-1 text-[12px] leading-relaxed text-muted">
        <p class="text-[10.5px] font-semibold tracking-wider text-faint uppercase">Screen recording</p>
        <p class="mt-1">{view.video.length} {view.video.length === 1 ? "piece" : "pieces"}. Click anywhere on the timeline, or on something that happened, and the video goes there.</p>
        {#if playMs != null}<p class="mono mt-1 text-fg">{fmt(playMs, true)}</p>{/if}
        {#if videoError}<p class="mt-1 text-err">{videoError}</p>{/if}
        <p class="mt-2 text-faint">There is a gap of a moment between two pieces of video{view.video.length > 1 ? "" : ", which a single piece does not have"}.</p>
      </div>
    </div>
  {/if}

  {#if !lanes.length}
    <p class="py-10 text-center text-[12.5px] text-muted">Nothing was recorded on the timeline. Record with touches and video, or with the network, and it fills in.</p>
  {:else}
    <div class="mb-2 flex items-center gap-1.5">
      <span class="mr-auto text-[11.5px] text-muted">
        {fmt(from, true)} to {fmt(to, true)} of {fmt(total, true)}
        {#if tl.clock === "estimated"}<span class="ml-2 text-warn" title="This recording did not measure the device's clock. Log times are worked out from the first line.">· log times estimated</span>{/if}
      </span>
      <button class={button} onclick={() => zoom(0.5)} aria-label="Zoom in" title="Zoom in (or ⌘ and scroll)"><Icon name="plus" size={13} /></button>
      <button class={button} onclick={() => zoom(2)} aria-label="Zoom out" title="Zoom out"><Icon name="minus" size={13} /></button>
      <button class={button} onclick={fit} title="Show the whole run">Fit</button>
    </div>

    <div bind:clientWidth={width} class="overflow-hidden rounded-xl border border-line bg-panel">
      <svg {width} {height} role="application" aria-label="Timeline" onwheel={wheel} class="block touch-none select-none">
        {#each lanes as lane, i (lane)}
          <rect x="0" y={i * ROW} {width} height={ROW} fill={i % 2 ? "transparent" : "var(--panel-2)"} opacity="0.55" />
          <text x="10" y={i * ROW + ROW / 2 + 4} font-size="11.5" fill="var(--muted)">{names[lane]}</text>
          {#if counts[lane] && counts[lane].shown < counts[lane].total}
            <text x="10" y={i * ROW + ROW / 2 + 15} font-size="9" fill="var(--faint)">{counts[lane].shown} of {counts[lane].total}</text>
          {/if}
        {/each}
        <line x1={LEFT} x2={LEFT} y1="0" y2={height - AXIS} stroke="var(--line)" />
        {#each ticks.marks as t (t)}
          <line x1={x(t)} x2={x(t)} y1="0" y2={height - AXIS + 4} stroke="var(--line)" stroke-dasharray="2 3" opacity="0.7" />
          <text x={x(t)} y={height - 8} font-size="10" text-anchor="middle" fill="var(--faint)">{ticks.step < 1000 ? fmt(t, true) : fmt(t)}</text>
        {/each}

        <!-- Dragging pans, a click sets the playhead -->
        <rect x={LEFT} y="0" width={plot} height={height - AXIS} fill="transparent" role="presentation" style="cursor: grab" onpointerdown={down} onpointermove={move} onpointerup={up} />

        {#each visible as e, i (i + ":" + e.lane + ":" + e.tMs + ":" + e.label)}
          {@const w = e.durMs ? Math.max((e.durMs / span) * plot, 4) : 0}
          {@const cy = laneY(e.lane)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <g
            role="button"
            tabindex="-1"
            aria-label={e.label}
            style="cursor: pointer"
            onmouseenter={() => (hover = e)}
            onmouseleave={() => (hover = null)}
            onclick={(ev) => {
              ev.stopPropagation();
              pick(e);
            }}
          >
            {#if e.lane === "video"}
              <rect x={x(e.tMs)} y={cy - 7} width={Math.max(w, 4)} height="14" rx="3" fill="var(--accent)" opacity="0.28" />
            {:else if w > 4}
              <rect x={x(e.tMs)} y={cy - 6} width={w} height="12" rx="3" fill={tone(e)} opacity={picked === e ? 1 : 0.8} />
            {:else}
              <circle cx={x(e.tMs)} {cy} r={picked === e ? 6 : 4.5} fill={tone(e)} opacity={picked === e ? 1 : 0.85} />
            {/if}
          </g>
        {/each}

        {#if playMs != null && playMs >= from && playMs <= to}
          <line x1={x(playMs)} x2={x(playMs)} y1="0" y2={height - AXIS + 4} stroke="var(--accent)" stroke-width="1.5" pointer-events="none" />
          <path d="M{x(playMs) - 5} 0 h10 l-5 7 z" fill="var(--accent)" pointer-events="none" />
        {/if}
      </svg>
    </div>

    <div class="mt-2 min-h-[44px] rounded-xl border border-line bg-panel-2 px-3 py-2 text-[12px]">
      {#if shown}
        <div class="flex items-start gap-3">
          <span class="mono mt-0.5 w-[64px] shrink-0 text-faint">{fmt(shown.tMs, true)}</span>
          <span class="mono w-[76px] shrink-0 rounded px-1.5 text-center text-[10.5px] uppercase" style="background: color-mix(in srgb, {tone(shown)} 16%, transparent); color: {tone(shown)}">{names[shown.lane]}</span>
          <span class="min-w-0 flex-1 break-words">{shown.label}{#if shown.durMs}<span class="ml-2 text-faint">{shown.durMs} ms</span>{/if}{#if shown.detail}<span class="mt-0.5 block text-muted">{shown.detail}</span>{/if}</span>
          {#if picked === shown && shown.index != null && ["requests", "logs", "gc", "screens", "crashes"].includes(shown.lane)}
            <button onclick={() => jump(shown)} class="h-6 shrink-0 rounded-md bg-hover px-2 text-[11.5px] font-medium hover:bg-line">Open</button>
          {/if}
        </div>
      {:else}
        <p class="text-muted">Hover over something to read it. Click it to put the video there. Drag to move along the run, ⌘ and scroll to zoom.</p>
      {/if}
    </div>
  {/if}
</div>

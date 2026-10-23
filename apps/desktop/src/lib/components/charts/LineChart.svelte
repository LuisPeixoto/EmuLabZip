<script lang="ts" module>
  export interface Series {
    label: string;
    /** A CSS colour, normally `var(--series-1)` or `var(--series-2)`. */
    color: string;
    values: (number | null)[];
  }
</script>

<script lang="ts">
  // A small time-series chart: one y-axis, 2px lines, a soft area under the first series, a
  // crosshair with a tooltip, and a legend when there is more than one series.

  let {
    title,
    series,
    max,
    format,
    height = 168,
    empty = "No data yet",
  }: {
    title: string;
    series: Series[];
    /** Fixed top of the y-axis (100 for percentages). Otherwise it follows the data. */
    max?: number;
    format: (v: number) => string;
    height?: number;
    empty?: string;
  } = $props();

  const PAD = { top: 10, right: 12, bottom: 8, left: 52 };
  let width = $state(400);
  let hover = $state<number | null>(null);

  const count = $derived(Math.max(2, ...series.map((s) => s.values.length)));
  const hasData = $derived(series.some((s) => s.values.some((v) => v != null)));
  const top = $derived.by(() => {
    if (max != null) return max;
    const peak = Math.max(0, ...series.flatMap((s) => s.values.filter((v): v is number => v != null)));
    if (peak <= 0) return 1;
    // Round up to a tidy number so the grid labels read well.
    const step = 10 ** Math.floor(Math.log10(peak));
    return Math.ceil((peak * 1.15) / step) * step;
  });

  const plotW = $derived(Math.max(10, width - PAD.left - PAD.right));
  const plotH = $derived(height - PAD.top - PAD.bottom);
  const x = (i: number) => PAD.left + (i / (count - 1)) * plotW;
  const y = (v: number) => PAD.top + plotH - (Math.min(v, top) / top) * plotH;

  function line(values: (number | null)[]): string {
    let d = "";
    let pen = false;
    values.forEach((v, i) => {
      if (v == null) {
        pen = false;
        return;
      }
      d += `${pen ? "L" : "M"}${x(i).toFixed(1)},${y(v).toFixed(1)}`;
      pen = true;
    });
    return d;
  }

  function area(values: (number | null)[]): string {
    const first = values.findIndex((v) => v != null);
    if (first < 0) return "";
    let last = first;
    values.forEach((v, i) => v != null && (last = i));
    return `${line(values)}L${x(last).toFixed(1)},${y(0)}L${x(first).toFixed(1)},${y(0)}Z`;
  }

  function onmove(e: PointerEvent) {
    const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
    const i = Math.round(((e.clientX - rect.left - PAD.left) / plotW) * (count - 1));
    hover = i >= 0 && i < count && series.some((s) => s.values[i] != null) ? i : null;
  }

  const gradientId = `area-${Math.random().toString(36).slice(2)}`;
</script>

<section class="rounded-2xl border border-line bg-panel p-4">
  <header class="mb-2 flex items-center gap-3">
    <h3 class="flex-1 text-[12.5px] font-semibold">{title}</h3>
    {#if series.length > 1}
      <div class="flex items-center gap-3 text-[11px] text-muted">
        {#each series as s (s.label)}
          <span class="flex items-center gap-1.5"><span class="h-0.5 w-3 rounded-full" style="background:{s.color}"></span>{s.label}</span>
        {/each}
      </div>
    {/if}
  </header>

  <div class="relative" bind:clientWidth={width}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <svg {width} {height} class="block overflow-visible" onpointermove={onmove} onpointerleave={() => (hover = null)} role="img" aria-label={title}>
      <defs>
        <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0" stop-color={series[0]?.color} stop-opacity="0.22" />
          <stop offset="1" stop-color={series[0]?.color} stop-opacity="0" />
        </linearGradient>
      </defs>
      {#each [0, 0.5, 1] as f (f)}
        <line x1={PAD.left} x2={PAD.left + plotW} y1={y(top * f)} y2={y(top * f)} stroke="var(--line)" stroke-dasharray={f === 0 ? "" : "3 4"} />
        {#if hasData || max != null}
          <text x={PAD.left - 8} y={y(top * f) + 3.5} text-anchor="end" class="mono" font-size="10" fill="var(--faint)">{format(top * f)}</text>
        {/if}
      {/each}
      {#if hasData}
        <path d={area(series[0].values)} fill="url(#{gradientId})" />
        {#each series as s (s.label)}
          <path d={line(s.values)} fill="none" stroke={s.color} stroke-width="2" stroke-linejoin="round" stroke-linecap="round" />
        {/each}
        {#if hover != null}
          <line x1={x(hover)} x2={x(hover)} y1={PAD.top} y2={PAD.top + plotH} stroke="var(--line-strong)" />
          {#each series as s (s.label)}
            {@const v = s.values[hover]}
            {#if v != null}<circle cx={x(hover)} cy={y(v)} r="4" fill={s.color} stroke="var(--panel)" stroke-width="2" />{/if}
          {/each}
        {/if}
      {:else}
        <text x={PAD.left + plotW / 2} y={PAD.top + plotH / 2} text-anchor="middle" font-size="12" fill="var(--faint)">{empty}</text>
      {/if}
    </svg>

    {#if hover != null}
      <div
        class="shadow-pop pointer-events-none absolute top-1 z-10 rounded-lg border border-line bg-elevated px-2.5 py-1.5 text-[11.5px]"
        style="left:{Math.min(x(hover) + 12, width - 150)}px"
      >
        {#each series as s (s.label)}
          {@const v = s.values[hover]}
          <div class="flex items-center gap-2 whitespace-nowrap">
            <span class="size-2 rounded-full" style="background:{s.color}"></span>
            <span class="text-muted">{s.label}</span>
            <span class="mono ml-auto pl-3 font-medium text-fg">{v == null ? "—" : format(v)}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</section>

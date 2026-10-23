<script lang="ts">
  // A flame graph drawn as an icicle: the root on top, callees below, width in proportion to the
  // time spent. Click a frame to zoom into it, click the breadcrumb to zoom out.
  import { onMount } from "svelte";
  import type { FlameNode, Origin } from "../../api";

  let {
    root,
    format,
    search = "",
    height = 460,
  }: {
    root: FlameNode;
    /** How a weight is shown: "12 ms", "3.4k events". */
    format: (weight: number) => string;
    /** Frames whose name contains this stay bright, the rest fade. */
    search?: string;
    height?: number;
  } = $props();

  const ROW = 20;
  const ORIGINS: Record<Origin, { label: string; token: string }> = {
    app: { label: "App", token: "--flame-app" },
    framework: { label: "Framework", token: "--flame-framework" },
    native: { label: "Native", token: "--flame-native" },
    kernel: { label: "Kernel", token: "--flame-kernel" },
  };

  interface Box {
    x: number;
    y: number;
    w: number;
    node: FlameNode;
  }

  let canvas: HTMLCanvasElement | undefined = $state();
  let width = $state(800);
  let path = $state<FlameNode[]>([]);
  let hovered = $state<Box | null>(null);
  let pointer = $state({ x: 0, y: 0 });
  let theme = $state(0);
  let boxes: Box[] = [];

  // A new tree starts zoomed all the way out.
  $effect(() => {
    root;
    path = [];
  });

  const focus = $derived(path.at(-1) ?? root);
  const needle = $derived(search.trim().toLowerCase());
  const depth = $derived.by(() => {
    const walk = (n: FlameNode): number => 1 + Math.max(0, ...n.children.map(walk));
    return walk(focus);
  });
  const canvasHeight = $derived(Math.max(height, depth * ROW + 8));

  function css(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  /** Black or white, whichever reads better on `hex`. */
  function ink(hex: string): string {
    const m = /^#([0-9a-f]{6})$/i.exec(hex);
    if (!m) return "#fff";
    const [r, g, b] = [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16) / 255).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
    return 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.3 ? "#0b0d12" : "#ffffff";
  }

  function draw() {
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(canvasHeight * dpr);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, width, canvasHeight);
    ctx.font = '11px "Inter Variable", Inter, system-ui, sans-serif';
    ctx.textBaseline = "middle";

    const colors = Object.fromEntries(Object.entries(ORIGINS).map(([k, v]) => [k, css(v.token)])) as Record<Origin, string>;
    const total = focus.value || 1;
    const next: Box[] = [];
    const place = (node: FlameNode, x: number, y: number, w: number) => {
      next.push({ x, y, w, node });
      const faded = needle && !node.name.toLowerCase().includes(needle);
      ctx.globalAlpha = faded ? 0.28 : 1;
      ctx.fillStyle = colors[node.origin];
      // A 1px gap, where the surface shows through, keeps neighbours apart without borders.
      ctx.fillRect(x, y, Math.max(w - 1, 0.5), ROW - 1);
      if (w > 34) {
        ctx.fillStyle = ink(colors[node.origin]);
        const room = w - 8;
        let text = node.name;
        if (ctx.measureText(text).width > room) {
          let end = Math.max(1, Math.floor((text.length * room) / ctx.measureText(text).width) - 1);
          while (end > 1 && ctx.measureText(text.slice(0, end) + "…").width > room) end--;
          text = text.slice(0, end) + "…";
        }
        ctx.fillText(text, x + 4, y + ROW / 2);
      }
      ctx.globalAlpha = 1;
      let cx = x;
      for (const child of node.children) {
        const cw = (child.value / total) * width;
        if (cw >= 0.6) place(child, cx, y + ROW, cw);
        cx += cw;
      }
    };
    place(focus, 0, 0, width);
    boxes = next;
  }

  function pick(e: MouseEvent): Box | null {
    const r = canvas!.getBoundingClientRect();
    pointer = { x: e.clientX - r.left, y: e.clientY - r.top };
    const row = Math.floor(pointer.y / ROW);
    return boxes.find((b) => b.y === row * ROW && pointer.x >= b.x && pointer.x < b.x + b.w) ?? null;
  }

  function zoomTo(node: FlameNode) {
    // The chain from the root down to `node`, so the breadcrumb can walk back up.
    const chain: FlameNode[] = [];
    const find = (n: FlameNode): boolean => {
      if (n === node) return true;
      for (const c of n.children) {
        chain.push(c);
        if (find(c)) return true;
        chain.pop();
      }
      return false;
    };
    path = find(root) ? chain : [];
  }

  $effect(() => {
    // Redraw when anything it shows changes.
    focus;
    needle;
    width;
    canvasHeight;
    theme;
    draw();
  });

  onMount(() => {
    const resize = new ResizeObserver(([entry]) => (width = Math.max(200, Math.floor(entry.contentRect.width))));
    resize.observe(canvas!.parentElement!);
    // Colours come from CSS variables, so a theme change needs a redraw.
    const watcher = new MutationObserver(() => theme++);
    watcher.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => {
      resize.disconnect();
      watcher.disconnect();
    };
  });

  const share = (n: FlameNode) => `${((n.value / (root.value || 1)) * 100).toFixed(1)}%`;
</script>

<div class="min-w-0">
  <div class="mb-2 flex flex-wrap items-center gap-x-4 gap-y-1.5 text-[11.5px]">
    <nav class="flex min-w-0 flex-1 flex-wrap items-center gap-1 text-muted" aria-label="Zoom">
      <button onclick={() => (path = [])} class="rounded px-1.5 py-0.5 hover:bg-hover hover:text-fg {path.length ? '' : 'font-medium text-fg'}">{root.name}</button>
      {#each path as step, i (i)}
        <span class="text-faint">›</span>
        <button
          onclick={() => (path = path.slice(0, i + 1))}
          class="max-w-[220px] truncate rounded px-1.5 py-0.5 hover:bg-hover hover:text-fg {i === path.length - 1 ? 'font-medium text-fg' : ''}"
          title={step.name}>{step.name}</button
        >
      {/each}
    </nav>
    <ul class="flex items-center gap-3 text-muted">
      {#each Object.entries(ORIGINS) as [key, o] (key)}
        <li class="flex items-center gap-1.5"><span class="size-2.5 rounded-[3px]" style="background: var({o.token})"></span>{o.label}</li>
      {/each}
    </ul>
  </div>

  <div class="relative overflow-hidden rounded-xl border border-line bg-panel">
    <canvas
      bind:this={canvas}
      style="width: 100%; height: {canvasHeight}px; display: block"
      class="cursor-pointer"
      aria-label="Flame graph of {root.name}"
      onmousemove={(e) => (hovered = pick(e))}
      onmouseleave={() => (hovered = null)}
      onclick={(e) => {
        const box = pick(e);
        if (box) zoomTo(box.node);
      }}
    ></canvas>
    {#if hovered}
      <div
        class="pointer-events-none absolute z-10 max-w-[360px] rounded-lg border border-line-strong bg-elevated px-3 py-2 text-[11.5px] shadow-[var(--shadow)]"
        style="left: {Math.min(pointer.x + 14, Math.max(0, width - 300))}px; top: {pointer.y + 18}px"
      >
        <p class="mono text-[11.5px] font-semibold break-all">{hovered.node.name}</p>
        <p class="mt-1 flex items-center gap-1.5 text-muted">
          <span class="size-2 rounded-[2px]" style="background: var({ORIGINS[hovered.node.origin].token})"></span>
          {ORIGINS[hovered.node.origin].label}
        </p>
        <p class="mono mt-1 text-muted">
          total {format(hovered.node.value)} · {share(hovered.node)}<br />self {format(hovered.node.own)} · {hovered.node.samples} samples
        </p>
      </div>
    {/if}
  </div>
</div>

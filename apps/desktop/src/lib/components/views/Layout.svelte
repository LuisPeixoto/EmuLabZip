<script lang="ts">
  import { untrack } from "svelte";
  import type { Bounds, Severity, UiNode } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { layout, nodeAt, title } from "../../stores/layout.svelte";
  import Icon from "../Icon.svelte";

  const result = $derived(layout.result);
  const screen = $derived(result?.layout.screen);
  const unit = $derived(result?.layout.platform === "ios" ? "pt" : "px");

  // Start with the screen the first time the view opens, and start over for another device.
  $effect(() => {
    layout.attach(app.deviceKey);
    untrack(() => {
      if (!layout.result && !layout.loading && !layout.error && app.target && app.online) layout.capture();
    });
  });

  let overlay: HTMLDivElement | undefined = $state();

  function pointer(e: MouseEvent): UiNode | null {
    if (!overlay || !result || !screen) return null;
    const r = overlay.getBoundingClientRect();
    const x = ((e.clientX - r.left) / r.width) * screen.width;
    const y = ((e.clientY - r.top) / r.height) * screen.height;
    return nodeAt(result.layout.root, x, y);
  }

  function onclick(e: MouseEvent) {
    const hit = pointer(e);
    layout.select(hit?.id ?? null);
    if (hit) layout.reveal(hit.id);
  }

  const box = (b: Bounds) => {
    const s = screen!;
    return `left:${(b.x / s.width) * 100}%;top:${(b.y / s.height) * 100}%;width:${(b.width / s.width) * 100}%;height:${(b.height / s.height) * 100}%`;
  };

  const hovered = $derived(layout.hoverId != null ? layout.byId.get(layout.hoverId) : null);
  const findings = $derived(layout.audit?.findings ?? []);
  const tone: Record<Severity, string> = { error: "bg-err", warning: "bg-warn", info: "bg-info" };
  const ring: Record<Severity, string> = { error: "border-err", warning: "border-warn", info: "border-info" };
  const count = (s: Severity) => findings.filter((f) => f.severity === s).length;

  /** The nodes the tree lists: everything, minus what is collapsed; or only what the search found. */
  const rows = $derived.by(() => {
    if (!result) return [];
    const out: { node: UiNode; depth: number }[] = [];
    if (layout.matches) {
      for (const n of layout.nodes) if (layout.matches.has(n.id)) out.push({ node: n, depth: 0 });
      return out.slice(0, 600);
    }
    const visit = (n: UiNode, depth: number) => {
      out.push({ node: n, depth });
      if (!layout.collapsed.has(n.id)) n.children.forEach((c) => visit(c, depth + 1));
    };
    visit(result.layout.root, 0);
    return out;
  });

  let treeEl: HTMLDivElement | undefined = $state();
  $effect(() => {
    const id = layout.selectedId;
    if (id != null && treeEl) treeEl.querySelector(`[data-node="${id}"]`)?.scrollIntoView({ block: "nearest" });
  });

  const dp = (n: number) => (screen && result?.layout.platform === "android" ? `${Math.round(n / screen.density)} dp` : `${n} ${unit}`);

  const props = $derived.by(() => {
    const n = layout.selected;
    if (!n) return [];
    const rows: [string, string][] = [
      ["Class", n.class],
      ["Text", n.text ?? ""],
      ["Label", n.label ?? ""],
      ["Hint", n.hint ?? ""],
      ["Id", n.resourceId ?? ""],
      ["Package", n.package ?? ""],
      ["Position", `${n.bounds.x}, ${n.bounds.y} ${unit}`],
      ["Size", `${n.bounds.width} × ${n.bounds.height} ${unit} (${dp(n.bounds.width)} × ${dp(n.bounds.height)})`],
      ["Flags", [n.clickable && "clickable", n.focusable && "focusable", n.editable && "editable", n.scrollable && "scrollable", !n.enabled && "disabled", n.checked != null && (n.checked ? "checked" : "unchecked")].filter(Boolean).join(", ")],
    ];
    return rows.filter(([, v]) => v);
  });

  async function copy(text: string) {
    await navigator.clipboard?.writeText(text).catch(() => {});
    app.toast("Copied", "ok");
  }
</script>

<div class="flex h-full flex-col">
  <header class="flex shrink-0 items-center gap-3 border-b border-line bg-panel px-4 py-3">
    <div class="min-w-0">
      <h1 class="text-[15px] font-semibold tracking-tight">Layout</h1>
      <p class="text-[11.5px] text-muted">
        {#if result}{layout.nodes.length} views on a {screen?.width} × {screen?.height} {unit} screen{:else}The views on the screen, and what a screen reader would trip over{/if}
      </p>
    </div>
    <span class="flex-1"></span>
    <label class="flex items-center gap-1.5 text-[12px] text-muted">
      <input type="checkbox" bind:checked={layout.showAll} class="accent-[var(--accent)]" /> Outline every view
    </label>
    <button onclick={() => layout.capture()} disabled={layout.loading || !app.online} class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">
      <Icon name="refresh" size={13} class={layout.loading ? "animate-spin" : ""} />{result ? "Capture again" : "Capture"}
    </button>
  </header>

  {#if layout.error}
    <div class="m-4 flex items-start gap-2 rounded-xl border border-err/30 bg-err/8 px-4 py-3 text-[12.5px] text-err"><Icon name="alert" size={14} class="mt-0.5 shrink-0" /><span>{layout.error}</span></div>
  {/if}

  {#if !result && !layout.error}
    <div class="grid flex-1 place-items-center px-6 text-center">
      <div class="max-w-[420px]">
        <span class="mx-auto mb-4 grid size-12 place-items-center rounded-2xl bg-accent-soft text-accent"><Icon name="layers" size={22} /></span>
        <p class="text-[13.5px] font-medium">{layout.loading ? "Reading the screen…" : "Capture the screen"}</p>
        <p class="mt-1.5 text-[12.5px] leading-relaxed text-muted">Shows every view with its size and name, and checks labels, touch targets and contrast.</p>
      </div>
    </div>
  {:else if result && screen}
    <div class="flex min-h-0 flex-1">
      <!-- Screen -->
      <div class="flex min-w-0 flex-1 items-center justify-center overflow-hidden bg-panel-2 p-5">
        <div class="relative h-full max-h-full" style="aspect-ratio:{screen.width}/{screen.height};max-width:100%">
          <img src="data:image/png;base64,{result.screenshot}" alt="Screenshot" class="absolute inset-0 size-full rounded-lg shadow-lg select-none" draggable="false" />
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div bind:this={overlay} class="absolute inset-0 cursor-crosshair" {onclick} onmousemove={(e) => (layout.hoverId = pointer(e)?.id ?? null)} onmouseleave={() => (layout.hoverId = null)}>
            {#if layout.showAll}
              {#each layout.nodes as n (n.id)}
                {#if n.bounds.width > 0 && n.id !== 0}<div class="pointer-events-none absolute border border-accent/35" style={box(n.bounds)}></div>{/if}
              {/each}
            {/if}
            {#each findings as f, i (i)}
              <div class="pointer-events-none absolute border-2 border-dashed {ring[f.severity]} opacity-80" style={box(f.bounds)}></div>
            {/each}
            {#if hovered}<div class="pointer-events-none absolute border-2 border-accent bg-accent/15" style={box(hovered.bounds)}></div>{/if}
            {#if layout.selected}<div class="pointer-events-none absolute border-2 border-accent bg-accent/25" style={box(layout.selected.bounds)}></div>{/if}
          </div>
          {#if hovered}
            <div class="mono pointer-events-none absolute bottom-2 left-2 max-w-[90%] truncate rounded-md bg-black/75 px-2 py-1 text-[11px] text-white">{title(hovered)} · {hovered.bounds.width} × {hovered.bounds.height}</div>
          {/if}
        </div>
      </div>

      <!-- Panel -->
      <aside class="flex w-[400px] shrink-0 flex-col border-l border-line bg-panel">
        <div class="flex shrink-0 items-center gap-1 border-b border-line px-3" role="tablist">
          {#each [["tree", "Views"], ["audit", "Accessibility"]] as [id, label] (id)}
            <button role="tab" aria-selected={layout.tab === id} onclick={() => (layout.tab = id as "tree" | "audit")} class="-mb-px flex h-9 items-center gap-1.5 border-b-2 px-2.5 text-[12px] {layout.tab === id ? 'border-accent font-medium text-accent' : 'border-transparent text-muted hover:text-fg'}">
              {label}
              {#if id === "audit"}
                {#if count("error")}<span class="mono rounded-full bg-err px-1.5 text-[10px] font-semibold text-white">{count("error")}</span>{/if}
                {#if count("warning")}<span class="mono rounded-full bg-warn px-1.5 text-[10px] font-semibold text-white">{count("warning")}</span>{/if}
                {#if !findings.length}<Icon name="check" size={12} class="text-ok" />{/if}
              {/if}
            </button>
          {/each}
        </div>

        {#if layout.tab === "tree"}
          <label class="relative m-2 flex shrink-0 items-center">
            <Icon name="search" size={12} class="pointer-events-none absolute left-2.5 text-faint" />
            <input bind:value={layout.search} spellcheck="false" placeholder="Find by text, label or id" class="h-8 w-full rounded-lg border border-line bg-panel-2 pr-2 pl-7 text-[12px] outline-none placeholder:text-faint focus:border-accent" />
          </label>
          <div bind:this={treeEl} class="min-h-0 flex-1 overflow-y-auto pb-2">
            {#each rows as { node, depth } (node.id)}
              {@const bad = layout.findingsByNode.get(node.id)}
              <div data-node={node.id} class="flex items-center {layout.selectedId === node.id ? 'bg-select' : 'hover:bg-hover'}" style="padding-left:{8 + depth * 12}px" onmouseenter={() => (layout.hoverId = node.id)} onmouseleave={() => (layout.hoverId = null)} role="presentation">
                {#if node.children.length && !layout.matches}
                  <button onclick={() => layout.toggle(node.id)} class="grid size-5 shrink-0 place-items-center text-faint hover:text-fg" aria-label="Expand or collapse"><Icon name={layout.collapsed.has(node.id) ? "chevronRight" : "chevronDown"} size={11} /></button>
                {:else}<span class="size-5 shrink-0"></span>{/if}
                <button onclick={() => layout.select(node.id)} class="flex min-w-0 flex-1 items-center gap-2 py-1 pr-2 text-left text-[12px]">
                  <span class="mono shrink-0 text-[10.5px] text-faint">{node.class.split(".").pop()}</span>
                  <span class="min-w-0 flex-1 truncate {node.label || node.text ? '' : 'text-faint'}">{title(node)}</span>
                  {#if bad}<span class="size-2 shrink-0 rounded-full {tone[bad[0].severity]}"></span>{/if}
                </button>
              </div>
            {:else}
              <p class="px-4 py-6 text-center text-[12px] text-muted">Nothing matches.</p>
            {/each}
          </div>
          {#if layout.selected}
            <div class="max-h-[42%] shrink-0 overflow-y-auto border-t border-line bg-panel-2 px-4 py-3">
              <dl class="selectable grid grid-cols-[72px_1fr] gap-x-3 gap-y-1 text-[11.5px]">
                {#each props as [k, v] (k)}
                  <dt class="text-faint">{k}</dt><dd class="mono min-w-0 break-all">{v}</dd>
                {/each}
              </dl>
              {#each layout.findingsByNode.get(layout.selected.id) ?? [] as f, i (i)}
                <p class="mt-2 flex items-start gap-1.5 rounded-lg bg-hover px-2.5 py-1.5 text-[11.5px]"><span class="mt-1 size-2 shrink-0 rounded-full {tone[f.severity]}"></span>{f.message}</p>
              {/each}
              {#if layout.selected.resourceId}
                <button onclick={() => copy(layout.selected!.resourceId!)} class="mt-2 flex h-6 items-center gap-1.5 rounded-md px-2 text-[11.5px] text-muted hover:bg-hover hover:text-fg"><Icon name="copy" size={12} />Copy the id</button>
              {/if}
            </div>
          {/if}
        {:else}
          <div class="min-h-0 flex-1 overflow-y-auto">
            {#if layout.audit?.contrastSkipped}
              <p class="border-b border-line bg-warn/10 px-4 py-2 text-[11.5px] text-warn">Contrast was not checked: the layout came without a screenshot.</p>
            {/if}
            <p class="px-4 pt-3 pb-1 text-[11.5px] text-muted">
              {layout.audit?.checked ?? 0} views checked: {count("error")} errors, {count("warning")} warnings, {count("info")} notes. This is a first pass; try the app with TalkBack or VoiceOver too.
            </p>
            {#each findings as f, i (i)}
              <button onclick={() => { layout.select(f.node); layout.reveal(f.node); }} class="block w-full border-b border-line/50 px-4 py-2.5 text-left hover:bg-hover {layout.selectedId === f.node ? 'bg-select' : ''}">
                <span class="flex items-center gap-2">
                  <span class="size-2 shrink-0 rounded-full {tone[f.severity]}"></span>
                  <span class="mono rounded bg-hover px-1.5 text-[10.5px] text-muted">{f.rule}</span>
                  <span class="mono min-w-0 flex-1 truncate text-[10.5px] text-faint">{f.class.split(".").pop()}{f.resourceId ? ` · ${f.resourceId.split("/").pop()}` : ""}</span>
                </span>
                <span class="mt-1 block text-[12px] leading-snug">{f.message}</span>
                {#if layout.selectedId === f.node}
                  <span class="mt-1.5 block text-[11.5px] leading-snug text-muted">{f.fix}</span>
                  <span class="mono mt-1 block text-[10.5px] text-faint">WCAG {f.wcag}</span>
                {/if}
              </button>
            {:else}
              <div class="grid place-items-center px-6 py-12 text-center">
                <span class="mb-3 grid size-10 place-items-center rounded-2xl bg-ok/12 text-ok"><Icon name="shieldCheck" size={20} /></span>
                <p class="text-[13px] font-medium">Nothing found</p>
                <p class="mt-1 text-[12px] text-muted">Labels, touch targets and contrast look fine on this screen.</p>
              </div>
            {/each}
          </div>
        {/if}
      </aside>
    </div>
  {/if}
</div>

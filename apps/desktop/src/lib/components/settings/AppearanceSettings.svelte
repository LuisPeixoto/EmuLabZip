<script lang="ts">
  import { api } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { theme as store } from "../../stores/theme.svelte";
  import { LIMITS } from "../../theme/select";
  import { fromAccent, isFontList, parse, serialize } from "../../theme/theme";
  import { BUILTIN } from "../../theme/builtin";
  import type { Theme } from "../../theme/tokens";
  import Icon from "../Icon.svelte";
  import ThemeCard from "./ThemeCard.svelte";
  import ThemeEditor from "./ThemeEditor.svelte";

  let editing = $state<Theme | null>(null);
  let pasting = $state(false);
  let pasted = $state("");
  let importError = $state("");
  let pending = $state<Theme | null>(null);

  const a = $derived(store.appearance);
  const chosen = (t: Theme) => (t.base === "dark" ? a.darkId : a.lightId) === t.id;

  function edit(t: Theme) {
    editing = t;
  }

  /** A copy to edit: a built-in theme cannot be changed, so editing it makes one of your own. */
  function duplicate(t: Theme) {
    edit({ ...structuredClone($state.snapshot(t)), name: `${t.name} copy`, author: undefined });
  }

  function create() {
    const t = fromAccent("My theme", "#6a4dff", store.base);
    edit(t);
  }

  async function exportTheme(t: Theme) {
    try {
      const path = await api.themeExport(t.id, serialize(t));
      if (path) app.toast(`Saved ${path}`, "ok");
    } catch (e) {
      app.toast(String(e), "err");
    }
  }

  function offer(text: string) {
    const r = parse(text);
    if (r.ok) {
      pending = r.theme;
      importError = "";
    } else {
      pending = null;
      importError = r.error;
    }
  }

  async function importFile() {
    try {
      const text = await api.themeImport();
      if (text !== null) offer(text);
    } catch (e) {
      pending = null;
      importError = String(e);
    }
  }

  function keep() {
    if (!pending) return;
    const t = BUILTIN.some((b) => b.id === pending!.id) ? { ...pending, id: `${pending.id}-imported` } : pending;
    store.install(t);
    store.choose(t);
    app.toast(`Installed ${t.name}`, "ok");
    pending = null;
    pasting = false;
    pasted = "";
  }

  const installedIds = $derived(new Set(store.installed.map((t) => t.id)));
  const densities = [
    { label: "Compact", value: 0.85 },
    { label: "Normal", value: 1 },
    { label: "Roomy", value: 1.15 },
  ];
  const pct = (n: number) => `${Math.round(n * 100)}%`;
  const SANS = [
    ["Theme's own", null],
    ["System", 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif'],
    ["Inter", 'Inter, "Inter Variable", sans-serif'],
    ["Serif", 'Georgia, "Times New Roman", serif'],
  ] as const;
  const MONO = [
    ["Theme's own", null],
    ["Menlo", 'Menlo, "SF Mono", Consolas, monospace'],
    ["JetBrains Mono", '"JetBrains Mono", Menlo, Consolas, monospace'],
    ["Courier", '"Courier New", Courier, monospace'],
  ] as const;
  let customSans = $state("");
  let customMono = $state("");
</script>

{#if editing}
  <ThemeEditor initial={editing} onclose={() => (editing = null)} />
{:else}
  <div class="space-y-6 p-5">
    <section>
      <div class="mb-2 flex items-center gap-4">
        <div class="flex-1">
          <h4 class="text-md font-medium">Mode</h4>
          <p class="text-sm text-muted">One theme is kept for dark and one for light. "System" follows the computer.</p>
        </div>
        <div class="flex rounded-lg border border-line bg-panel-2 p-0.5">
          {#each ["dark", "light", "system"] as const as m (m)}
            <button onclick={() => store.setMode(m)} class="flex h-7 items-center gap-1.5 rounded-md px-2.5 text-base capitalize {a.mode === m ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}">
              <Icon name={m === "dark" ? "moon" : m === "light" ? "sun" : "laptop"} size={13} />{m}
            </button>
          {/each}
        </div>
      </div>
    </section>

    <section>
      <div class="mb-2 flex flex-wrap items-center gap-2">
        <h4 class="flex-1 text-md font-medium">Themes</h4>
        <button onclick={create} class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90"><Icon name="plus" size={13} />New theme</button>
        <button onclick={importFile} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover"><Icon name="download" size={13} />Import file</button>
        <button onclick={() => ((pasting = !pasting), (importError = ""), (pending = null))} class="h-8 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover">Paste JSON</button>
      </div>

      {#if pasting}
        <div class="mb-3 space-y-2 rounded-xl border border-line bg-panel-2 p-3">
          <textarea
            bind:value={pasted}
            oninput={() => (pasted.trim() ? offer(pasted) : ((pending = null), (importError = "")))}
            placeholder="Paste a theme here"
            spellcheck="false"
            class="mono h-24 w-full resize-none rounded-lg border border-line bg-panel p-2 text-sm outline-none focus:border-accent"
          ></textarea>
        </div>
      {/if}
      {#if importError}
        <p class="mb-3 rounded-lg border border-err/50 bg-err/10 px-3 py-2 text-base text-err">{importError}</p>
      {/if}
      {#if pending}
        <div class="mb-3 flex items-center gap-3 rounded-xl border border-accent bg-accent-soft p-3">
          <div class="w-52 shrink-0"><ThemeCard theme={pending} onuse={() => {}} /></div>
          <div class="min-w-0 flex-1">
            <p class="text-md font-medium">Install “{pending.name}”?</p>
            <p class="text-sm text-muted">It only sets colours and font names. Nothing in it can run or load anything.</p>
          </div>
          <button onclick={() => ((pending = null), (pasted = ""))} class="h-8 rounded-lg border border-line px-3 text-base hover:bg-hover">Discard</button>
          <button onclick={keep} class="h-8 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90">Install</button>
        </div>
      {/if}

      <div class="grid grid-cols-3 gap-3">
        {#each store.all as t (t.id)}
          <ThemeCard theme={t} active={chosen(t)} badge={installedIds.has(t.id) ? "yours" : ""} onuse={() => store.choose(t)}>
            <button onclick={() => duplicate(t)} class="flex h-6 items-center gap-1 rounded-md px-1.5 text-sm text-muted hover:bg-hover hover:text-fg"><Icon name="copy" size={11} />Duplicate</button>
            {#if installedIds.has(t.id)}
              <button onclick={() => edit(t)} class="h-6 rounded-md px-1.5 text-sm text-muted hover:bg-hover hover:text-fg">Edit</button>
            {/if}
            <button onclick={() => exportTheme(t)} class="h-6 rounded-md px-1.5 text-sm text-muted hover:bg-hover hover:text-fg">Export</button>
            {#if installedIds.has(t.id)}
              <button onclick={() => store.remove(t.id)} class="ml-auto grid size-6 place-items-center rounded-md text-faint hover:bg-hover hover:text-err" aria-label="Delete {t.name}"><Icon name="trash" size={11} /></button>
            {/if}
          </ThemeCard>
        {/each}
      </div>
    </section>

    <section class="space-y-3">
      <h4 class="text-md font-medium">Fine-tune</h4>

      <div class="flex items-center gap-4">
        <div class="flex-1"><p class="text-md">Text size</p><p class="text-sm text-muted">All text, from 85% to 140%.</p></div>
        <input type="range" min={LIMITS.fontScale[0]} max={LIMITS.fontScale[1]} step="0.05" value={a.fontScale} oninput={(e) => store.update({ fontScale: +e.currentTarget.value })} class="w-44 accent-[var(--accent)]" />
        <span class="mono w-12 text-right text-base">{pct(a.fontScale)}</span>
      </div>

      <div class="flex items-center gap-4">
        <div class="flex-1"><p class="text-md">Interface scale</p><p class="text-sm text-muted">Zooms everything. ⌘+ and ⌘- change it, ⌘0 restores it.</p></div>
        <input type="range" min={LIMITS.zoom[0]} max={LIMITS.zoom[1]} step="0.05" value={a.zoom} oninput={(e) => store.update({ zoom: +e.currentTarget.value })} class="w-44 accent-[var(--accent)]" />
        <span class="mono w-12 text-right text-base">{pct(a.zoom)}</span>
      </div>

      <div class="flex items-center gap-4">
        <div class="flex-1"><p class="text-md">Density</p><p class="text-sm text-muted">How much room padding and gaps take.</p></div>
        <div class="flex rounded-lg border border-line bg-panel-2 p-0.5">
          {#each densities as d (d.value)}
            <button onclick={() => store.update({ density: d.value })} class="h-7 rounded-md px-2.5 text-base {a.density === d.value ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}">{d.label}</button>
          {/each}
        </div>
      </div>

      <div class="flex items-center gap-4">
        <div class="flex-1"><p class="text-md">Roundness</p><p class="text-sm text-muted">{a.roundness === null ? "The theme's own." : "Your own, over the theme's."}</p></div>
        <input type="range" min="0" max="2" step="0.05" value={a.roundness ?? store.active.shape.radius} oninput={(e) => store.update({ roundness: +e.currentTarget.value })} class="w-44 accent-[var(--accent)]" />
        <button onclick={() => store.update({ roundness: null })} disabled={a.roundness === null} class="h-7 rounded-lg px-2 text-sm text-muted hover:bg-hover disabled:opacity-40">Theme's</button>
      </div>

      {#each [{ label: "Interface font", key: "fontSans", presets: SANS, custom: customSans }, { label: "Code font", key: "fontMono", presets: MONO, custom: customMono }] as f (f.key)}
        <div class="flex items-center gap-4">
          <div class="flex-1"><p class="text-md">{f.label}</p><p class="text-sm text-muted">Must be installed on this computer.</p></div>
          <select
            onchange={(e) => store.update({ [f.key]: e.currentTarget.value || null })}
            class="h-8 rounded-lg border border-line bg-panel-2 px-2 text-base outline-none focus:border-accent"
          >
            {#each f.presets as [label, value] (label)}
              <option value={value ?? ""} selected={a[f.key as "fontSans" | "fontMono"] === value}>{label}</option>
            {/each}
            {#if a[f.key as "fontSans" | "fontMono"] && !f.presets.some(([, v]) => v === a[f.key as "fontSans" | "fontMono"])}
              <option value={a[f.key as "fontSans" | "fontMono"]} selected>Custom</option>
            {/if}
          </select>
          <input
            placeholder="Or a family name"
            value={f.key === "fontSans" ? customSans : customMono}
            oninput={(e) => (f.key === "fontSans" ? (customSans = e.currentTarget.value) : (customMono = e.currentTarget.value))}
            onchange={(e) => {
              const v = e.currentTarget.value.trim();
              if (v && isFontList(v)) store.update({ [f.key]: v });
            }}
            class="h-8 w-44 rounded-lg border border-line bg-panel-2 px-2.5 text-base outline-none focus:border-accent"
          />
        </div>
      {/each}

      <div class="flex justify-end pt-1">
        <button
          onclick={() => store.update({ fontScale: 1, zoom: 1, density: 1, roundness: null, fontSans: null, fontMono: null })}
          class="h-8 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover">Reset the fine-tuning</button>
      </div>
    </section>
  </div>
{/if}

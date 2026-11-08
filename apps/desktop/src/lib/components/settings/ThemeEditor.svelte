<script lang="ts">
  import { onDestroy } from "svelte";
  import { checks } from "../../theme/contrast";
  import { fromAccent, isColor, isFontList, slug } from "../../theme/theme";
  import { BUILTIN } from "../../theme/builtin";
  import { COLOR_GROUPS, DEFAULT_FONTS, type Base, type Theme } from "../../theme/tokens";
  import { theme as store } from "../../stores/theme.svelte";
  import Icon from "../Icon.svelte";

  let { initial, onclose }: { initial: Theme; onclose: () => void } = $props();

  // svelte-ignore state_referenced_locally
  let draft = $state<Theme>(structuredClone($state.snapshot(initial)) as Theme);
  // svelte-ignore state_referenced_locally
  let accent = $state(initial.colors.accent);

  /** The id follows the name, and never takes a built-in theme's. */
  const id = $derived.by(() => {
    const base = slug(draft.name) || "my-theme";
    return BUILTIN.some((t) => t.id === base) ? `${base}-custom` : base;
  });
  const result = $derived<Theme>({ ...draft, id });
  const problems = $derived(checks(result).filter((c) => !c.ok));

  // The whole app shows the theme as it is edited. Cancel, or closing, puts the chosen one back.
  $effect(() => store.setPreview(result));
  onDestroy(() => store.setPreview(null));

  function setColor(key: string, value: string) {
    if (isColor(value)) draft.colors[key] = value;
  }

  /** `<input type=color>` only takes #rrggbb. */
  const pickable = (v: string) => (/^#[0-9a-f]{6}$/i.test(v) ? v : /^#[0-9a-f]{3}$/i.test(v) ? `#${[...v.slice(1)].map((c) => c + c).join("")}` : "#000000");

  function generate() {
    if (!isColor(accent)) return;
    const next = fromAccent(draft.name, accent, draft.base);
    draft.colors = { ...draft.colors, ...next.colors };
  }

  function setBase(base: Base) {
    draft.base = base;
  }

  const fontsOk = $derived(isFontList(draft.fonts.sans) && isFontList(draft.fonts.mono));
  const valid = $derived(draft.name.trim().length > 0 && fontsOk);

  function save() {
    store.install(result);
    store.choose(result);
    onclose();
  }
</script>

<div class="flex h-full min-h-0 flex-col">
  <div class="flex items-center gap-2 border-b border-line px-5 py-3">
    <button onclick={onclose} class="grid size-7 place-items-center rounded-lg text-muted hover:bg-hover hover:text-fg" aria-label="Back">
      <Icon name="chevronRight" size={14} class="rotate-180" />
    </button>
    <h3 class="flex-1 text-lg font-semibold">Edit theme</h3>
    <span class="text-sm text-muted">The whole app shows your changes as you make them.</span>
  </div>

  <div class="min-h-0 flex-1 space-y-5 overflow-y-auto px-5 py-4">
    <div class="grid grid-cols-[1fr_1fr_auto] gap-3">
      <label class="block">
        <span class="text-sm text-muted">Name</span>
        <input bind:value={draft.name} maxlength="60" class="mt-1 h-8 w-full rounded-lg border border-line bg-panel-2 px-2.5 text-md outline-none focus:border-accent" />
      </label>
      <label class="block">
        <span class="text-sm text-muted">Author</span>
        <input bind:value={draft.author} maxlength="60" placeholder="Optional" class="mt-1 h-8 w-full rounded-lg border border-line bg-panel-2 px-2.5 text-md outline-none focus:border-accent" />
      </label>
      <div>
        <span class="text-sm text-muted">Base</span>
        <div class="mt-1 flex rounded-lg border border-line bg-panel-2 p-0.5">
          {#each ["dark", "light"] as const as b (b)}
            <button onclick={() => setBase(b)} class="h-7 rounded-md px-3 text-base capitalize {draft.base === b ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}">{b}</button>
          {/each}
        </div>
      </div>
    </div>
    <label class="block">
      <span class="text-sm text-muted">Description</span>
      <input bind:value={draft.description} maxlength="160" placeholder="Optional" class="mt-1 h-8 w-full rounded-lg border border-line bg-panel-2 px-2.5 text-md outline-none focus:border-accent" />
    </label>

    <div class="flex flex-wrap items-end gap-3 rounded-xl border border-line bg-panel-2 p-3">
      <div class="min-w-0 flex-1">
        <p class="text-md font-medium">Start from one colour</p>
        <p class="text-sm text-muted">Fills the surfaces and the text from an accent, in the lightness this base needs. You can still change each colour after.</p>
      </div>
      <input type="color" value={pickable(accent)} oninput={(e) => (accent = e.currentTarget.value)} class="size-8 cursor-pointer rounded-md border border-line bg-transparent" aria-label="Accent colour" />
      <button onclick={generate} class="h-8 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover">Generate</button>
    </div>

    {#if problems.length}
      <div class="rounded-xl border border-warn/50 bg-warn/10 p-3">
        <p class="flex items-center gap-1.5 text-md font-medium"><Icon name="alert" size={14} class="text-warn" />Some text may be hard to read</p>
        <ul class="mt-1.5 space-y-0.5 text-sm text-muted">
          {#each problems as p (p.what)}
            <li>{p.what}: {p.ratio ? p.ratio.toFixed(1) : "?"}:1, needs {p.needed}:1</li>
          {/each}
        </ul>
      </div>
    {/if}

    {#each COLOR_GROUPS as group (group.id)}
      <section>
        <h4 class="mb-2 text-2xs font-semibold tracking-wider text-faint uppercase">{group.label}</h4>
        <div class="grid grid-cols-2 gap-x-4 gap-y-1.5">
          {#each group.keys as { key, label } (key)}
            <div class="flex items-center gap-2">
              <input type="color" value={pickable(draft.colors[key])} oninput={(e) => setColor(key, e.currentTarget.value)} class="size-7 shrink-0 cursor-pointer rounded-md border border-line bg-transparent" aria-label={label} />
              <span class="min-w-0 flex-1 truncate text-base">{label}</span>
              <input
                value={draft.colors[key]}
                onchange={(e) => (isColor(e.currentTarget.value) ? setColor(key, e.currentTarget.value) : (e.currentTarget.value = draft.colors[key]))}
                spellcheck="false"
                class="mono h-7 w-[132px] rounded-md border border-line bg-panel-2 px-2 text-sm outline-none focus:border-accent"
              />
            </div>
          {/each}
        </div>
      </section>
    {/each}

    <section class="grid grid-cols-2 gap-4">
      <label class="block">
        <span class="text-sm text-muted">Interface font</span>
        <input bind:value={draft.fonts.sans} spellcheck="false" class="mono mt-1 h-8 w-full rounded-lg border bg-panel-2 px-2.5 text-sm outline-none focus:border-accent {isFontList(draft.fonts.sans) ? 'border-line' : 'border-err'}" />
      </label>
      <label class="block">
        <span class="text-sm text-muted">Code font</span>
        <input bind:value={draft.fonts.mono} spellcheck="false" class="mono mt-1 h-8 w-full rounded-lg border bg-panel-2 px-2.5 text-sm outline-none focus:border-accent {isFontList(draft.fonts.mono) ? 'border-line' : 'border-err'}" />
      </label>
      <p class="col-span-2 -mt-2 text-sm text-faint">Family names only, separated by commas. Fonts must already be installed; nothing is downloaded.
        <button onclick={() => (draft.fonts = { ...DEFAULT_FONTS })} class="text-accent hover:underline">Use the default fonts</button></p>
      <label class="col-span-2 block">
        <span class="text-sm text-muted">Roundness: {draft.shape.radius.toFixed(2)}</span>
        <input type="range" min="0" max="2" step="0.05" bind:value={draft.shape.radius} class="mt-1 w-full accent-[var(--accent)]" />
      </label>
    </section>
  </div>

  <div class="flex items-center gap-2 border-t border-line bg-panel-2 px-5 py-3">
    <span class="mono text-sm text-faint">{id}</span>
    <span class="flex-1"></span>
    <button onclick={onclose} class="h-8 rounded-lg border border-line px-3.5 text-base font-medium hover:bg-hover">Cancel</button>
    <button onclick={save} disabled={!valid} class="h-8 rounded-lg bg-accent px-3.5 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40">Save theme</button>
  </div>
</div>

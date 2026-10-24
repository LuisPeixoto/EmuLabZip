<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import type { Rule, RuleAction } from "../../api";
  import { net } from "../../stores/net.svelte";
  import Icon from "../Icon.svelte";

  type Kind = RuleAction["type"];

  const blank = (action: RuleAction, name: string): Rule => ({
    id: 0,
    name,
    enabled: true,
    matcher: { methods: [], host: null, path: null, urlContains: null },
    action,
  });

  const presets: { label: string; make: () => Rule }[] = [
    {
      label: "Mock a JSON response",
      make: () =>
        blank(
          { type: "mock", status: 200, headers: [["content-type", "application/json"]], body: '{"ok": true}', base64: false, delayMs: 0 },
          "Mock JSON",
        ),
    },
    { label: "Server error (500)", make: () => blank({ type: "mock", status: 500, headers: [], body: "Internal Server Error", base64: false, delayMs: 0 }, "Server error") },
    { label: "Slow network (2 s)", make: () => blank({ type: "delay", ms: 2000 }, "Slow network") },
    { label: "Block", make: () => blank({ type: "block" }, "Block") },
    { label: "Hold the request", make: () => blank({ type: "breakpoint", request: true, response: false }, "Breakpoint") },
  ];

  let rules = $state<Rule[]>(structuredClone($state.snapshot(net.rules)));
  let selected = $state<number>(0);
  let error = $state<string | null>(null);
  let saving = $state(false);

  // A rule handed over from a request ("Mock this response") starts selected.
  if (net.draft) {
    rules.push(structuredClone($state.snapshot(net.draft)));
    selected = rules.length - 1;
    net.draft = null;
  }

  const rule = $derived(rules[selected]);
  const dirty = $derived(JSON.stringify(rules) !== JSON.stringify($state.snapshot(net.rules)));

  function add(make: () => Rule) {
    rules.push(make());
    selected = rules.length - 1;
  }

  function remove(i: number) {
    rules.splice(i, 1);
    selected = Math.max(0, Math.min(selected, rules.length - 1));
  }

  function move(i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= rules.length) return;
    [rules[i], rules[j]] = [rules[j], rules[i]];
    selected = j;
  }

  function setKind(kind: Kind) {
    if (!rule || rule.action.type === kind) return;
    rule.action =
      kind === "mock"
        ? { type: "mock", status: 200, headers: [], body: "", base64: false, delayMs: 0 }
        : kind === "delay"
          ? { type: "delay", ms: 1000 }
          : kind === "block"
            ? { type: "block" }
            : { type: "breakpoint", request: true, response: false };
  }

  const methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];
  const toggleMethod = (m: string) => {
    if (!rule) return;
    const list = rule.matcher.methods;
    rule.matcher.methods = list.includes(m) ? list.filter((x) => x !== m) : [...list, m];
  };

  const blankToNull = (v: string) => (v.trim() ? v : null);

  async function save() {
    saving = true;
    error = null;
    const ok = await net.saveRules($state.snapshot(rules) as Rule[]);
    saving = false;
    if (ok) net.rulesOpen = false;
    else error = "A rule is not valid. Check the status codes, delays and patterns.";
  }

  const summary = (r: Rule) =>
    r.action.type === "mock" ? `Answers ${r.action.status}` : r.action.type === "delay" ? `Waits ${r.action.ms} ms` : r.action.type === "block" ? "Fails the request" : "Holds " + [r.action.request && "request", r.action.response && "response"].filter(Boolean).join(" and ");

  const field = "h-8 w-full rounded-lg border border-line bg-panel-2 px-2.5 text-[12px] outline-none placeholder:text-faint focus:border-accent focus:bg-panel";
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (net.rulesOpen = false)} />

<div transition:fade={{ duration: 120 }} class="fixed inset-0 z-50 grid place-items-center bg-black/45 p-8 backdrop-blur-[2px]">
  <button class="absolute inset-0 cursor-default" aria-label="Close" onclick={() => (net.rulesOpen = false)}></button>
  <div transition:scale={{ start: 0.97, duration: 160 }} class="shadow-pop relative flex h-full max-h-[640px] w-[860px] max-w-full flex-col overflow-hidden rounded-2xl border border-line bg-elevated">
    <header class="flex items-center gap-3 border-b border-line px-5 py-3.5">
      <span class="grid size-8 place-items-center rounded-xl bg-accent-soft text-accent"><Icon name="sliders" size={17} /></span>
      <div class="min-w-0 flex-1">
        <h2 class="text-[13.5px] font-semibold">Rules</h2>
        <p class="text-[11.5px] text-muted">Mock, slow down, block or hold requests. The first matching rule wins.</p>
      </div>
      <button onclick={() => (net.rulesOpen = false)} class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg" aria-label="Close"><Icon name="x" size={14} /></button>
    </header>

    <div class="flex min-h-0 flex-1">
      <!-- List -->
      <div class="flex w-[270px] shrink-0 flex-col border-r border-line">
        <div class="min-h-0 flex-1 overflow-y-auto p-2">
          {#each rules as r, i (i)}
            <div class="group flex items-center gap-2 rounded-lg px-2 py-1.5 {selected === i ? 'bg-select' : 'hover:bg-hover'}">
              <input type="checkbox" bind:checked={r.enabled} aria-label="Enabled" class="accent-[var(--accent)]" />
              <button onclick={() => (selected = i)} class="min-w-0 flex-1 text-left">
                <span class="block truncate text-[12.5px] font-medium {r.enabled ? '' : 'text-faint'}">{r.name || "Untitled rule"}</span>
                <span class="block truncate text-[11px] text-muted">{summary(r)}</span>
              </button>
              <span class="flex opacity-0 group-hover:opacity-100">
                <button onclick={() => move(i, -1)} class="grid size-6 place-items-center rounded text-faint hover:text-fg" aria-label="Move up"><Icon name="arrowUp" size={12} /></button>
                <button onclick={() => move(i, 1)} class="grid size-6 place-items-center rounded text-faint hover:text-fg" aria-label="Move down"><Icon name="arrowDown" size={12} /></button>
                <button onclick={() => remove(i)} class="grid size-6 place-items-center rounded text-faint hover:text-err" aria-label="Delete"><Icon name="trash" size={12} /></button>
              </span>
            </div>
          {:else}
            <p class="px-3 py-6 text-center text-[12px] leading-relaxed text-muted">No rules yet. Start from one of these:</p>
          {/each}
        </div>
        <div class="border-t border-line p-2">
          <p class="px-1 pb-1.5 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Add</p>
          <div class="flex flex-wrap gap-1.5">
            {#each presets as p (p.label)}
              <button onclick={() => add(p.make)} class="rounded-md border border-line px-2 py-1 text-[11.5px] text-muted hover:bg-hover hover:text-fg">{p.label}</button>
            {/each}
          </div>
        </div>
      </div>

      <!-- Editor -->
      <div class="min-w-0 flex-1 overflow-y-auto px-5 py-4">
        {#if rule}
          <label class="block">
            <span class="mb-1 block text-[11.5px] font-medium text-muted">Name</span>
            <input bind:value={rule.name} class={field} spellcheck="false" />
          </label>

          <h3 class="mt-5 mb-2 text-[10.5px] font-semibold tracking-wider text-faint uppercase">When the request matches</h3>
          <div class="grid grid-cols-2 gap-3">
            <label class="block">
              <span class="mb-1 block text-[11.5px] font-medium text-muted">Host</span>
              <input value={rule.matcher.host ?? ""} oninput={(e) => (rule.matcher.host = blankToNull(e.currentTarget.value))} placeholder="api.example.com or *.example.com" class="{field} mono" spellcheck="false" />
            </label>
            <label class="block">
              <span class="mb-1 block text-[11.5px] font-medium text-muted">Path</span>
              <input value={rule.matcher.path ?? ""} oninput={(e) => (rule.matcher.path = blankToNull(e.currentTarget.value))} placeholder="/v1/users/*" class="{field} mono" spellcheck="false" />
            </label>
            <label class="col-span-2 block">
              <span class="mb-1 block text-[11.5px] font-medium text-muted">URL contains</span>
              <input value={rule.matcher.urlContains ?? ""} oninput={(e) => (rule.matcher.urlContains = blankToNull(e.currentTarget.value))} placeholder="Any text of the address, query included" class="{field} mono" spellcheck="false" />
            </label>
          </div>
          <div class="mt-3 flex items-center gap-1.5">
            <span class="mr-1 text-[11.5px] font-medium text-muted">Methods</span>
            {#each methods as m (m)}
              <button onclick={() => toggleMethod(m)} class="mono h-6 rounded-md border px-2 text-[11px] {rule.matcher.methods.includes(m) ? 'border-accent bg-accent-soft font-semibold text-accent' : 'border-line text-muted hover:text-fg'}">{m}</button>
            {/each}
            <span class="text-[11px] text-faint">{rule.matcher.methods.length ? "" : "any"}</span>
          </div>

          <h3 class="mt-5 mb-2 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Then</h3>
          <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="group" aria-label="Action">
            {#each [["mock", "Answer"], ["delay", "Delay"], ["block", "Block"], ["breakpoint", "Hold"]] as [k, label] (k)}
              <button onclick={() => setKind(k as Kind)} class="h-7 flex-1 rounded-md text-[12px] {rule.action.type === k ? 'bg-panel font-semibold text-fg shadow-sm' : 'text-muted hover:text-fg'}">{label}</button>
            {/each}
          </div>

          <div class="mt-3 space-y-3">
            {#if rule.action.type === "mock"}
              {@const a = rule.action}
              <div class="grid grid-cols-2 gap-3">
                <label class="block">
                  <span class="mb-1 block text-[11.5px] font-medium text-muted">Status</span>
                  <input type="number" min="100" max="599" bind:value={a.status} class="{field} mono" />
                </label>
                <label class="block">
                  <span class="mb-1 block text-[11.5px] font-medium text-muted">Delay (ms)</span>
                  <input type="number" min="0" max="60000" bind:value={a.delayMs} class="{field} mono" />
                </label>
              </div>
              <label class="block">
                <span class="mb-1 block text-[11.5px] font-medium text-muted">Headers (one per line, <span class="mono">name: value</span>)</span>
                <textarea
                  rows="3"
                  spellcheck="false"
                  value={a.headers.map(([k, v]) => `${k}: ${v}`).join("\n")}
                  oninput={(e) => (a.headers = e.currentTarget.value.split("\n").filter((l) => l.includes(":")).map((l) => [l.slice(0, l.indexOf(":")).trim(), l.slice(l.indexOf(":") + 1).trim()]))}
                  class="mono w-full rounded-lg border border-line bg-panel-2 px-2.5 py-2 text-[12px] outline-none focus:border-accent focus:bg-panel"
                ></textarea>
              </label>
              <label class="block">
                <span class="mb-1 flex items-center gap-2 text-[11.5px] font-medium text-muted">
                  Body
                  <label class="ml-auto flex items-center gap-1.5 font-normal"><input type="checkbox" bind:checked={a.base64} class="accent-[var(--accent)]" /> base64 (binary content)</label>
                </span>
                <textarea rows="7" spellcheck="false" bind:value={a.body} class="mono w-full rounded-lg border border-line bg-panel-2 px-2.5 py-2 text-[12px] outline-none focus:border-accent focus:bg-panel"></textarea>
              </label>
            {:else if rule.action.type === "delay"}
              <label class="block max-w-[200px]">
                <span class="mb-1 block text-[11.5px] font-medium text-muted">Wait before forwarding (ms)</span>
                <input type="number" min="0" max="60000" bind:value={rule.action.ms} class="{field} mono" />
              </label>
            {:else if rule.action.type === "block"}
              <p class="text-[12.5px] leading-relaxed text-muted">The app gets a gateway error, as if the network had failed. Nothing is sent to the server.</p>
            {:else}
              {@const a = rule.action}
              <div class="flex gap-4 text-[12.5px]">
                <label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={a.request} class="accent-[var(--accent)]" /> Hold the request</label>
                <label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={a.response} class="accent-[var(--accent)]" /> Hold the response</label>
              </div>
              <p class="text-[12px] leading-relaxed text-muted">Held messages appear at the bottom of the Network view, where you can edit them. Anything left alone continues by itself after 2 minutes.</p>
            {/if}
          </div>
        {:else}
          <div class="grid h-full place-items-center text-center">
            <p class="max-w-[300px] text-[12.5px] leading-relaxed text-muted">Pick a rule to edit it, or add one from the presets.</p>
          </div>
        {/if}
      </div>
    </div>

    <footer class="flex items-center gap-3 border-t border-line px-5 py-3">
      {#if error}<span class="min-w-0 flex-1 truncate text-[12px] text-err">{error}</span>{:else}<span class="flex-1 text-[11.5px] text-faint">Rules apply to the next request and are kept for the next launch.</span>{/if}
      <button onclick={() => (net.rulesOpen = false)} class="h-8 rounded-lg px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">Cancel</button>
      <button onclick={save} disabled={saving || !dirty} class="h-8 rounded-lg bg-accent px-3.5 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">Save rules</button>
    </footer>
  </div>
</div>

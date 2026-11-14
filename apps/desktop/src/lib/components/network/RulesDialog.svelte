<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { api, errorText, type Rule, type RuleAction, type ScriptInfo } from "../../api";
  import { net } from "../../stores/net.svelte";
  import Icon from "../Icon.svelte";

  type Kind = RuleAction["type"];

  const blank = (action: RuleAction, name: string): Rule => ({
    id: 0,
    name,
    enabled: true,
    matcher: { methods: [], host: null, path: null, urlContains: null },
    action,
    thenContinue: false,
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
    { label: "Serve a local file", make: () => blank({ type: "mapLocal", path: "", contentType: null, status: null }, "Local file") },
    { label: "Send to another server", make: () => blank({ type: "mapRemote", scheme: null, host: "localhost", port: 3000, pathPrefix: null, preserveHost: false }, "Another server") },
    { label: "Check against OpenAPI", make: () => blank({ type: "validate", spec: "" }, "Contract") },
    { label: "Run a script", make: () => blank({ type: "script", path: "" }, "Script") },
  ];

  const kinds: [Kind, string][] = [
    ["mock", "Answer"],
    ["delay", "Delay"],
    ["block", "Block"],
    ["breakpoint", "Hold"],
    ["mapLocal", "File"],
    ["mapRemote", "Server"],
    ["validate", "Contract"],
    ["script", "Script"],
  ];
  /** What answers, blocks or holds a request ends the search for rules; the others can let the next rule apply too. */
  const ends = (kind: Kind) => kind === "mock" || kind === "block" || kind === "breakpoint" || kind === "mapLocal";

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
    const fresh: Record<Kind, RuleAction> = {
      mock: { type: "mock", status: 200, headers: [], body: "", base64: false, delayMs: 0 },
      delay: { type: "delay", ms: 1000 },
      block: { type: "block" },
      breakpoint: { type: "breakpoint", request: true, response: false },
      mapLocal: { type: "mapLocal", path: "", contentType: null, status: null },
      mapRemote: { type: "mapRemote", scheme: null, host: "localhost", port: 3000, pathPrefix: null, preserveHost: false },
      validate: { type: "validate", spec: "" },
      script: { type: "script", path: "" },
    };
    rule.action = fresh[kind];
  }

  // The script of the selected rule: what it is, and whether it may run.
  let script = $state<{ path: string; info?: ScriptInfo; error?: string } | null>(null);
  let approving = $state(false);

  $effect(() => {
    const a = rule?.action;
    if (a?.type !== "script" || !a.path) {
      script = null;
      return;
    }
    const path = a.path;
    api
      .scriptInfo(path)
      .then((info) => (script = { path, info }))
      .catch((e) => (script = { path, error: errorText(e) }));
  });

  async function approve(path: string) {
    approving = true;
    try {
      script = { path, info: await api.scriptApprove(path) };
    } catch (e) {
      script = { path, error: errorText(e) };
    } finally {
      approving = false;
    }
  }

  async function choose(kind: "openapi" | "script" | "folder", set: (path: string) => void) {
    const path = kind === "folder" ? await api.rulePickFolder() : await api.rulePickFile(kind);
    if (path) set(path);
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
    else error = "The rules could not be saved. The message above says which one is wrong and why.";
  }

  const leaf = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  const summary = (r: Rule) => {
    const a = r.action;
    switch (a.type) {
      case "mock":
        return `Answers ${a.status}`;
      case "delay":
        return `Waits ${a.ms} ms`;
      case "block":
        return "Fails the request";
      case "breakpoint":
        return "Holds " + [a.request && "request", a.response && "response"].filter(Boolean).join(" and ");
      case "mapLocal":
        return a.path ? `Serves ${leaf(a.path)}` : "Serves a local file";
      case "mapRemote":
        return `Sends to ${[a.scheme && `${a.scheme}://`, a.host, a.port && `:${a.port}`].filter(Boolean).join("") || "another path"}`;
      case "validate":
        return a.spec ? `Checks against ${leaf(a.spec)}` : "Checks against OpenAPI";
      case "script":
        return a.path ? `Runs ${leaf(a.path)}` : "Runs a script";
    }
  };

  const field = "h-8 w-full rounded-lg border border-line bg-panel-2 px-2.5 text-base outline-none placeholder:text-faint focus:border-accent focus:bg-panel";
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (net.rulesOpen = false)} />

<div transition:fade={{ duration: 120 }} class="fixed inset-0 z-50 grid place-items-center bg-black/45 p-8 backdrop-blur-[2px]">
  <button class="absolute inset-0 cursor-default" aria-label="Close" onclick={() => (net.rulesOpen = false)}></button>
  <div transition:scale={{ start: 0.97, duration: 160 }} class="shadow-pop relative flex h-full max-h-[640px] w-[860px] max-w-full flex-col overflow-hidden rounded-2xl border border-line bg-elevated">
    <header class="flex items-center gap-3 border-b border-line px-5 py-3.5">
      <span class="grid size-8 place-items-center rounded-xl bg-accent-soft text-accent"><Icon name="sliders" size={17} /></span>
      <div class="min-w-0 flex-1">
        <h2 class="text-xl font-semibold">Rules</h2>
        <p class="text-sm text-muted">Mock, slow down, block, hold, redirect or check requests. The first matching rule wins, unless it says to keep looking.</p>
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
                <span class="block truncate text-md font-medium {r.enabled ? '' : 'text-faint'}">{r.name || "Untitled rule"}</span>
                <span class="block truncate text-xs text-muted">{summary(r)}</span>
              </button>
              <span class="flex opacity-0 group-hover:opacity-100">
                <button onclick={() => move(i, -1)} class="grid size-6 place-items-center rounded text-faint hover:text-fg" aria-label="Move up"><Icon name="arrowUp" size={12} /></button>
                <button onclick={() => move(i, 1)} class="grid size-6 place-items-center rounded text-faint hover:text-fg" aria-label="Move down"><Icon name="arrowDown" size={12} /></button>
                <button onclick={() => remove(i)} class="grid size-6 place-items-center rounded text-faint hover:text-err" aria-label="Delete"><Icon name="trash" size={12} /></button>
              </span>
            </div>
          {:else}
            <p class="px-3 py-6 text-center text-base leading-relaxed text-muted">No rules yet. Start from one of these:</p>
          {/each}
        </div>
        <div class="border-t border-line p-2">
          <p class="px-1 pb-1.5 text-2xs font-semibold tracking-wider text-faint uppercase">Add</p>
          <div class="flex flex-wrap gap-1.5">
            {#each presets as p (p.label)}
              <button onclick={() => add(p.make)} class="rounded-md border border-line px-2 py-1 text-sm text-muted hover:bg-hover hover:text-fg">{p.label}</button>
            {/each}
          </div>
        </div>
      </div>

      <!-- Editor -->
      <div class="min-w-0 flex-1 overflow-y-auto px-5 py-4">
        {#if rule}
          <label class="block">
            <span class="mb-1 block text-sm font-medium text-muted">Name</span>
            <input bind:value={rule.name} class={field} spellcheck="false" />
          </label>

          <h3 class="mt-5 mb-2 text-2xs font-semibold tracking-wider text-faint uppercase">When the request matches</h3>
          <div class="grid grid-cols-2 gap-3">
            <label class="block">
              <span class="mb-1 block text-sm font-medium text-muted">Host</span>
              <input value={rule.matcher.host ?? ""} oninput={(e) => (rule.matcher.host = blankToNull(e.currentTarget.value))} placeholder="api.example.com or *.example.com" class="{field} mono" spellcheck="false" />
            </label>
            <label class="block">
              <span class="mb-1 block text-sm font-medium text-muted">Path</span>
              <input value={rule.matcher.path ?? ""} oninput={(e) => (rule.matcher.path = blankToNull(e.currentTarget.value))} placeholder="/v1/users/*" class="{field} mono" spellcheck="false" />
            </label>
            <label class="col-span-2 block">
              <span class="mb-1 block text-sm font-medium text-muted">URL contains</span>
              <input value={rule.matcher.urlContains ?? ""} oninput={(e) => (rule.matcher.urlContains = blankToNull(e.currentTarget.value))} placeholder="Any text of the address, query included" class="{field} mono" spellcheck="false" />
            </label>
          </div>
          <div class="mt-3 flex items-center gap-1.5">
            <span class="mr-1 text-sm font-medium text-muted">Methods</span>
            {#each methods as m (m)}
              <button onclick={() => toggleMethod(m)} class="mono h-6 rounded-md border px-2 text-xs {rule.matcher.methods.includes(m) ? 'border-accent bg-accent-soft font-semibold text-accent' : 'border-line text-muted hover:text-fg'}">{m}</button>
            {/each}
            <span class="text-xs text-faint">{rule.matcher.methods.length ? "" : "any"}</span>
          </div>

          <h3 class="mt-5 mb-2 text-2xs font-semibold tracking-wider text-faint uppercase">Then</h3>
          <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="group" aria-label="Action">
            {#each kinds as [k, label] (k)}
              <button onclick={() => setKind(k)} class="h-7 flex-1 rounded-md px-1 text-base {rule.action.type === k ? 'bg-panel font-semibold text-fg shadow-sm' : 'text-muted hover:text-fg'}">{label}</button>
            {/each}
          </div>

          <div class="mt-3 space-y-3">
            {#if rule.action.type === "mock"}
              {@const a = rule.action}
              <div class="grid grid-cols-2 gap-3">
                <label class="block">
                  <span class="mb-1 block text-sm font-medium text-muted">Status</span>
                  <input type="number" min="100" max="599" bind:value={a.status} class="{field} mono" />
                </label>
                <label class="block">
                  <span class="mb-1 block text-sm font-medium text-muted">Delay (ms)</span>
                  <input type="number" min="0" max="60000" bind:value={a.delayMs} class="{field} mono" />
                </label>
              </div>
              <label class="block">
                <span class="mb-1 block text-sm font-medium text-muted">Headers (one per line, <span class="mono">name: value</span>)</span>
                <textarea
                  rows="3"
                  spellcheck="false"
                  value={a.headers.map(([k, v]) => `${k}: ${v}`).join("\n")}
                  oninput={(e) => (a.headers = e.currentTarget.value.split("\n").filter((l) => l.includes(":")).map((l) => [l.slice(0, l.indexOf(":")).trim(), l.slice(l.indexOf(":") + 1).trim()]))}
                  class="mono w-full rounded-lg border border-line bg-panel-2 px-2.5 py-2 text-base outline-none focus:border-accent focus:bg-panel"
                ></textarea>
              </label>
              <label class="block">
                <span class="mb-1 flex items-center gap-2 text-sm font-medium text-muted">
                  Body
                  <label class="ml-auto flex items-center gap-1.5 font-normal"><input type="checkbox" bind:checked={a.base64} class="accent-[var(--accent)]" /> base64 (binary content)</label>
                </span>
                <textarea rows="7" spellcheck="false" bind:value={a.body} class="mono w-full rounded-lg border border-line bg-panel-2 px-2.5 py-2 text-base outline-none focus:border-accent focus:bg-panel"></textarea>
              </label>
            {:else if rule.action.type === "delay"}
              <label class="block max-w-[200px]">
                <span class="mb-1 block text-sm font-medium text-muted">Wait before forwarding (ms)</span>
                <input type="number" min="0" max="60000" bind:value={rule.action.ms} class="{field} mono" />
              </label>
            {:else if rule.action.type === "block"}
              <p class="text-md leading-relaxed text-muted">The app gets a gateway error, as if the network had failed. Nothing is sent to the server.</p>
            {:else if rule.action.type === "breakpoint"}
              {@const a = rule.action}
              <div class="flex gap-4 text-md">
                <label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={a.request} class="accent-[var(--accent)]" /> Hold the request</label>
                <label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={a.response} class="accent-[var(--accent)]" /> Hold the response</label>
              </div>
              <p class="text-base leading-relaxed text-muted">Held messages appear at the bottom of the Network view, where you can edit them. Anything left alone continues by itself after 2 minutes.</p>
            {:else if rule.action.type === "mapLocal"}
              {@const a = rule.action}
              <p class="text-base leading-relaxed text-muted">Answers with a file on this computer, or with the file of a folder that the path asks for: with the path <span class="mono">/assets/*</span>, a request for <span class="mono">/assets/app.js</span> gets <span class="mono">app.js</span> from the folder. Nothing outside the folder is ever served.</p>
              <div class="flex gap-2">
                <input bind:value={a.path} placeholder="/full/path/to/a/file/or/folder" class="{field} mono" spellcheck="false" aria-label="File or folder" />
                <button onclick={() => choose("folder", (p) => (a.path = p))} class="h-8 shrink-0 rounded-lg border border-line px-2.5 text-base hover:bg-hover">Folder…</button>
              </div>
              <div class="grid grid-cols-2 gap-3">
                <label class="block">
                  <span class="mb-1 block text-sm font-medium text-muted">Content type (from the extension when empty)</span>
                  <input value={a.contentType ?? ""} oninput={(e) => (a.contentType = blankToNull(e.currentTarget.value))} placeholder="application/json" class="{field} mono" spellcheck="false" />
                </label>
                <label class="block">
                  <span class="mb-1 block text-sm font-medium text-muted">Status (200 when empty)</span>
                  <input type="number" min="100" max="599" value={a.status ?? ""} oninput={(e) => (a.status = e.currentTarget.value ? Number(e.currentTarget.value) : null)} class="{field} mono" />
                </label>
              </div>
            {:else if rule.action.type === "mapRemote"}
              {@const a = rule.action}
              <p class="text-base leading-relaxed text-muted">Sends the request to another server, as if the app had asked for it. What you leave empty stays as the app asked, so a host and a port alone send the same path to a local server. The Network view still shows what the app asked, and where it was sent.</p>
              <div class="grid grid-cols-3 gap-3">
                <label class="block">
                  <span class="mb-1 block text-sm font-medium text-muted">Scheme</span>
                  <select value={a.scheme ?? ""} onchange={(e) => (a.scheme = blankToNull(e.currentTarget.value))} class={field}><option value="">As asked</option><option value="http">http</option><option value="https">https</option></select>
                </label>
                <label class="block">
                  <span class="mb-1 block text-sm font-medium text-muted">Host</span>
                  <input value={a.host ?? ""} oninput={(e) => (a.host = blankToNull(e.currentTarget.value))} placeholder="localhost" class="{field} mono" spellcheck="false" />
                </label>
                <label class="block">
                  <span class="mb-1 block text-sm font-medium text-muted">Port</span>
                  <input type="number" min="1" max="65535" value={a.port ?? ""} oninput={(e) => (a.port = e.currentTarget.value ? Number(e.currentTarget.value) : null)} class="{field} mono" />
                </label>
              </div>
              <label class="block">
                <span class="mb-1 block text-sm font-medium text-muted">Replace the path before the first <span class="mono">*</span> of the rule's pattern with</span>
                <input value={a.pathPrefix ?? ""} oninput={(e) => (a.pathPrefix = blankToNull(e.currentTarget.value))} placeholder="/v2/  (with the path /api/*, /api/users becomes /v2/users)" class="{field} mono" spellcheck="false" />
              </label>
              <label class="flex items-center gap-1.5 text-md"><input type="checkbox" bind:checked={a.preserveHost} class="accent-[var(--accent)]" /> Keep telling the new server the original host</label>
            {:else if rule.action.type === "validate"}
              {@const a = rule.action}
              <p class="text-base leading-relaxed text-muted">Checks each response against an OpenAPI 3 description (JSON or YAML) after it has been sent, and shows what does not match in the request's Contract tab. It does not change anything.</p>
              <div class="flex gap-2">
                <input bind:value={a.spec} placeholder="/full/path/to/openapi.yaml" class="{field} mono" spellcheck="false" aria-label="OpenAPI file" />
                <button onclick={() => choose("openapi", (p) => (a.spec = p))} class="h-8 shrink-0 rounded-lg border border-line px-2.5 text-base hover:bg-hover">Choose…</button>
              </div>
            {:else if rule.action.type === "script"}
              {@const a = rule.action}
              <p class="text-base leading-relaxed text-muted">Runs a WebAssembly script on the request and on the response. It can change them, answer by itself, or fail the request. It has no files, no network and no clock, and it is stopped after a fixed amount of work. See <span class="mono">docs/rule-scripts.md</span>.</p>
              <div class="flex gap-2">
                <input bind:value={a.path} placeholder="/full/path/to/script.wasm" class="{field} mono" spellcheck="false" aria-label="Script file" />
                <button onclick={() => choose("script", (p) => (a.path = p))} class="h-8 shrink-0 rounded-lg border border-line px-2.5 text-base hover:bg-hover">Choose…</button>
              </div>
              {#if script?.error}
                <p class="rounded-lg border border-err/30 bg-err/8 px-3 py-2 text-base text-err">{script.error}</p>
              {:else if script?.info}
                {@const i = script.info}
                <div class="rounded-xl border border-line bg-panel-2 p-3">
                  <p class="text-md font-medium">{i.name}</p>
                  <p class="mono mt-0.5 text-xs break-all text-faint">{i.sha256}</p>
                  <p class="mt-1 text-sm text-muted">Looks at {[i.onRequest && "requests", i.onResponse && "responses"].filter(Boolean).join(" and ")}.</p>
                  {#if i.approved}
                    <p class="mt-2 flex items-center gap-1.5 text-md text-ok"><Icon name="check" size={13} />Approved: it runs for matching requests.</p>
                  {:else}
                    <p class="mt-2 text-base leading-relaxed text-warn">This script has not been approved, so it does not run. Approve it only if you trust it: it will see the requests and responses this rule matches, including their headers and bodies. The approval is for these exact bytes; if the file changes it has to be approved again.</p>
                    <button onclick={() => approve(a.path)} disabled={approving} class="mt-2 h-8 rounded-lg bg-accent px-3.5 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40">Approve this script</button>
                  {/if}
                </div>
              {/if}
            {/if}

            {#if !ends(rule.action.type)}
              <label class="flex items-start gap-2 rounded-lg border border-line bg-panel-2 px-3 py-2 text-base" title="Without this, the rules below are not looked at for a request this rule matched">
                <input type="checkbox" bind:checked={rule.thenContinue} class="mt-0.5 accent-[var(--accent)]" />
                <span>Keep looking at the rules below this one <span class="text-muted">(so this can be combined with another, such as a delay before an answer)</span></span>
              </label>
            {/if}
          </div>
        {:else}
          <div class="grid h-full place-items-center text-center">
            <p class="max-w-[300px] text-md leading-relaxed text-muted">Pick a rule to edit it, or add one from the presets.</p>
          </div>
        {/if}
      </div>
    </div>

    <footer class="flex items-center gap-3 border-t border-line px-5 py-3">
      {#if error}<span class="min-w-0 flex-1 truncate text-base text-err">{error}</span>{:else}<span class="flex-1 text-sm text-faint">Rules apply to the next request and are kept for the next launch.</span>{/if}
      <button onclick={() => (net.rulesOpen = false)} class="h-8 rounded-lg px-3 text-base font-medium text-muted hover:bg-hover hover:text-fg">Cancel</button>
      <button onclick={save} disabled={saving || !dirty} class="h-8 rounded-lg bg-accent px-3.5 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40">Save rules</button>
    </footer>
  </div>
</div>

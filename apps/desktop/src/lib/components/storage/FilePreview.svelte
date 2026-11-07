<script lang="ts">
  import { api, type Edited, type FileAt, type Preview, type PrefValue } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { confirmed } from "../../confirm";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";
  import KeyValueViewer from "./KeyValueViewer.svelte";
  import RowsViewer from "./RowsViewer.svelte";
  import SqliteViewer from "./SqliteViewer.svelte";

  let {
    preview,
    onsave,
    at = null,
    editing = false,
    initialTable = null,
    ontable,
    onedited,
  }: {
    preview: Preview;
    onsave: () => void;
    at?: FileAt | null;
    editing?: boolean;
    initialTable?: string | null;
    ontable?: (name: string) => void;
    onedited?: (e: Edited) => void;
  } = $props();

  const canEditPrefs = $derived(editing && at != null && preview.kind === "sharedPrefs");
  let editKey = $state<{ key: string; text: string } | null>(null);
  let adding = $state<{ key: string; kind: string; text: string } | null>(null);

  function toPref(kind: string, text: string): PrefValue {
    switch (kind) {
      case "int":
      case "long":
      case "float": {
        const n = Number(text);
        if (text.trim() === "" || !Number.isFinite(n)) throw new Error(`“${text}” is not a number for a ${kind}.`);
        return { type: kind, value: n } as PrefValue;
      }
      case "boolean":
        if (text !== "true" && text !== "false") throw new Error("A boolean is true or false.");
        return { type: "boolean", value: text === "true" };
      case "set":
        return { type: "set", value: text === "" ? [] : text.split(",").map((x) => x.trim()) };
      default:
        return { type: "string", value: text };
    }
  }

  async function writePref(key: string, value: PrefValue | null, message: string) {
    if (!at) return;
    if (!(await confirmed(`${message}\n\nThe app is stopped first. The old file is kept, and Undo puts it back.`, "Change the app's data", "Change it"))) return;
    const done = await app.attempt(() => api.editPref(at, key, value));
    if (done) onedited?.(done);
  }

  async function commitPref(kind: string) {
    const e = editKey;
    editKey = null;
    if (!e) return;
    try {
      await writePref(e.key, toPref(kind, e.text), `Set ${e.key} to “${e.text}”?`);
    } catch (x) {
      app.toast(x instanceof Error ? x.message : String(x), "err");
    }
  }

  async function addPref() {
    const a = adding;
    if (!a || !a.key.trim()) return;
    try {
      await writePref(a.key.trim(), toPref(a.kind, a.text), `Add ${a.key.trim()} = “${a.text}”?`);
      adding = null;
    } catch (x) {
      app.toast(x instanceof Error ? x.message : String(x), "err");
    }
  }

  const kindLabel: Record<string, string> = {
    sqlite: "SQLite database",
    sharedPrefs: "SharedPreferences",
    plist: "Property list",
    json: "JSON",
    dataStore: "Preferences DataStore",
    protoDataStore: "Proto DataStore",
    mmkv: "MMKV",
    realm: "Realm database",
    asyncStorage: "AsyncStorage",
    image: "Image",
    text: "Text",
    binary: "Binary",
  };

  const pretty = $derived.by(() => {
    if (preview.plist !== null && preview.plist !== undefined) return JSON.stringify(preview.plist, null, 2);
    if (preview.kind === "json" && preview.text) {
      try {
        return JSON.stringify(JSON.parse(preview.text), null, 2);
      } catch {
        return preview.text;
      }
    }
    return null;
  });
</script>

<div class="flex h-full min-h-0 flex-col">
  <header class="flex shrink-0 items-center gap-3 border-b border-line px-4 py-2.5">
    <Icon name={preview.kind === "sqlite" || preview.kind === "realm" ? "database" : preview.kind === "image" ? "eye" : "file"} size={15} class="text-accent" />
    <div class="min-w-0 flex-1">
      <p class="truncate text-[13px] font-semibold">{preview.name}</p>
      <p class="text-[11px] text-muted">{kindLabel[preview.kind ?? "binary"]} · {bytes(preview.size)}{preview.truncated ? " · preview truncated" : ""}</p>
    </div>
    <button onclick={onsave} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">
      <Icon name="download" size={13} /> Save…
    </button>
  </header>
  {#if preview.error}
    <p class="shrink-0 border-b border-line bg-warn/10 px-4 py-2 text-[12px] text-warn">{preview.error}</p>
  {/if}
  <div class="min-h-0 flex-1 overflow-auto">
    {#if preview.realm}
      <RowsViewer
        id={preview.id}
        label="Realm classes"
        groups={preview.realm.tables.map((t) => ({
          name: t.name,
          rows: t.rows,
          types: Object.fromEntries(t.columns.map((c) => [c.name, `${c.collection ?? ""}${c.collection ? " of " : ""}${c.kind}${c.nullable ? "?" : ""}`])),
          note: t.columns.some((c) => !c.supported) ? `Columns of a kind that cannot be read yet are shown as empty: ${t.columns.filter((c) => !c.supported).map((c) => c.name).join(", ")}.` : undefined,
        }))}
        load={(name, offset, limit) => api.realmRows(preview.id, name, offset, limit)}
      />
    {:else if preview.entities && preview.tables}
      <div class="flex h-full min-h-0 flex-col">
        <p class="shrink-0 border-b border-line bg-panel-2 px-4 py-1.5 text-[11.5px] text-muted">Core Data store: {preview.entities.length} entities. The raw tables are below the entities.</p>
        <div class="min-h-0 flex-1">
          <RowsViewer
            id={preview.id}
            label="Core Data entities"
            groups={preview.entities.map((e) => ({ name: e.name, rows: e.rows, types: Object.fromEntries(e.attributes.map((a) => [a.name, a.declType])) }))}
            load={(name, offset, limit) => api.coredataRows(preview.id, name, offset, limit)}
          />
        </div>
      </div>
    {:else if preview.tables}
      <SqliteViewer id={preview.id} tables={preview.tables} {at} {editing} {initialTable} {ontable} {onedited} />
    {:else if preview.mmkv}
      <KeyValueViewer
        items={preview.mmkv.map((m) => ({
          key: m.key,
          kind: `${m.kind} (guessed)`,
          value: m.value,
          detail: `${m.items?.length ? `items: ${m.items.join(", ")}\n` : ""}hex: ${m.hex}`,
        }))}
      />
    {:else if preview.kv}
      <KeyValueViewer
        items={preview.kv.map((k) => ({
          key: k.key,
          kind: k.json != null ? "json" : "text",
          value: k.value ?? "(in a file)",
          detail: k.json != null ? JSON.stringify(k.json, null, 2) : undefined,
          note: k.inFile ? "kept in its own file" : undefined,
        }))}
      />
    {:else if preview.prefs}
      <table class="selectable w-full text-[12px]">
        <thead class="sticky top-0 bg-panel-2 text-left text-[11px] text-muted">
          <tr><th class="px-4 py-2 font-medium">Key</th><th class="px-2 py-2 font-medium">Type</th><th class="px-4 py-2 font-medium">Value</th>{#if canEditPrefs}<th class="w-8"></th>{/if}</tr>
        </thead>
        <tbody>
          {#each preview.prefs as p (p.key)}
            <tr class="border-t border-line/60 align-top hover:bg-hover">
              <td class="mono px-4 py-1.5 font-medium">{p.key}</td>
              <td class="px-2 py-1.5"><span class="rounded-md bg-hover px-1.5 py-px text-[10.5px] text-muted">{p.kind}</span></td>
              {#if canEditPrefs && editKey?.key === p.key}
                <td class="p-0">
                  <!-- svelte-ignore a11y_autofocus -->
                  <input autofocus bind:value={editKey.text} onkeydown={(e) => (e.key === "Enter" ? commitPref(p.kind) : e.key === "Escape" ? (editKey = null) : null)} onblur={() => (editKey = null)} class="mono h-7 w-full bg-accent-soft px-4 text-[12px] outline-none" />
                </td>
              {:else}
                <td class="mono px-4 py-1.5 break-all {canEditPrefs ? 'cursor-text' : ''}" ondblclick={() => canEditPrefs && (editKey = { key: p.key, text: p.kind === "set" && p.items ? p.items.join(", ") : p.value })} title={canEditPrefs ? "Double-click to edit" : ""}>{p.value}</td>
              {/if}
              {#if canEditPrefs}
                <td class="px-1 text-center"><button onclick={() => writePref(p.key, null, `Remove ${p.key}?`)} title="Remove this key" aria-label="Remove this key" class="text-faint hover:text-err"><Icon name="trash" size={12} /></button></td>
              {/if}
            </tr>
          {/each}
          {#if canEditPrefs}
            <tr class="border-t border-line/60">
              {#if adding}
                <td class="px-4 py-1.5"><input bind:value={adding.key} placeholder="key" spellcheck="false" class="mono h-7 w-full rounded-md border border-line bg-panel-2 px-2 text-[12px] outline-none focus:border-accent" /></td>
                <td class="px-2 py-1.5">
                  <select bind:value={adding.kind} class="h-7 rounded-md border border-line bg-panel-2 px-1 text-[12px]">
                    {#each ["string", "int", "long", "float", "boolean", "set"] as k (k)}<option>{k}</option>{/each}
                  </select>
                </td>
                <td class="px-4 py-1.5"><input bind:value={adding.text} onkeydown={(e) => e.key === "Enter" && addPref()} placeholder="value" spellcheck="false" class="mono h-7 w-full rounded-md border border-line bg-panel-2 px-2 text-[12px] outline-none focus:border-accent" /></td>
                <td class="px-1"><button onclick={addPref} aria-label="Add" class="text-accent"><Icon name="check" size={13} /></button></td>
              {:else}
                <td colspan="4" class="px-4 py-1.5"><button onclick={() => (adding = { key: "", kind: "string", text: "" })} class="flex items-center gap-1.5 text-[12px] text-muted hover:text-fg"><Icon name="plus" size={12} /> New key</button></td>
              {/if}
            </tr>
          {/if}
        </tbody>
      </table>
    {:else if preview.image}
      <div class="grid min-h-full place-items-center bg-[repeating-conic-gradient(var(--hover)_0_25%,transparent_0_50%)] bg-[length:20px_20px] p-6">
        <img src={preview.image} alt={preview.name} class="max-h-[70vh] max-w-full rounded-lg shadow-lg" />
      </div>
    {:else if pretty}
      <pre class="mono selectable p-4 text-[12px] leading-relaxed whitespace-pre-wrap">{pretty}</pre>
    {:else if preview.text != null}
      <pre class="mono selectable p-4 text-[12px] leading-relaxed whitespace-pre-wrap">{preview.text}</pre>
    {:else if preview.hex}
      <pre class="mono selectable p-4 text-[11.5px] leading-relaxed text-muted">{preview.hex}</pre>
    {/if}
  </div>
</div>

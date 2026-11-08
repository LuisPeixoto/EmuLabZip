<script lang="ts">
  import { api, errorText, type Cell, type Edited, type FileAt, type KeyedRows, type RowKey, type Rows, type Table } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { confirmed } from "../../confirm";
  import Icon from "../Icon.svelte";
  import Split from "../layout/Split.svelte";

  let {
    id,
    tables,
    at = null,
    editing = false,
    initialTable = null,
    ontable,
    onedited,
  }: {
    id: number;
    tables: Table[];
    /** Where the file is on the device; without it the database cannot be changed. */
    at?: FileAt | null;
    editing?: boolean;
    initialTable?: string | null;
    ontable?: (name: string) => void;
    onedited?: (e: Edited) => void;
  } = $props();

  const PAGE = 100;
  let table = $state<Table | null>(null);
  let offset = $state(0);
  let rows = $state<Rows | null>(null);
  let error = $state<string | null>(null);
  let sql = $state("");
  let queryMode = $state(false);
  let loading = $state(false);
  let keyed = $state<KeyedRows | null>(null);
  let editCell = $state<{ row: number; col: number; text: string } | null>(null);
  let adding = $state<Record<string, string> | null>(null);
  const canEdit = $derived(editing && at != null && !queryMode && keyed?.editable === true);

  $effect(() => {
    // Open the first table of each newly opened database.
    id;
    table =
      tables.find((t) => t.name === initialTable) ?? tables.find((t) => t.kind === "table") ?? tables[0] ?? null;
    offset = 0;
    queryMode = false;
  });

  $effect(() => {
    if (!queryMode && table) load(table.name, offset, editing);
  });

  async function load(name: string, from: number, withKeys: boolean) {
    loading = true;
    error = null;
    editCell = null;
    try {
      if (withKeys) {
        keyed = await api.sqliteKeyedRows(id, name, from, PAGE);
        rows = keyed;
      } else {
        keyed = null;
        rows = await api.sqliteRows(id, name, from, PAGE);
      }
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  async function run() {
    if (!sql.trim()) return;
    queryMode = true;
    loading = true;
    error = null;
    try {
      rows = await api.sqliteQuery(id, sql);
    } catch (e) {
      error = errorText(e);
      rows = null;
    } finally {
      loading = false;
    }
  }

  function pick(t: Table) {
    queryMode = false;
    table = t;
    offset = 0;
    adding = null;
    ontable?.(t.name);
    sql = `SELECT * FROM "${t.name.replaceAll('"', '""')}" LIMIT 100`;
  }

  /** The cell a typed text means: a number where the cell held one, NULL for the word NULL, otherwise text. */
  function toCell(text: string, was: unknown): Cell {
    if (text === "NULL") return { type: "null" };
    const n = Number(text);
    if (text.trim() !== "" && Number.isFinite(n) && (typeof was === "number" || was == null)) {
      return Number.isInteger(n) && !/[.eE]/.test(text) ? { type: "integer", value: n } : { type: "real", value: n };
    }
    return { type: "text", value: text };
  }

  async function write(message: string, work: () => Promise<Edited>) {
    if (!(await confirmed(message, "Change the app's data", "Change it"))) return false;
    const done = await app.attempt(work);
    if (!done) return false;
    onedited?.(done);
    return true;
  }

  async function commit() {
    const e = editCell;
    if (!e || !at || !table || !keyed) return;
    const was = keyed.rows[e.row][e.col];
    const column = keyed.columns[e.col];
    editCell = null;
    if (String(was ?? "NULL") === e.text) return;
    const key: RowKey = keyed.keys[e.row];
    await write(
      `Set ${table.name}.${column} to ${e.text === "NULL" ? "NULL" : `“${e.text}”`}?\n\nThe app is stopped first. The old file is kept, and Undo puts it back.`,
      () => api.editCells(at, table!.name, key, [[column, toCell(e.text, was)]]),
    );
  }

  async function remove(row: number) {
    if (!at || !table || !keyed) return;
    await write(
      `Delete this row of ${table.name}?\n\nThe app is stopped first. The old file is kept, and Undo puts it back.`,
      () => api.editDelete(at, table!.name, keyed!.keys[row]),
    );
  }

  async function insert() {
    if (!at || !table || !adding) return;
    const values: [string, Cell][] = Object.entries(adding)
      .filter(([, v]) => v !== "")
      .map(([c, v]) => [c, toCell(v, table!.columns.find((x) => x.name === c)?.declType.match(/INT|REAL|FLOA|DOUB|NUM/i) ? 0 : "")]);
    if (await write(`Add a row to ${table.name}?\n\nThe app is stopped first. The old file is kept, and Undo puts it back.`, () => api.editInsert(at, table!.name, values))) adding = null;
  }

  const cell = (v: unknown) => (v == null ? "NULL" : typeof v === "string" ? v : JSON.stringify(v));
</script>

<Split id="storage.tables" initial={200} min={120} max={420} minOther={300}>
 {#snippet first()}
  <nav class="h-full overflow-y-auto bg-panel-2 p-1.5">
    {#each tables as t (t.name)}
      <button
        onclick={() => pick(t)}
        title={t.sql}
        class="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-base {!queryMode && table?.name === t.name ? 'bg-accent-soft font-medium text-accent' : 'hover:bg-hover'}"
      >
        <Icon name={t.kind === "view" ? "eye" : "database"} size={13} class="shrink-0 text-faint" />
        <span class="min-w-0 flex-1 truncate">{t.name}</span>
        <span class="mono text-2xs text-faint">{t.rows.toLocaleString()}</span>
      </button>
    {:else}
      <p class="p-3 text-base text-muted">The database has no tables.</p>
    {/each}
  </nav>
 {/snippet}
 {#snippet second()}
  <div class="flex h-full min-w-0 flex-col">
    <form onsubmit={(e) => (e.preventDefault(), run())} class="flex shrink-0 items-center gap-2 border-b border-line p-2">
      <input
        bind:value={sql}
        spellcheck="false"
        placeholder="SELECT … (read-only)"
        class="mono h-8 flex-1 rounded-lg border border-line bg-panel-2 px-2.5 text-base outline-none placeholder:text-faint focus:border-accent"
      />
      <button type="submit" class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90">
        <Icon name="play" size={12} /> Run
      </button>
    </form>
    {#if error}
      <p class="border-b border-line bg-err/8 px-3 py-2 text-base text-err">{error}</p>
    {/if}
    {#if editing && keyed && !keyed.editable && !queryMode}
      <p class="border-b border-line bg-warn/10 px-3 py-2 text-base text-warn">This table cannot be changed here: {keyed.reason ?? "it has no key to tell its rows apart"}.</p>
    {/if}
    {#if canEdit && table}
      <div class="shrink-0 border-b border-line px-3 py-2">
        {#if adding}
          <div class="flex flex-wrap items-end gap-2">
            {#each table.columns as c (c.name)}
              <label class="text-2xs text-muted">{c.name}<span class="text-faint"> {c.declType}</span>
                <input bind:value={adding[c.name]} spellcheck="false" class="mono mt-0.5 block h-7 w-32 rounded-md border border-line bg-panel-2 px-2 text-sm text-fg outline-none focus:border-accent" />
              </label>
            {/each}
            <button onclick={insert} class="h-7 rounded-md bg-accent px-3 text-base font-semibold text-white hover:opacity-90">Add row</button>
            <button onclick={() => (adding = null)} class="h-7 rounded-md border border-line px-3 text-base text-muted hover:bg-hover">Cancel</button>
          </div>
        {:else}
          <button onclick={() => (adding = Object.fromEntries(table!.columns.map((c) => [c.name, ""])))} class="flex h-7 items-center gap-1.5 rounded-md border border-line px-2.5 text-base text-muted hover:bg-hover hover:text-fg"><Icon name="plus" size={12} /> New row</button>
        {/if}
      </div>
    {/if}
    <div class="selectable min-h-0 flex-1 overflow-auto">
      {#if rows}
        <table class="mono w-max min-w-full border-separate border-spacing-0 text-sm">
          <thead class="sticky top-0 bg-panel-2">
            <tr>
              {#if canEdit}<th class="w-8 border-b border-line"></th>{/if}
              {#each rows.columns as c, i (i)}
                {@const col = !queryMode ? table?.columns.find((x) => x.name === c) : undefined}
                <th class="border-b border-line px-3 py-1.5 text-left font-semibold whitespace-nowrap">
                  {c}
                  {#if col?.primaryKey}<span class="ml-1 text-tiny text-accent">PK</span>{/if}
                  {#if col?.declType}<span class="ml-1 text-tiny font-normal text-faint">{col.declType}</span>{/if}
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each rows.rows as row, r (r)}
              <tr class="hover:bg-hover">
                {#if canEdit}
                  <td class="border-b border-line/50 px-1 text-center">
                    <button onclick={() => remove(r)} title="Delete this row" aria-label="Delete this row" class="text-faint hover:text-err"><Icon name="trash" size={12} /></button>
                  </td>
                {/if}
                {#each row as v, c (c)}
                  {#if canEdit && editCell?.row === r && editCell?.col === c}
                    <td class="border-b border-line/50 p-0">
                      <!-- svelte-ignore a11y_autofocus -->
                      <input
                        autofocus
                        bind:value={editCell.text}
                        onkeydown={(e) => (e.key === "Enter" ? commit() : e.key === "Escape" ? (editCell = null) : null)}
                        onblur={() => (editCell = null)}
                        class="mono h-7 w-full min-w-[120px] border-0 bg-accent-soft px-3 text-sm outline-none"
                      />
                    </td>
                  {:else}
                    <td
                      ondblclick={() => canEdit && (editCell = { row: r, col: c, text: v == null ? "NULL" : String(v) })}
                      class="max-w-[360px] truncate border-b border-line/50 px-3 py-1 {v == null ? 'text-faint italic' : ''} {canEdit ? 'cursor-text' : ''}"
                      title={canEdit ? `${cell(v)}  (double-click to edit)` : cell(v)}>{cell(v)}</td>
                  {/if}
                {/each}
              </tr>
            {:else}
              <tr><td class="px-3 py-4 text-muted" colspan={rows.columns.length || 1}>No rows.</td></tr>
            {/each}
          </tbody>
        </table>
      {:else if loading}
        <div class="p-4"><div class="skeleton h-4 w-48"></div></div>
      {/if}
    </div>
    {#if !queryMode && table && rows}
      <footer class="flex shrink-0 items-center gap-2 border-t border-line px-3 py-1.5 text-sm text-muted">
        <span class="mono">{(offset + 1).toLocaleString()}–{(offset + rows.rows.length).toLocaleString()} of {table.rows.toLocaleString()}</span>
        <span class="flex-1"></span>
        <button disabled={offset === 0} onclick={() => (offset = Math.max(0, offset - PAGE))} class="h-7 rounded-md border border-line px-2.5 hover:bg-hover disabled:opacity-40">Previous</button>
        <button disabled={!rows.truncated} onclick={() => (offset += PAGE)} class="h-7 rounded-md border border-line px-2.5 hover:bg-hover disabled:opacity-40">Next</button>
      </footer>
    {:else if queryMode && rows}
      <footer class="shrink-0 border-t border-line px-3 py-1.5 text-sm text-muted">
        {rows.rows.length.toLocaleString()} rows{rows.truncated ? " (first 1,000)" : ""}
      </footer>
    {/if}
  </div>
 {/snippet}
</Split>

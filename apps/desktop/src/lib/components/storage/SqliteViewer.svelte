<script lang="ts">
  import { api, errorText, type Rows, type Table } from "../../api";
  import Icon from "../Icon.svelte";

  let { id, tables }: { id: number; tables: Table[] } = $props();

  const PAGE = 100;
  let table = $state<Table | null>(null);
  let offset = $state(0);
  let rows = $state<Rows | null>(null);
  let error = $state<string | null>(null);
  let sql = $state("");
  let queryMode = $state(false);
  let loading = $state(false);

  $effect(() => {
    // Open the first table of each newly opened database.
    id;
    table = tables.find((t) => t.kind === "table") ?? tables[0] ?? null;
    offset = 0;
    queryMode = false;
  });

  $effect(() => {
    if (!queryMode && table) load(table.name, offset);
  });

  async function load(name: string, at: number) {
    loading = true;
    error = null;
    try {
      rows = await api.sqliteRows(id, name, at, PAGE);
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
    sql = `SELECT * FROM "${t.name.replaceAll('"', '""')}" LIMIT 100`;
  }

  const cell = (v: unknown) => (v == null ? "NULL" : typeof v === "string" ? v : JSON.stringify(v));
</script>

<div class="flex h-full min-h-0">
  <nav class="w-[200px] shrink-0 overflow-y-auto border-r border-line bg-panel-2 p-1.5">
    {#each tables as t (t.name)}
      <button
        onclick={() => pick(t)}
        title={t.sql}
        class="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-[12px] {!queryMode && table?.name === t.name ? 'bg-accent-soft font-medium text-accent' : 'hover:bg-hover'}"
      >
        <Icon name={t.kind === "view" ? "eye" : "database"} size={13} class="shrink-0 text-faint" />
        <span class="min-w-0 flex-1 truncate">{t.name}</span>
        <span class="mono text-[10.5px] text-faint">{t.rows.toLocaleString()}</span>
      </button>
    {:else}
      <p class="p-3 text-[12px] text-muted">The database has no tables.</p>
    {/each}
  </nav>
  <div class="flex min-w-0 flex-1 flex-col">
    <form onsubmit={(e) => (e.preventDefault(), run())} class="flex shrink-0 items-center gap-2 border-b border-line p-2">
      <input
        bind:value={sql}
        spellcheck="false"
        placeholder="SELECT … (read-only)"
        class="mono h-8 flex-1 rounded-lg border border-line bg-panel-2 px-2.5 text-[12px] outline-none placeholder:text-faint focus:border-accent"
      />
      <button type="submit" class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90">
        <Icon name="play" size={12} /> Run
      </button>
    </form>
    {#if error}
      <p class="border-b border-line bg-err/8 px-3 py-2 text-[12px] text-err">{error}</p>
    {/if}
    <div class="selectable min-h-0 flex-1 overflow-auto">
      {#if rows}
        <table class="mono w-max min-w-full border-separate border-spacing-0 text-[11.5px]">
          <thead class="sticky top-0 bg-panel-2">
            <tr>
              {#each rows.columns as c, i (i)}
                {@const col = !queryMode ? table?.columns.find((x) => x.name === c) : undefined}
                <th class="border-b border-line px-3 py-1.5 text-left font-semibold whitespace-nowrap">
                  {c}
                  {#if col?.primaryKey}<span class="ml-1 text-[9.5px] text-accent">PK</span>{/if}
                  {#if col?.declType}<span class="ml-1 text-[10px] font-normal text-faint">{col.declType}</span>{/if}
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each rows.rows as row, r (r)}
              <tr class="hover:bg-hover">
                {#each row as v, c (c)}
                  <td class="max-w-[360px] truncate border-b border-line/50 px-3 py-1 {v == null ? 'text-faint italic' : ''}" title={cell(v)}>{cell(v)}</td>
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
      <footer class="flex shrink-0 items-center gap-2 border-t border-line px-3 py-1.5 text-[11.5px] text-muted">
        <span class="mono">{(offset + 1).toLocaleString()}–{(offset + rows.rows.length).toLocaleString()} of {table.rows.toLocaleString()}</span>
        <span class="flex-1"></span>
        <button disabled={offset === 0} onclick={() => (offset = Math.max(0, offset - PAGE))} class="h-7 rounded-md border border-line px-2.5 hover:bg-hover disabled:opacity-40">Previous</button>
        <button disabled={!rows.truncated} onclick={() => (offset += PAGE)} class="h-7 rounded-md border border-line px-2.5 hover:bg-hover disabled:opacity-40">Next</button>
      </footer>
    {:else if queryMode && rows}
      <footer class="shrink-0 border-t border-line px-3 py-1.5 text-[11.5px] text-muted">
        {rows.rows.length.toLocaleString()} rows{rows.truncated ? " (first 1,000)" : ""}
      </footer>
    {/if}
  </div>
</div>

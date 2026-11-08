<script lang="ts">
  import { errorText, type Rows } from "../../api";
  import Icon from "../Icon.svelte";

  /** Rows of named groups (Realm classes, Core Data entities), read a page at a time. Read-only. */
  interface Group {
    name: string;
    rows: number;
    /** Column name to a short description of its type, shown under the header. */
    types?: Record<string, string>;
    note?: string;
  }
  let { id, groups, load, label }: { id: number; groups: Group[]; load: (name: string, offset: number, limit: number) => Promise<Rows>; label: string } = $props();

  const PAGE = 100;
  let current = $state<Group | null>(null);
  let offset = $state(0);
  let rows = $state<Rows | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  $effect(() => {
    id;
    current = groups[0] ?? null;
    offset = 0;
  });

  $effect(() => {
    if (current) read(current.name, offset);
  });

  async function read(name: string, at: number) {
    loading = true;
    error = null;
    try {
      rows = await load(name, at, PAGE);
    } catch (e) {
      error = errorText(e);
      rows = null;
    } finally {
      loading = false;
    }
  }

  const cell = (v: unknown) => (v == null ? "null" : typeof v === "string" ? v : JSON.stringify(v));
</script>

<div class="flex h-full min-h-0">
  <nav class="w-[200px] shrink-0 overflow-y-auto border-r border-line bg-panel-2 p-1.5" aria-label={label}>
    {#each groups as g (g.name)}
      <button
        onclick={() => ((current = g), (offset = 0))}
        class="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-base {current?.name === g.name ? 'bg-accent-soft font-medium text-accent' : 'hover:bg-hover'}"
      >
        <Icon name="database" size={13} class="shrink-0 text-faint" />
        <span class="min-w-0 flex-1 truncate">{g.name}</span>
        <span class="mono text-2xs text-faint">{g.rows.toLocaleString()}</span>
      </button>
    {:else}
      <p class="p-3 text-base text-muted">Nothing to show.</p>
    {/each}
  </nav>
  <div class="flex min-w-0 flex-1 flex-col">
    {#if current?.note}
      <p class="shrink-0 border-b border-line bg-warn/10 px-3 py-2 text-base text-warn">{current.note}</p>
    {/if}
    {#if error}
      <p class="border-b border-line bg-err/8 px-3 py-2 text-base text-err">{error}</p>
    {/if}
    <div class="selectable min-h-0 flex-1 overflow-auto">
      {#if rows}
        <table class="mono w-max min-w-full border-separate border-spacing-0 text-sm">
          <thead class="sticky top-0 bg-panel-2">
            <tr>
              {#each rows.columns as c, i (i)}
                <th class="border-b border-line px-3 py-1.5 text-left font-semibold whitespace-nowrap">
                  {c}
                  {#if current?.types?.[c]}<span class="ml-1 text-tiny font-normal text-faint">{current.types[c]}</span>{/if}
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
    {#if current && rows}
      <footer class="flex shrink-0 items-center gap-2 border-t border-line px-3 py-1.5 text-sm text-muted">
        <span class="mono">{rows.rows.length ? (offset + 1).toLocaleString() : 0}–{(offset + rows.rows.length).toLocaleString()} of {current.rows.toLocaleString()}</span>
        <span class="flex-1"></span>
        <button disabled={offset === 0} onclick={() => (offset = Math.max(0, offset - PAGE))} class="h-7 rounded-md border border-line px-2.5 hover:bg-hover disabled:opacity-40">Previous</button>
        <button disabled={!rows.truncated} onclick={() => (offset += PAGE)} class="h-7 rounded-md border border-line px-2.5 hover:bg-hover disabled:opacity-40">Next</button>
      </footer>
    {/if}
  </div>
</div>

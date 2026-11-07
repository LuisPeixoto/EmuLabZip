<script lang="ts">
  /** Keys and values of a store that is not a table: MMKV and AsyncStorage. Read-only. */
  interface Item {
    key: string;
    kind?: string;
    value: string;
    /** Longer text shown when the row is opened: pretty JSON, hex. */
    detail?: string;
    note?: string;
  }
  let { items, valueHeader = "Value" }: { items: Item[]; valueHeader?: string } = $props();
  let open = $state<string | null>(null);
</script>

<table class="selectable w-full text-[12px]">
  <thead class="sticky top-0 bg-panel-2 text-left text-[11px] text-muted">
    <tr><th class="px-4 py-2 font-medium">Key</th><th class="px-2 py-2 font-medium">Type</th><th class="px-4 py-2 font-medium">{valueHeader}</th></tr>
  </thead>
  <tbody>
    {#each items as it (it.key)}
      <tr class="cursor-pointer border-t border-line/60 align-top hover:bg-hover" onclick={() => (open = open === it.key ? null : it.key)}>
        <td class="mono px-4 py-1.5 font-medium break-all">{it.key}</td>
        <td class="px-2 py-1.5">{#if it.kind}<span class="rounded-md bg-hover px-1.5 py-px text-[10.5px] text-muted">{it.kind}</span>{/if}</td>
        <td class="mono px-4 py-1.5 break-all">{it.value}{#if it.note}<span class="ml-2 text-[10.5px] text-faint">{it.note}</span>{/if}</td>
      </tr>
      {#if open === it.key && it.detail}
        <tr class="bg-panel-2"><td colspan="3"><pre class="mono px-4 py-2 text-[11.5px] whitespace-pre-wrap">{it.detail}</pre></td></tr>
      {/if}
    {:else}
      <tr><td class="px-4 py-6 text-center text-muted" colspan="3">No keys.</td></tr>
    {/each}
  </tbody>
</table>

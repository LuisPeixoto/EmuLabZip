<script lang="ts">
  import { size } from "../../format";
  import { traces } from "../../stores/traces.svelte";
  import Icon from "../Icon.svelte";

  const p = $derived(traces.processor);
  const fraction = $derived(traces.download?.total ? traces.download.done / traces.download.total : null);
</script>

<div class="mx-auto max-w-[520px] rounded-2xl border border-line bg-panel p-6">
  <span class="grid size-10 place-items-center rounded-xl bg-accent-soft text-accent"><Icon name="download" size={18} /></span>
  <h3 class="mt-3 text-[14px] font-semibold">One download to read Perfetto traces</h3>
  {#if p && !p.supported}
    <p class="mt-1.5 text-[12.5px] text-muted">The Perfetto trace processor has no build for this operating system, so Perfetto traces cannot be summarized here. You can still open them in the Perfetto UI.</p>
  {:else}
    <p class="mt-1.5 text-[12.5px] leading-relaxed text-muted">
      EmuLabZip reads traces with Google's <span class="mono">trace_processor_shell</span> {p?.version ?? ""}. It is downloaded once from Perfetto's GitHub release
      {#if p?.sizeBytes}({size(p.sizeBytes)}){/if}, checked against a pinned SHA-256, and kept in EmuLabZip's data folder. Nothing else is sent anywhere.
    </p>
    {#if p?.url}<p class="mono mt-2 truncate text-[11px] text-faint" title={p.url}>{p.url}</p>{/if}
    {#if traces.downloading}
      <div class="mt-4 h-1.5 overflow-hidden rounded-full bg-hover" role="progressbar" aria-valuenow={fraction == null ? undefined : Math.round(fraction * 100)}>
        <div class="h-full rounded-full bg-accent transition-[width] duration-200 {fraction == null ? 'w-1/3 animate-pulse' : ''}" style={fraction == null ? "" : `width: ${fraction * 100}%`}></div>
      </div>
      <p class="mono mt-1.5 text-[11.5px] text-muted">{size(traces.download?.done ?? 0)}{traces.download?.total ? ` / ${size(traces.download.total)}` : ""}</p>
    {:else}
      <button onclick={() => traces.installProcessor()} class="mt-4 flex h-9 items-center gap-1.5 rounded-lg bg-accent px-4 text-[12.5px] font-semibold text-white hover:opacity-90">
        <Icon name="download" size={13} /> Download and verify
      </button>
    {/if}
  {/if}
  <button onclick={() => traces.openInPerfetto()} class="mt-3 block text-[12px] text-accent hover:underline">Open in the Perfetto UI instead</button>
</div>

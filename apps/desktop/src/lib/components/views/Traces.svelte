<script lang="ts">
  import { onMount } from "svelte";
  import type { Recording } from "../../api";
  import { traces, type Tab } from "../../stores/traces.svelte";
  import { size } from "../../format";
  import CpuProfile from "../traces/CpuProfile.svelte";
  import ProcessorPrompt from "../traces/ProcessorPrompt.svelte";
  import Recorder from "../traces/Recorder.svelte";
  import SqlConsole from "../traces/SqlConsole.svelte";
  import Summary from "../traces/Summary.svelte";
  import Icon from "../Icon.svelte";
  import PlatformIcon from "../PlatformIcon.svelte";

  onMount(() => {
    traces.init();
  });

  const rec = $derived(traces.recording);
  const needsProcessor = $derived(rec?.format === "perfetto" && traces.processor != null && !traces.processor.installed);
  const tabs = $derived<{ id: Tab; label: string }[]>(rec?.format === "perfetto" ? [{ id: "summary", label: "Summary" }, { id: "sql", label: "SQL" }] : [{ id: "flame", label: "Flame graph" }]);

  function title(r: Recording): string {
    const q = r.request;
    if (q.kind === "perfetto") return { cpu: "CPU & scheduling", frames: "Frames & jank", memory: "Memory", custom: "Custom trace" }[q.preset];
    if (q.kind === "cpuProfile") return q.systemWide ? "CPU profile · whole system" : `CPU profile · ${q.app}`;
    return q.template;
  }

  function when(ms: number): string {
    return new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
  }

  function confirmDelete(r: Recording) {
    traces.remove(r.id);
  }
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1280px] p-6">
    <header class="mb-5">
      <h1 class="text-[18px] font-semibold tracking-tight">Traces</h1>
      <p class="text-[12.5px] text-muted">Record what the system and an app did, then find out why it was slow.</p>
    </header>

    <div class="grid items-start gap-5 lg:grid-cols-[340px_1fr]">
      <div class="space-y-4">
        <Recorder />

        <section class="rounded-2xl border border-line bg-panel p-2">
          <h2 class="px-2 pt-1.5 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">Recordings</h2>
          {#if !traces.recordings.length}
            <p class="px-2 py-4 text-center text-[12px] text-muted">Nothing recorded yet.</p>
          {/if}
          <ul class="max-h-[420px] space-y-0.5 overflow-y-auto">
            {#each traces.recordings as r (r.id)}
              <li class="group relative">
                <button
                  onclick={() => traces.open(r.id)}
                  class="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-2 text-left {r.id === traces.selectedId ? 'bg-accent-soft' : 'hover:bg-hover'}"
                >
                  <PlatformIcon platform={r.platform} size={15} />
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-[12.5px] font-medium">{title(r)}</span>
                    <span class="block truncate text-[11px] text-muted">{r.device} · {when(r.createdMs)} · {size(r.sizeBytes)}</span>
                  </span>
                </button>
                <button
                  onclick={() => confirmDelete(r)}
                  title="Delete this recording"
                  aria-label="Delete this recording"
                  class="absolute top-1/2 right-2 grid size-7 -translate-y-1/2 place-items-center rounded-lg text-faint opacity-0 group-hover:opacity-100 hover:bg-hover hover:text-err focus-visible:opacity-100"
                >
                  <Icon name="trash" size={13} />
                </button>
              </li>
            {/each}
          </ul>
        </section>
      </div>

      <section class="min-w-0">
        {#if !rec}
          <div class="grid place-items-center rounded-2xl border border-dashed border-line-strong bg-panel/50 px-6 py-24 text-center">
            <Icon name="route" size={26} class="mb-3 text-faint" />
            <p class="text-[13.5px] font-medium">Record a trace, or pick one on the left</p>
            <p class="mt-1 max-w-[440px] text-[12.5px] text-muted">
              System traces open as a summary with SQL. CPU profiles open as a flame graph. Anything can also be opened in Perfetto or Instruments for the full timeline.
            </p>
          </div>
        {:else}
          <div class="mb-4 flex flex-wrap items-center gap-3">
            <div class="min-w-0 flex-1">
              <h2 class="truncate text-[15px] font-semibold">{title(rec)}</h2>
              <p class="truncate text-[12px] text-muted">{rec.device} · {when(rec.createdMs)} · {size(rec.sizeBytes)} · {rec.request.durationS} s</p>
            </div>
            {#if rec.format === "perfetto"}
              <button onclick={() => traces.openInPerfetto()} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">
                <Icon name="link" size={13} /> Open in Perfetto UI
              </button>
            {:else if rec.format === "instrumentsBundle"}
              <button onclick={() => traces.showExternally()} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">
                <Icon name="link" size={13} /> Open in Instruments
              </button>
            {/if}
            {#if rec.format !== "instrumentsBundle"}
              <button onclick={() => traces.saveCopy()} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">
                <Icon name="download" size={13} /> Save…
              </button>
            {/if}
            <button onclick={() => traces.showExternally()} title="Show in folder" class="grid size-8 place-items-center rounded-lg border border-line text-muted hover:bg-hover hover:text-fg" aria-label="Show in folder">
              <Icon name="folder" size={13} />
            </button>
          </div>

          {#if tabs.length > 1}
            <div class="mb-4 inline-flex gap-1 rounded-xl bg-panel-2 p-1" role="tablist">
              {#each tabs as t (t.id)}
                <button role="tab" aria-selected={traces.tab === t.id} onclick={() => (traces.tab = t.id)} class="h-7 rounded-lg px-3.5 text-[12px] font-medium {traces.tab === t.id ? 'bg-panel text-fg shadow-sm' : 'text-muted hover:text-fg'}">{t.label}</button>
              {/each}
            </div>
          {/if}

          {#if needsProcessor}
            <ProcessorPrompt />
          {:else if traces.opening}
            <div class="grid place-items-center py-24 text-[12.5px] text-muted"><Icon name="refresh" size={18} class="mb-2 animate-spin text-faint" />Reading the trace…</div>
          {:else if traces.openError}
            <div class="flex items-start gap-2 rounded-xl border border-err/30 bg-err/8 px-4 py-3 text-[12.5px] text-err">
              <Icon name="alert" size={14} class="mt-0.5 shrink-0" /><span class="min-w-0 break-words">{traces.openError}</span>
            </div>
          {:else if traces.opened?.view === "perfetto"}
            {#if traces.tab === "sql"}<SqlConsole />{:else}<Summary summary={traces.opened.summary} />{/if}
          {:else if traces.cpu}
            <CpuProfile view={traces.cpu} />
          {:else if traces.opened?.view === "external"}
            <div class="grid place-items-center rounded-2xl border border-dashed border-line-strong bg-panel/50 px-6 py-16 text-center">
              <Icon name="gauge" size={26} class="mb-3 text-faint" />
              <p class="max-w-[440px] text-[12.5px] text-muted">{traces.opened.reason}</p>
              <button onclick={() => traces.showExternally()} class="mt-4 flex h-9 items-center gap-1.5 rounded-lg bg-accent px-4 text-[12.5px] font-semibold text-white hover:opacity-90"><Icon name="link" size={13} /> Open in Instruments</button>
            </div>
          {/if}
        {/if}
      </section>
    </div>
  </div>
</div>

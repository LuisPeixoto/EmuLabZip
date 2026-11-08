<script lang="ts">
  import type { HeapAnalysis } from "../../api";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";

  let { analysis, path }: { analysis: HeapAnalysis; path: string } = $props();

  type Tab = "leaks" | "dominators" | "classes";
  let tab = $state<Tab>("leaks");

  const stepText = (s: { class: string; field: string | null }) =>
    s.field == null ? s.class : s.field === "[]" ? `${s.class}[]` : `${s.class}.${s.field}`;
  const name = $derived(path.split(/[\\/]/).pop() ?? path);
</script>

<section class="rounded-2xl border border-line bg-panel">
  <header class="flex flex-wrap items-center gap-3 border-b border-line px-4 py-2.5">
    <h2 class="text-md font-semibold">Heap analysis</h2>
    <span class="mono min-w-0 flex-1 truncate text-sm text-faint" title={path}>{name} · {analysis.summary.objects.toLocaleString()} objects{analysis.summary.truncated ? " · cut short" : ""}</span>
    <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="tablist">
      {#each [["leaks", `Suspected leaks (${analysis.leaks.length})`], ["dominators", "Dominators"], ["classes", "Classes"]] as [id, label] (id)}
        <button role="tab" aria-selected={tab === id} onclick={() => (tab = id as Tab)} class="h-7 rounded-md px-2.5 text-sm {tab === id ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}">{label}</button>
      {/each}
    </div>
  </header>

  <div class="selectable">
    {#if tab === "leaks"}
      {#each analysis.leaks as l (l.id)}
        <article class="border-b border-line/60 px-4 py-3 last:border-0">
          <div class="flex items-center gap-2 text-md">
            <Icon name="alert" size={13} class={l.confidence === "certain" ? "text-err" : "text-warn"} />
            <span class="mono font-semibold">{l.class}</span>
            <span class="mono text-faint">{l.id}</span>
            <span class="ml-auto text-muted">keeps {bytes(l.retainedBytes)} alive</span>
          </div>
          <p class="mt-1 text-base text-muted">{l.reason}{l.confidence === "possible" ? " (possible)" : ""}</p>
          <p class="mono mt-1.5 text-sm leading-relaxed break-all text-muted">
            <span class="text-faint">{l.root}</span>{#each l.path as s, i (i)}<span class="text-faint"> → </span>{stepText(s)}{/each}
          </p>
        </article>
      {:else}
        <p class="px-4 py-6 text-center text-md text-muted">No destroyed Activity or removed Fragment is still alive.</p>
      {/each}
    {:else if tab === "dominators"}
      <table class="w-full text-base">
        <tbody>
          {#each analysis.dominators as d (d.id)}
            <tr class="border-b border-line/60 last:border-0">
              <td class="mono w-[90px] px-4 py-1.5 text-right font-medium">{bytes(d.retainedBytes)}</td>
              <td class="mono px-2 py-1.5">{d.class}</td>
              <td class="mono px-4 py-1.5 text-right text-faint">{d.id}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <table class="w-full text-base">
        <tbody>
          {#each analysis.classes as c (c.name)}
            <tr class="border-b border-line/60 last:border-0">
              <td class="mono w-[90px] px-4 py-1.5 text-right font-medium">{bytes(c.shallowBytes)}</td>
              <td class="mono w-[90px] px-2 py-1.5 text-right text-faint">{c.count.toLocaleString()}×</td>
              <td class="mono px-4 py-1.5">{c.name}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</section>

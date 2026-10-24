<script lang="ts">
  import type { EditsInput, Paused } from "../../api";
  import { net } from "../../stores/net.svelte";
  import Icon from "../Icon.svelte";

  let index = $state(0);
  const item = $derived<Paused | undefined>(net.paused[Math.min(index, net.paused.length - 1)]);

  const toLines = (h: [string, string][]) => h.map(([k, v]) => `${k}: ${v}`).join("\n");
  const toHeaders = (t: string): [string, string][] =>
    t
      .split("\n")
      .filter((l) => l.includes(":"))
      .map((l) => [l.slice(0, l.indexOf(":")).trim(), l.slice(l.indexOf(":") + 1).trim()]);

  // What is being edited belongs to the message shown, so it starts over when another one is picked.
  let headers = $state("");
  let body = $state("");
  let status = $state(200);
  let shownId = $state<number | null>(null);

  $effect(() => {
    if (item && item.id !== shownId) {
      shownId = item.id;
      headers = toLines(item.headers);
      body = item.bodyText ?? "";
      status = item.status ?? 200;
    }
  });

  function go() {
    if (!item) return;
    const edits: EditsInput = {};
    if (headers !== toLines(item.headers)) edits.headers = toHeaders(headers);
    if (item.bodyText != null && body !== item.bodyText) edits.bodyText = body;
    if (item.phase === "response" && status !== item.status) edits.status = Number(status);
    net.resume(item.id, false, edits);
  }
</script>

{#if item}
  <div class="flex h-[46%] min-h-[220px] shrink-0 flex-col border-t-2 border-warn bg-panel-2">
    <header class="flex shrink-0 items-center gap-2 border-b border-line px-4 py-2">
      <span class="grid size-6 place-items-center rounded-md bg-warn/15 text-warn"><Icon name="pause" size={13} /></span>
      <span class="text-[12.5px] font-semibold">Held {item.phase}</span>
      {#if net.paused.length > 1}
        <span class="flex items-center gap-1">
          {#each net.paused as p, i (p.id)}
            <button onclick={() => (index = i)} class="mono h-5 min-w-5 rounded px-1 text-[10.5px] {item.id === p.id ? 'bg-warn/20 font-semibold text-warn' : 'text-faint hover:text-fg'}">{i + 1}</button>
          {/each}
        </span>
      {/if}
      <span class="mono min-w-0 flex-1 truncate text-[11.5px] text-muted" title={item.url}>{item.method} {item.url}</span>
      {#if net.paused.length > 1}
        <button onclick={() => net.resumeAll()} class="h-7 rounded-lg px-2.5 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">Continue all</button>
      {/if}
      <button onclick={() => net.resume(item.id, true)} class="h-7 rounded-lg px-2.5 text-[12px] font-medium text-err hover:bg-err/10">Abort</button>
      <button onclick={go} class="h-7 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90">Continue</button>
    </header>

    <div class="grid min-h-0 flex-1 grid-cols-2 gap-3 p-3">
      <label class="flex min-h-0 flex-col">
        <span class="mb-1 flex items-center text-[11px] font-medium text-muted">
          Headers
          {#if item.phase === "response"}
            <span class="ml-auto flex items-center gap-1.5">Status <input type="number" min="100" max="599" bind:value={status} class="mono h-6 w-16 rounded-md border border-line bg-panel px-1.5 text-[11.5px] outline-none focus:border-accent" /></span>
          {/if}
        </span>
        <textarea bind:value={headers} spellcheck="false" class="mono min-h-0 flex-1 resize-none rounded-lg border border-line bg-panel px-2.5 py-2 text-[11.5px] outline-none focus:border-accent"></textarea>
      </label>
      <label class="flex min-h-0 flex-col">
        <span class="mb-1 text-[11px] font-medium text-muted">Body {#if item.bodyText == null}<span class="font-normal text-faint">· not text, sent as it is</span>{/if}</span>
        <textarea bind:value={body} disabled={item.bodyText == null} spellcheck="false" class="mono min-h-0 flex-1 resize-none rounded-lg border border-line bg-panel px-2.5 py-2 text-[11.5px] outline-none focus:border-accent disabled:opacity-50"></textarea>
      </label>
    </div>
  </div>
{/if}

<script lang="ts">
  import type { WsMessage } from "../../api";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";

  let { messages, dropped }: { messages: WsMessage[]; dropped: number } = $props();

  let filter = $state("");
  let open = $state<number | null>(null);

  const shown = $derived(
    messages.map((m, i) => ({ m, i })).filter(({ m }) => !filter.trim() || (m.text ?? "").toLowerCase().includes(filter.trim().toLowerCase())),
  );

  function hex(b64: string): string {
    const raw = atob(b64).slice(0, 256);
    return [...raw].map((c) => c.charCodeAt(0).toString(16).padStart(2, "0")).join(" ") + (atob(b64).length > 256 ? " …" : "");
  }

  function pretty(text: string): string {
    try {
      return JSON.stringify(JSON.parse(text), null, 2);
    } catch {
      return text;
    }
  }

  const preview = (m: WsMessage) =>
    m.kind === "text" ? (m.text ?? "").replace(/\s+/g, " ").slice(0, 200) : m.kind === "binary" ? `${bytes(m.len)} binary` : m.kind;

  const seconds = (ms: number) => `${(ms / 1000).toFixed(2)} s`;
</script>

<div class="flex items-center gap-2 border-b border-line px-4 py-2">
  <label class="relative flex flex-1 items-center">
    <Icon name="search" size={12} class="pointer-events-none absolute left-2.5 text-faint" />
    <input bind:value={filter} spellcheck="false" placeholder="Filter messages" class="h-7 w-full rounded-md border border-line bg-panel pr-2 pl-7 text-[12px] outline-none placeholder:text-faint focus:border-accent" />
  </label>
  <span class="mono text-[11px] text-faint">{shown.length} of {messages.length + dropped}</span>
</div>

{#if !messages.length}
  <p class="px-4 py-8 text-center text-[12.5px] text-muted">No messages yet.</p>
{:else}
  <ul class="selectable">
    {#each shown as { m, i } (i)}
      <li class="border-b border-line/40">
        <button onclick={() => (open = open === i ? null : i)} class="mono flex w-full items-center gap-2.5 px-4 py-1.5 text-left text-[11.5px] hover:bg-hover">
          <Icon name={m.direction === "up" ? "arrowUp" : "arrowDown"} size={12} class={m.direction === "up" ? "text-accent" : "text-ok"} />
          <span class="w-[52px] shrink-0 text-faint">{seconds(m.tMs)}</span>
          <span class="min-w-0 flex-1 truncate {m.kind === 'text' ? '' : 'text-muted'}">{preview(m)}</span>
          <span class="shrink-0 text-faint">{bytes(m.len)}</span>
        </button>
        {#if open === i}
          <pre class="mono max-h-[260px] overflow-auto bg-panel px-4 py-2.5 text-[11.5px] leading-relaxed break-all whitespace-pre-wrap">{m.text != null ? pretty(m.text) : m.base64 ? hex(m.base64) : "(empty)"}</pre>
          {#if m.truncated}<p class="bg-warn/10 px-4 py-1.5 text-[11.5px] text-warn">Only the first part of this message is stored.</p>{/if}
        {/if}
      </li>
    {/each}
  </ul>
  {#if dropped}
    <p class="px-4 py-2 text-[11.5px] text-warn">{dropped.toLocaleString()} later messages were not kept.</p>
  {/if}
{/if}

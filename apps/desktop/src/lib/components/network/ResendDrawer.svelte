<script lang="ts">
  import type { ResendInput } from "../../api";
  import { net } from "../../stores/net.svelte";
  import Icon from "../Icon.svelte";

  const d = $derived(net.resending);
  const flow = $derived(d?.flow);

  const toLines = (h: [string, string][]) => h.map(([k, v]) => `${k}: ${v}`).join("\n");
  const toHeaders = (t: string): [string, string][] =>
    t
      .split("\n")
      .filter((l) => l.includes(":"))
      .map((l) => [l.slice(0, l.indexOf(":")).trim(), l.slice(l.indexOf(":") + 1).trim()]);

  // What is edited belongs to the request shown, so it starts over when another one is opened.
  let method = $state("GET");
  let url = $state("");
  let headers = $state("");
  let body = $state("");
  let shownId = $state<number | null>(null);

  $effect(() => {
    if (flow && flow.id !== shownId) {
      shownId = flow.id;
      method = flow.method;
      url = flow.url;
      headers = toLines(flow.requestHeaders);
      body = d?.request?.raw ?? d?.request?.text ?? "";
    }
  });

  // A GraphQL body is shown as a query and variables, but it is edited as the JSON that was sent.
  const bodyEditable = $derived(!d?.request || d.request.raw != null || (d.request.text != null && !d.request.label));

  function send() {
    if (!flow) return;
    const edits: ResendInput = {};
    if (method !== flow.method) edits.method = method;
    if (url !== flow.url) edits.url = url;
    if (headers !== toLines(flow.requestHeaders)) edits.headers = toHeaders(headers);
    if (bodyEditable && body !== (d?.request?.raw ?? d?.request?.text ?? "")) edits.bodyText = body;
    net.resend(flow.id, edits);
  }
</script>

{#if flow}
  <div class="flex h-[46%] min-h-[240px] shrink-0 flex-col border-t-2 border-accent bg-panel-2">
    <header class="flex shrink-0 items-center gap-2 border-b border-line px-4 py-2">
      <span class="grid size-6 place-items-center rounded-md bg-accent-soft text-accent"><Icon name="refresh" size={13} /></span>
      <span class="text-[12.5px] font-semibold">Send again</span>
      <input bind:value={method} aria-label="Method" spellcheck="false" class="mono h-7 w-20 rounded-md border border-line bg-panel px-2 text-[11.5px] uppercase outline-none focus:border-accent" />
      <input bind:value={url} aria-label="Address" spellcheck="false" class="mono h-7 min-w-0 flex-1 rounded-md border border-line bg-panel px-2 text-[11.5px] outline-none focus:border-accent" />
      <button onclick={() => (net.resending = null)} class="h-7 rounded-lg px-2.5 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">Cancel</button>
      <button onclick={send} class="h-7 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90">Send</button>
    </header>
    <div class="grid min-h-0 flex-1 grid-cols-2 gap-3 p-3">
      <label class="flex min-h-0 flex-col">
        <span class="mb-1 text-[11px] font-medium text-muted">Headers</span>
        <textarea bind:value={headers} spellcheck="false" class="mono min-h-0 flex-1 resize-none rounded-lg border border-line bg-panel px-2.5 py-2 text-[11.5px] outline-none focus:border-accent"></textarea>
      </label>
      <label class="flex min-h-0 flex-col">
        <span class="mb-1 text-[11px] font-medium text-muted">
          Body {#if !bodyEditable}<span class="font-normal text-faint">· sent as it was, it is not plain text</span>{/if}
        </span>
        <textarea bind:value={body} disabled={!bodyEditable} spellcheck="false" class="mono min-h-0 flex-1 resize-none rounded-lg border border-line bg-panel px-2.5 py-2 text-[11.5px] outline-none focus:border-accent disabled:opacity-50"></textarea>
      </label>
    </div>
  </div>
{/if}

<script lang="ts">
  import { app } from "../../stores/app.svelte";
  import { net } from "../../stores/net.svelte";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";
  import BodyViewer from "./BodyViewer.svelte";

  type Tab = "headers" | "request" | "response" | "timing";
  let tab = $state<Tab>("headers");

  const d = $derived(net.detail);
  const flow = $derived(d?.flow);
  const tunnel = $derived(flow?.kind === "tunnel");

  async function copyUrl() {
    if (!flow) return;
    await navigator.clipboard?.writeText(flow.url).catch(() => {});
    app.toast("URL copied", "ok");
  }

  const tabs = $derived<{ id: Tab; label: string; note?: string }[]>([
    { id: "headers", label: "Headers" },
    { id: "request", label: "Request", note: d?.request ? bytes(d.request.size) : undefined },
    { id: "response", label: "Response", note: d?.response ? bytes(d.response.size) : undefined },
    { id: "timing", label: "Timing" },
  ]);

  const rows = $derived(
    flow
      ? [
          ["Waiting for the first byte", flow.timing.ttfbMs],
          ["Receiving the response", flow.timing.totalMs != null && flow.timing.ttfbMs != null ? Math.max(0, flow.timing.totalMs - flow.timing.ttfbMs) : null],
        ]
      : [],
  );
</script>

{#snippet headers(list: [string, string][], title: string)}
  <h3 class="px-4 pt-3 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">{title} <span class="font-normal normal-case">({list.length})</span></h3>
  <dl class="selectable px-2 pb-2">
    {#each list as [k, v], i (i)}
      <div class="flex gap-3 rounded-md px-2 py-1 hover:bg-hover">
        <dt class="mono w-[200px] shrink-0 truncate text-[11.5px] font-medium" title={k}>{k}</dt>
        <dd class="mono min-w-0 flex-1 text-[11.5px] break-all text-muted">{v}</dd>
      </div>
    {:else}
      <p class="px-2 py-1 text-[12px] text-faint">None</p>
    {/each}
  </dl>
{/snippet}

<div class="flex h-full min-h-0 flex-col bg-panel-2">
  {#if net.detailError}
    <p class="p-4 text-[12.5px] text-err">{net.detailError}</p>
  {:else if !flow}
    <div class="p-4"><div class="skeleton h-4 w-64"></div></div>
  {:else}
    <header class="flex shrink-0 items-center gap-2 px-4 pt-3 pb-1.5">
      <span class="mono rounded-md bg-hover px-1.5 py-px text-[11px] font-semibold">{tunnel ? "TUNNEL" : flow.method}</span>
      <span class="mono min-w-0 flex-1 truncate text-[12px]" title={flow.url}>{tunnel ? `${flow.host}:${flow.port}` : flow.url}</span>
      {#if flow.status}
        <span class="mono text-[12px] font-semibold {flow.status >= 400 ? 'text-err' : flow.status >= 300 ? 'text-info' : 'text-ok'}">{flow.status}</span>
      {/if}
      <button onclick={copyUrl} title="Copy the URL" class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg"><Icon name="copy" size={13} /></button>
      <button onclick={() => net.select(null)} title="Close" class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg"><Icon name="x" size={13} /></button>
    </header>

    {#if flow.error}
      <p class="mx-4 mb-1 flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2 text-[12px] text-warn">
        <Icon name="alert" size={14} class="mt-px shrink-0" />{flow.error}
      </p>
    {/if}

    {#if tunnel}
      <div class="min-h-0 flex-1 overflow-y-auto px-4 py-3 text-[12.5px] text-muted">
        <p>This connection was passed through without being decoded, so its contents are not available.</p>
        <p class="mono mt-3 text-[12px]">{bytes(flow.requestSize)} sent · {bytes(flow.responseSize)} received</p>
      </div>
    {:else}
      <div class="flex shrink-0 gap-1 border-b border-line px-3" role="tablist">
        {#each tabs as t (t.id)}
          <button
            role="tab"
            aria-selected={tab === t.id}
            onclick={() => (tab = t.id)}
            class="-mb-px flex h-8 items-center gap-1.5 border-b-2 px-2.5 text-[12px] {tab === t.id ? 'border-accent font-medium text-accent' : 'border-transparent text-muted hover:text-fg'}"
          >
            {t.label}{#if t.note}<span class="mono text-[10.5px] text-faint">{t.note}</span>{/if}
          </button>
        {/each}
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto">
        {#if tab === "headers"}
          {@render headers(flow.requestHeaders, "Request")}
          {@render headers(flow.responseHeaders, "Response")}
        {:else if tab === "request"}
          <BodyViewer body={d?.request ?? null} empty="This request has no body." />
        {:else if tab === "response"}
          <BodyViewer body={d?.response ?? null} empty={flow.state === "pending" ? "Waiting for the response…" : "This response has no body."} />
        {:else}
          <dl class="mono grid max-w-[460px] grid-cols-[1fr_auto] gap-x-6 gap-y-1.5 px-4 py-4 text-[12px]">
            <dt class="text-muted">Protocol</dt><dd>{flow.httpVersion}</dd>
            {#each rows as [label, ms] (label)}
              <dt class="text-muted">{label}</dt><dd>{ms == null ? "—" : `${ms} ms`}</dd>
            {/each}
            <dt class="font-semibold">Total</dt><dd class="font-semibold">{flow.timing.totalMs == null ? "—" : `${flow.timing.totalMs} ms`}</dd>
            <dt class="pt-2 text-muted">Started</dt><dd class="pt-2">{new Date(flow.startedMs).toLocaleTimeString()}</dd>
          </dl>
        {/if}
      </div>
    {/if}
  {/if}
</div>

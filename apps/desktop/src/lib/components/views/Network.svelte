<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import type { FlowSummary } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { net, type StatusFilter } from "../../stores/net.svelte";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";
  import CertificateDialog from "../network/CertificateDialog.svelte";
  import FlowDetail from "../network/FlowDetail.svelte";
  import BreakpointDrawer from "../network/BreakpointDrawer.svelte";
  import RulesDialog from "../network/RulesDialog.svelte";

  const ROW = 28;
  const OVERSCAN = 20;

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(500);
  let follow = $state(true);
  let searchEl: HTMLInputElement | undefined = $state();

  const rows = $derived(net.visible);
  const start = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const end = $derived(Math.min(rows.length, Math.ceil((scrollTop + viewport) / ROW) + OVERSCAN));
  const slice = $derived(rows.slice(start, end));
  const ios = $derived(app.device?.platform === "ios");

  onMount(() => {
    net.init();
    const timer = setInterval(() => net.refreshDevice(), 4000);
    return () => clearInterval(timer);
  });

  // Look at the device whenever another one is picked, or it comes online.
  $effect(() => {
    app.deviceKey;
    untrack(() => net.refreshDevice());
  });

  $effect(() => {
    rows.length;
    if (follow && scroller) tick().then(() => scroller && (scroller.scrollTop = scroller.scrollHeight));
  });

  function onscroll() {
    if (!scroller) return;
    scrollTop = scroller.scrollTop;
    follow = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < ROW * 2;
  }

  function jumpToEnd() {
    follow = true;
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  }

  function onkeydown(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === "f") {
      e.preventDefault();
      searchEl?.focus();
      searchEl?.select();
    } else if (mod && e.key.toLowerCase() === "k") {
      e.preventDefault();
      net.clear();
    } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && !(e.target instanceof HTMLInputElement)) {
      e.preventDefault();
      const i = rows.findIndex((f) => f.id === net.selectedId);
      const next = rows[Math.min(rows.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)))];
      if (next) net.select(next.id);
    }
  }

  const statusChips: { id: StatusFilter; label: string }[] = [
    { id: "all", label: "All" },
    { id: "2xx", label: "2xx" },
    { id: "3xx", label: "3xx" },
    { id: "4xx", label: "4xx" },
    { id: "5xx", label: "5xx" },
    { id: "failed", label: "Failed" },
  ];

  function statusText(f: FlowSummary): string {
    if (f.state === "paused") return "HOLD";
    if (f.status) return String(f.status);
    return f.state === "failed" ? "ERR" : f.kind === "tunnel" ? "—" : "…";
  }

  function statusTone(f: FlowSummary): string {
    if (f.state === "paused") return "text-warn bg-warn/15";
    const s = f.status ?? 0;
    if (s >= 500 || (f.state === "failed" && !s)) return "text-err bg-err/12";
    if (s >= 400) return "text-warn bg-warn/12";
    if (s >= 300) return "text-info bg-info/12";
    if (s >= 200) return "text-ok bg-ok/12";
    return "text-muted bg-hover";
  }

  const kind = (f: FlowSummary) => (f.kind === "tunnel" ? "TUNNEL" : f.kind === "webSocket" ? "WS" : f.method);

  function type(f: FlowSummary): string {
    if (f.kind !== "http") return f.kind === "webSocket" ? "websocket" : "";
    return (f.contentType ?? "").split("/").pop()?.replace(/^x-/, "") ?? "";
  }

  function time(ms: number | null): string {
    if (ms == null) return "";
    return ms >= 1000 ? `${(ms / 1000).toFixed(2)} s` : `${ms} ms`;
  }

  const canConnect = $derived(!!app.target && app.online);
  const protocol = (f: FlowSummary) => (f.kind === "http" ? (f.serverProtocol || "") : "");
  const enabledRules = $derived(net.rules.filter((r) => r.enabled).length);
  let menu = $state(false);
  let showDns = $state(false);
</script>

<svelte:window {onkeydown} />

<div class="flex h-full flex-col">
  <!-- Proxy and device -->
  <div class="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b border-line bg-panel px-4 py-3">
    <div class="min-w-0">
      <h1 class="text-[15px] font-semibold tracking-tight">Network</h1>
      <p class="flex items-center gap-1.5 text-[11.5px] text-muted">
        <span class="size-1.5 rounded-full {net.running ? 'live-dot bg-ok' : 'bg-faint'}"></span>
        {#if net.running}Proxy listening on <span class="mono">127.0.0.1:{net.state?.port}</span>{:else}Proxy stopped{/if}
      </p>
    </div>
    <span class="flex-1"></span>

    {#if net.running}
      <button onclick={() => net.stop()} disabled={net.busy === "stop"} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-50">
        <Icon name="stop" size={12} /> Stop proxy
      </button>
    {:else}
      <button onclick={() => net.start()} disabled={net.busy === "start"} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-50">
        <Icon name="play" size={12} /> Start proxy
      </button>
    {/if}

    <div class="h-5 w-px bg-line"></div>

    <div class="flex items-center gap-2 rounded-xl border border-line bg-panel-2 py-1 pr-1 pl-3">
      <span class="text-[12px] text-muted">
        {#if !app.device}No device
        {:else}<span class="font-medium text-fg">{app.device.name}</span>
          {#if net.connected}<span class="text-ok">· sending traffic here</span>{:else if net.device?.proxy}<span class="text-warn">· uses {net.device.proxy}</span>{:else}<span>· not connected</span>{/if}
        {/if}
      </span>
      {#if net.connected}
        <button onclick={() => net.disconnect()} disabled={net.busy === "disconnect"} class="flex h-7 items-center gap-1.5 rounded-lg bg-hover px-2.5 text-[12px] font-medium hover:bg-line disabled:opacity-50">
          <Icon name="x" size={12} /> Disconnect
        </button>
      {:else}
        <button onclick={() => net.connect()} disabled={!canConnect || net.busy === "connect"} class="flex h-7 items-center gap-1.5 rounded-lg bg-accent px-2.5 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">
          <Icon name={net.busy === "connect" ? "refresh" : "link"} size={12} class={net.busy === "connect" ? "animate-spin" : ""} /> Connect
        </button>
      {/if}
      <button onclick={() => net.installCertificate()} disabled={!canConnect || net.busy === "certificate"} title="Make the device trust the EmuLabZip certificate, so HTTPS can be decrypted" class="flex h-7 items-center gap-1.5 rounded-lg px-2.5 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-40">
        <Icon name={net.busy === "certificate" ? "refresh" : "shield"} size={12} class={net.busy === "certificate" ? "animate-spin" : ""} /> Certificate
      </button>
    </div>
  </div>

  {#if ios && net.connected}
    <div class="flex shrink-0 items-center gap-2 border-b border-warn/30 bg-warn/10 px-4 py-2 text-[12px] text-warn">
      <Icon name="alert" size={14} class="shrink-0" />
      <span>{net.device?.note ?? "All HTTP and HTTPS traffic from this Mac goes through EmuLabZip while this is on."}</span>
    </div>
  {:else if net.device?.note && !net.connected}
    <div class="flex shrink-0 items-center gap-2 border-b border-line bg-panel-2 px-4 py-2 text-[12px] text-muted">
      <Icon name="info" size={14} class="shrink-0" />{net.device.note}
    </div>
  {/if}

  {#if net.state?.bypassed.length}
    <div class="flex shrink-0 items-center gap-2 border-b border-line bg-panel-2 px-4 py-2 text-[12px] text-muted">
      <Icon name="lock" size={14} class="shrink-0 text-faint" />
      <span class="min-w-0 flex-1 truncate" title={net.state.bypassed.join(", ")}>
        {net.state.bypassed.length} {net.state.bypassed.length === 1 ? "host does" : "hosts do"} not trust the certificate and {net.state.bypassed.length === 1 ? "is" : "are"} passed through:
        <span class="mono">{net.state.bypassed.slice(0, 3).join(", ")}{net.state.bypassed.length > 3 ? "…" : ""}</span>
      </span>
      <button onclick={() => net.tryAgainOnPassedThrough()} class="flex h-6 shrink-0 items-center gap-1.5 rounded-md px-2 hover:bg-hover hover:text-fg"><Icon name="refresh" size={12} />Decrypt again</button>
    </div>
  {/if}

  {#if net.imported}
    {@const im = net.imported}
    <div class="shrink-0 border-b border-line bg-panel-2 px-4 py-2 text-[12px] text-muted">
      <div class="flex items-center gap-2">
        <Icon name="folderOpen" size={14} class="shrink-0 text-faint" />
        <span class="min-w-0 flex-1 truncate">
          <span class="mono text-fg">{im.file}</span>: {im.summary.packets.toLocaleString()} packets, {im.summary.httpRequests} requests, {im.summary.tlsConnections} encrypted connections, {im.dns.length} DNS lookups{im.summary.gaps ? ", some packets are missing" : ""}
        </span>
        {#if im.dns.length}<button onclick={() => (showDns = !showDns)} class="flex h-6 shrink-0 items-center gap-1 rounded-md px-2 hover:bg-hover hover:text-fg">DNS<Icon name={showDns ? "chevronDown" : "chevronRight"} size={12} /></button>{/if}
        <button onclick={() => (net.imported = null)} class="grid size-6 shrink-0 place-items-center rounded-md hover:bg-hover hover:text-fg" aria-label="Dismiss"><Icon name="x" size={12} /></button>
      </div>
      {#if showDns}
        <div class="mono selectable mt-2 max-h-[140px] overflow-y-auto rounded-lg border border-line bg-panel text-[11.5px]">
          {#each im.dns as l, i (i)}
            <div class="flex gap-3 border-b border-line/40 px-3 py-1">
              <span class="w-[44px] shrink-0 text-faint">{l.kind}</span>
              <span class="min-w-0 flex-1 truncate text-fg">{l.name}</span>
              <span class="min-w-0 flex-1 truncate">{l.answers.join(", ") || "—"}</span>
              <span class="w-[72px] shrink-0 text-right text-faint">{l.status ?? "no answer"}</span>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  <!-- Filters -->
  <div class="flex shrink-0 flex-wrap items-center gap-2 border-b border-line bg-panel px-3 py-2">
    <label class="relative flex min-w-[220px] flex-1 items-center">
      <Icon name="search" size={13} class="pointer-events-none absolute left-2.5 text-faint" />
      <input bind:this={searchEl} bind:value={net.filter.text} spellcheck="false" placeholder="Filter by URL, method or status  (⌘F)" class="h-8 w-full rounded-lg border border-line bg-panel-2 pr-3 pl-8 text-[12px] outline-none placeholder:text-faint focus:border-accent focus:bg-panel" />
    </label>
    <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="group" aria-label="Status">
      {#each statusChips as c (c.id)}
        <button onclick={() => (net.filter.status = c.id)} class="mono h-6 rounded-md px-2 text-[11px] {net.filter.status === c.id ? 'bg-panel font-semibold text-fg shadow-sm' : 'text-muted hover:text-fg'}">{c.label}</button>
      {/each}
    </div>
    <select bind:value={net.filter.method} aria-label="Method" class="mono h-8 rounded-lg border border-line bg-panel-2 px-2 text-[12px] outline-none focus:border-accent">
      <option value="all">All methods</option>
      {#each net.methods as m (m)}<option value={m}>{m}</option>{/each}
    </select>
    <label class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-2.5 text-[12px] text-muted">
      <input type="checkbox" bind:checked={net.filter.tunnels} class="accent-[var(--accent)]" /> Connections
    </label>
    <div class="h-5 w-px bg-line"></div>
    <button onclick={() => net.openRules()} title="Rules: mock, delay, block or hold requests" class="relative flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">
      <Icon name="sliders" size={14} />Rules{#if enabledRules}<span class="mono rounded-full bg-accent px-1.5 text-[10px] font-semibold text-white">{enabledRules}</span>{/if}
    </button>
    <div class="relative">
      <button onclick={() => (menu = !menu)} title="HTTP/3 and other proxy options" class="flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg {net.state?.http3 ? 'text-accent' : ''}">
        <Icon name="zap" size={14} />HTTP/3
      </button>
      {#if menu}
        <button class="fixed inset-0 z-30 cursor-default" aria-label="Close" onclick={() => (menu = false)}></button>
        <div class="shadow-pop absolute top-9 right-0 z-40 w-[330px] rounded-xl border border-line bg-elevated p-3">
          <label class="flex items-start gap-2.5 text-[12.5px]">
            <input type="checkbox" checked={net.state?.http3 ?? false} onchange={(e) => net.setHttp3(e.currentTarget.checked)} class="mt-0.5 accent-[var(--accent)]" />
            <span><b class="font-medium">Use HTTP/3 to servers</b><span class="mt-0.5 block text-[11.5px] leading-relaxed text-muted">After a server says it supports it, the proxy talks to it over QUIC. Falls back to HTTP/2 or 1.1 if that fails. The device still talks to the proxy over HTTP/1.1 or 2.</span></span>
          </label>
          <label class="mt-3 flex items-start gap-2.5 text-[12.5px]">
            <input type="checkbox" checked={net.state?.hideHttp3 ?? true} onchange={(e) => net.setHideHttp3(e.currentTarget.checked)} class="mt-0.5 accent-[var(--accent)]" />
            <span><b class="font-medium">Hide HTTP/3 from apps</b><span class="mt-0.5 block text-[11.5px] leading-relaxed text-muted">Removes <span class="mono">h3</span> from the <span class="mono">Alt-Svc</span> header the app sees, so it does not try QUIC around the proxy and vanish from this list.</span></span>
          </label>
        </div>
      {/if}
    </div>
    <button onclick={() => net.importPcap()} title="Open a .pcap or .pcapng capture as requests" class="flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg"><Icon name="folderOpen" size={14} />Import</button>
    <button onclick={() => net.exportHar()} disabled={!net.flows.length} title="Save the requests as a HAR file" class="grid size-8 place-items-center rounded-lg text-muted hover:bg-hover hover:text-fg disabled:opacity-40"><Icon name="download" size={15} /></button>
    <button onclick={() => net.clear()} disabled={!net.flows.length} title="Clear (⌘K)" class="grid size-8 place-items-center rounded-lg text-muted hover:bg-hover hover:text-fg disabled:opacity-40"><Icon name="trash" size={15} /></button>
  </div>

  <!-- List -->
  <div class="mono flex h-7 shrink-0 items-center gap-3 border-b border-line bg-panel-2 px-3 text-[10.5px] tracking-wide text-faint uppercase">
    <span class="w-[44px] shrink-0">Status</span>
    <span class="w-[58px] shrink-0">Method</span>
    <span class="w-[210px] shrink-0">Host</span>
    <span class="flex-1">Path</span>
    <span class="w-[34px] shrink-0">Proto</span>
    <span class="w-[82px] shrink-0">Type</span>
    <span class="w-[68px] shrink-0 text-right">Size</span>
    <span class="w-[72px] shrink-0 text-right">Time</span>
  </div>

  <div class="relative min-h-0 flex-1">
    <div bind:this={scroller} bind:clientHeight={viewport} {onscroll} class="absolute inset-0 overflow-y-auto bg-panel">
      {#if rows.length}
        <div style="height:{rows.length * ROW}px" class="relative">
          <div style="transform: translateY({start * ROW}px)">
            {#each slice as f (f.id)}
              <button
                onclick={() => net.select(net.selectedId === f.id ? null : f.id)}
                style="height:{ROW}px"
                class="mono flex w-full items-center gap-3 border-b border-line/40 px-3 text-left text-[11.5px] {net.selectedId === f.id ? 'bg-select' : 'hover:bg-hover'} {f.kind === 'tunnel' ? 'text-muted' : ''}"
              >
                <span class="w-[44px] shrink-0"><span class="inline-block min-w-[34px] rounded px-1.5 py-px text-center text-[10.5px] font-semibold {statusTone(f)}">{statusText(f)}</span></span>
                <span class="w-[58px] shrink-0 font-medium">{kind(f)}{#if f.mocked}<span class="ml-0.5 text-accent" title="Answered by a rule">•</span>{/if}</span>
                <span class="w-[210px] shrink-0 truncate" title="{f.host}:{f.port}">{f.host}</span>
                <span class="min-w-0 flex-1 truncate text-muted" title={f.path}>{f.path}</span>
                <span class="w-[34px] shrink-0 text-faint {protocol(f) === 'h3' ? 'font-semibold text-accent' : ''}">{protocol(f)}</span>
                <span class="w-[82px] shrink-0 truncate text-faint">{type(f)}</span>
                <span class="w-[68px] shrink-0 text-right text-muted">{f.state === "pending" ? "" : bytes(f.responseSize)}</span>
                <span class="w-[72px] shrink-0 text-right text-faint">{time(f.durationMs)}</span>
              </button>
            {/each}
          </div>
        </div>
      {:else}
        <div class="grid h-full place-items-center px-6 text-center">
          <div class="max-w-[440px]">
            <span class="mx-auto mb-4 grid size-12 place-items-center rounded-2xl bg-accent-soft text-accent"><Icon name="globe" size={22} /></span>
            {#if net.flows.length}
              <p class="text-[13.5px] font-medium">No request matches the filters</p>
              <button onclick={() => net.resetFilter()} class="mt-2 text-[12px] text-accent hover:underline">Reset filters</button>
            {:else if !net.running}
              <p class="text-[13.5px] font-medium">Start the proxy to see traffic</p>
              <p class="mt-1.5 text-[12.5px] leading-relaxed text-muted">Press <b>Connect</b> to point {app.device?.name ?? "a device"} at it. HTTPS needs the device to trust the EmuLabZip certificate: use <b>Certificate</b> once per device.</p>
            {:else if net.connected}
              <p class="text-[13.5px] font-medium">Waiting for traffic</p>
              <p class="mt-1.5 text-[12.5px] leading-relaxed text-muted">Use an app on {app.device?.name}. Requests appear here as they finish.</p>
            {:else}
              <p class="text-[13.5px] font-medium">The proxy is running</p>
              <p class="mt-1.5 text-[12.5px] leading-relaxed text-muted">Press <b>Connect</b> to send {app.device?.name ?? "a device"}'s traffic through it.</p>
            {/if}
          </div>
        </div>
      {/if}
    </div>
    {#if !follow && rows.length}
      <button onclick={jumpToEnd} class="shadow-pop absolute right-5 bottom-4 flex h-8 items-center gap-1.5 rounded-full bg-accent px-3.5 text-[12px] font-semibold text-white hover:opacity-90">
        <Icon name="arrowDownToLine" size={14} /> Jump to latest
      </button>
    {/if}
  </div>

  <BreakpointDrawer />

  <!-- Detail -->
  {#if net.selectedId != null}
    <div class="h-[42%] min-h-[180px] shrink-0 border-t border-line"><FlowDetail /></div>
  {/if}

  <!-- Status -->
  <div class="flex h-7 shrink-0 items-center gap-3 border-t border-line bg-panel px-3 text-[11px] text-muted">
    <span class="mono">{rows.length.toLocaleString()} shown</span>
    <span class="mono text-faint">{net.flows.length.toLocaleString()} captured</span>
    <span class="flex-1"></span>
    <span class="text-faint">Click a request for its headers and bodies · ↑ ↓ to move</span>
  </div>
</div>

{#if net.rulesOpen}<RulesDialog />{/if}
{#if net.certificate}<CertificateDialog result={net.certificate} onclose={() => (net.certificate = null)} />{/if}

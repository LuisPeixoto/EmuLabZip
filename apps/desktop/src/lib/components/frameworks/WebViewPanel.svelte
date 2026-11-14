<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import { api, errorText, type WebLine, type WebViewInfo, type WebPage } from "../../api";
  import { app } from "../../stores/app.svelte";
  import Icon from "../Icon.svelte";

  const MAX_LINES = 1000;
  const ios = $derived(app.device?.platform === "ios");

  let found = $state<WebViewInfo[] | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let picked = $state<{ socket: string; page: WebPage } | null>(null);
  let network = $state(false);
  let lines = $state<WebLine[]>([]);
  let following = $state(false);
  let session: number | null = null;
  let expression = $state("");
  let answers = $state<{ expression: string; value: string; error: boolean }[]>([]);
  let log: HTMLElement | undefined = $state();
  let stickToEnd = true;
  let ticket = 0;

  async function stop() {
    const id = session;
    session = null;
    following = false;
    if (id !== null) await api.stopSession(id).catch(() => {});
  }

  async function refresh() {
    const target = app.target;
    if (!target || ios) return;
    const mine = ++ticket;
    loading = true;
    error = null;
    try {
      const list = await api.webviewList(target);
      if (mine !== ticket) return;
      found = list;
      // A page that is gone cannot be followed any more.
      if (picked && !list.some((w) => w.socket.socket === picked!.socket && w.pages.some((p) => p.id === picked!.page.id))) {
        await stop();
        picked = null;
      }
    } catch (e) {
      if (mine === ticket) {
        found = null;
        error = errorText(e);
      }
    } finally {
      if (mine === ticket) loading = false;
    }
  }

  // Look again whenever another device is picked. The device list is polled, so only its identity is watched.
  $effect(() => {
    void app.deviceKey;
    untrack(() => {
      void stop();
      picked = null;
      lines = [];
      answers = [];
      found = null;
      void refresh();
    });
  });

  async function follow(socket: string, page: WebPage) {
    const target = app.target;
    if (!target) return;
    await stop();
    picked = { socket, page };
    lines = [];
    error = null;
    try {
      session = await api.webviewStart(target, socket, page.id, network, (m) => {
        if (m.type === "ended") {
          following = false;
          session = null;
          return;
        }
        lines = [...lines.slice(-(MAX_LINES - 1)), { level: m.level, text: m.text, source: m.source }];
      });
      following = true;
    } catch (e) {
      error = errorText(e);
    }
  }

  async function run() {
    const target = app.target;
    const text = expression.trim();
    if (!target || !picked || !text) return;
    try {
      const r = await api.webviewEval(target, picked.socket, picked.page.id, text);
      answers = [...answers.slice(-49), { expression: text, value: r.value, error: r.error }];
      expression = "";
    } catch (e) {
      error = errorText(e);
    }
  }

  async function reload() {
    const target = app.target;
    if (target && picked) await api.webviewReload(target, picked.socket, picked.page.id).catch((e) => (error = errorText(e)));
  }

  // Keep the newest line in view unless the person scrolled up to read.
  $effect(() => {
    void lines.length;
    if (!log || !stickToEnd) return;
    tick().then(() => log && (log.scrollTop = log.scrollHeight));
  });

  function scrolled() {
    if (log) stickToEnd = log.scrollHeight - log.scrollTop - log.clientHeight < 24;
  }

  onDestroy(() => void stop());

  const color = (level: string) => (level === "error" ? "text-err" : level === "warn" ? "text-warn" : level === "info" ? "text-info" : level === "debug" ? "text-muted" : "text-fg");
  const total = $derived(found?.reduce((n, w) => n + w.pages.length, 0) ?? 0);
</script>

<section class="rounded-2xl border border-line bg-panel">
  <header class="flex flex-wrap items-center gap-2 border-b border-line px-4 py-2.5">
    <h2 class="text-lg font-semibold">Web pages</h2>
    <span class="text-sm text-muted">WebViews and Chrome</span>
    <span class="flex-1"></span>
    {#if !ios}
      <button onclick={refresh} disabled={loading} class="flex h-7 items-center gap-1.5 rounded-lg border border-line px-2.5 text-base text-muted hover:bg-hover hover:text-fg disabled:opacity-50">
        <Icon name="refresh" size={12} class={loading ? "animate-spin" : ""} />Refresh
      </button>
    {/if}
  </header>

  {#if ios}
    <div class="space-y-2 px-4 py-4 text-md">
      <p class="font-medium">iOS web pages open in Safari's Web Inspector</p>
      <p class="text-muted">Apple does not let other tools read a WKWebView, so EmuLabZip hands you to Safari, which can inspect any page in the Simulator:</p>
      <ol class="list-decimal space-y-0.5 pl-5 text-muted">
        <li>In Safari, open Settings → Advanced and turn on “Show features for web developers”.</li>
        <li>Open the page in your app. A WKWebView must have <span class="mono">isInspectable = true</span> (iOS 16.4 and later; debug builds usually set it).</li>
        <li>In Safari's Develop menu, pick the Simulator, then the page.</li>
      </ol>
      <button onclick={() => api.openSafari().catch((e) => (error = errorText(e)))} class="mt-1 flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90">
        <Icon name="external" size={13} />Open Safari
      </button>
      {#if error}<p class="text-err">{error}</p>{/if}
    </div>
  {:else if !app.device || app.device.state !== "online"}
    <p class="px-4 py-6 text-center text-md text-muted">Pick a running device to see its web pages.</p>
  {:else}
    {#if error}
      <p class="flex items-start gap-2 border-b border-line bg-err/8 px-4 py-2.5 text-md text-err"><Icon name="alert" size={14} class="mt-px shrink-0" />{error}</p>
    {/if}

    {#if found && !total}
      <div class="px-4 py-6 text-center">
        <p class="text-md font-medium">No web page can be inspected</p>
        <p class="mx-auto mt-1 max-w-[520px] text-sm text-muted">A WebView shows up here when its app is debuggable and calls <span class="mono">WebView.setWebContentsDebuggingEnabled(true)</span>. Chrome shows up while it is open.</p>
      </div>
    {:else if found}
      <ul class="divide-y divide-line border-b border-line">
        {#each found as w (w.socket.socket)}
          {#each w.pages as p (p.id)}
            {@const active = picked?.socket === w.socket.socket && picked.page.id === p.id}
            <li>
              <button onclick={() => follow(w.socket.socket, p)} class="flex w-full items-center gap-3 px-4 py-2 text-left hover:bg-hover {active ? 'bg-accent-soft' : ''}">
                <Icon name="globe" size={14} class={active ? "text-accent" : "text-faint"} />
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-md font-medium">{p.title || p.url}</span>
                  <span class="mono block truncate text-xs text-faint">{p.url}</span>
                </span>
                <span class="shrink-0 rounded-md bg-hover px-1.5 py-px text-2xs text-muted">{w.socket.app ?? w.socket.kind}</span>
              </button>
            </li>
          {/each}
        {/each}
      </ul>
    {/if}

    {#if picked}
      <div class="flex flex-wrap items-center gap-2 border-b border-line px-4 py-2">
        <span class="flex items-center gap-1.5 text-sm {following ? 'text-ok' : 'text-muted'}"><span class="size-1.5 rounded-full {following ? 'live-dot bg-ok' : 'bg-faint'}"></span>{following ? "Following" : "Stopped"}</span>
        <label class="flex items-center gap-1.5 text-sm text-muted" title="Also list this page's requests in Network, marked WEBVIEW. Takes effect when you pick the page again.">
          <input type="checkbox" bind:checked={network} class="accent-[var(--accent)]" />Show requests in Network
        </label>
        <span class="flex-1"></span>
        <button onclick={reload} class="h-7 rounded-lg border border-line px-2.5 text-base text-muted hover:bg-hover hover:text-fg">Reload</button>
        <button onclick={() => (lines = [])} class="h-7 rounded-lg border border-line px-2.5 text-base text-muted hover:bg-hover hover:text-fg">Clear</button>
        {#if following}<button onclick={stop} class="h-7 rounded-lg border border-line px-2.5 text-base text-muted hover:bg-hover hover:text-fg">Stop</button>{/if}
      </div>
      <div bind:this={log} onscroll={scrolled} class="selectable mono max-h-[360px] min-h-[120px] overflow-auto px-4 py-2 text-sm">
        {#each lines as l, i (i)}
          <div class="flex gap-2 py-px {color(l.level)}">
            <span class="w-11 shrink-0 text-faint uppercase">{l.level}</span>
            <span class="min-w-0 flex-1 break-words whitespace-pre-wrap">{l.text}</span>
            {#if l.source}<span class="max-w-[220px] shrink-0 truncate text-faint" title={l.source}>{l.source}</span>{/if}
          </div>
        {:else}
          <p class="py-4 text-center text-muted">Nothing yet. Use the page, or reload it.</p>
        {/each}
      </div>
      {#if answers.length}
        <div class="selectable mono max-h-[140px] space-y-0.5 overflow-auto border-t border-line px-4 py-2 text-sm">
          {#each answers as a, i (i)}
            <p class="text-faint">› {a.expression}</p>
            <p class="break-words whitespace-pre-wrap {a.error ? 'text-err' : 'text-fg'}">{a.value}</p>
          {/each}
        </div>
      {/if}
      <form onsubmit={(e) => (e.preventDefault(), run())} class="flex items-center gap-2 border-t border-line px-4 py-2">
        <span class="mono text-faint">›</span>
        <input bind:value={expression} spellcheck="false" placeholder="Run JavaScript in the page, such as document.title" class="mono h-7 flex-1 rounded-md border border-line bg-panel-2 px-2 text-sm outline-none placeholder:text-faint focus:border-accent" />
        <button type="submit" disabled={!expression.trim()} class="h-7 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40">Run</button>
      </form>
    {/if}
  {/if}
</section>

<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type LinkHistoryEntry, type LinkLevel, type LinkReport } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { arrange } from "../../stores/arrange.svelte";
  import Icon from "../Icon.svelte";

  let url = $state("");
  let history = $state<LinkHistoryEntry[]>([]);
  let report = $state<LinkReport | null>(null);
  let busy = $state<"open" | "check" | "verify" | null>(null);
  let error = $state<string | null>(null);
  let useApp = $state(true);

  const appId = $derived(useApp ? app.appId : null);
  const ready = $derived(!!app.target && app.online);
  const android = $derived(app.device?.platform === "android");

  onMount(() => void api.linkHistory().then((h) => (history = h)).catch(() => {}));

  const mark: Record<LinkLevel, { icon: string; tone: string; label: string }> = {
    pass: { icon: "check", tone: "text-ok", label: "Passed" },
    info: { icon: "info", tone: "text-muted", label: "Note" },
    warn: { icon: "alert", tone: "text-warn", label: "Warning" },
    fail: { icon: "x", tone: "text-err", label: "Problem" },
  };

  async function run(kind: "open" | "check") {
    const target = app.target;
    const link = url.trim();
    if (!target || !link) return;
    busy = kind;
    error = null;
    try {
      if (kind === "open") {
        await api.linkOpen(target, link, appId);
        app.toast("Link opened", "ok");
      } else {
        report = await api.linkCheck(target, link, appId);
      }
      history = await api.linkHistory();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  async function verify() {
    const target = app.target;
    if (!target || !appId) return;
    busy = "verify";
    try {
      await api.linkVerify(target, appId);
      app.toast("The device was asked to verify again. Check in a few seconds.", "ok");
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  async function pin(h: LinkHistoryEntry) {
    history = await api.linkPin(h.url, !h.pinned);
  }

  async function forget(h: LinkHistoryEntry) {
    history = await api.linkForget(h.url);
  }

  function reuse(h: LinkHistoryEntry) {
    url = h.url;
    report = null;
  }

  const field = "h-9 w-full rounded-lg border border-line bg-panel-2 px-3 text-md outline-none focus:border-accent";
  const primary = "flex h-9 items-center justify-center gap-1.5 rounded-lg bg-accent px-4 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40";
  const ghost = "flex h-9 items-center gap-1.5 rounded-lg border border-line px-3.5 text-base font-medium hover:bg-hover disabled:opacity-40";
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto {arrange.pageMid} space-y-4 p-6">
    <header>
      <h1 class="text-3xl font-semibold tracking-tight">Deep links</h1>
      <p class="text-md text-muted">Open a link on the device, and find out why it does or does not open your app.</p>
    </header>

    {#if error}
      <div class="flex items-center gap-2 rounded-xl border border-err/30 bg-err/8 px-4 py-3 text-md text-err"><Icon name="alert" size={14} />{error}</div>
    {/if}

    <section class="rounded-2xl border border-line bg-panel p-5">
      <form class="space-y-3" onsubmit={(e) => (e.preventDefault(), run("check"))}>
        <input class="{field} mono" bind:value={url} placeholder="https://shop.example.com/cart/3  or  myapp://open/3" aria-label="Link" spellcheck="false" maxlength="2000" />
        <div class="flex flex-wrap items-center gap-2">
          <label class="flex items-center gap-1.5 text-base text-muted" title={app.appId ? "" : "Pick an app in the toolbar"}>
            <input type="checkbox" bind:checked={useApp} disabled={!app.appId} class="accent-[var(--accent)]" />
            It should open {app.selectedApp?.name ?? "the picked app"}
          </label>
          <span class="flex-1"></span>
          <button type="button" class={ghost} disabled={!ready || !url.trim() || busy != null} onclick={() => run("open")}><Icon name="external" size={13} />Open on the device</button>
          <button type="submit" class={primary} disabled={!ready || !url.trim() || busy != null}><Icon name={busy === "check" ? "refresh" : "shield"} size={13} class={busy === "check" ? "animate-spin" : ""} />Check</button>
        </div>
      </form>
    </section>

    {#if report}
      <section class="rounded-2xl border border-line bg-panel">
        <header class="flex flex-wrap items-center gap-2 border-b border-line px-5 py-3">
          <Icon name={mark[report.worst].icon} size={15} class={mark[report.worst].tone} />
          <h2 class="text-lg font-semibold">{report.worst === "fail" ? "Something is wrong" : report.worst === "warn" ? "Works, with warnings" : "Looks right"}</h2>
          <span class="mono min-w-0 flex-1 truncate text-xs text-faint">{report.fileUrl ?? report.url}</span>
          {#if android && appId && report.domains.length}
            <button class={ghost} onclick={verify} disabled={busy != null} title="pm verify-app-links --re-verify">Verify again</button>
          {/if}
        </header>
        <ul class="selectable divide-y divide-line">
          {#each report.findings as f, i (i)}
            <li class="flex gap-3 px-5 py-3">
              <Icon name={mark[f.level].icon} size={15} class="mt-0.5 shrink-0 {mark[f.level].tone}" />
              <div class="min-w-0 flex-1">
                <p class="text-md font-medium">{f.title}<span class="sr-only"> ({mark[f.level].label})</span></p>
                <p class="text-base text-muted">{f.detail}</p>
                {#if f.fix}<p class="mt-1 text-base"><span class="text-faint">Fix: </span><span class="mono text-sm">{f.fix}</span></p>{/if}
              </div>
            </li>
          {/each}
        </ul>
        {#if report.domains.length}
          <div class="border-t border-line px-5 py-3">
            <h3 class="mb-1.5 text-2xs font-semibold tracking-wider text-faint uppercase">Domains the device decided about</h3>
            <dl class="mono grid grid-cols-[1fr_auto] gap-x-6 gap-y-0.5 text-sm">
              {#each report.domains as d (d.domain)}
                <dt class="truncate">{d.domain}</dt>
                <dd class={d.state === "verified" ? "text-ok" : "text-warn"}>{d.state}</dd>
              {/each}
            </dl>
          </div>
        {/if}
      </section>
    {/if}

    <section class="rounded-2xl border border-line bg-panel">
      <h2 class="border-b border-line px-5 py-3 text-lg font-semibold">History</h2>
      {#if !history.length}
        <p class="px-5 py-8 text-center text-md text-muted">Links you open or check are kept here. Pin the ones you use often.</p>
      {:else}
        <ul class="divide-y divide-line">
          {#each history as h (h.url)}
            <li class="flex items-center gap-2 px-5 py-2">
              {#if h.result}<Icon name={mark[h.result].icon} size={13} class="shrink-0 {mark[h.result].tone}" />{:else}<span class="size-[13px] shrink-0"></span>{/if}
              <button class="mono min-w-0 flex-1 truncate text-left text-sm hover:text-accent" onclick={() => reuse(h)} title={h.url}>{h.url}</button>
              {#if h.app}<span class="hidden shrink-0 rounded-md bg-hover px-1.5 py-px text-2xs text-muted sm:inline">{h.app}</span>{/if}
              <button class="grid size-7 place-items-center rounded-md hover:bg-hover {h.pinned ? 'text-accent' : 'text-faint'}" onclick={() => pin(h)} aria-label={h.pinned ? "Unpin" : "Pin"} aria-pressed={h.pinned}><Icon name="star" size={13} /></button>
              <button class="grid size-7 place-items-center rounded-md text-faint hover:bg-hover hover:text-err" onclick={() => forget(h)} aria-label="Forget this link"><Icon name="x" size={12} /></button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>
</div>

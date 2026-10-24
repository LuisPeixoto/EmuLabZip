<script lang="ts">
  import { onMount } from "svelte";
  import type { CrashKind, Origin } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { crashes, label } from "../../stores/crashes.svelte";
  import Icon from "../Icon.svelte";

  let tab = $state<"report" | "raw">("report");
  let appCodeOnly = $state(false);

  onMount(() => {
    crashes.markSeen();
    crashes.refresh(app.target, false);
  });

  // Opening the view clears the badge, and so does a crash that arrives while it is open.
  $effect(() => {
    crashes.list;
    crashes.markSeen();
  });

  // Show the newest crash as soon as there is one and nothing is open.
  $effect(() => {
    if (!crashes.selectedId && crashes.visible[0]) crashes.open(crashes.visible[0].id);
  });

  const kinds: { id: CrashKind | "all"; text: string }[] = [
    { id: "all", text: "All" },
    { id: "crash", text: "Crashes" },
    { id: "nativeCrash", text: "Native" },
    { id: "anr", text: "ANRs" },
  ];
  const origins: Record<Origin, string> = { app: "App", framework: "Framework", native: "Native", kernel: "Kernel" };
  const tone: Record<CrashKind, string> = { crash: "bg-err/12 text-err", nativeCrash: "bg-err/12 text-err", anr: "bg-warn/15 text-warn" };

  const report = $derived(crashes.report);
  const count = (k: CrashKind | "all") => crashes.list.filter((c) => k === "all" || c.kind === k).length;

  function when(ms: number | null): string {
    return ms ? new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "medium" }) : "unknown time";
  }

  async function copy(text: string) {
    await navigator.clipboard?.writeText(text).catch(() => {});
    app.toast("Copied", "ok");
  }
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1280px] p-6">
    <header class="mb-5 flex flex-wrap items-center gap-3">
      <div class="min-w-0 flex-1">
        <h1 class="text-[18px] font-semibold tracking-tight">Crashes</h1>
        <p class="text-[12.5px] text-muted">Crashes, native crashes and ANRs the device recorded, with their stacks.</p>
      </div>
      <button
        onclick={() => crashes.refresh(app.target, false)}
        class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg"
      >
        <Icon name="refresh" size={13} /> Refresh
      </button>
    </header>

    <div class="grid items-start gap-5 lg:grid-cols-[360px_1fr]">
      <aside class="rounded-2xl border border-line bg-panel p-2">
        <div class="flex flex-wrap items-center gap-1 p-1" role="tablist" aria-label="Kind">
          {#each kinds as k (k.id)}
            <button
              role="tab"
              aria-selected={crashes.kind === k.id}
              onclick={() => (crashes.kind = k.id)}
              class="h-7 rounded-lg px-2.5 text-[11.5px] font-medium {crashes.kind === k.id ? 'bg-accent-soft text-accent' : 'text-muted hover:bg-hover hover:text-fg'}"
            >
              {k.text} <span class="mono ml-0.5 text-faint">{count(k.id)}</span>
            </button>
          {/each}
        </div>
        {#if app.selectedApp}
          <label class="flex cursor-pointer items-center gap-2 px-2.5 py-1.5 text-[11.5px] text-muted">
            <input type="checkbox" bind:checked={crashes.onlyApp} class="accent-[var(--accent)]" /> Only {app.selectedApp.name}
          </label>
        {/if}
        {#if !crashes.visible.length}
          <p class="px-3 py-10 text-center text-[12px] text-muted">
            {crashes.list.length ? "No crashes match." : "No crashes recorded on this device."}
          </p>
        {/if}
        <ul class="max-h-[640px] space-y-0.5 overflow-y-auto">
          {#each crashes.visible as c (c.id)}
            <li>
              <button
                onclick={() => crashes.open(c.id)}
                class="w-full rounded-xl px-2.5 py-2 text-left {c.id === crashes.selectedId ? 'bg-accent-soft' : 'hover:bg-hover'}"
              >
                <span class="flex items-center gap-2">
                  <span class="rounded-md px-1.5 py-0.5 text-[10.5px] font-semibold {tone[c.kind]}">{label(c.kind)}</span>
                  <span class="min-w-0 flex-1 truncate text-[12px] font-medium">{c.app ?? c.process}</span>
                </span>
                <span class="mt-1 block truncate text-[11.5px] text-muted" title={c.title}>{c.title}</span>
                <span class="mono mt-0.5 block text-[10.5px] text-faint">{when(c.timeMs)}</span>
              </button>
            </li>
          {/each}
        </ul>
      </aside>

      <section class="min-w-0">
        {#if crashes.error}
          <div class="mb-3 flex items-start gap-2 rounded-xl border border-err/30 bg-err/8 px-4 py-3 text-[12.5px] text-err">
            <Icon name="alert" size={14} class="mt-0.5 shrink-0" /><span class="min-w-0 break-words">{crashes.error}</span>
          </div>
        {/if}
        {#if crashes.loading}
          <div class="grid place-items-center py-24 text-[12.5px] text-muted"><Icon name="refresh" size={18} class="mb-2 animate-spin text-faint" />Reading the report…</div>
        {:else if report}
          <div class="mb-4 flex flex-wrap items-start gap-3">
            <div class="min-w-0 flex-1">
              <p class="flex items-center gap-2">
                <span class="rounded-md px-1.5 py-0.5 text-[10.5px] font-semibold {tone[report.summary.kind]}">{label(report.summary.kind)}</span>
                <span class="truncate text-[12px] text-muted">{report.summary.app ?? report.summary.process}{report.summary.pid ? ` · pid ${report.summary.pid}` : ""} · {when(report.summary.timeMs)}</span>
              </p>
              <h2 class="mt-1.5 text-[15px] leading-snug font-semibold break-words">{report.summary.title}</h2>
            </div>
            <div class="inline-flex gap-1 rounded-xl bg-panel-2 p-1" role="tablist">
              {#each [["report", "Report"], ["raw", "Raw"]] as [id, text] (id)}
                <button role="tab" aria-selected={tab === id} onclick={() => (tab = id as "report" | "raw")} class="h-7 rounded-lg px-3.5 text-[12px] font-medium {tab === id ? 'bg-panel text-fg shadow-sm' : 'text-muted hover:text-fg'}">{text}</button>
              {/each}
            </div>
            <button onclick={() => crashes.save()} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">
              <Icon name="download" size={13} /> Save…
            </button>
          </div>

          {#if tab === "raw"}
            <div class="relative rounded-xl border border-line bg-panel">
              <button onclick={() => copy(report.raw)} class="absolute top-2 right-2 flex h-7 items-center gap-1.5 rounded-lg border border-line bg-panel px-2.5 text-[11.5px] text-muted hover:text-fg"><Icon name="copy" size={12} /> Copy</button>
              <pre class="mono max-h-[620px] overflow-auto p-4 text-[11.5px] leading-relaxed whitespace-pre-wrap select-text">{report.raw}</pre>
            </div>
          {:else}
            {#if report.hints.length}
              <section class="mb-4 space-y-2 rounded-2xl border border-line bg-panel p-4">
                <h3 class="text-[12.5px] font-semibold">What to try</h3>
                {#each report.hints as hint (hint.text)}
                  <div class="text-[12px]">
                    <p class="text-muted">{hint.text}</p>
                    {#if hint.command}
                      <div class="mt-1 flex items-center gap-2 rounded-lg bg-panel-2 px-2.5 py-1.5">
                        <code class="mono min-w-0 flex-1 truncate text-[11.5px] select-text" title={hint.command}>{hint.command}</code>
                        <button onclick={() => copy(hint.command!)} aria-label="Copy the command" class="grid size-6 place-items-center rounded-md text-faint hover:bg-hover hover:text-fg"><Icon name="copy" size={12} /></button>
                      </div>
                    {/if}
                  </div>
                {/each}
              </section>
            {/if}

            {#if report.details.length}
              <dl class="mb-4 grid gap-x-6 gap-y-1.5 rounded-2xl border border-line bg-panel p-4 text-[12px] sm:grid-cols-[auto_1fr]">
                {#each report.details as [k, v] (k)}
                  <dt class="text-faint">{k}</dt>
                  <dd class="mono min-w-0 break-words select-text">{v}</dd>
                {/each}
              </dl>
            {/if}

            <div class="mb-2 flex items-center gap-4 text-[11.5px]">
              <label class="flex cursor-pointer items-center gap-2 text-muted"><input type="checkbox" bind:checked={appCodeOnly} class="accent-[var(--accent)]" /> Only the app's code</label>
              <ul class="ml-auto flex items-center gap-3 text-muted">
                {#each Object.entries(origins) as [key, text] (key)}
                  <li class="flex items-center gap-1.5"><span class="size-2.5 rounded-[3px]" style="background: var(--flame-{key})"></span>{text}</li>
                {/each}
              </ul>
            </div>

            <div class="space-y-2">
              {#each report.threads as thread, i (i)}
                {@const frames = appCodeOnly ? thread.frames.filter((f) => f.origin === "app") : thread.frames}
                <details open={thread.crashed} class="rounded-xl border border-line bg-panel">
                  <summary class="flex cursor-pointer items-center gap-2 px-3.5 py-2.5 text-[12.5px] font-medium">
                    {#if thread.crashed}<Icon name="alert" size={13} class="text-err" />{/if}
                    <span class="min-w-0 flex-1 truncate" title={thread.name}>{thread.name}</span>
                    <span class="mono text-[11px] text-faint">{frames.length} frames</span>
                  </summary>
                  <table class="w-full border-t border-line text-[11.5px]">
                    <tbody>
                      {#each frames as f (f.index)}
                        <tr class="border-b border-line last:border-b-0 hover:bg-hover">
                          <td class="mono w-10 py-1 pr-1 pl-3.5 text-right text-faint">{f.index}</td>
                          <td class="w-4 py-1"><span class="block size-2.5 rounded-[3px]" style="background: var(--flame-{f.origin})" title={origins[f.origin]}></span></td>
                          <td class="mono max-w-[420px] truncate py-1 pr-3 select-text" title={f.symbol}>{f.symbol || "—"}</td>
                          <td class="mono max-w-[260px] truncate py-1 pr-3.5 text-right text-muted" title="{f.image ?? ''} {f.location ?? ''}">{f.image ?? ""}{f.location ? ` ${f.location}` : ""}</td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </details>
              {/each}
            </div>
          {/if}
        {:else if !crashes.list.length}
          <div class="grid place-items-center rounded-2xl border border-dashed border-line-strong bg-panel/50 px-6 py-24 text-center">
            <Icon name="shieldCheck" size={26} class="mb-3 text-faint" />
            <p class="text-[13.5px] font-medium">Nothing has crashed</p>
            <p class="mt-1 max-w-[440px] text-[12.5px] text-muted">New crashes show up here by themselves, with a notification, while EmuLabZip is open.</p>
          </div>
        {/if}
      </section>
    </div>
  </div>
</div>

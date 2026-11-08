<script lang="ts">
  import { api, type Differences, type StateInfo } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { confirmed } from "../../confirm";
  import { bytes, dateTime } from "../../format";
  import Icon from "../Icon.svelte";

  /** Save an app's data to a file, put it back, or compare two files: the structure of what changed. */
  let { onrestored }: { onrestored?: () => void } = $props();

  let busy = $state<string | null>(null);
  let info = $state<StateInfo | null>(null);
  let diff = $state<{ title: string; result: Differences } | null>(null);
  let other = $state<string | null>(null);

  const pkg = $derived(app.selectedApp?.id ?? null);

  async function save() {
    if (!app.target || !pkg) return;
    busy = "save";
    const saved = await app.attempt(() => api.stateSave(app.target!, pkg, false));
    busy = null;
    if (saved) {
      app.toast(`Saved ${saved.meta.files} files (${bytes(saved.meta.bytes)}) to ${saved.path}`, "ok");
      info = null;
    }
  }

  async function pick() {
    const path = await app.attempt(() => api.statePick());
    if (!path) return null;
    return app.attempt(() => api.stateInfo(path));
  }

  async function open() {
    const got = await pick();
    if (got) (info = got), (diff = null), (other = null);
  }

  async function restore() {
    if (!info || !app.target) return;
    const m = info.meta;
    const ok = await confirmed(
      `Replace the data of ${m.app} on ${app.device?.name ?? "the device"} with the state saved from ${m.device} on ${dateTime(m.takenMs / 1000)}?\n\nEverything the app has stored now is replaced. The data as it is now is saved first, so this can be undone by restoring that file.`,
      "Restore app state",
      "Replace the data",
    );
    if (!ok) return;
    busy = "restore";
    const done = await app.attempt(() => api.stateRestore(app.target!, info!.path));
    busy = null;
    if (done) {
      for (const w of done.warnings) app.toast(w, "info");
      app.toast(`Restored. The data it replaced is in ${done.backup}`, "ok");
      onrestored?.();
    }
  }

  async function compareNow() {
    if (!info || !app.target) return;
    busy = "diff";
    const result = await app.attempt(() => api.stateCompareNow(app.target!, info!.path));
    busy = null;
    if (result) diff = { title: "This state against the app now", result };
  }

  async function compareFile() {
    if (!info) return;
    const second = await pick();
    if (!second) return;
    other = second.path;
    busy = "diff";
    const result = await app.attempt(() => api.stateCompare(info!.path, second.path));
    busy = null;
    if (result) diff = { title: `${name(info!.path)} against ${name(second.path)}`, result };
  }

  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const sign = (c: string) => (c === "added" ? "+" : c === "removed" ? "−" : "~");
</script>

<div class="flex items-center gap-1.5">
  <button onclick={save} disabled={!pkg || busy != null} title="Save everything the app keeps to one file" class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-2.5 text-base font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-40">
    <Icon name={busy === "save" ? "refresh" : "download"} size={13} class={busy === "save" ? "animate-spin" : ""} /> Save state…
  </button>
  <button onclick={open} disabled={busy != null} title="Open a saved state to restore it or compare it" class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-2.5 text-base font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-40">
    <Icon name="folderOpen" size={13} /> Open state…
  </button>
</div>

{#if info}
  <div class="fixed inset-0 z-40 grid place-items-center bg-black/40 p-6" role="presentation" onclick={(e) => e.target === e.currentTarget && ((info = null), (diff = null))}>
    <div class="flex max-h-[80vh] w-[640px] max-w-full flex-col rounded-2xl border border-line bg-panel shadow-2xl" role="dialog" aria-label="Saved state">
      <header class="flex items-start gap-3 border-b border-line px-5 py-3.5">
        <div class="min-w-0 flex-1">
          <p class="truncate text-xl font-semibold">{info.meta.app} {info.meta.appVersion ?? ""}</p>
          <p class="text-sm text-muted">{info.meta.platform} · {info.meta.device} · {dateTime(info.meta.takenMs / 1000)} · {info.meta.files} files, {bytes(info.meta.bytes)}</p>
          <p class="mono mt-0.5 truncate text-2xs text-faint">{info.path}</p>
        </div>
        <button onclick={() => ((info = null), (diff = null))} aria-label="Close" class="text-muted hover:text-fg"><Icon name="x" size={16} /></button>
      </header>
      <div class="flex flex-wrap gap-2 border-b border-line px-5 py-3">
        <button onclick={compareNow} disabled={busy != null || !pkg || pkg !== info.meta.app} title={pkg !== info.meta.app ? "Pick this state's app first" : ""} class="h-8 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover disabled:opacity-40">Compare with the app now</button>
        <button onclick={compareFile} disabled={busy != null} class="h-8 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover disabled:opacity-40">Compare with another file…</button>
        <span class="flex-1"></span>
        <button onclick={restore} disabled={busy != null || !pkg || pkg !== info.meta.app} title={pkg !== info.meta.app ? "Pick this state's app first" : ""} class="h-8 rounded-lg bg-err px-3 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40">Restore…</button>
      </div>
      <div class="selectable min-h-0 flex-1 overflow-auto px-5 py-3">
        {#if diff}
          <p class="mb-2 text-base font-semibold">{diff.title}</p>
          {#each diff.result.changes as c, i (i)}
            <p class="mono py-0.5 text-sm {c.change === 'added' ? 'text-ok' : c.change === 'removed' ? 'text-err' : 'text-warn'}">{sign(c.change)} {diff.result.lines[i]?.slice(2)}</p>
          {:else}
            <p class="text-base text-muted">No difference in the structure of the data: the same tables, columns, indexes and keys.</p>
          {/each}
        {:else}
          <p class="mb-2 text-base font-semibold">What is inside</p>
          {#each info.files as f (f.path)}
            <p class="mono flex justify-between gap-3 py-0.5 text-sm"><span class="truncate">{f.path}</span><span class="shrink-0 text-faint">{f.tables.length ? `${f.tables.length} tables` : `${f.keys.length} keys`}</span></p>
          {:else}
            <p class="text-base text-muted">No database or preferences file.</p>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

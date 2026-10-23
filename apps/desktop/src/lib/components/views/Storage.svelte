<script lang="ts">
  import { untrack } from "svelte";
  import { api, errorText, type FileEntry, type Preview, type StorageRoot } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { bytes, dateTime } from "../../format";
  import Icon from "../Icon.svelte";
  import FilePreview from "../storage/FilePreview.svelte";

  let roots = $state<StorageRoot[]>([]);
  let rootId = $state<string | null>(null);
  let path = $state("");
  let entries = $state<FileEntry[] | null>(null);
  let error = $state<string | null>(null);
  let preview = $state<Preview | null>(null);
  let opening = $state<string | null>(null);
  /** Root and path of the file in the preview, which may differ from the folder on screen. */
  let previewAt = $state<{ root: string; path: string } | null>(null);

  const root = $derived(roots.find((r) => r.id === rootId) ?? null);
  const crumbs = $derived(path ? path.split("/") : []);
  const ios = $derived(app.device?.platform === "ios");

  // Reload the roots when the device or the app changes.
  $effect(() => {
    const key = `${app.deviceKey}|${app.appId ?? ""}`;
    untrack(() => loadRoots(key));
  });

  async function loadRoots(key: string) {
    roots = [];
    rootId = null;
    entries = null;
    preview = null;
    error = null;
    path = "";
    if (!app.target || !app.selectedApp) return;
    try {
      const list = await api.storageRoots(app.target, app.selectedApp.id);
      if (key !== `${app.deviceKey}|${app.appId ?? ""}`) return;
      roots = list;
      const first = list.find((r) => !r.unavailable) ?? list[0];
      if (first) open(first.id, "");
    } catch (e) {
      error = errorText(e);
    }
  }

  async function open(id: string, dir: string) {
    rootId = id;
    path = dir;
    entries = null;
    error = null;
    const r = roots.find((x) => x.id === id);
    if (r?.unavailable || !app.target || !app.selectedApp) return;
    try {
      entries = await api.listFiles(app.target, app.selectedApp.id, id, dir);
    } catch (e) {
      error = errorText(e);
    }
  }

  async function select(e: FileEntry) {
    if (!rootId) return;
    if (e.isDir) return open(rootId, e.path);
    if (!app.target || !app.selectedApp) return;
    opening = e.path;
    try {
      preview = await api.openFile(app.target, app.selectedApp.id, rootId, e.path);
      previewAt = { root: rootId, path: e.path };
    } catch (err) {
      app.toast(errorText(err), "err");
    } finally {
      opening = null;
    }
  }

  async function save() {
    const at = previewAt;
    if (!at || !app.target || !app.selectedApp) return;
    const where = await app.attempt(() => api.saveFile(app.target!, app.selectedApp!.id, at.root, at.path));
    if (where) app.toast(`Saved to ${where}`, "ok");
  }

  function icon(e: FileEntry) {
    if (e.isDir) return "folder";
    if (/\.(db|sqlite3?|realm)$/i.test(e.name)) return "database";
    if (/\.(png|jpe?g|gif|webp)$/i.test(e.name)) return "eye";
    return "file";
  }
</script>

{#if !app.selectedApp}
  <div class="grid h-full place-items-center p-10 text-center">
    <div>
      <span class="mx-auto mb-4 grid size-12 place-items-center rounded-2xl bg-accent-soft text-accent"><Icon name="database" size={22} /></span>
      <p class="text-[15px] font-semibold">Pick an app to browse its files</p>
      <p class="mt-1 max-w-[420px] text-[12.5px] text-muted">
        {ios
          ? "Browse the data container, the bundle and app group containers, with viewers for SQLite databases and property lists."
          : "Browse the private data of debuggable apps and their shared storage, with viewers for SQLite databases and SharedPreferences."}
      </p>
    </div>
  </div>
{:else}
  <div class="flex h-full min-h-0">
    <section class="flex w-[360px] shrink-0 flex-col border-r border-line bg-panel">
      <div class="flex shrink-0 gap-1 overflow-x-auto border-b border-line p-2">
        {#each roots as r (r.id)}
          <button
            onclick={() => open(r.id, "")}
            title={r.location}
            class="flex h-7 shrink-0 items-center gap-1.5 rounded-lg px-2.5 text-[12px] {r.id === rootId ? 'bg-accent-soft font-medium text-accent' : 'text-muted hover:bg-hover hover:text-fg'}"
          >
            {#if r.unavailable}<span class="size-1.5 rounded-full bg-faint"></span>{/if}{r.label}
          </button>
        {/each}
      </div>
      <nav class="flex shrink-0 items-center gap-1 overflow-x-auto border-b border-line px-3 py-2 text-[12px]">
        <button onclick={() => rootId && open(rootId, "")} class="shrink-0 text-muted hover:text-fg"><Icon name="folder" size={13} /></button>
        {#each crumbs as c, i (i)}
          <Icon name="chevronRight" size={12} class="shrink-0 text-faint" />
          <button onclick={() => rootId && open(rootId, crumbs.slice(0, i + 1).join("/"))} class="shrink-0 truncate {i === crumbs.length - 1 ? 'font-medium' : 'text-muted hover:text-fg'}">{c}</button>
        {/each}
      </nav>
      <div class="min-h-0 flex-1 overflow-y-auto p-1.5">
        {#if error}
          <p class="p-3 text-[12px] text-err">{error}</p>
        {:else if root?.unavailable}
          <div class="p-4 text-center text-[12px] text-muted">
            <Icon name="alert" size={18} class="mx-auto mb-2 text-faint" />
            {root.unavailable}
            <p class="mono mt-2 text-[10.5px] break-all text-faint">{root.location}</p>
          </div>
        {:else if !entries}
          {#each Array(6) as _, i (i)}<div class="skeleton m-2 h-5"></div>{/each}
        {:else}
          {#if path}
            <button onclick={() => rootId && open(rootId, crumbs.slice(0, -1).join("/"))} class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[12.5px] text-muted hover:bg-hover">
              <Icon name="arrowUp" size={14} /> ..
            </button>
          {/if}
          {#each entries as e (e.path)}
            <button
              onclick={() => select(e)}
              class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left {previewAt?.root === rootId && previewAt?.path === e.path ? 'bg-accent-soft' : 'hover:bg-hover'}"
            >
              <Icon name={opening === e.path ? "refresh" : icon(e)} size={14} class="shrink-0 {e.isDir ? 'text-accent' : 'text-faint'} {opening === e.path ? 'animate-spin' : ''}" />
              <span class="min-w-0 flex-1 truncate text-[12.5px]">{e.name}</span>
              <span class="shrink-0 text-right leading-tight">
                {#if !e.isDir}<span class="mono block text-[10.5px] text-muted">{bytes(e.size)}</span>{/if}
                <span class="block text-[10px] text-faint">{dateTime(e.modified)}</span>
              </span>
            </button>
          {:else}
            <p class="p-4 text-center text-[12px] text-muted">This folder is empty.</p>
          {/each}
        {/if}
      </div>
    </section>
    <section class="min-w-0 flex-1 bg-panel">
      {#if preview}
        <FilePreview {preview} onsave={save} />
      {:else}
        <div class="grid h-full place-items-center text-center text-[12.5px] text-muted">
          <div>
            <Icon name="file" size={24} class="mx-auto mb-3 text-faint" />
            Select a file to preview it.<br />Databases, preferences, property lists, images and text open in place.
          </div>
        </div>
      {/if}
    </section>
  </div>
{/if}

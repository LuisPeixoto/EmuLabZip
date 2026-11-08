<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { app } from "../../stores/app.svelte";
  import { theme as store } from "../../stores/theme.svelte";
  import { GALLERY_ORIGIN, fetchGallery, shareUrl } from "../../theme/gallery";
  import type { Theme } from "../../theme/tokens";
  import Icon from "../Icon.svelte";
  import ThemeCard from "./ThemeCard.svelte";

  let themes = $state<Theme[]>([]);
  let status = $state<"loading" | "ready" | "offline">("loading");
  let sharing = $state<Theme | null>(null);

  async function load() {
    status = "loading";
    try {
      themes = await fetchGallery();
      status = "ready";
    } catch {
      status = "offline";
    }
  }
  onMount(load);

  const have = $derived(new Set(store.installed.map((t) => t.id)));

  function install(t: Theme) {
    if (store.install(t)) {
      store.choose(t);
      app.toast(`Installed ${t.name}`, "ok");
    }
  }

  const mine = $derived(store.installed);
</script>

<div class="space-y-6 p-5">
  <section>
    <div class="mb-2 flex items-center gap-2">
      <div class="flex-1">
        <h4 class="text-md font-medium">Community themes</h4>
        <p class="text-sm text-muted">Shared by other people at <button onclick={() => openUrl(`${GALLERY_ORIGIN}/themes/`)} class="text-accent hover:underline">{GALLERY_ORIGIN.replace("https://", "")}</button>. A theme only holds colours and font names.</p>
      </div>
      <button onclick={load} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-base font-medium hover:bg-hover"><Icon name="refresh" size={13} />Refresh</button>
    </div>

    {#if status === "loading"}
      <p class="py-8 text-center text-md text-muted">Loading the gallery…</p>
    {:else if status === "offline"}
      <div class="rounded-xl border border-line bg-panel-2 p-6 text-center">
        <p class="text-md font-medium">The gallery could not be reached.</p>
        <p class="mt-1 text-sm text-muted">Check your connection. Your themes and the built-in ones work without it.</p>
      </div>
    {:else if !themes.length}
      <p class="py-8 text-center text-md text-muted">No themes in the gallery yet. Be the first to share one.</p>
    {:else}
      <div class="grid grid-cols-3 gap-3">
        {#each themes as t (t.id)}
          <ThemeCard theme={t} onuse={() => install(t)}>
            <button onclick={() => install(t)} class="flex h-6 items-center gap-1 rounded-md bg-accent px-2 text-sm font-medium text-white hover:opacity-90">
              <Icon name={have.has(t.id) ? "check" : "download"} size={11} />{have.has(t.id) ? "Use" : "Install"}
            </button>
            {#if t.author}<span class="ml-auto truncate text-sm text-faint">{t.author}</span>{/if}
          </ThemeCard>
        {/each}
      </div>
    {/if}
  </section>

  <section>
    <h4 class="text-md font-medium">Share a theme</h4>
    <p class="mb-2 text-sm text-muted">Pick one of yours. GitHub opens with the file ready; sign in, press “Propose new file”, and it becomes a pull request that is checked and, if it reads well, added to the gallery.</p>
    {#if mine.length}
      <div class="flex flex-wrap gap-2">
        {#each mine as t (t.id)}
          <button onclick={() => (sharing = sharing?.id === t.id ? null : t)} class="h-8 rounded-lg border px-3 text-base {sharing?.id === t.id ? 'border-accent bg-accent-soft' : 'border-line hover:bg-hover'}">{t.name}</button>
        {/each}
      </div>
      {#if sharing}
        {#if sharing.author && sharing.description}
          <div class="mt-3 flex items-center gap-3 rounded-xl border border-line bg-panel-2 p-3">
            <p class="min-w-0 flex-1 text-sm text-muted">This opens <span class="mono">site/static/themes/{sharing.id}.json</span> on GitHub, in a pull request to EmuLabZip.</p>
            <button onclick={() => sharing && openUrl(shareUrl(sharing))} class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90"><Icon name="external" size={13} />Open on GitHub</button>
          </div>
        {:else}
          <p class="mt-3 rounded-lg border border-warn/50 bg-warn/10 px-3 py-2 text-base">The gallery needs an author and a description. Add them in the theme editor first.</p>
        {/if}
      {/if}
    {:else}
      <p class="rounded-xl border border-line bg-panel-2 p-4 text-sm text-muted">You have no themes of your own yet. Make one in Appearance, then come back to share it.</p>
    {/if}
  </section>
</div>

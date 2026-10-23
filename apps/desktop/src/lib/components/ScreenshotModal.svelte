<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { api, errorText } from "../api";
  import { app } from "../stores/app.svelte";
  import { fileStamp, slug } from "../format";
  import Icon from "./Icon.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let png = $state<string | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function capture() {
    if (!app.target) return;
    busy = true;
    error = null;
    try {
      png = await api.screenshot(app.target);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function saveAs() {
    if (!png || !app.device) return;
    const path = await app.attempt(() => api.savePng(`${slug(app.device!.name)}-${fileStamp()}.png`, png!));
    if (path) app.toast(`Saved to ${path}`, "ok");
  }

  capture();
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div transition:fade={{ duration: 120 }} class="fixed inset-0 z-50 grid place-items-center bg-black/45 p-8 backdrop-blur-[2px]">
  <button class="absolute inset-0 cursor-default" aria-label="Close" onclick={onclose}></button>
  <div transition:scale={{ start: 0.97, duration: 160 }} class="shadow-pop relative flex max-h-full w-[460px] flex-col overflow-hidden rounded-2xl border border-line bg-elevated">
    <div class="flex items-center gap-2 border-b border-line px-4 py-3">
      <Icon name="camera" size={15} class="text-accent" />
      <h2 class="flex-1 text-[13px] font-semibold">Screenshot · {app.device?.name}</h2>
      <button onclick={onclose} class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg" aria-label="Close">
        <Icon name="x" size={14} />
      </button>
    </div>
    <div class="grid min-h-[360px] flex-1 place-items-center overflow-auto bg-panel-2 p-5">
      {#if png}
        <img src="data:image/png;base64,{png}" alt="Device screen" class="max-h-[62vh] rounded-xl border border-line shadow-lg {busy ? 'opacity-60' : ''}" />
      {:else if error}
        <p class="max-w-[320px] text-center text-[12.5px] text-err">{error}</p>
      {:else}
        <div class="skeleton h-[420px] w-[200px]"></div>
      {/if}
    </div>
    <div class="flex items-center gap-2 border-t border-line px-4 py-3">
      <button onclick={capture} disabled={busy} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium hover:bg-hover disabled:opacity-50">
        <Icon name="refresh" size={14} class={busy ? "animate-spin" : ""} /> Retake
      </button>
      <span class="flex-1"></span>
      <button onclick={saveAs} disabled={!png} class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3.5 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-50">
        <Icon name="download" size={14} /> Save PNG…
      </button>
    </div>
  </div>
</div>

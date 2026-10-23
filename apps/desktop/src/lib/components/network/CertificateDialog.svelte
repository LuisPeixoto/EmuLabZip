<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import type { CaInstall } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { net } from "../../stores/net.svelte";
  import Icon from "../Icon.svelte";

  let { result, onclose }: { result: CaInstall; onclose: () => void } = $props();

  const titles = { system: "Trusted by every app", simulator: "Trusted by the simulator", user: "One more step" } as const;

  async function copySnippet() {
    if (!result.snippet) return;
    await navigator.clipboard?.writeText(result.snippet).catch(() => {});
    app.toast("Copied", "ok");
  }

  async function reveal() {
    if (net.state?.caPath) await revealItemInDir(net.state.caPath).catch(() => {});
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div transition:fade={{ duration: 120 }} class="fixed inset-0 z-50 grid place-items-center bg-black/45 p-8 backdrop-blur-[2px]">
  <button class="absolute inset-0 cursor-default" aria-label="Close" onclick={onclose}></button>
  <div transition:scale={{ start: 0.97, duration: 160 }} class="shadow-pop relative flex max-h-full w-[560px] flex-col overflow-hidden rounded-2xl border border-line bg-elevated">
    <header class="flex items-center gap-3 border-b border-line px-5 py-3.5">
      <span class="grid size-8 place-items-center rounded-xl {result.done ? 'bg-ok/12 text-ok' : 'bg-warn/12 text-warn'}">
        <Icon name={result.done ? "shieldCheck" : "shield"} size={17} />
      </span>
      <div class="min-w-0 flex-1">
        <h2 class="text-[13.5px] font-semibold">{titles[result.kind]}</h2>
        <p class="text-[11.5px] text-muted">Certificate for {app.device?.name}</p>
      </div>
      <button onclick={onclose} class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg" aria-label="Close"><Icon name="x" size={14} /></button>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
      <p class="text-[12.5px] leading-relaxed">{result.message}</p>

      {#if result.steps.length}
        <ol class="mt-4 space-y-2.5">
          {#each result.steps as step, i (i)}
            <li class="flex gap-3 text-[12.5px] leading-relaxed">
              <span class="mono mt-px grid size-5 shrink-0 place-items-center rounded-full bg-accent-soft text-[10.5px] font-semibold text-accent">{i + 1}</span>
              <span>{step}</span>
            </li>
          {/each}
        </ol>
      {/if}

      {#if result.snippet}
        <div class="mt-4 overflow-hidden rounded-xl border border-line">
          <div class="flex items-center gap-2 border-b border-line bg-panel-2 px-3 py-1.5">
            <span class="mono flex-1 text-[11px] text-muted">res/xml/network_security_config.xml</span>
            <button onclick={copySnippet} class="flex h-6 items-center gap-1.5 rounded-md px-2 text-[11.5px] text-muted hover:bg-hover hover:text-fg"><Icon name="copy" size={12} />Copy</button>
          </div>
          <pre class="mono selectable overflow-x-auto bg-panel-2 px-3 py-2.5 text-[11.5px] leading-relaxed">{result.snippet}</pre>
        </div>
      {/if}
    </div>

    <footer class="flex items-center gap-2 border-t border-line bg-panel-2 px-5 py-3">
      {#if net.state?.caPath}
        <button onclick={reveal} class="flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[12px] text-muted hover:bg-hover hover:text-fg"><Icon name="folderOpen" size={13} />Show the certificate file</button>
      {/if}
      <span class="flex-1"></span>
      <button onclick={onclose} class="h-8 rounded-lg bg-accent px-4 text-[12px] font-semibold text-white hover:opacity-90">Done</button>
    </footer>
  </div>
</div>

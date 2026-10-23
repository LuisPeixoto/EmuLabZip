<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Icon from "../Icon.svelte";

  let { icon, title, phase, summary, features }: { icon: string; title: string; phase: string; summary: string; features: string[] } = $props();
</script>

<div class="grid h-full place-items-center overflow-y-auto p-10">
  <div class="w-full max-w-[560px] text-center">
    <span class="mx-auto mb-5 grid size-14 place-items-center rounded-2xl bg-accent-soft text-accent"><Icon name={icon} size={26} /></span>
    <p class="mb-2 text-[11px] font-semibold tracking-wider text-accent uppercase">{phase}</p>
    <h1 class="text-[22px] font-semibold tracking-tight">{title}</h1>
    <p class="mx-auto mt-2 max-w-[460px] text-[13px] leading-relaxed text-muted">{summary}</p>
    <ul class="mx-auto mt-6 max-w-[420px] space-y-2 text-left">
      {#each features as f (f)}
        <li class="flex items-start gap-2.5 rounded-xl border border-line bg-panel px-3.5 py-2.5 text-[12.5px]">
          <Icon name="check" size={14} class="mt-0.5 shrink-0 text-accent" />{f}
        </li>
      {/each}
    </ul>
    <button
      onclick={() => openUrl("https://github.com/LuisPeixoto/EmuLabZip/blob/main/docs/roadmap.md")}
      class="mt-6 inline-flex h-8 items-center gap-1.5 rounded-lg border border-line px-3.5 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg"
    >
      <Icon name="external" size={13} /> See the roadmap
    </button>
  </div>
</div>

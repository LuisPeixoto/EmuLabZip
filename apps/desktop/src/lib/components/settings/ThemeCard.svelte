<script lang="ts">
  import type { Theme } from "../../theme/tokens";
  import Icon from "../Icon.svelte";

  let {
    theme,
    active = false,
    badge = "",
    onuse,
    children,
  }: { theme: Theme; active?: boolean; badge?: string; onuse: () => void; children?: import("svelte").Snippet } = $props();

  const c = $derived(theme.colors);
</script>

<div class="group flex flex-col overflow-hidden rounded-xl border bg-panel-2 {active ? 'border-accent ring-1 ring-accent' : 'border-line hover:border-line-strong'}">
  <button onclick={onuse} class="block text-left" aria-label="Use {theme.name}" aria-pressed={active}>
    <!-- A small picture of the app in this theme: sidebar, a list row, a selected row and a button. -->
    <div class="flex h-[84px] overflow-hidden" style="background:{c.bg}">
      <div class="w-[22%] space-y-1.5 p-2" style="background:{c['panel-2']};border-right:1px solid {c.line}">
        <div class="h-1.5 w-full rounded-full" style="background:{c.accent}"></div>
        <div class="h-1.5 w-3/4 rounded-full" style="background:{c.faint}"></div>
        <div class="h-1.5 w-2/3 rounded-full" style="background:{c.faint}"></div>
      </div>
      <div class="flex-1 space-y-1.5 p-2">
        <div class="flex items-center gap-1.5 rounded px-1.5 py-1" style="background:{c.panel};border:1px solid {c.line}">
          <span class="size-1.5 rounded-full" style="background:{c.ok}"></span>
          <span class="h-1.5 w-1/2 rounded-full" style="background:{c.fg}"></span>
          <span class="ml-auto h-1.5 w-1/6 rounded-full" style="background:{c.muted}"></span>
        </div>
        <div class="flex items-center gap-1.5 rounded px-1.5 py-1" style="background:{c['accent-soft']}">
          <span class="size-1.5 rounded-full" style="background:{c.warn}"></span>
          <span class="h-1.5 w-2/5 rounded-full" style="background:{c.accent}"></span>
        </div>
        <div class="flex gap-1.5">
          <span class="h-3 w-8 rounded" style="background:{c.accent}"></span>
          <span class="h-3 w-6 rounded" style="background:{c.err}"></span>
          <span class="h-3 w-6 rounded" style="background:{c.info}"></span>
        </div>
      </div>
    </div>
    <div class="flex items-center gap-2 px-3 pt-2.5 pb-1">
      <span class="min-w-0 flex-1 truncate text-md font-medium">{theme.name}</span>
      {#if active}<Icon name="check" size={13} class="text-accent" />{/if}
      <span class="rounded-full bg-hover px-1.5 py-px text-tiny text-muted capitalize">{badge || theme.base}</span>
    </div>
    <p class="line-clamp-2 min-h-8 px-3 pb-2 text-sm text-muted">{theme.description ?? (theme.author ? `By ${theme.author}` : "")}</p>
  </button>
  {#if children}<div class="mt-auto flex flex-wrap items-center gap-1 border-t border-line px-2 py-1.5">{@render children()}</div>{/if}
</div>

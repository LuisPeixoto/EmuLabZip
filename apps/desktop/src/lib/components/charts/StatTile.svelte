<script lang="ts">
  import Icon from "../Icon.svelte";

  let { icon, label, value, detail = "" }: { icon: string; label: string; value: string; detail?: string } = $props();
</script>

<div class="min-w-0 rounded-2xl border border-line bg-panel px-4 py-3">
  <p class="flex items-center gap-1.5 text-[11.5px] text-muted"><Icon name={icon} size={13} class="text-faint" />{label}</p>
  <p class="mono mt-1.5 truncate text-[21px] font-semibold tracking-tight text-fg">{value}</p>
  <p class="mt-0.5 truncate text-[11px] text-faint">{detail || " "}</p>
</div>

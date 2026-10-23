<script lang="ts">
  import type { Preview } from "../../api";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";
  import SqliteViewer from "./SqliteViewer.svelte";

  let { preview, onsave }: { preview: Preview; onsave: () => void } = $props();

  const kindLabel: Record<string, string> = {
    sqlite: "SQLite database",
    sharedPrefs: "SharedPreferences",
    plist: "Property list",
    json: "JSON",
    image: "Image",
    text: "Text",
    binary: "Binary",
  };

  const pretty = $derived.by(() => {
    if (preview.plist !== null && preview.plist !== undefined) return JSON.stringify(preview.plist, null, 2);
    if (preview.kind === "json" && preview.text) {
      try {
        return JSON.stringify(JSON.parse(preview.text), null, 2);
      } catch {
        return preview.text;
      }
    }
    return null;
  });
</script>

<div class="flex h-full min-h-0 flex-col">
  <header class="flex shrink-0 items-center gap-3 border-b border-line px-4 py-2.5">
    <Icon name={preview.kind === "sqlite" ? "database" : preview.kind === "image" ? "eye" : "file"} size={15} class="text-accent" />
    <div class="min-w-0 flex-1">
      <p class="truncate text-[13px] font-semibold">{preview.name}</p>
      <p class="text-[11px] text-muted">{kindLabel[preview.kind ?? "binary"]} · {bytes(preview.size)}{preview.truncated ? " · preview truncated" : ""}</p>
    </div>
    <button onclick={onsave} class="flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg">
      <Icon name="download" size={13} /> Save…
    </button>
  </header>
  {#if preview.error}
    <p class="shrink-0 border-b border-line bg-warn/10 px-4 py-2 text-[12px] text-warn">{preview.error}</p>
  {/if}
  <div class="min-h-0 flex-1 overflow-auto">
    {#if preview.tables}
      <SqliteViewer id={preview.id} tables={preview.tables} />
    {:else if preview.prefs}
      <table class="selectable w-full text-[12px]">
        <thead class="sticky top-0 bg-panel-2 text-left text-[11px] text-muted">
          <tr><th class="px-4 py-2 font-medium">Key</th><th class="px-2 py-2 font-medium">Type</th><th class="px-4 py-2 font-medium">Value</th></tr>
        </thead>
        <tbody>
          {#each preview.prefs as p (p.key)}
            <tr class="border-t border-line/60 align-top hover:bg-hover">
              <td class="mono px-4 py-1.5 font-medium">{p.key}</td>
              <td class="px-2 py-1.5"><span class="rounded-md bg-hover px-1.5 py-px text-[10.5px] text-muted">{p.kind}</span></td>
              <td class="mono px-4 py-1.5 break-all">{p.value}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else if preview.image}
      <div class="grid min-h-full place-items-center bg-[repeating-conic-gradient(var(--hover)_0_25%,transparent_0_50%)] bg-[length:20px_20px] p-6">
        <img src={preview.image} alt={preview.name} class="max-h-[70vh] max-w-full rounded-lg shadow-lg" />
      </div>
    {:else if pretty}
      <pre class="mono selectable p-4 text-[12px] leading-relaxed whitespace-pre-wrap">{pretty}</pre>
    {:else if preview.text != null}
      <pre class="mono selectable p-4 text-[12px] leading-relaxed whitespace-pre-wrap">{preview.text}</pre>
    {:else if preview.hex}
      <pre class="mono selectable p-4 text-[11.5px] leading-relaxed text-muted">{preview.hex}</pre>
    {/if}
  </div>
</div>

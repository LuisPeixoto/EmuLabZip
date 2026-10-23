<script lang="ts">
  import type { BodyView } from "../../api";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";

  let { body, empty = "No body" }: { body: BodyView | null; empty?: string } = $props();

  let copied = $state(false);

  async function copy() {
    if (!body?.text) return;
    await navigator.clipboard?.writeText(body.text).catch(() => {});
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }
</script>

{#if !body}
  <p class="px-4 py-8 text-center text-[12.5px] text-muted">{empty}</p>
{:else}
  <div class="flex items-center gap-2 border-b border-line px-4 py-2 text-[11.5px] text-muted">
    <span class="mono">{bytes(body.size)}</span>
    {#if body.encoding}
      <span class="rounded-md bg-hover px-1.5 py-px">{body.encoding}</span>
      <span class="text-faint">→ {bytes(body.decodedSize)} decoded</span>
    {/if}
    {#if body.truncated}<span class="rounded-md bg-warn/12 px-1.5 py-px text-warn">only the first part is stored</span>{/if}
    <span class="flex-1"></span>
    {#if body.text}
      <button onclick={copy} class="flex h-6 items-center gap-1.5 rounded-md px-2 hover:bg-hover hover:text-fg">
        <Icon name={copied ? "check" : "copy"} size={12} class={copied ? "text-ok" : ""} />{copied ? "Copied" : "Copy"}
      </button>
    {/if}
  </div>
  {#if body.error}
    <p class="border-b border-line bg-warn/10 px-4 py-2 text-[12px] text-warn">{body.error}</p>
  {/if}
  {#if body.image}
    <div class="grid place-items-center bg-[repeating-conic-gradient(var(--hover)_0_25%,transparent_0_50%)] bg-[length:20px_20px] p-5">
      <img src={body.image} alt="Response body" class="max-h-[40vh] max-w-full rounded-lg shadow-lg" />
    </div>
  {:else if body.text != null}
    <pre class="mono selectable px-4 py-3 text-[12px] leading-relaxed break-words whitespace-pre-wrap">{body.text}</pre>
  {:else if body.hex}
    <pre class="mono selectable px-4 py-3 text-[11.5px] leading-relaxed text-muted">{body.hex}</pre>
  {/if}
{/if}

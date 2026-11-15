<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../../api";
  import { clients, snippet, type Client } from "../../ai";
  import { app } from "../../stores/app.svelte";
  import Icon from "../Icon.svelte";

  let client = $state<Client>("claude-code");
  let allowWrites = $state(false);
  let path = $state<string | null>(null);
  let looked = $state(false);

  onMount(async () => {
    try {
      path = (await api.aiCliInfo()).path;
    } catch {
      path = null;
    }
    looked = true;
  });

  const command = $derived(path ?? "emulabzip");
  const text = $derived(snippet(client, command, allowWrites));
  const current = $derived(clients.find((c) => c.id === client)!);

  async function copy() {
    await navigator.clipboard?.writeText(text).catch(() => {});
    app.toast("Copied", "ok");
  }
</script>

<div class="space-y-3 p-3">
  <p class="px-2 pt-1 text-2xs font-semibold tracking-wider text-faint uppercase">AI assistants</p>
  <p class="px-2 text-md text-muted">
    Let an assistant look at your emulators and simulators: it can read logs, crashes, requests and app data, and drive the screen. It starts the <span class="mono">emulabzip</span> command, which runs on this computer.
  </p>

  <div class="flex gap-1 px-2">
    {#each clients as c (c.id)}
      <button
        onclick={() => (client = c.id)}
        class="h-8 rounded-lg px-3 text-base {client === c.id ? 'bg-accent-soft font-medium text-accent' : 'text-fg/85 hover:bg-hover'}"
      >{c.label}</button>
    {/each}
  </div>

  <div class="mx-2 rounded-xl border border-line bg-panel-2">
    <div class="flex items-center gap-2 border-b border-line px-3 py-2">
      <p class="flex-1 text-sm text-muted">{current.where}</p>
      <button onclick={copy} class="flex h-7 items-center gap-1.5 rounded-lg border border-line px-2.5 text-base font-medium hover:bg-hover">
        <Icon name="copy" size={13} />Copy
      </button>
    </div>
    <pre class="mono overflow-x-auto p-3 text-sm leading-relaxed whitespace-pre">{text}</pre>
  </div>

  {#if looked && !path}
    <p class="mx-2 rounded-xl border border-line bg-panel-2 px-3 py-2 text-sm text-muted">
      The <span class="mono">emulabzip</span> command was not found on this computer. Install it with
      <span class="mono">cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli</span>, then open this page again so the full path is filled in.
    </p>
  {/if}

  <label class="mx-2 flex items-center gap-4 rounded-xl px-2 py-2">
    <div class="min-w-0 flex-1">
      <p class="text-md font-medium">Let the assistant change data</p>
      <p class="text-sm text-muted">Adds <span class="mono">--allow-writes</span>: wiping and deleting devices, restoring states and editing app files become available. Each one asks you to confirm first. Leave it off to keep the assistant read-only.</p>
    </div>
    <input type="checkbox" bind:checked={allowWrites} class="size-4 accent-[var(--accent)]" />
  </label>
</div>

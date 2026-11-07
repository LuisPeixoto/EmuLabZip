<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, errorText, type AnalyticsItem } from "../../api";
  import { dateTime } from "../../format";
  import Icon from "../Icon.svelte";

  let items = $state<AnalyticsItem[]>([]);
  let service = $state("all");
  let needle = $state("");
  let open = $state<number | null>(null);
  let error = $state<string | null>(null);
  let after = 0;
  let timer: ReturnType<typeof setInterval> | null = null;

  async function poll() {
    try {
      const fresh = await api.analyticsEvents(after);
      if (fresh.length) {
        after = Math.max(after, ...fresh.map((i) => i.flow));
        items = [...fresh.reverse(), ...items].slice(0, 500);
      }
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(() => {
    poll();
    timer = setInterval(poll, 1500);
  });
  onDestroy(() => timer && clearInterval(timer));

  function clear() {
    items = [];
    open = null;
  }

  const shown = $derived(
    items.filter(
      (i) => (service === "all" || i.event.service === service) && (!needle || `${i.event.name} ${i.event.user ?? ""} ${JSON.stringify(i.event.properties)}`.toLowerCase().includes(needle.toLowerCase())),
    ),
  );
  const label: Record<string, string> = { segment: "Segment", amplitude: "Amplitude", mixpanel: "Mixpanel", firebase: "Firebase" };
</script>

<div class="flex h-full min-h-0 flex-col">
  <header class="flex shrink-0 flex-wrap items-center gap-3 border-b border-line px-6 py-3">
    <div class="mr-3">
      <h1 class="text-[18px] font-semibold tracking-tight">Analytics</h1>
      <p class="text-[12px] text-muted">The events the app sends to Segment, Amplitude, Mixpanel and Firebase, read from the requests the proxy records.</p>
    </div>
    <span class="flex-1"></span>
    <select bind:value={service} class="h-8 rounded-lg border border-line bg-panel-2 px-2 text-[12px]">
      <option value="all">All services</option>
      {#each Object.entries(label) as [id, l] (id)}<option value={id}>{l}</option>{/each}
    </select>
    <label class="relative flex items-center">
      <Icon name="search" size={12} class="pointer-events-none absolute left-2.5 text-faint" />
      <input bind:value={needle} spellcheck="false" placeholder="Find an event or property" class="h-8 w-56 rounded-lg border border-line bg-panel-2 pr-2 pl-7 text-[12px] outline-none focus:border-accent" />
    </label>
    <button onclick={clear} class="h-8 rounded-lg border border-line px-3 text-[12px] text-muted hover:bg-hover hover:text-fg">Clear</button>
  </header>
  {#if error}<p class="border-b border-line bg-err/8 px-6 py-2 text-[12px] text-err">{error}</p>{/if}
  <div class="selectable min-h-0 flex-1 overflow-auto">
    {#each shown as it, i (it.flow + ":" + i + it.event.name)}
      <button class="block w-full border-b border-line/60 px-6 py-2 text-left hover:bg-hover" onclick={() => (open = open === i ? null : i)}>
        <p class="flex items-center gap-3 text-[12.5px]">
          <span class="w-20 shrink-0 text-[11px] text-faint">{dateTime(it.startedMs / 1000)}</span>
          <span class="w-20 shrink-0 rounded-md bg-hover px-1.5 py-px text-center text-[10.5px] font-medium text-muted">{label[it.event.service]}</span>
          <span class="w-16 shrink-0 text-[11px] text-faint">{it.event.kind}</span>
          <span class="min-w-0 flex-1 truncate font-medium">{it.event.name}</span>
          <span class="mono max-w-[200px] truncate text-[11px] text-muted">{it.event.user ?? ""}</span>
        </p>
        {#if open === i}
          <pre class="mono mt-2 rounded-lg bg-panel-2 p-3 text-[11.5px] whitespace-pre-wrap">{JSON.stringify(it.event.properties, null, 2)}</pre>
        {/if}
      </button>
    {:else}
      <p class="px-6 py-14 text-center text-[12.5px] text-muted">No analytics events yet. Start the proxy on the Network view and connect the device, then use the app. Calls from SDKs that pin their certificate pass through unchanged and cannot be read.</p>
    {/each}
  </div>
</div>

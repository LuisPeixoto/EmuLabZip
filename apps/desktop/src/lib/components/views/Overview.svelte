<script lang="ts">
  import { api, errorText, osLabel, type InfoSection } from "../../api";
  import { app } from "../../stores/app.svelte";
  import Icon from "../Icon.svelte";
  import PlatformIcon from "../PlatformIcon.svelte";

  let sections = $state<InfoSection[] | null>(null);
  let error = $state<string | null>(null);
  let screen = $state<string | null>(null);
  let screenBusy = $state(false);
  let copied = $state<string | null>(null);

  const sectionIcon: Record<string, string> = {
    Device: "smartphone",
    Software: "layers",
    Hardware: "cpu",
    Display: "monitor",
    Battery: "battery",
    Locale: "globe",
    Storage: "database",
    Host: "laptop",
  };

  async function load(key: string) {
    const target = app.target;
    if (!target || !app.online) return;
    sections = null;
    error = null;
    try {
      const out = await api.deviceInfo(target);
      if (key === app.deviceKey) sections = out;
    } catch (e) {
      if (key === app.deviceKey) error = errorText(e);
    }
    refreshScreen();
  }

  async function refreshScreen() {
    if (!app.target || screenBusy) return;
    screenBusy = true;
    try {
      screen = await api.screenshot(app.target);
    } catch {
      screen = null;
    } finally {
      screenBusy = false;
    }
  }

  async function copy(key: string, value: string) {
    await navigator.clipboard?.writeText(value).catch(() => {});
    copied = key;
    setTimeout(() => copied === key && (copied = null), 1200);
  }

  $effect(() => {
    const key = app.deviceKey;
    screen = null;
    load(key);
  });

  const chips = $derived(
    app.device
      ? [
          app.device.kind === "emulator" ? "Emulator" : app.device.kind === "simulator" ? "Simulator" : "Physical device",
          app.device.model,
          app.device.abi,
        ].filter(Boolean)
      : [],
  );
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1180px] p-6">
    {#if app.device}
      <section class="relative mb-5 flex items-center gap-5 overflow-hidden rounded-2xl border border-line bg-panel p-5">
        <div class="brand-gradient pointer-events-none absolute -top-24 -right-24 size-64 rounded-full opacity-[0.12] blur-3xl"></div>
        <PlatformIcon platform={app.device.platform} size={26} class="rounded-2xl" />
        <div class="min-w-0 flex-1">
          <h1 class="truncate text-[20px] font-semibold tracking-tight">{app.device.name}</h1>
          <p class="mt-0.5 text-[12.5px] text-muted">{osLabel(app.device)}</p>
          <div class="mt-2.5 flex flex-wrap gap-1.5">
            {#each chips as chip (chip)}
              <span class="rounded-full border border-line bg-panel-2 px-2 py-0.5 text-[11px] text-muted">{chip}</span>
            {/each}
            <span class="mono rounded-full border border-line bg-panel-2 px-2 py-0.5 text-[11px] text-faint">{app.device.id}</span>
          </div>
        </div>
        <div class="flex flex-col items-end gap-2 text-right">
          <span class="flex items-center gap-1.5 rounded-full bg-ok/12 px-2.5 py-1 text-[11.5px] font-medium text-ok">
            <span class="live-dot size-1.5 rounded-full bg-ok"></span> Connected
          </span>
          <span class="text-[11.5px] text-faint">{app.apps.filter((a) => a.pid != null).length || "—"} apps running</span>
        </div>
      </section>

      <div class="grid gap-5 lg:grid-cols-[1fr_260px]">
        <div class="grid content-start gap-4 md:grid-cols-2">
          {#if error}
            <div class="rounded-2xl border border-err/30 bg-err/8 p-4 text-[12.5px] text-err md:col-span-2">{error}</div>
          {:else if !sections}
            {#each Array(4) as _, i (i)}
              <div class="rounded-2xl border border-line bg-panel p-4">
                <div class="skeleton mb-4 h-4 w-28"></div>
                {#each Array(4) as __, j (j)}<div class="skeleton mb-2.5 h-3"></div>{/each}
              </div>
            {/each}
          {:else}
            {#each sections as s (s.title)}
              <section class="rounded-2xl border border-line bg-panel">
                <h2 class="flex items-center gap-2 border-b border-line px-4 py-2.5 text-[12.5px] font-semibold">
                  <Icon name={sectionIcon[s.title] ?? "info"} size={14} class="text-accent" />{s.title}
                </h2>
                <dl class="px-2 py-1.5">
                  {#each s.items as [k, v] (k)}
                    <button
                      onclick={() => copy(`${s.title}.${k}`, v)}
                      title="Click to copy"
                      class="group flex w-full items-baseline gap-3 rounded-lg px-2 py-1.5 text-left hover:bg-hover"
                    >
                      <dt class="w-[118px] shrink-0 text-[12px] text-muted">{k}</dt>
                      <dd class="min-w-0 flex-1 text-[12.5px] break-words {v.length > 40 ? 'mono text-[11px]' : ''}">{v}</dd>
                      <Icon name={copied === `${s.title}.${k}` ? "check" : "copy"} size={12} class="shrink-0 {copied === `${s.title}.${k}` ? 'text-ok' : 'text-faint opacity-0 group-hover:opacity-100'}" />
                    </button>
                  {/each}
                </dl>
              </section>
            {/each}
          {/if}
        </div>

        <aside class="flex flex-col items-center gap-3 self-start rounded-2xl border border-line bg-panel p-4 lg:sticky lg:top-0">
          <div class="flex w-full items-center justify-between">
            <h2 class="text-[12.5px] font-semibold">Screen</h2>
            <button onclick={refreshScreen} title="Refresh" class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg">
              <Icon name="refresh" size={13} class={screenBusy ? "animate-spin" : ""} />
            </button>
          </div>
          {#if screen}
            <img src="data:image/png;base64,{screen}" alt="Device screen" class="w-full rounded-[18px] border-[5px] border-[#11141b] shadow-xl" />
          {:else}
            <div class="skeleton aspect-[9/19.5] w-full rounded-[18px]"></div>
          {/if}
        </aside>
      </div>
    {/if}
  </div>
</div>

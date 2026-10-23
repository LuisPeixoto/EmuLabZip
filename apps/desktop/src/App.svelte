<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { app, type View } from "./lib/stores/app.svelte";
  import { logs } from "./lib/stores/logs.svelte";
  import EmptyState from "./lib/components/EmptyState.svelte";
  import ScreenshotModal from "./lib/components/ScreenshotModal.svelte";
  import SettingsModal from "./lib/components/SettingsModal.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import Toolbar from "./lib/components/Toolbar.svelte";
  import Logs from "./lib/components/views/Logs.svelte";
  import Memory from "./lib/components/views/Memory.svelte";
  import Overview from "./lib/components/views/Overview.svelte";
  import Performance from "./lib/components/views/Performance.svelte";
  import Storage from "./lib/components/views/Storage.svelte";
  import Upcoming from "./lib/components/views/Upcoming.svelte";

  let screenshotOpen = $state(false);

  onMount(() => {
    app.init();
    // Process ids change when apps start and stop; keep the app list fresh.
    const timer = setInterval(() => app.online && app.refreshApps(), 5000);
    return () => clearInterval(timer);
  });

  $effect(() => {
    document.documentElement.dataset.theme = app.theme;
  });

  // Follow the log of the selected device as soon as it is ready, whatever view is open, so
  // history is already there when the user switches to Logs.
  $effect(() => {
    const key = app.deviceKey;
    untrack(() => {
      const target = app.target;
      if (target && app.online) {
        logs.attach(key, target);
        app.refreshApps();
      } else {
        logs.detach();
      }
    });
  });

  const shortcuts: View[] = ["overview", "logs", "performance", "memory", "storage"];

  function onkeydown(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key === ",") {
      e.preventDefault();
      app.settingsOpen = true;
    } else if (mod && e.key.toLowerCase() === "r") {
      e.preventDefault(); // never reload the webview
      app.refreshDevices();
    } else if (mod && /^[1-5]$/.test(e.key)) {
      e.preventDefault();
      app.setView(shortcuts[Number(e.key) - 1]);
    }
  }
</script>

<svelte:window {onkeydown} oncontextmenu={(e) => e.preventDefault()} />

<div class="flex h-full flex-col">
  <Toolbar onScreenshot={() => (screenshotOpen = true)} />
  <div class="flex min-h-0 flex-1">
    <Sidebar />
    <main class="min-w-0 flex-1 bg-bg">
      {#if !app.device || (!app.online && app.view !== "logs")}
        {#if app.device && app.device.state !== "online"}
          <div class="grid h-full place-items-center text-center">
            <div>
              <span class="mx-auto mb-4 block size-3 rounded-full {app.device.state === 'booting' ? 'live-dot bg-warn' : 'bg-faint'}"></span>
              <p class="text-[15px] font-semibold">{app.device.name} is {app.device.state}</p>
              <p class="mt-1 text-[12.5px] text-muted">
                {app.device.state === "unauthorized"
                  ? "Accept the USB debugging prompt on the device."
                  : app.device.state === "booting"
                    ? "Inspection starts as soon as it finishes booting."
                    : "Waiting for it to come back online."}
              </p>
            </div>
          </div>
        {:else}
          <EmptyState />
        {/if}
      {:else if app.view === "overview"}
        <Overview />
      {:else if app.view === "logs"}
        <Logs />
      {:else if app.view === "performance"}
        <Performance />
      {:else if app.view === "memory"}
        <Memory />
      {:else if app.view === "storage"}
        <Storage />
      {:else if app.view === "network"}
        <Upcoming
          icon="globe"
          phase="Phase 3"
          title="Network"
          summary="See every request your app makes, without changing its code."
          features={["Built-in HTTP(S) proxy, wired to the emulator or simulator in one click", "Certificate installation for the device", "HAR export and pcap import"]}
        />
      {:else}
        <Upcoming
          icon="route"
          phase="Phase 4"
          title="Traces"
          summary="Record system traces and see exactly what happened, frame by frame."
          features={["Perfetto recording with ready-made presets", "SQL over traces with trace_processor", "CPU flame graphs with simpleperf", "Instruments traces through xctrace"]}
        />
      {/if}
    </main>
  </div>
</div>

{#if app.settingsOpen}<SettingsModal />{/if}
{#if screenshotOpen}<ScreenshotModal onclose={() => (screenshotOpen = false)} />{/if}
<Toasts />

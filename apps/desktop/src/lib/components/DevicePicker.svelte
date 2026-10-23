<script lang="ts">
  import { fly } from "svelte/transition";
  import { osLabel, type Device } from "../api";
  import { clickOutside } from "../actions";
  import { app } from "../stores/app.svelte";
  import Icon from "./Icon.svelte";
  import PlatformIcon from "./PlatformIcon.svelte";

  let open = $state(false);

  const stateLabel: Record<Device["state"], string> = {
    online: "Online",
    booting: "Booting",
    offline: "Offline",
    unauthorized: "Unauthorized",
  };
  const stateDot: Record<Device["state"], string> = {
    online: "bg-ok",
    booting: "bg-warn",
    offline: "bg-faint",
    unauthorized: "bg-err",
  };

  const groups = $derived([
    { label: "Android", items: app.devices.filter((d) => d.platform === "android") },
    { label: "iOS Simulator", items: app.devices.filter((d) => d.platform === "ios") },
  ]);

  function pick(d: Device) {
    app.selectDevice(d.id);
    open = false;
  }
</script>

<div class="relative" use:clickOutside={() => (open = false)}>
  <button
    onclick={() => (open = !open)}
    title="Choose a device"
    class="flex h-9 min-w-[230px] items-center gap-2.5 rounded-lg border px-2 text-left transition {open
      ? 'border-accent bg-panel ring-3 ring-accent-soft'
      : 'border-line bg-panel-2 hover:border-line-strong'}"
  >
    {#if app.device}
      <PlatformIcon platform={app.device.platform} />
      <span class="min-w-0 flex-1 leading-tight">
        <span class="block truncate text-[12.5px] font-semibold">{app.device.name}</span>
        <span class="block truncate text-[10.5px] text-muted">{osLabel(app.device)}</span>
      </span>
      <span class="size-2 shrink-0 rounded-full {stateDot[app.device.state]} {app.device.state === 'online' ? 'live-dot' : ''}"></span>
    {:else}
      <span class="grid size-6 place-items-center rounded-md bg-hover text-faint"><Icon name="smartphone" size={14} /></span>
      <span class="flex-1 text-[12.5px] text-muted">{app.devicesLoaded ? "No device running" : "Looking for devices…"}</span>
    {/if}
    <Icon name="chevronDown" size={14} class="shrink-0 text-faint" />
  </button>

  {#if open}
    <div
      transition:fly={{ y: -6, duration: 140 }}
      class="shadow-pop absolute top-11 left-0 z-50 w-[340px] overflow-hidden rounded-xl border border-line bg-elevated"
    >
      <div class="max-h-[420px] overflow-y-auto p-1.5">
        {#each groups as group (group.label)}
          {#if group.items.length}
            <p class="px-2.5 pt-2 pb-1 text-[10.5px] font-semibold tracking-wider text-faint uppercase">{group.label}</p>
            {#each group.items as d (d.id)}
              <button
                onclick={() => pick(d)}
                class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left {d.id === app.deviceId ? 'bg-accent-soft' : 'hover:bg-hover'}"
              >
                <PlatformIcon platform={d.platform} />
                <span class="min-w-0 flex-1 leading-tight">
                  <span class="block truncate text-[12.5px] font-medium {d.id === app.deviceId ? 'text-accent' : ''}">{d.name}</span>
                  <span class="mono block truncate text-[10.5px] text-faint">{osLabel(d)} · {d.id}</span>
                </span>
                <span class="flex items-center gap-1.5 text-[10.5px] text-muted">
                  <span class="size-1.5 rounded-full {stateDot[d.state]}"></span>{stateLabel[d.state]}
                </span>
              </button>
            {/each}
          {/if}
        {/each}
        {#if !app.devices.length}
          <div class="px-3 py-6 text-center text-[12px] text-muted">
            <Icon name="smartphone" size={22} class="mx-auto mb-2 text-faint" />
            Start an Android emulator or boot an iOS simulator.<br />It shows up here within a few seconds.
          </div>
        {/if}
      </div>
      <div class="flex items-center gap-3 border-t border-line bg-panel-2 px-3 py-2 text-[11px] text-muted">
        {#each app.platforms as p (p.platform)}
          <span class="flex items-center gap-1.5" title={p.error ?? (p.available ? "Ready" : "Not available on this computer")}>
            <span class="size-1.5 rounded-full {p.available && !p.error ? 'bg-ok' : p.error ? 'bg-err' : 'bg-faint'}"></span>
            {p.platform === "android" ? "adb" : "simctl"}
            <span class="text-faint">{p.available ? (p.error ? "error" : "ready") : "unavailable"}</span>
          </span>
        {/each}
        <button onclick={() => app.refreshDevices()} title="Refresh" class="ml-auto grid size-6 place-items-center rounded-md text-faint hover:bg-hover hover:text-fg">
          <Icon name="refresh" size={13} />
        </button>
      </div>
    </div>
  {/if}
</div>

<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type Choice, type Device, type DeviceOptions, type EmulatorSnapshot, type Platform } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { confirmed } from "../../confirm";
  import Icon from "../Icon.svelte";
  import PlatformIcon from "../PlatformIcon.svelte";

  let devices = $state<Device[] | null>(null);
  let problems = $state<string[]>([]);
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);
  /** The emulator whose snapshots are open. */
  let snapshotsOf = $state<string | null>(null);
  let snapshots = $state<EmulatorSnapshot[]>([]);
  let snapName = $state("");

  let creating = $state(false);
  let platform = $state<Platform>("android");
  let options = $state<DeviceOptions | null>(null);
  let name = $state("");
  let image = $state("");
  let model = $state("");

  const target = (d: Device) => ({ platform: d.platform, id: d.id });
  const running = (d: Device) => d.state !== "shutdown";

  async function refresh() {
    try {
      const all = await api.devicesAll();
      devices = all.devices;
      problems = all.problems;
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(() => {
    refresh();
    const timer = setInterval(refresh, 4000);
    return () => clearInterval(timer);
  });

  /** Runs one change to a device, shows what went wrong, and looks again. */
  async function act(d: Device, label: string, work: () => Promise<unknown>) {
    busy = `${d.id}:${label}`;
    error = null;
    try {
      await work();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
      refresh();
      app.refreshDevices();
    }
  }

  const boot = (d: Device, headless = false) =>
    act(d, "boot", () => api.deviceBoot(target(d), { wipe: false, cold: false, headless }, false));
  const shutdown = (d: Device) => act(d, "shutdown", () => api.deviceShutdown(target(d)));

  async function wipe(d: Device) {
    const word = d.platform === "ios" ? "Erase" : "Wipe";
    if (!(await confirmed(`${word} ${d.name}?\n\nEverything on it is thrown away: apps, their data, accounts and settings. This cannot be undone.`, `${word} ${d.name}`, word))) return;
    act(d, "wipe", () => api.deviceWipe(target(d)));
  }

  async function remove(d: Device) {
    if (running(d)) return (error = `${d.name} is running. Shut it down first.`);
    if (!(await confirmed(`Delete ${d.name}?\n\nThe device and everything on it is removed for good.`, `Delete ${d.name}`, "Delete"))) return;
    act(d, "delete", () => api.deviceDelete(target(d)));
  }

  async function openSnapshots(d: Device) {
    if (snapshotsOf === d.id) return (snapshotsOf = null);
    snapshotsOf = d.id;
    await readSnapshots(d);
  }

  async function readSnapshots(d: Device) {
    try {
      snapshots = await api.emulatorSnapshots(target(d));
    } catch (e) {
      snapshots = [];
      error = errorText(e);
    }
  }

  async function snap(d: Device, action: "save" | "load" | "delete", n: string) {
    if (action === "load" && !(await confirmed(`Go back to “${n}”?\n\nWhat ${d.name} is doing now is lost.`, "Load snapshot", "Load it"))) return;
    if (action === "delete" && !(await confirmed(`Delete the snapshot “${n}”?`, "Delete snapshot", "Delete"))) return;
    await act(d, `snap-${action}`, () => api.emulatorSnapshot(target(d), action, n));
    if (action !== "load") await readSnapshots(d);
    if (action === "save") snapName = "";
  }

  async function startCreate() {
    creating = !creating;
    if (creating) await loadOptions();
  }

  async function loadOptions() {
    options = null;
    error = null;
    try {
      options = await api.deviceOptions(platform);
      image = options.images.find((c) => c.installed)?.id ?? "";
      model = options.models[0]?.id ?? "";
    } catch (e) {
      error = errorText(e);
    }
  }

  async function create() {
    busy = "create";
    error = null;
    try {
      await api.deviceCreate(platform, { name: name.trim(), image, model });
      creating = false;
      name = "";
      app.toast("Created.", "ok");
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
      refresh();
    }
  }

  const validName = $derived(/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(name));
  const label = (c: Choice) => `${c.label}${c.installed ? "" : " (not installed)"}`;
</script>

<div class="h-full overflow-auto p-6">
  <div class="mx-auto max-w-[900px]">
    <header class="mb-4 flex items-center gap-3">
      <div class="flex-1">
        <h1 class="text-[17px] font-semibold">Devices</h1>
        <p class="text-[12.5px] text-muted">Android emulators and iOS simulators, running or not. Start, stop, wipe, delete and snapshot them.</p>
      </div>
      <button onclick={startCreate} class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90"><Icon name="plus" size={13} /> New device…</button>
    </header>

    {#if error}
      <p class="mb-3 flex items-start gap-2 rounded-xl border border-line bg-err/8 px-4 py-2.5 text-[12.5px] text-err"><Icon name="alert" size={14} class="mt-px shrink-0" />{error}</p>
    {/if}
    {#each problems as p (p)}
      <p class="mb-2 text-[12px] text-muted">{p}</p>
    {/each}

    {#if creating}
      <section class="mb-4 rounded-2xl border border-line bg-panel p-4">
        <div class="grid grid-cols-2 gap-3">
          <label class="text-[11.5px] text-muted">Platform
            <select bind:value={platform} onchange={loadOptions} class="mt-1 block h-8 w-full rounded-lg border border-line bg-panel-2 px-2 text-[12.5px] text-fg">
              <option value="android">Android emulator</option>
              <option value="ios">iOS simulator</option>
            </select>
          </label>
          <label class="text-[11.5px] text-muted">Name
            <input bind:value={name} spellcheck="false" placeholder="Pixel_Test" class="mt-1 block h-8 w-full rounded-lg border border-line bg-panel-2 px-2.5 text-[12.5px] text-fg outline-none focus:border-accent" />
          </label>
          <label class="text-[11.5px] text-muted">{platform === "android" ? "System image" : "Runtime"}
            <select bind:value={image} class="mt-1 block h-8 w-full rounded-lg border border-line bg-panel-2 px-2 text-[12.5px] text-fg">
              {#each options?.images ?? [] as c (c.id)}<option value={c.id} disabled={!c.installed}>{label(c)}</option>{/each}
            </select>
          </label>
          <label class="text-[11.5px] text-muted">{platform === "android" ? "Device" : "Device type"}
            <select bind:value={model} class="mt-1 block h-8 w-full rounded-lg border border-line bg-panel-2 px-2 text-[12.5px] text-fg">
              {#each options?.models ?? [] as c (c.id)}<option value={c.id}>{c.label}</option>{/each}
            </select>
          </label>
        </div>
        <div class="mt-3 flex items-center gap-2">
          <button onclick={create} disabled={!validName || !image || !model || busy === "create"} class="h-8 rounded-lg bg-accent px-4 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">{busy === "create" ? "Creating…" : "Create"}</button>
          <button onclick={() => (creating = false)} class="h-8 rounded-lg border border-line px-3 text-[12px] text-muted hover:bg-hover">Cancel</button>
          {#if name && !validName}<span class="text-[11.5px] text-warn">Letters, digits, . _ and - only.</span>{/if}
        </div>
      </section>
    {/if}

    {#if !devices}
      {#each Array(3) as _, i (i)}<div class="skeleton mb-2 h-14 rounded-xl"></div>{/each}
    {:else if devices.length === 0}
      <p class="rounded-2xl border border-line bg-panel px-4 py-10 text-center text-[12.5px] text-muted">No emulators or simulators were found. Make one with New device.</p>
    {:else}
      <ul class="space-y-2">
        {#each devices as d (d.platform + d.id)}
          <li class="rounded-2xl border border-line bg-panel">
            <div class="flex items-center gap-3 px-4 py-3">
              <PlatformIcon platform={d.platform} />
              <div class="min-w-0 flex-1">
                <p class="truncate text-[13px] font-semibold">{d.name}</p>
                <p class="text-[11.5px] text-muted">{d.osVersion ? `${d.platform === "android" ? "Android" : "iOS"} ${d.osVersion}` : d.model ?? ""} · <span class={d.state === "online" ? "text-ok" : d.state === "booting" ? "text-warn" : ""}>{d.state}</span></p>
              </div>
              {#if busy?.startsWith(d.id + ":")}
                <Icon name="refresh" size={14} class="animate-spin text-muted" />
              {/if}
              {#if running(d)}
                {#if d.platform === "android" && d.state === "online"}
                  <button onclick={() => openSnapshots(d)} class="h-7 rounded-md border border-line px-2.5 text-[12px] text-muted hover:bg-hover hover:text-fg">Snapshots</button>
                {/if}
                <button onclick={() => shutdown(d)} disabled={busy != null} class="h-7 rounded-md border border-line px-2.5 text-[12px] text-muted hover:bg-hover hover:text-fg disabled:opacity-40">Shut down</button>
              {:else}
                <button onclick={() => boot(d)} disabled={busy != null} class="h-7 rounded-md bg-accent px-3 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40">Start</button>
                {#if d.platform === "android"}
                  <button onclick={() => boot(d, true)} disabled={busy != null} title="Start without a window" class="h-7 rounded-md border border-line px-2.5 text-[12px] text-muted hover:bg-hover disabled:opacity-40">Headless</button>
                {/if}
              {/if}
              <button onclick={() => wipe(d)} disabled={busy != null || (d.platform === "android" && running(d))} title={d.platform === "android" && running(d) ? "Shut it down first" : "Throw its data away"} class="h-7 rounded-md border border-line px-2.5 text-[12px] text-muted hover:bg-hover hover:text-err disabled:opacity-40">{d.platform === "ios" ? "Erase" : "Wipe"}</button>
              <button onclick={() => remove(d)} disabled={busy != null} aria-label="Delete {d.name}" title="Delete this device" class="grid size-7 place-items-center rounded-md border border-line text-muted hover:bg-hover hover:text-err disabled:opacity-40"><Icon name="trash" size={13} /></button>
            </div>
            {#if snapshotsOf === d.id}
              <div class="border-t border-line px-4 py-3">
                <form onsubmit={(e) => (e.preventDefault(), snapName && snap(d, "save", snapName))} class="mb-2 flex items-center gap-2">
                  <input bind:value={snapName} spellcheck="false" placeholder="Snapshot name" class="h-7 w-48 rounded-md border border-line bg-panel-2 px-2 text-[12px] outline-none focus:border-accent" />
                  <button type="submit" disabled={!/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(snapName) || busy != null} class="h-7 rounded-md bg-accent px-3 text-[12px] font-semibold text-white disabled:opacity-40">Save snapshot</button>
                </form>
                {#each snapshots as s (s.name)}
                  <div class="flex items-center gap-3 py-1 text-[12px]">
                    <span class="mono flex-1 truncate">{s.name}</span>
                    <span class="text-faint">{s.size} · {s.date}</span>
                    <button onclick={() => snap(d, "load", s.name)} class="text-muted hover:text-fg">Load</button>
                    <button onclick={() => snap(d, "delete", s.name)} class="text-muted hover:text-err">Delete</button>
                  </div>
                {:else}
                  <p class="text-[12px] text-muted">No snapshots yet.</p>
                {/each}
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<script lang="ts">
  import type { Appearance, Grant } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { PLACES, SERVICES, controls } from "../../stores/controls.svelte";
  import Icon from "../Icon.svelte";

  const d = controls.draft;
  const ios = $derived(app.device?.platform === "ios");
  const packageName = $derived(app.selectedApp?.id ?? null);
  const busy = $derived(controls.busy);

  // Read the device, and again when the device or the picked app changes.
  $effect(() => {
    app.deviceKey;
    app.appId;
    controls.refresh();
  });

  const button = "flex h-8 items-center justify-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-40";
  const primary = "flex h-8 items-center justify-center gap-1.5 rounded-lg bg-accent px-3.5 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40";
  const field = "h-8 w-full rounded-lg border border-line bg-panel-2 px-2.5 text-[12px] outline-none focus:border-accent";

  function setAppearance(appearance: Appearance) {
    controls.apply("appearance", { action: "appearance", appearance }, `Appearance set to ${appearance}`);
  }

  function grant(permission: string, grant: Grant) {
    if (!packageName) return;
    controls.apply(`permission:${permission}`, { action: "permission", app: packageName, permission, grant }, `${permission}: ${grant}`);
  }

  const granted = $derived(controls.info?.permissions.filter((p) => p.granted).length ?? 0);
</script>

{#snippet card(title: string, icon: string, hint: string)}
  <header class="mb-3 flex items-start gap-2.5">
    <span class="grid size-8 shrink-0 place-items-center rounded-lg bg-accent-soft text-accent"><Icon name={icon} size={15} /></span>
    <div class="min-w-0">
      <h2 class="text-[13px] font-semibold">{title}</h2>
      <p class="text-[11.5px] leading-snug text-muted">{hint}</p>
    </div>
  </header>
{/snippet}

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1180px] p-6">
    <header class="mb-5">
      <h1 class="text-[18px] font-semibold tracking-tight">Controls</h1>
      <p class="text-[12.5px] text-muted">Try your app somewhere else, in the dark, with a clean status bar or one permission less.</p>
    </header>

    {#if controls.error}
      <div class="mb-4 flex items-center gap-2 rounded-xl border border-err/30 bg-err/8 px-4 py-3 text-[12.5px] text-err"><Icon name="alert" size={14} />{controls.error}</div>
    {/if}
    {#each controls.info?.notes ?? [] as note (note)}
      <p class="mb-2 flex items-start gap-2 text-[11.5px] text-muted"><Icon name="info" size={12} class="mt-0.5 shrink-0 text-faint" />{note}</p>
    {/each}

    <div class="mt-3 grid gap-4 lg:grid-cols-2">
      {#if controls.supports("location")}
        <section class="rounded-2xl border border-line bg-panel p-4">
          {@render card("Location", "globe", ios ? "The simulator's GPS position." : "The emulator's GPS position, through its console.")}
          <div class="mb-2 flex flex-wrap gap-1.5">
            {#each PLACES as p (p.name)}
              <button onclick={() => { d.lat = p.lat; d.lon = p.lon; }} class="rounded-full border border-line px-2.5 py-0.5 text-[11.5px] text-muted hover:border-line-strong hover:text-fg">{p.name}</button>
            {/each}
          </div>
          <div class="grid grid-cols-2 gap-2">
            <label class="text-[11px] text-muted">Latitude<input class="{field} mono mt-1" bind:value={d.lat} inputmode="decimal" /></label>
            <label class="text-[11px] text-muted">Longitude<input class="{field} mono mt-1" bind:value={d.lon} inputmode="decimal" /></label>
          </div>
          <div class="mt-3 flex gap-2">
            <button class={primary} disabled={busy != null} onclick={() => controls.apply("location", { action: "location", lat: d.lat, lon: d.lon }, "Location set")}>Set location</button>
            <button class={button} disabled={busy != null} onclick={() => controls.apply("location", { action: "clearLocation" }, ios ? "Location cleared" : "Location back to the default")}>Reset</button>
          </div>
        </section>
      {/if}

      {#if controls.supports("appearance")}
        <section class="rounded-2xl border border-line bg-panel p-4">
          {@render card("Appearance", "moon", "Light or dark mode for the whole device.")}
          <div class="inline-flex gap-1 rounded-xl bg-panel-2 p-1" role="radiogroup" aria-label="Appearance">
            {#each [["light", "Light", "sun"], ["dark", "Dark", "moon"]] as [id, text, icon] (id)}
              <button
                role="radio"
                aria-checked={controls.info?.appearance === id}
                disabled={busy != null}
                onclick={() => setAppearance(id as Appearance)}
                class="flex h-8 items-center gap-1.5 rounded-lg px-4 text-[12px] font-medium disabled:opacity-50 {controls.info?.appearance === id ? 'bg-panel text-fg shadow-sm' : 'text-muted hover:text-fg'}"
              ><Icon name={icon} size={13} />{text}</button>
            {/each}
          </div>
        </section>
      {/if}

      {#if controls.supports("statusBar")}
        <section class="rounded-2xl border border-line bg-panel p-4">
          {@render card("Status bar", "smartphone", "A clean bar for screenshots: a fixed time, full battery, full signal.")}
          <div class="grid grid-cols-2 gap-3">
            <label class="text-[11px] text-muted">Time<input class="{field} mono mt-1" bind:value={d.time} placeholder="9:41" /></label>
            <label class="text-[11px] text-muted">Battery <span class="mono text-fg">{d.battery}%</span><input type="range" min="0" max="100" bind:value={d.battery} class="mt-2.5 w-full accent-[var(--accent)]" /></label>
          </div>
          <div class="mt-2 flex gap-5 text-[12px]">
            <label class="flex cursor-pointer items-center gap-2"><input type="checkbox" bind:checked={d.wifi} class="accent-[var(--accent)]" /> Wi-Fi</label>
            <label class="flex cursor-pointer items-center gap-2"><input type="checkbox" bind:checked={d.cellular} class="accent-[var(--accent)]" /> Cellular</label>
          </div>
          <div class="mt-3 flex gap-2">
            <button class={primary} disabled={busy != null} onclick={() => controls.apply("statusBar", { action: "statusBar", statusBar: { time: d.time || null, battery: d.battery, wifi: d.wifi, cellular: d.cellular } }, "Status bar overridden")}>Apply</button>
            <button class={button} disabled={busy != null} onclick={() => controls.apply("statusBar", { action: "clearStatusBar" }, "Status bar back to normal")}>Clear</button>
          </div>
        </section>
      {/if}

      {#if controls.supports("battery")}
        <section class="rounded-2xl border border-line bg-panel p-4">
          {@render card("Battery", "zap", "Pretend the battery is low or charging, to see how the app reacts.")}
          <label class="text-[11px] text-muted">Level <span class="mono text-fg">{d.level}%</span><input type="range" min="0" max="100" bind:value={d.level} class="mt-2 w-full accent-[var(--accent)]" /></label>
          <label class="mt-2 flex cursor-pointer items-center gap-2 text-[12px]"><input type="checkbox" bind:checked={d.charging} class="accent-[var(--accent)]" /> Charging</label>
          <div class="mt-3 flex gap-2">
            <button class={primary} disabled={busy != null} onclick={() => controls.apply("battery", { action: "battery", level: d.level, charging: d.charging }, "Battery overridden")}>Apply</button>
            <button class={button} disabled={busy != null} onclick={() => controls.apply("battery", { action: "resetBattery" }, "Battery back to normal")}>Reset</button>
          </div>
        </section>
      {/if}

      {#if controls.supports("permission")}
        <section class="rounded-2xl border border-line bg-panel p-4 lg:col-span-2">
          {@render card("Permissions", "shield", `What ${app.selectedApp?.name ?? "the app"} may access.`)}
          {#if ios}
            <div class="flex flex-wrap items-center gap-2">
              <select bind:value={d.service} class="{field} max-w-[220px]">
                {#each SERVICES as s (s)}<option value={s}>{s}</option>{/each}
              </select>
              {#each [["grant", "Grant"], ["revoke", "Revoke"], ["reset", "Ask again"]] as [id, text] (id)}
                <button class={id === "grant" ? primary : button} disabled={busy != null} onclick={() => grant(d.service, id as Grant)}>{text}</button>
              {/each}
            </div>
          {:else if controls.info?.permissions.length}
            <p class="mb-2 text-[11.5px] text-muted">{granted} of {controls.info.permissions.length} granted.</p>
            <ul class="grid gap-x-6 sm:grid-cols-2">
              {#each controls.info.permissions as p (p.name)}
                <li class="flex items-center gap-3 border-b border-line py-1.5 last:border-b-0">
                  <span class="mono min-w-0 flex-1 truncate text-[11.5px]" title={p.name}>{p.name.replace("android.permission.", "")}</span>
                  <button
                    role="switch"
                    aria-checked={p.granted}
                    aria-label={p.name}
                    disabled={busy != null}
                    onclick={() => grant(p.name, p.granted ? "revoke" : "grant")}
                    class="h-5 w-9 shrink-0 rounded-full p-0.5 transition disabled:opacity-50 {p.granted ? 'bg-accent' : 'bg-line-strong'}"
                  ><span class="block size-4 rounded-full bg-white shadow transition {p.granted ? 'translate-x-4' : ''}"></span></button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="text-[12px] text-muted">This app asks for no runtime permissions.</p>
          {/if}
        </section>
      {/if}
      {#if !controls.supports("permission") && controls.info}
        <section class="rounded-2xl border border-dashed border-line-strong bg-panel/50 p-4 text-[12px] text-muted lg:col-span-2">
          <Icon name="shield" size={14} class="mr-1.5 inline text-faint" />Pick an app in the toolbar to change its permissions.
        </section>
      {/if}

      {#if controls.supports("notification")}
        <section class="rounded-2xl border border-line bg-panel p-4">
          {@render card("Notification", "bell", ios ? "A simulated push to the picked app." : "A notification from the shell, shown in the shade.")}
          <div class="space-y-2">
            <input class={field} bind:value={d.title} placeholder="Title" aria-label="Title" />
            <input class={field} bind:value={d.body} placeholder="Text" aria-label="Text" />
            {#if ios}
              <textarea class="{field} mono h-16 py-1.5" bind:value={d.payload} placeholder="Optional: the whole JSON payload, such as an aps dictionary" aria-label="Payload"></textarea>
            {/if}
          </div>
          <button
            class="{primary} mt-3"
            disabled={busy != null || (ios && !packageName)}
            title={ios && !packageName ? "Pick the app that receives the push" : ""}
            onclick={() => controls.apply("notification", { action: "notification", app: packageName ?? "com.android.shell", title: d.title, body: d.body, payload: ios && d.payload.trim() ? d.payload : null }, "Notification sent")}
          >{ios ? "Send push" : "Post notification"}</button>
        </section>
      {/if}

      {#if controls.supports("openUrl")}
        <section class="rounded-2xl border border-line bg-panel p-4">
          {@render card("Open a link", "link", "A web address or one of your app's own schemes, opened on the device.")}
          <input class="{field} mono" bind:value={d.url} placeholder="https://… or myapp://…" aria-label="Link" />
          <button class="{primary} mt-3" disabled={busy != null || !d.url.trim()} onclick={() => controls.apply("openUrl", { action: "openUrl", url: d.url.trim() }, "Link opened")}>Open on the device</button>
        </section>
      {/if}
    </div>
  </div>
</div>

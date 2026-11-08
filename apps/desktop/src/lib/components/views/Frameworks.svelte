<script lang="ts">
  import { arrange } from "../../stores/arrange.svelte";
  import { untrack } from "svelte";
  import { api, errorText, type Framework } from "../../api";
  import { app } from "../../stores/app.svelte";
  import Icon from "../Icon.svelte";
  import FlutterPanel from "../frameworks/FlutterPanel.svelte";
  import ReactPanel from "../frameworks/ReactPanel.svelte";

  let found = $state<Framework[] | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let ticket = 0;

  // Look at the app's files whenever another app or device is picked. The device is polled, which makes a new
  // target object every time, so only what identifies the device and the app is watched.
  $effect(() => {
    const key = `${app.deviceKey}|${app.appId ?? ""}`;
    untrack(() => {
      const target = app.target;
      const id = app.appId;
      found = null;
      error = null;
      if (!key || !target || !id) return;
      const mine = ++ticket;
      loading = true;
      api
        .frameworksDetect(target, id)
        .then((list) => mine === ticket && (found = list))
        .catch((e) => mine === ticket && (error = errorText(e)))
        .finally(() => mine === ticket && (loading = false));
    });
  });

  const has = (id: string) => found?.some((f) => f.id === id) ?? false;
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto {arrange.pageMax} space-y-4 p-6">
    <header>
      <h1 class="text-3xl font-semibold tracking-tight">Frameworks</h1>
      <p class="text-md text-muted">
        {#if app.selectedApp}
          {app.selectedApp.name} · what it is built with, and a look inside Flutter and React Native apps
        {:else}
          Pick an app in the toolbar to see which UI frameworks it is built with.
        {/if}
      </p>
    </header>

    {#if error}
      <div class="flex items-center gap-2 rounded-xl border border-err/30 bg-err/8 px-4 py-3 text-md text-err"><Icon name="alert" size={14} />{error}</div>
    {/if}

    {#if loading}
      <div class="skeleton h-16 rounded-2xl"></div>
    {:else if found}
      <section class="rounded-2xl border border-line bg-panel p-5">
        {#if found.length}
          <div class="flex flex-wrap gap-3">
            {#each found as f (f.id)}
              <div class="min-w-[200px] rounded-xl border border-line bg-panel-2 px-4 py-3">
                <p class="text-xl font-semibold">{f.name}{#if f.version}<span class="mono ml-1.5 text-base font-normal text-muted">{f.version}</span>{/if}</p>
                <p class="mono mt-0.5 max-w-[320px] truncate text-xs text-faint" title={f.evidence}>{f.evidence}</p>
              </div>
            {/each}
          </div>
          <p class="mt-3 text-sm text-faint">Found from the files in the app. Layout marks the views each framework draws.</p>
        {:else}
          <p class="text-lg font-medium">No Flutter, React Native, Compose or SwiftUI found</p>
          <p class="mt-1 text-md text-muted">It looks like a plain native app, so the views in Layout are what it draws.</p>
        {/if}
      </section>

      {#if has("flutter")}<FlutterPanel />{/if}
      {#if has("react-native")}<ReactPanel />{/if}
      {#if has("compose") || has("swiftui")}
        <section class="rounded-2xl border border-line bg-panel p-5 text-md text-muted">
          {has("compose") ? "Jetpack Compose" : "SwiftUI"} screens show up in <b class="text-fg">Layout</b>, with the views it draws marked. Test tags and accessibility identifiers are shown as ids.
        </section>
      {/if}
    {:else if !app.selectedApp}
      <div class="grid place-items-center rounded-2xl border border-dashed border-line-strong bg-panel/50 px-6 py-16 text-center">
        <Icon name="code" size={26} class="mb-3 text-faint" />
        <p class="text-xl font-medium">No app picked</p>
        <p class="mt-1 max-w-[420px] text-md text-muted">Pick an app in the toolbar and EmuLabZip tells you what it is built with.</p>
      </div>
    {/if}
  </div>
</div>

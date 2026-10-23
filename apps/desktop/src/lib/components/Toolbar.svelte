<script lang="ts">
  import { app } from "../stores/app.svelte";
  import AppPicker from "./AppPicker.svelte";
  import DevicePicker from "./DevicePicker.svelte";
  import Icon from "./Icon.svelte";

  let { onScreenshot }: { onScreenshot: () => void } = $props();
  const isMac = $derived(app.host?.os === "macos");
</script>

<header
  data-tauri-drag-region
  class="flex h-[56px] shrink-0 items-center gap-2 border-b border-line bg-panel px-3"
  class:pl-[84px]={isMac}
>
  <DevicePicker />
  <Icon name="chevronRight" size={14} class="text-faint" />
  <AppPicker />

  <div class="mx-1 h-5 w-px bg-line"></div>

  <button
    onclick={onScreenshot}
    disabled={!app.online}
    title="Take a screenshot"
    class="flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-40 disabled:hover:bg-transparent"
  >
    <Icon name="camera" size={15} /> <span class="hidden xl:inline">Screenshot</span>
  </button>

  <div data-tauri-drag-region class="h-full flex-1"></div>

  <button
    title="Toggle theme"
    onclick={() => app.setTheme(app.theme === "dark" ? "light" : "dark")}
    class="grid size-8 place-items-center rounded-lg text-muted hover:bg-hover hover:text-fg"
  >
    <Icon name={app.theme === "dark" ? "sun" : "moon"} />
  </button>
  <button
    title="Settings (⌘,)"
    onclick={() => (app.settingsOpen = true)}
    class="grid size-8 place-items-center rounded-lg text-muted hover:bg-hover hover:text-fg"
  >
    <Icon name="settings" />
  </button>
</header>

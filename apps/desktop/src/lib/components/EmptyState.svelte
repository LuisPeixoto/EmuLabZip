<script lang="ts">
  import { app } from "../stores/app.svelte";
  import Icon from "./Icon.svelte";

  const android = $derived(app.platforms.find((p) => p.platform === "android"));
  const ios = $derived(app.platforms.find((p) => p.platform === "ios"));
  const isMac = $derived(app.host?.os === "macos");
</script>

{#snippet step(icon: string, title: string, body: string, command: string, status: { available: boolean; error: string | null } | undefined, missing: string)}
  <div class="rounded-2xl border border-line bg-panel p-5">
    <div class="mb-3 flex items-center gap-2.5">
      <span class="grid size-9 place-items-center rounded-xl bg-accent-soft text-accent"><Icon name={icon} size={18} /></span>
      <div class="flex-1">
        <h3 class="text-[13.5px] font-semibold">{title}</h3>
        <p class="flex items-center gap-1.5 text-[11.5px] text-muted">
          <span class="size-1.5 rounded-full {status?.available && !status.error ? 'bg-ok' : 'bg-faint'}"></span>
          {status?.available ? (status.error ?? "Tools found") : missing}
        </p>
      </div>
    </div>
    <p class="mb-3 text-[12.5px] leading-relaxed text-muted">{body}</p>
    <pre class="mono selectable overflow-x-auto rounded-lg border border-line bg-panel-2 px-3 py-2 text-fg">{command}</pre>
  </div>
{/snippet}

<div class="grid h-full place-items-center overflow-y-auto p-10">
  <div class="w-full max-w-[760px]">
    <div class="mb-8 flex flex-col items-center text-center">
      <span class="brand-gradient mb-5 grid size-16 place-items-center rounded-[20px] text-white shadow-[0_18px_40px_-14px_#5b5bf0]">
        <Icon name="activity" size={30} stroke={2.2} />
      </span>
      <h1 class="text-[22px] font-semibold tracking-tight">{app.devicesLoaded ? "Start a device to begin" : "Looking for devices…"}</h1>
      <p class="mt-2 max-w-[520px] text-[13px] leading-relaxed text-muted">
        EmuLabZip connects to running Android emulators and iOS simulators automatically. Start one and it appears in the
        device picker within a few seconds.
      </p>
    </div>
    <div class="grid gap-4 md:grid-cols-2">
      {@render step(
        "android",
        "Android emulator",
        "Start an AVD from Android Studio's Device Manager, or from a terminal. USB devices with debugging enabled work too.",
        "emulator -list-avds\nemulator -avd Pixel_9",
        android,
        "adb was not found. Install the Android SDK platform tools.",
      )}
      {@render step(
        "apple",
        "iOS Simulator",
        isMac ? "Open Simulator from Xcode, or boot one from a terminal." : "The iOS Simulator needs a Mac with Xcode installed.",
        'xcrun simctl list devices available\nxcrun simctl boot "iPhone 17"\nopen -a Simulator',
        ios,
        isMac ? "xcrun was not found. Install Xcode." : "Only available on macOS.",
      )}
    </div>
  </div>
</div>

<script lang="ts">
  import { api, errorText, type Audit, type SecurityAudit, type Severity, type Simulated } from "../../api";
  import { app } from "../../stores/app.svelte";
  import Icon from "../Icon.svelte";

  type Tab = "security" | "text" | "colour";
  let tab = $state<Tab>("security");
  let busy = $state<Tab | null>(null);
  let error = $state<string | null>(null);

  let security = $state<SecurityAudit | null>(null);
  let text = $state<Audit | null>(null);
  let rtl = $state(false);
  let vision = $state("deuteranopia");
  let simulated = $state<Simulated | null>(null);

  let owner = "";
  $effect(() => {
    const key = `${app.deviceKey}|${app.appId ?? ""}`;
    if (key !== owner) {
      owner = key;
      security = text = simulated = null;
      error = null;
    }
  });

  async function run<T>(which: Tab, work: () => Promise<T>, done: (r: T) => void) {
    busy = which;
    error = null;
    try {
      done(await work());
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  const audit = () => app.target && app.appId && run("security", () => api.auditSecurity(app.target!, app.appId!), (r) => (security = r));
  const check = () => app.target && run("text", () => api.textCheck(app.target!, rtl), (r) => (text = r));
  const simulate = () => app.target && run("colour", () => api.colourSimulate(app.target!, vision), (r) => (simulated = r));

  const tone: Record<Severity, string> = { error: "bg-err/12 text-err", warning: "bg-warn/12 text-warn", info: "bg-hover text-muted" };
  const primary = "flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3.5 text-base font-semibold text-white hover:opacity-90 disabled:opacity-40";
</script>

{#snippet badge(s: Severity)}
  <span class="rounded-md px-1.5 py-px text-2xs font-medium {tone[s]}">{s}</span>
{/snippet}

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1000px] p-6">
    <header class="mb-4">
      <h1 class="text-3xl font-semibold tracking-tight">Audit</h1>
      <p class="text-md text-muted">Problems that are not about speed: secrets left lying around, text that does not fit, colours that stop working.</p>
    </header>
    <div class="mb-4 flex rounded-lg border border-line bg-panel-2 p-0.5 w-fit" role="tablist">
      {#each [["security", "Security and privacy"], ["text", "Text that does not fit"], ["colour", "Colour vision"]] as [id, label] (id)}
        <button role="tab" aria-selected={tab === id} onclick={() => (tab = id as Tab)} class="h-8 rounded-md px-3 text-base {tab === id ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}">{label}</button>
      {/each}
    </div>
    {#if error}
      <p class="mb-3 flex items-start gap-2 rounded-xl border border-line bg-err/8 px-4 py-2.5 text-md text-err"><Icon name="alert" size={14} class="mt-px shrink-0" />{error}</p>
    {/if}

    {#if tab === "security"}
      <div class="mb-3 flex items-center gap-3">
        <button class={primary} disabled={busy != null || !app.appId} onclick={audit}>
          <Icon name={busy === "security" ? "refresh" : "shield"} size={13} class={busy === "security" ? "animate-spin" : ""} />{security ? "Audit again" : "Audit the app"}
        </button>
        {#if !app.appId}<span class="text-base text-muted">Pick an app in the toolbar first.</span>{/if}
      </div>
      {#if security}
        {#each security.findings as f, i (i)}
          <article class="mb-2 rounded-xl border border-line bg-panel px-4 py-3">
            <p class="flex items-center gap-2 text-md">{@render badge(f.severity)}<span class="mono font-medium">{f.rule}</span><span class="text-faint">{f.area}</span></p>
            <p class="mt-1 text-md">{f.message}</p>
            {#if f.location || f.evidence}
              <p class="mono mt-1 text-xs break-all text-muted">{[f.location, f.evidence && `saw: ${f.evidence}`].filter(Boolean).join(" · ")}</p>
            {/if}
            <p class="mt-1 text-sm text-faint">{f.fix}</p>
          </article>
        {:else}
          <p class="rounded-xl border border-line bg-panel px-4 py-8 text-center text-md text-muted">Nothing found in what was looked at.</p>
        {/each}
        <p class="mt-2 text-sm text-faint">Looked at: {security.checked.join(", ") || "nothing"}. Start the proxy on the Network view first to include requests; secrets are cut down to their first and last characters.</p>
        {#each security.skipped as [area, why] (area)}<p class="text-sm text-faint">Could not read the {area}: {why}</p>{/each}
      {:else if !busy}
        <p class="rounded-xl border border-line bg-panel px-4 py-8 text-center text-md text-muted">Reads what the app declares (debuggable, allowBackup, exposed components, App Transport Security), and looks for secrets and personal data in the log, preferences and databases, and for plain HTTP in recorded requests.</p>
      {/if}
    {:else if tab === "text"}
      <div class="mb-3 flex flex-wrap items-center gap-3">
        <button class={primary} disabled={busy != null || !app.target} onclick={check}>
          <Icon name={busy === "text" ? "refresh" : "search"} size={13} class={busy === "text" ? "animate-spin" : ""} />Check this screen
        </button>
        <label class="flex items-center gap-1.5 text-base"><input type="checkbox" bind:checked={rtl} class="accent-[var(--accent)]" /> Right-to-left language</label>
        <span class="text-sm text-muted">Change the language and text size in Controls first, then check.</span>
      </div>
      {#if text}
        {#each text.findings as f, i (i)}
          <article class="mb-2 rounded-xl border border-line bg-panel px-4 py-3">
            <p class="flex items-center gap-2 text-md">{@render badge(f.severity)}<span class="mono font-medium">{f.rule}</span><span class="text-faint">{f.class.split(".").pop()}</span></p>
            <p class="mt-1 text-md">{f.message}</p>
            <p class="mt-1 text-sm text-faint">{f.fix}</p>
          </article>
        {:else}
          <p class="rounded-xl border border-line bg-panel px-4 py-8 text-center text-md text-muted">No text that looks cut off on this screen ({text.checked} views checked).</p>
        {/each}
        {#if text.contrastSkipped}<p class="text-sm text-faint">No screenshot came with the layout, so cut-off text was not looked for.</p>{/if}
      {/if}
    {:else}
      <div class="mb-3 flex flex-wrap items-center gap-3">
        <select bind:value={vision} class="h-8 rounded-lg border border-line bg-panel-2 px-2 text-base">
          {#each ["protanopia", "deuteranopia", "tritanopia", "achromatopsia"] as v (v)}<option>{v}</option>{/each}
        </select>
        <button class={primary} disabled={busy != null || !app.target} onclick={simulate}>
          <Icon name={busy === "colour" ? "refresh" : "eye"} size={13} class={busy === "colour" ? "animate-spin" : ""} />Show the screen like this
        </button>
      </div>
      {#if simulated}
        <div class="grid gap-4 md:grid-cols-[320px_1fr]">
          <img src="data:image/png;base64,{simulated.png}" alt="The screen as seen with {vision}" class="max-h-[560px] rounded-xl border border-line" />
          <div>
            {#each simulated.audit.findings as f, i (i)}
              <article class="mb-2 rounded-xl border border-line bg-panel px-4 py-3">
                <p class="text-md">{f.message}</p>
                <p class="mt-1 text-sm text-faint">{f.fix}</p>
              </article>
            {:else}
              <p class="rounded-xl border border-line bg-panel px-4 py-6 text-center text-md text-muted">Every text that is readable normally is still readable with {vision}.</p>
            {/each}
          </div>
        </div>
      {/if}
    {/if}
  </div>
</div>

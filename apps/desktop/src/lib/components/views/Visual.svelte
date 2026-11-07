<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type VisualResult } from "../../api";
  import { app } from "../../stores/app.svelte";
  import { confirmed } from "../../confirm";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";

  let baselines = $state<[string, number][]>([]);
  let name = $state("");
  let selected = $state<string | null>(null);
  let cleanBar = $state(true);
  let threshold = $state(0.1);
  let result = $state<VisualResult | null>(null);
  let shown = $state<"diff" | "baseline" | "current">("diff");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const validName = $derived(/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(name));

  async function refresh() {
    try {
      baselines = await api.visualList();
    } catch (e) {
      error = errorText(e);
    }
  }
  onMount(refresh);

  async function work(job: () => Promise<void>) {
    busy = true;
    error = null;
    try {
      await job();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
      refresh();
    }
  }

  const capture = () =>
    app.target &&
    work(async () => {
      await api.visualCapture(app.target!, name, cleanBar, false);
      selected = name;
      name = "";
      app.toast("Baseline saved.", "ok");
    });

  const compare = (n: string) =>
    app.target &&
    work(async () => {
      selected = n;
      result = await api.visualCompare(app.target!, n, cleanBar, threshold);
      shown = result.comparison.changed > 0 ? "diff" : "current";
    });

  async function update(n: string) {
    if (!app.target) return;
    if (!(await confirmed(`Replace the baseline “${n}” with the screen as it is now?\n\nThe old picture is overwritten.`, "Update baseline", "Replace it"))) return;
    work(async () => {
      await api.visualCapture(app.target!, n, cleanBar, true);
      result = null;
      app.toast("Baseline updated.", "ok");
    });
  }

  const src = $derived(result ? `data:image/png;base64,${shown === "diff" ? result.diff : shown === "baseline" ? result.baseline : result.current}` : "");
  const btn = "flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-40";
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1100px] p-6">
    <header class="mb-4">
      <h1 class="text-[18px] font-semibold tracking-tight">Visual</h1>
      <p class="text-[12.5px] text-muted">Keep a picture of how a screen should look, and see what changed when it does not.</p>
    </header>
    {#if error}
      <p class="mb-3 flex items-start gap-2 rounded-xl border border-line bg-err/8 px-4 py-2.5 text-[12.5px] text-err"><Icon name="alert" size={14} class="mt-px shrink-0" />{error}</p>
    {/if}
    <div class="mb-4 flex flex-wrap items-center gap-3">
      <input bind:value={name} spellcheck="false" placeholder="Baseline name" class="h-8 w-52 rounded-lg border border-line bg-panel-2 px-2.5 text-[12px] outline-none focus:border-accent" />
      <button class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3.5 text-[12px] font-semibold text-white hover:opacity-90 disabled:opacity-40" disabled={busy || !validName || !app.online} onclick={capture}><Icon name="camera" size={13} />Save the screen as a baseline</button>
      <label class="flex items-center gap-1.5 text-[12px]"><input type="checkbox" bind:checked={cleanBar} class="accent-[var(--accent)]" /> Clean status bar</label>
      <label class="flex items-center gap-1.5 text-[12px] text-muted">Limit <input type="number" min="0" max="100" step="0.05" bind:value={threshold} class="mono h-8 w-20 rounded-lg border border-line bg-panel-2 px-2 text-[12px] text-fg" />%</label>
    </div>
    <div class="grid gap-5 lg:grid-cols-[260px_1fr]">
      <section class="rounded-2xl border border-line bg-panel p-2">
        {#each baselines as [n, size] (n)}
          <div class="flex items-center gap-2 rounded-lg px-2.5 py-1.5 {selected === n ? 'bg-accent-soft' : ''}">
            <span class="min-w-0 flex-1 truncate text-[12.5px] font-medium">{n}</span>
            <span class="mono text-[10.5px] text-faint">{bytes(size)}</span>
            <button class="text-[11.5px] text-accent hover:underline disabled:opacity-40" disabled={busy || !app.online} onclick={() => compare(n)}>Compare</button>
            <button class="text-[11.5px] text-muted hover:text-err disabled:opacity-40" disabled={busy || !app.online} onclick={() => update(n)}>Update</button>
          </div>
        {:else}
          <p class="p-4 text-center text-[12px] text-muted">No baselines yet. Open the screen you want to keep, name it and save it.</p>
        {/each}
      </section>
      <section>
        {#if result}
          <div class="mb-3 flex flex-wrap items-center gap-3">
            <p class="text-[13px] font-semibold {result.exceeds ? 'text-err' : 'text-ok'}">
              {result.comparison.changed === 0 ? "Identical" : result.exceeds ? "Different" : "Within the limit"}
            </p>
            <p class="mono text-[11.5px] text-muted">{result.comparison.percent.toFixed(3)}% changed ({result.comparison.changed.toLocaleString()} of {result.comparison.compared.toLocaleString()} pixels){result.comparison.bounds ? `, in ${result.comparison.bounds.width}×${result.comparison.bounds.height} at ${result.comparison.bounds.x},${result.comparison.bounds.y}` : ""}</p>
            <span class="flex-1"></span>
            <div class="flex rounded-lg border border-line bg-panel-2 p-0.5" role="tablist">
              {#each [["diff", "Difference"], ["baseline", "Baseline"], ["current", "Now"]] as [id, label] (id)}
                <button role="tab" aria-selected={shown === id} onclick={() => (shown = id as typeof shown)} class="h-7 rounded-md px-2.5 text-[12px] {shown === id ? 'bg-panel font-medium text-fg shadow-sm' : 'text-muted'}">{label}</button>
              {/each}
            </div>
          </div>
          <img {src} alt="The {shown} screenshot" class="max-h-[640px] rounded-xl border border-line" />
        {:else}
          <p class="rounded-2xl border border-line bg-panel px-4 py-12 text-center text-[12.5px] text-muted">Pick a baseline and press Compare to see what changed on the screen in front of you.</p>
        {/if}
      </section>
    </div>
  </div>
</div>

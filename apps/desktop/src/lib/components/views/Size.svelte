<script lang="ts">
  import { api, errorText, type PackageDiff, type PackageView } from "../../api";
  import { bytes } from "../../format";
  import Icon from "../Icon.svelte";

  let file = $state<string | null>(null);
  let pkg = $state<PackageView | null>(null);
  let other = $state<string | null>(null);
  let diff = $state<PackageDiff | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const signed = (n: number) => `${n > 0 ? "+" : n < 0 ? "−" : ""}${bytes(Math.abs(n))}`;
  const share = (n: number) => (pkg ? Math.max(1, (n * 100) / Math.max(1, pkg.total.compressed)) : 0);

  async function open() {
    error = null;
    try {
      const path = await api.packagePick();
      if (!path) return;
      busy = true;
      file = path;
      other = null;
      diff = null;
      pkg = await api.packageAnalyze(path);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function compare() {
    if (!file) return;
    error = null;
    try {
      const path = await api.packagePick();
      if (!path) return;
      busy = true;
      other = path;
      diff = await api.packageDiff(file, path);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  const btn = "flex h-8 items-center gap-1.5 rounded-lg border border-line px-3 text-[12px] font-medium text-muted hover:bg-hover hover:text-fg disabled:opacity-40";
</script>

<div class="h-full overflow-y-auto">
  <div class="mx-auto max-w-[1000px] p-6">
    <header class="mb-4 flex items-center gap-3">
      <div class="flex-1">
        <h1 class="text-[18px] font-semibold tracking-tight">Build size</h1>
        <p class="text-[12.5px] text-muted">Where the bytes of an APK, an AAB or an IPA are, and what changed between two builds. Reads the file; no device needed.</p>
      </div>
      <button class={btn} onclick={open} disabled={busy}><Icon name="folderOpen" size={13} />Open a build…</button>
      {#if pkg}<button class={btn} onclick={compare} disabled={busy}><Icon name="columns" size={13} />Compare with…</button>{/if}
    </header>
    {#if error}
      <p class="mb-3 flex items-start gap-2 rounded-xl border border-line bg-err/8 px-4 py-2.5 text-[12.5px] text-err"><Icon name="alert" size={14} class="mt-px shrink-0" />{error}</p>
    {/if}

    {#if diff && file && other}
      <section class="mb-5 rounded-2xl border border-line bg-panel p-4">
        <p class="text-[12px] text-muted">{name(file)} → {name(other)}</p>
        <p class="mono mt-1 text-[16px] font-semibold {diff.delta > 0 ? 'text-warn' : diff.delta < 0 ? 'text-ok' : ''}">{signed(diff.delta)} <span class="text-[12px] font-normal text-muted">({diff.percent.toFixed(1)}%, {bytes(diff.before)} → {bytes(diff.after)})</span></p>
        {#if diff.methodsBefore !== diff.methodsAfter}<p class="mono text-[11.5px] text-muted">methods {diff.methodsBefore.toLocaleString()} → {diff.methodsAfter.toLocaleString()}</p>{/if}
        <div class="mt-3 grid gap-4 md:grid-cols-2">
          <div>
            <p class="mb-1 text-[11px] font-medium text-muted">By kind</p>
            {#each diff.kinds as [k, d] (k)}
              <p class="mono flex justify-between text-[11.5px]"><span>{k}</span><span class={d > 0 ? "text-warn" : "text-ok"}>{signed(d)}</span></p>
            {/each}
            {#if diff.folders.length}
              <p class="mt-3 mb-1 text-[11px] font-medium text-muted">By folder</p>
              {#each diff.folders.slice(0, 6) as [f, d] (f)}
                <p class="mono flex justify-between text-[11.5px]"><span>{f}/</span><span class={d > 0 ? "text-warn" : "text-ok"}>{signed(d)}</span></p>
              {/each}
            {/if}
          </div>
          <div>
            <p class="mb-1 text-[11px] font-medium text-muted">Biggest changes ({diff.added} added, {diff.removed} removed)</p>
            {#each diff.files.slice(0, 12) as f (f.path)}
              <p class="mono flex justify-between gap-3 text-[11.5px]"><span class="truncate" title={f.path}>{f.path}</span><span class="shrink-0 {f.delta > 0 ? 'text-warn' : 'text-ok'}">{signed(f.delta)}</span></p>
            {/each}
          </div>
        </div>
      </section>
    {/if}

    {#if pkg && file}
      <section class="mb-5 rounded-2xl border border-line bg-panel p-4">
        <p class="text-[13px] font-semibold">{name(file)} <span class="rounded-md bg-hover px-1.5 py-px text-[10.5px] font-medium text-muted uppercase">{pkg.format}</span></p>
        <p class="mono mt-1 text-[12px] text-muted">{bytes(pkg.total.compressed)} download · {bytes(pkg.total.uncompressed)} installed · {pkg.total.files} files{pkg.methods ? ` · ${pkg.methods.toLocaleString()} methods` : ""}</p>
        <div class="mt-4 space-y-2">
          {#each Object.entries(pkg.kinds).sort((a, b) => b[1].compressed - a[1].compressed) as [kind, s] (kind)}
            <div>
              <div class="flex justify-between text-[11.5px]"><span class="font-medium">{kind}</span><span class="mono text-muted">{bytes(s.compressed)} · {s.files} files</span></div>
              <div class="mt-0.5 h-2 rounded-full bg-hover"><div class="h-2 rounded-full bg-accent" style="width:{share(s.compressed)}%"></div></div>
            </div>
          {/each}
        </div>
      </section>
      <div class="grid gap-5 md:grid-cols-2">
        {#if Object.keys(pkg.native).length}
          <section class="rounded-2xl border border-line bg-panel p-4">
            <p class="mb-2 text-[12.5px] font-semibold">Native code</p>
            {#each Object.entries(pkg.native) as [where, s] (where)}
              <p class="mono flex justify-between text-[11.5px]"><span>{where}</span><span class="text-muted">{bytes(s.compressed)}</span></p>
            {/each}
          </section>
        {/if}
        {#if pkg.dex.length}
          <section class="rounded-2xl border border-line bg-panel p-4">
            <p class="mb-2 text-[12.5px] font-semibold">Dex files</p>
            {#each pkg.dex as d (d.path)}
              <p class="mono flex justify-between gap-3 text-[11.5px]"><span class="truncate">{d.path}</span><span class="shrink-0 text-muted">{d.methods.toLocaleString()} methods · {bytes(d.bytes)}</span></p>
            {/each}
          </section>
        {/if}
      </div>
      <section class="mt-5 rounded-2xl border border-line bg-panel p-4">
        <p class="mb-2 text-[12.5px] font-semibold">Biggest files</p>
        {#each pkg.largest.slice(0, 15) as e (e.path)}
          <p class="mono flex justify-between gap-3 text-[11.5px]"><span class="truncate" title={e.path}>{e.path}</span><span class="shrink-0 text-muted">{e.kind} · {bytes(e.compressed)}</span></p>
        {/each}
      </section>
    {:else if !busy}
      <p class="rounded-2xl border border-line bg-panel px-4 py-12 text-center text-[12.5px] text-muted">Open an .apk, .aab or .ipa to see where its size goes. The same analysis is `emulabzip size`, with a budget for CI.</p>
    {:else}
      <div class="skeleton h-24 rounded-2xl"></div>
    {/if}
  </div>
</div>

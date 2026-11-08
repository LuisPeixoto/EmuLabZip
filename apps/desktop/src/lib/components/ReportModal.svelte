<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { api, errorText } from "../api";
  import { load, save } from "../prefs";
  import { app } from "../stores/app.svelte";
  import Icon from "./Icon.svelte";

  const session = app.report?.session ?? null;
  const close = () => (app.report = null);

  // The tracker is remembered; the words of the report are not.
  const prefs = load("report", { issue: "", screenshot: true, logs: true, requests: true, crashes: true, video: true });
  let title = $state("");
  let description = $state("");
  let issue = $state(prefs.issue);
  let screenshot = $state(prefs.screenshot);
  let logs = $state(prefs.logs);
  let requests = $state(prefs.requests);
  let crashes = $state(prefs.crashes);
  let video = $state(prefs.video ?? true);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let notes = $state<string[]>([]);
  let savedTo = $state<string | null>(null);

  const trackers = [
    ["GitHub", "github:owner/repo"],
    ["GitLab", "gitlab:group/project"],
    ["Jira", "jira:site.atlassian.net/10000/10001"],
  ];

  async function make() {
    busy = true;
    error = null;
    savedTo = null;
    save("report", { issue, screenshot, logs, requests, crashes, video });
    try {
      const saved = await api.reportSave(session ? null : app.target, {
        title, description, screenshot, logs, requests, crashes, video: session != null && video, issue, session, app: app.selectedApp?.id ?? null,
      });
      if (saved) {
        savedTo = saved.path;
        notes = saved.notes;
        app.toast(saved.issueUrl ? "Saved. The issue is open in your browser: attach the file to it." : `Saved to ${saved.path}`, "ok");
      }
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  const field = "w-full rounded-lg border border-line bg-panel-2 px-2.5 py-1.5 text-md outline-none focus:border-accent";
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div transition:fade={{ duration: 120 }} class="fixed inset-0 z-50 grid place-items-center bg-black/45 p-8 backdrop-blur-[2px]">
  <button class="absolute inset-0 cursor-default" aria-label="Close" onclick={close}></button>
  <div transition:scale={{ start: 0.97, duration: 160 }} class="shadow-pop relative flex max-h-full w-[520px] flex-col overflow-hidden rounded-2xl border border-line bg-elevated" role="dialog" aria-label="Report a bug">
    <div class="flex items-center gap-2 border-b border-line px-4 py-3">
      <Icon name="bug" size={15} class="text-accent" />
      <h2 class="flex-1 text-lg font-semibold">Report a bug{session ? " from this session" : app.selectedApp ? ` · ${app.selectedApp.name}` : ""}</h2>
      <button onclick={close} class="grid size-7 place-items-center rounded-lg text-faint hover:bg-hover hover:text-fg" aria-label="Close"><Icon name="x" size={14} /></button>
    </div>

    <div class="space-y-3.5 overflow-y-auto p-4">
      <label class="block text-sm font-medium text-muted">Title
        <input bind:value={title} placeholder="Left empty, the crash or the app names it" class="{field} mt-1" />
      </label>
      <label class="block text-sm font-medium text-muted">What happened
        <textarea bind:value={description} rows="3" placeholder="Steps to reproduce, what you expected" class="{field} mt-1 resize-y"></textarea>
      </label>

      <fieldset>
        <legend class="mb-1 text-sm font-medium text-muted">Include</legend>
        <div class="grid grid-cols-2 gap-1.5 text-base">
          <label class="flex cursor-pointer items-center gap-2"><input type="checkbox" bind:checked={screenshot} class="accent-[var(--accent)]" />Screenshot</label>
          <label class="flex cursor-pointer items-center gap-2"><input type="checkbox" bind:checked={logs} class="accent-[var(--accent)]" />Warnings and errors of the log</label>
          <label class="flex cursor-pointer items-center gap-2"><input type="checkbox" bind:checked={requests} class="accent-[var(--accent)]" />Failed requests</label>
          <label class="flex cursor-pointer items-center gap-2"><input type="checkbox" bind:checked={crashes} class="accent-[var(--accent)]" />Crashes, made readable</label>
          {#if session}
            <label class="flex cursor-pointer items-center gap-2" title="The piece of the screen recording that holds the crash, and the last minute on a strip"><input type="checkbox" bind:checked={video} class="accent-[var(--accent)]" />Screen recording</label>
          {/if}
        </div>
      </fieldset>

      <label class="block text-sm font-medium text-muted">Open an issue in <span class="font-normal text-faint">(optional, a pre-filled link: nothing is sent)</span>
        <input bind:value={issue} list="trackers" placeholder="github:owner/repo" spellcheck="false" class="{field} mono mt-1" />
        <datalist id="trackers">{#each trackers as [, example] (example)}<option value={example}></option>{/each}</datalist>
      </label>

      {#if !session && !app.selectedApp}
        <p class="text-sm text-muted">No app is picked, so the log and the crashes are the device's as a whole.</p>
      {/if}
      {#if !session}
        <p class="text-sm text-muted">Failed requests come from the Network view: start it and connect the device to include them.</p>
      {/if}
      {#if error}<p class="flex items-start gap-2 text-base text-err"><Icon name="alert" size={13} class="mt-0.5 shrink-0" /><span class="min-w-0 break-words">{error}</span></p>{/if}
      {#if savedTo}
        <p class="text-base text-ok">Saved to <span class="mono break-all select-text">{savedTo}</span></p>
        {#each notes as n (n)}<p class="text-sm text-muted">{n}</p>{/each}
      {/if}
    </div>

    <div class="flex items-center gap-2 border-t border-line px-4 py-3">
      <span class="flex-1 text-sm text-muted">One HTML file (a long video goes next to it). You choose where.</span>
      <button onclick={close} class="h-8 rounded-lg border border-line px-3 text-base font-medium text-muted hover:bg-hover hover:text-fg">Close</button>
      <button onclick={make} disabled={busy} class="flex h-8 items-center gap-1.5 rounded-lg bg-accent px-3 text-base font-semibold text-white hover:opacity-90 disabled:opacity-50">
        <Icon name={busy ? "refresh" : "download"} size={13} class={busy ? "animate-spin" : ""} />{issue.trim() ? "Save and open issue…" : "Save HTML…"}
      </button>
    </div>
  </div>
</div>

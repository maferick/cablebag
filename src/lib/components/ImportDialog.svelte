<script lang="ts">
  // Importing sessions from other programs and files. Every source is listed, also when nothing of it is
  // found on this PC, so it is clear what can be imported.
  import Modal from "./Modal.svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { api, type ImportReport, type ImportSource } from "../api";
  import { app } from "../state.svelte";
  import { kindInfo } from "../kinds";
  import { FolderOpen, Check } from "@lucide/svelte";

  let sources = $state<ImportSource[]>([]);
  let chosen = $state("");
  let files = $state<Record<string, string[]>>({});
  let busy = $state(false);
  let report = $state<ImportReport | null>(null);
  let error = $state("");

  $effect(() => {
    api.importSources().then((s) => {
      sources = s;
      chosen ||= s.find((x) => (x.found ?? 0) > 0)?.id ?? s[0]?.id ?? "";
    });
  });

  const source = $derived(sources.find((s) => s.id === chosen));
  const ready = $derived(!!source && ((files[source.id]?.length ?? 0) > 0 || (source.found ?? 0) > 0));

  async function browse(s: ImportSource) {
    const many = s.id === "rdp";
    const f = await openDialog({
      title: many ? "Choose Remote Desktop files" : `Choose a ${s.name} file`,
      multiple: many,
      filters: [{ name: s.name, extensions: s.extensions }],
    });
    if (!f) return;
    files[s.id] = Array.isArray(f) ? f : [f];
    chosen = s.id;
  }

  async function run() {
    if (!source) return;
    busy = true;
    error = "";
    try {
      report = await api.importSessions(source.id, files[source.id] ?? []);
      await app.reloadLibrary();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  const name = (p: string) => p.split(/[\\/]/).pop();
</script>

<Modal title="Import sessions" width={620} onclose={() => (app.importOpen = false)}>
  {#if !report}
    <p class="muted lead">Each source gets its own folder. Importing again updates those sessions instead of adding copies; the other program is not changed.</p>
    <div class="list" role="radiogroup" aria-label="Import from">
      {#each sources as s (s.id)}
        {@const none = s.found === 0}
        <div class="src" class:on={chosen === s.id} class:none role="radio" aria-checked={chosen === s.id} tabindex="0" onclick={() => (chosen = s.id)} onkeydown={(e) => e.key === "Enter" && (chosen = s.id)}>
          <span class="tick">{#if chosen === s.id}<Check size={14} />{/if}</span>
          <span class="txt">
            <span class="nm">{s.name}</span>
            <span class="muted small">{s.description}</span>
            {#if files[s.id]?.length}
              <span class="small files">{files[s.id].map(name).join(", ")}</span>
            {/if}
          </span>
          {#if s.found !== null}
            <span class="count" class:zero={none}>{none ? "none on this PC" : `${s.found} found`}</span>
          {/if}
          {#if s.extensions.length}
            <button class="btn" onclick={(e) => { e.stopPropagation(); browse(s); }}><FolderOpen size={14} /> Choose</button>
          {/if}
        </div>
      {/each}
    </div>
    {#if error}<p class="error">{error}</p>{/if}
  {:else}
    <p>
      Imported <strong>{report.sessions}</strong> new sessions{report.updated ? ` and updated ${report.updated}` : ""}
      in <strong>{report.folders}</strong> folders, from {report.file}.
    </p>
    <ul>
      {#each Object.entries(report.by_kind) as [kind, n]}
        {@const k = kindInfo(kind)}
        <li><k.icon size={14} /> {k.label}: {n}</li>
      {/each}
    </ul>
    <p class="muted small">Passwords are not imported; Cablebag finds them in the vault or asks once.</p>
  {/if}
  {#snippet footer()}
    {#if !report}
      <button class="btn" onclick={() => (app.importOpen = false)}>Cancel</button>
      <button class="btn primary" disabled={!ready || busy} onclick={run}>{busy ? "Importing..." : `Import from ${source?.name ?? ""}`}</button>
    {:else}
      <button class="btn primary" onclick={() => (app.importOpen = false)}>Done</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .lead {
    margin: 0 0 12px;
    line-height: 1.5;
  }
  .list {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .src {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px 8px 8px;
    border-bottom: 1px solid var(--border);
    cursor: pointer;
  }
  .src:last-child {
    border-bottom: none;
  }
  .src:hover {
    background: var(--hover);
  }
  .src.on {
    background: var(--selected);
  }
  .src.none .nm {
    color: var(--muted);
  }
  .tick {
    width: 16px;
    display: inline-flex;
    color: var(--accent);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nm {
    font-weight: 600;
  }
  .files {
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: 12px;
    color: var(--ok-text);
    white-space: nowrap;
  }
  .count.zero {
    color: var(--faint);
  }
  .small {
    font-size: 12px;
  }
  .error {
    color: var(--danger-text);
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 8px 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 0;
  }
  p {
    line-height: 1.5;
  }
</style>

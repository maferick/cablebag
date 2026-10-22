<script lang="ts">
  // Run on many servers: one command on every SSH session of a folder, a few at a time, with the results side by
  // side (uptime, pending updates, disk space, the state of a service).
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { app } from "../../state.svelte";
  import type { Session } from "../../api";
  import { writeText } from "../../clipboard";
  import { ListChecks, Play, Square, Copy, Download, ChevronRight } from "@lucide/svelte";
  import "./tool.css";

  interface Row {
    session: Session;
    on: boolean;
    state: "waiting" | "running" | "done" | "failed";
    exit?: number | null;
    output?: string;
    ms?: number;
  }

  const EXAMPLES = [
    { label: "Uptime and load", command: "uptime" },
    { label: "Disk space", command: "df -h -x tmpfs -x devtmpfs -x overlay" },
    { label: "Pending updates (Debian, Ubuntu)", command: "apt list --upgradable 2>/dev/null | tail -n +2 | wc -l" },
    { label: "Operating system", command: ". /etc/os-release && echo $PRETTY_NAME" },
    { label: "Failed services", command: "systemctl --failed --no-legend --no-pager" },
  ];

  let folderId = $state<string>(app.runManyFolder ?? "");
  let command = $state("uptime");
  let root = $state(false);
  let parallel = $state(6);
  let rows = $state<Row[]>([]);
  let running = $state(false);
  let stopped = false;
  let open = $state<Record<string, boolean>>({});

  const folders = $derived(app.folders.map((f) => ({ id: f.id, path: app.folderPath(f.id) })).sort((a, b) => a.path.localeCompare(b.path)));

  // The SSH sessions of the folder and its subfolders, with the folder's defaults applied.
  $effect(() => {
    const list = app.sessions
      .filter((s) => s.kind === "ssh" && (folderId === "" ? true : app.inFolder(s.folder_id, folderId)))
      .sort((a, b) => a.name.localeCompare(b.name));
    // The library can change during a run (a vault login gets linked): rows keep their results.
    const old = new Map(untrack(() => rows).map((r) => [r.session.id, r]));
    rows = list.map((s) => {
      const r = old.get(s.id);
      return r ? { ...r, session: s } : { session: s, on: true, state: "waiting" as const };
    });
  });

  const chosen = $derived(rows.filter((r) => r.on));
  const done = $derived(rows.filter((r) => r.state === "done" || r.state === "failed").length);

  async function run() {
    if (!command.trim() || !chosen.length) return;
    running = true;
    stopped = false;
    for (const r of rows) {
      r.state = "waiting";
      r.output = undefined;
      r.exit = undefined;
    }
    const queue = [...chosen];
    const worker = async () => {
      for (let r = queue.shift(); r && !stopped; r = queue.shift()) {
        r.state = "running";
        const t0 = Date.now();
        try {
          const res = await invoke<{ output: string; exit: number | null; timed_out: boolean }>("run_once", {
            session: app.withFolderDefaults($state.snapshot(r.session) as Session),
            command,
            root,
            timeoutSecs: 60,
          });
          r.output = res.output + (res.timed_out ? "\n(stopped after 60 seconds)" : "");
          r.exit = res.exit;
          r.state = res.exit === 0 ? "done" : "failed";
        } catch (e) {
          r.output = String(e);
          r.state = "failed";
        }
        r.ms = Date.now() - t0;
      }
    };
    await Promise.all(Array.from({ length: Math.max(1, Math.min(20, parallel)) }, worker));
    running = false;
  }

  const firstLine = (s = "") => s.trim().split("\n")[0] ?? "";

  function asText() {
    return chosen
      .map((r) => `== ${r.session.name} (${r.session.host})${r.exit != null ? `, exit ${r.exit}` : ""}\r\n${(r.output ?? "").trimEnd().replaceAll("\n", "\r\n")}\r\n`)
      .join("\r\n");
  }

  async function saveText() {
    const path = await saveDialog({ title: "Save the results", defaultPath: "results.txt", filters: [{ name: "Text", extensions: ["txt"] }] });
    if (path) invoke("write_text_file", { path, text: `${command}\r\n\r\n${asText()}` }).then(() => app.notify(`Saved to ${path}`), (e) => app.notify(String(e), "error"));
  }
</script>

<div class="tool-view">
  <header>
    <ListChecks size={20} />
    <div>
      <h2>Run on many servers</h2>
      <p class="muted">One command on every SSH session of a folder, a few at a time, with the answers side by side. Each server gets its own short connection.</p>
    </div>
  </header>

  <div class="controls">
    <label class="field"><span>Folder</span>
      <select bind:value={folderId} style="width: 260px">
        <option value="">All SSH sessions</option>
        {#each folders as f}<option value={f.id}>{f.path}</option>{/each}
      </select>
    </label>
    <label class="field grow"><span>Command</span>
      <input bind:value={command} class="mono" placeholder="uptime" onkeydown={(e) => e.key === "Enter" && !running && run()} />
    </label>
    <label class="field"><span>Examples</span>
      <select value="" onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v) command = v; (e.target as HTMLSelectElement).value = ""; }} style="width: 200px">
        <option value="">Choose...</option>
        {#each EXAMPLES as x}<option value={x.command}>{x.label}</option>{/each}
      </select>
    </label>
  </div>
  <div class="controls">
    <label class="check"><input type="checkbox" bind:checked={root} /> As root, with sudo (only where sudo needs no password)</label>
    <label class="field inline"><span>At the same time</span><input type="number" min="1" max="20" bind:value={parallel} style="width: 64px" /></label>
    <span class="grow"></span>
    {#if running}
      <button class="btn" onclick={() => (stopped = true)}><Square size={14} /> Stop</button>
    {/if}
    <button class="btn primary" disabled={running || !command.trim() || !chosen.length} onclick={run}><Play size={14} /> Run on {chosen.length} {chosen.length === 1 ? "server" : "servers"}</button>
  </div>
  <p class="muted small">It does exactly what you type on every chosen server; there is no undo. Logins come from the sessions, their folders and the vault; a server that needs a typed password asks for it.</p>

  {#if rows.length}
    <div class="bar">
      <span class="muted small">{done} of {chosen.length} done{rows.some((r) => r.state === "failed") ? `, ${rows.filter((r) => r.state === "failed").length} with an error or another exit code` : ""}</span>
      <span class="grow"></span>
      <button class="btn small" disabled={!done} onclick={() => writeText(asText()).then(() => app.notify("Results copied"))}><Copy size={13} /> Copy all</button>
      <button class="btn small" disabled={!done} onclick={saveText}><Download size={13} /> Save</button>
    </div>
    <table class="grid">
      <thead><tr><th style="width: 28px"><input type="checkbox" checked={rows.every((r) => r.on)} onchange={(e) => rows.forEach((r) => (r.on = (e.target as HTMLInputElement).checked))} /></th><th>Server</th><th>Result</th><th style="width: 70px">Time</th></tr></thead>
      <tbody>
        {#each rows as r (r.session.id)}
          <tr class={r.state} onclick={() => r.output && (open[r.session.id] = !open[r.session.id])}>
            <td><input type="checkbox" bind:checked={r.on} onclick={(e) => e.stopPropagation()} /></td>
            <td><span class="nm">{r.session.name}</span> <span class="muted small mono">{r.session.host}</span></td>
            <td class="res">
              {#if r.state === "running"}<span class="smark connecting"></span> running...
              {:else if r.state === "waiting"}<span class="muted">{r.on ? (running ? "waiting" : "") : "skipped"}</span>
              {:else}
                {#if r.output?.trim().includes("\n")}<span class="chev" class:openc={open[r.session.id]}><ChevronRight size={12} /></span>{/if}
                <span class="mono">{r.exit != null && r.exit !== 0 ? `exit ${r.exit}: ` : ""}{firstLine(r.output) || "(no output)"}</span>
              {/if}
            </td>
            <td class="muted small">{r.ms != null ? `${(r.ms / 1000).toFixed(1)} s` : ""}</td>
          </tr>
          {#if open[r.session.id] && r.output}
            <tr class="out"><td></td><td colspan="3"><pre>{r.output}</pre></td></tr>
          {/if}
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="muted empty">This folder has no SSH sessions.</p>
  {/if}
</div>

<style>
  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 12px;
    margin-bottom: 10px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field > span {
    font-size: 12px;
    color: var(--muted);
  }
  .field.inline {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
  .grow {
    flex: 1;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 14px 0 6px;
  }
  tr {
    cursor: default;
  }
  tr.failed .res {
    color: var(--danger-text);
  }
  .res {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 0;
    width: 60%;
  }
  .chev {
    display: inline-flex;
    transition: transform 0.1s;
  }
  .chev.openc {
    transform: rotate(90deg);
  }
  tr.out pre {
    margin: 0 0 8px;
    padding: 8px 10px;
    max-height: 320px;
    overflow: auto;
    background: var(--field);
    border: 1px solid var(--border);
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre-wrap;
  }
</style>

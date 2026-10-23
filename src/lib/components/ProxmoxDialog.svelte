<script lang="ts">
  // Proxmox from the tree: a guest's snapshots (take, roll back, delete) and the task log of a server or guest.
  import { onMount } from "svelte";
  import Modal from "./Modal.svelte";
  import { app } from "../state.svelte";
  import { ask } from "../ask.svelte";
  import { proxmox } from "../proxmox.svelte";
  import type { ProxmoxSnapshot, ProxmoxTask } from "../api";
  import { Camera, RotateCcw, Trash2, RefreshCw, ChevronRight } from "@lucide/svelte";

  const d = proxmox.dialog!;
  const g = d.g;
  const what = g ? `${g.name} (${g.kind === "lxc" ? "container" : "VM"} ${g.vmid})` : d.src.name;

  let snaps = $state<ProxmoxSnapshot[]>([]);
  let tasks = $state<ProxmoxTask[]>([]);
  let loading = $state(true);
  let error = $state("");
  let busy = $state("");
  let newName = $state(proxmox.snapshotName());
  let newDesc = $state("");
  let ram = $state(false);
  let openTask = $state<string | null>(null);
  let log = $state("");

  onMount(() => {
    app.dialogs++;
    load();
    return () => app.dialogs--;
  });

  async function load() {
    loading = true;
    error = "";
    try {
      if (d.kind === "snapshots" && g) snaps = (await proxmox.snapshots(d.src, g)).reverse();
      else tasks = await proxmox.tasks(d.src, g?.vmid);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function act(action: "create" | "rollback" | "delete", name: string) {
    if (!g) return;
    if (action === "rollback" && !(await ask.confirm("Roll back", `Put ${what} back to the snapshot ${name}? Everything changed since then is lost${g.kind === "qemu" ? ", and the VM is stopped unless the snapshot holds its memory" : ""}.`, { okLabel: "Roll back", danger: true }))) return;
    if (action === "delete" && !(await ask.confirm("Delete the snapshot", `Delete the snapshot ${name} of ${what}? The guest itself stays as it is.`, { okLabel: "Delete", danger: true }))) return;
    busy = action === "create" ? "Taking the snapshot..." : action === "rollback" ? "Rolling back..." : "Deleting the snapshot...";
    try {
      const result = await proxmox.snapshot(d.src, g, action, name, newDesc, ram);
      if (result === "OK") app.notify(action === "create" ? `Snapshot ${name} taken` : action === "rollback" ? `${g.name} is back at ${name}` : `Snapshot ${name} deleted`);
      else app.notify(`Proxmox: ${result}`, "error");
      if (action === "create") {
        newName = proxmox.snapshotName();
        newDesc = "";
      }
    } catch (e) {
      app.notify(String(e), "error");
    } finally {
      busy = "";
      load();
      if (action === "rollback") setTimeout(() => proxmox.refresh(d.src.id, true), 2000);
    }
  }

  async function showLog(t: ProxmoxTask) {
    if (openTask === t.upid) return (openTask = null);
    openTask = t.upid;
    log = "Loading...";
    log = await proxmox.taskLog(d.src, t).catch((e) => String(e));
  }

  const when = (t: number) => (t ? new Date(t * 1000).toLocaleString() : "");
  const took = (t: ProxmoxTask) => (t.end && t.start ? `${Math.max(0, t.end - t.start)} s` : "");
  // Proxmox's task types in words; the rest as Proxmox names them.
  const KIND: Record<string, string> = {
    qmsnapshot: "Snapshot", vzsnapshot: "Snapshot", qmrollback: "Rollback", vzrollback: "Rollback", qmdelsnapshot: "Delete snapshot",
    vzdelsnapshot: "Delete snapshot", vzdump: "Backup", qmstart: "Start", vzstart: "Start", qmstop: "Stop", vzstop: "Stop",
    qmshutdown: "Shut down", vzshutdown: "Shut down", qmreboot: "Reboot", vzreboot: "Reboot", qmcreate: "Create", vzcreate: "Create",
    qmdestroy: "Delete", vzdestroy: "Delete", qmigrate: "Migrate", vzmigrate: "Migrate", aptupdate: "Package update check",
    vncproxy: "Console", vncshell: "Node shell", termproxy: "Node shell", imgcopy: "Copy disk", qmclone: "Clone", vzclone: "Clone",
  };
</script>

<Modal title={d.kind === "snapshots" ? `Snapshots of ${what}` : `Tasks of ${what}`} width={760} onclose={() => (proxmox.dialog = null)}>
  {#if error}
    <p class="err">{error}</p>
  {/if}
  {#if d.kind === "snapshots" && g}
    <div class="take">
      <label class="field grow"><span>Name</span><input bind:value={newName} class="mono" /></label>
      <label class="field grow"><span>Description (optional)</span><input bind:value={newDesc} placeholder="Before the update" /></label>
      {#if g.kind === "qemu"}<label class="check" title="A rollback then resumes the running VM; the snapshot takes as much room as the VM's memory."><input type="checkbox" bind:checked={ram} /> With memory</label>{/if}
      <button class="btn primary" disabled={!!busy || !newName.trim()} onclick={() => act("create", newName.trim())}><Camera size={14} /> Take a snapshot</button>
    </div>
    {#if busy}<p class="muted small"><span class="smark connecting"></span> {busy}</p>{/if}
    {#if loading}
      <p class="muted">Loading...</p>
    {:else if !snaps.length}
      <p class="muted">{what} has no snapshots yet.</p>
    {:else}
      <table class="grid">
        <thead><tr><th>Snapshot</th><th>Taken</th><th></th></tr></thead>
        <tbody>
          {#each snaps as s (s.name)}
            <tr>
              <td><span class="mono">{s.name}</span>{#if s.ram} <span class="tag">with memory</span>{/if}{#if s.description}<div class="muted small">{s.description}</div>{/if}</td>
              <td class="muted small">{when(s.time)}</td>
              <td class="acts">
                <button class="btn small" disabled={!!busy} onclick={() => act("rollback", s.name)}><RotateCcw size={13} /> Roll back</button>
                <button class="icon-btn" title="Delete the snapshot" aria-label="Delete the snapshot" disabled={!!busy} onclick={() => act("delete", s.name)}><Trash2 size={13} /></button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {:else}
    {#if loading}
      <p class="muted">Loading...</p>
    {:else if !tasks.length}
      <p class="muted">No recent tasks.</p>
    {:else}
      <div class="tasks">
        {#each tasks as t (t.upid)}
          <button class="trow" class:bad={t.status && t.status !== "OK"} onclick={() => showLog(t)}>
            <span class="chev" class:open={openTask === t.upid}><ChevronRight size={12} /></span>
            <span class="tk">{KIND[t.kind] ?? t.kind}{t.id ? ` ${t.id}` : ""}</span>
            <span class="muted small">{t.node}</span>
            <span class="muted small">{t.user}</span>
            <span class="muted small">{when(t.start)}</span>
            <span class="st" title={t.status}>{t.status ? (t.status === "OK" ? "OK" : `Failed: ${t.status}`) : "running"}{t.status === "OK" && took(t) ? `, ${took(t)}` : ""}</span>
          </button>
          {#if openTask === t.upid}<pre class="log">{log}</pre>{/if}
        {/each}
      </div>
    {/if}
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={load}><RefreshCw size={14} /> Refresh</button>
    <span class="grow"></span>
    <button class="btn primary" onclick={() => (proxmox.dialog = null)}>Close</button>
  {/snippet}
</Modal>

<style>
  .take {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    margin-bottom: 12px;
    flex-wrap: wrap;
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
  .grow {
    flex: 1;
  }
  .err {
    color: var(--danger-text);
  }
  .tag {
    font-size: 11px;
    padding: 0 5px;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--muted);
  }
  table.grid {
    width: 100%;
    border-collapse: collapse;
  }
  table.grid th {
    text-align: left;
    font-size: 12px;
    color: var(--muted);
    font-weight: 500;
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
  }
  table.grid td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    vertical-align: top;
  }
  .acts {
    text-align: right;
    white-space: nowrap;
  }
  .tasks {
    display: flex;
    flex-direction: column;
    max-height: 60vh;
    overflow: auto;
  }
  .trow {
    flex-shrink: 0;
    display: grid;
    grid-template-columns: 16px 1.6fr 0.6fr 1fr 1.3fr 1fr;
    gap: 8px;
    align-items: center;
    padding: 5px 6px;
    border: none;
    border-bottom: 1px solid var(--border);
    background: none;
    color: var(--text);
    text-align: left;
  }
  .trow:hover {
    background: var(--hover);
  }
  .trow.bad .st {
    color: var(--danger-text);
  }
  .st {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chev {
    display: inline-flex;
    transition: transform 0.1s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .log {
    flex-shrink: 0;
    margin: 0 0 6px 22px;
    padding: 8px 10px;
    max-height: 300px;
    overflow: auto;
    background: var(--field);
    border: 1px solid var(--border);
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre-wrap;
  }
  .small {
    font-size: 12px;
  }
</style>

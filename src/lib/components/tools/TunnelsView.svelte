<script lang="ts">
  import { app } from "../../state.svelte";
  import { tunnels, describeTunnel } from "../../tunnels.svelte";
  import { ask } from "../../ask.svelte";
  import type { TunnelDef } from "../../api";
  import Modal from "../Modal.svelte";
  import WireGuardList from "./WireGuardList.svelte";
  import { Play, Square, Plus, Pencil, Trash2, Waypoints } from "@lucide/svelte";
  import "./tool.css";

  let editing = $state<TunnelDef | null>(null);

  const sshSessions = $derived(
    app.sessions
      .filter((s) => s.kind === "ssh")
      .map((s) => ({ s, label: `${app.folderPath(s.folder_id) ? app.folderPath(s.folder_id) + " / " : ""}${s.name}` }))
      .sort((a, b) => a.label.localeCompare(b.label)),
  );

  function blank(): TunnelDef {
    return {
      id: crypto.randomUUID(),
      name: "",
      kind: "local",
      bind_address: "127.0.0.1",
      bind_port: 8080,
      dest_host: "localhost",
      dest_port: 80,
      ssh: { host: "", port: 22, user: "", key_path: "" },
      jump_hosts: [],
      autostart: false,
    };
  }

  function useSession(id: string) {
    if (!editing) return;
    const s = app.sessions.find((x) => x.id === id);
    editing.session_id = id || undefined;
    if (!s) return;
    editing.ssh = { host: s.host, port: s.port || 22, user: s.username, key_path: s.data?.key_path ?? "" };
    editing.jump_hosts = (s.data?.jump_hosts ?? []).map((j: any) => ({ host: j.host, port: j.port ?? 22, user: j.user ?? "", key_path: j.key_path ?? "" }));
  }

  async function save() {
    if (!editing) return;
    if (!editing.ssh.host.trim()) return app.notify("Choose the SSH server for this tunnel.", "error");
    if (editing.kind !== "dynamic" && !editing.dest_host.trim()) return app.notify("Fill in where the tunnel goes to.", "error");
    if (!editing.name.trim()) editing.name = describeTunnel(editing);
    const def = $state.snapshot(editing) as TunnelDef;
    const wasRunning = tunnels.state(def.id) === "running";
    await tunnels.upsert(def);
    editing = null;
    if (wasRunning) {
      await tunnels.stop(def.id);
      tunnels.start(def);
    }
  }

  async function remove(t: TunnelDef) {
    if (await ask.confirm("Delete tunnel", `Delete the tunnel "${t.name}"?`, { okLabel: "Delete", danger: true })) tunnels.remove(t.id);
  }

  const STATE_LABEL: Record<string, string> = { running: "Running", starting: "Starting", stopped: "Stopped", failed: "Failed" };
  const MARK: Record<string, string> = { running: "connected", starting: "connecting", stopped: "closed", failed: "failed" };
  const KIND_LABEL: Record<string, string> = { local: "Local", remote: "Remote", dynamic: "SOCKS proxy" };

  /** The route as stops from where the port listens to where the traffic ends up. */
  function stops(t: TunnelDef): { label: string; value: string }[] {
    const ssh = `${t.ssh.user ? t.ssh.user + "@" : ""}${t.ssh.host}`;
    const jumps = (t.jump_hosts ?? []).filter((j) => j.host).map((j) => ({ label: "Via", value: j.host }));
    const dest = `${t.dest_host}:${t.dest_port}`;
    if (t.kind === "local")
      return [{ label: "Listens on this PC", value: `${t.bind_address || "localhost"}:${t.bind_port}` }, ...jumps, { label: "Through", value: ssh }, { label: "Reaches", value: dest }];
    if (t.kind === "remote")
      return [{ label: "Listens on the server", value: `${ssh}, port ${t.bind_port}` }, ...jumps, { label: "Back through", value: "this PC" }, { label: "Reaches", value: dest }];
    return [{ label: "SOCKS proxy on this PC", value: `${t.bind_address || "localhost"}:${t.bind_port}` }, ...jumps, { label: "Leaves through", value: ssh }];
  }
</script>

<div class="tool-view">
  <header>
    <Waypoints size={20} />
    <div>
      <h2>Tunnels</h2>
      <p class="muted">Port forwards over SSH that keep running while you work in other tabs, and WireGuard networks for sessions.</p>
    </div>
    <span class="grow"></span>
    <button class="btn" onclick={() => tunnels.startAll()} disabled={!tunnels.list.length}><Play size={14} /> Start all</button>
    <button class="btn" onclick={() => tunnels.stopAll()} disabled={!tunnels.list.length}><Square size={14} /> Stop all</button>
    <button class="btn primary" onclick={() => (editing = blank())}><Plus size={14} /> New tunnel</button>
  </header>

  {#if !tunnels.list.length}
    <div class="empty">
      <p>No tunnels yet.</p>
      <p class="muted">
        A tunnel makes a port on this PC reach something behind an SSH server (local), lets the SSH server reach
        something near this PC (remote), or gives you a SOCKS proxy that leaves through the SSH server (dynamic).
      </p>
    </div>
  {:else}
    <div class="tlist">
      {#each tunnels.list as t (t.id)}
        {@const st = tunnels.status[t.id]}
        {@const state = tunnels.state(t.id)}
        <div class="tcard">
          <div class="thead">
            <span class="smark {MARK[state]}"></span>
            <strong class="name">{t.name}</strong>
            <span class="tag">{KIND_LABEL[t.kind]}</span>
            {#if t.autostart}<span class="tag">starts with Cablebag</span>{/if}
            <span class="grow"></span>
            <span class="state {state}">{STATE_LABEL[state]}{#if state === "running" && st?.connections}, {st.connections} open{/if}</span>
            {#if state === "running" || state === "starting"}
              <button class="btn" onclick={() => tunnels.stop(t.id)}><Square size={14} /> Stop</button>
            {:else}
              <button class="btn" onclick={() => tunnels.start(t)}><Play size={14} /> Start</button>
            {/if}
            <button class="icon-btn" title="Edit" aria-label="Edit" onclick={() => (editing = JSON.parse(JSON.stringify(t)))}><Pencil size={15} /></button>
            <button class="icon-btn" title="Delete" aria-label="Delete" onclick={() => remove(t)}><Trash2 size={15} /></button>
          </div>
          <div class="route">
            {#each stops(t) as stop, i}
              {#if i}<span class="arrow" aria-hidden="true"></span>{/if}
              <span class="stop"><em>{stop.label}</em><span class="mono">{stop.value}</span></span>
            {/each}
          </div>
          {#if st?.message && state !== "running"}<div class="msg">{st.message}</div>{/if}
        </div>
      {/each}
    </div>
  {/if}

  <WireGuardList />
</div>

{#if editing}
  <Modal title={tunnels.list.some((t) => t.id === editing?.id) ? "Edit tunnel" : "New tunnel"} width={640} onclose={() => (editing = null)}>
    <div class="form">
      <div class="kinds">
        {#each [["local", "Local", "A port on this PC reaches a host behind the SSH server"], ["remote", "Remote", "A port on the SSH server reaches a host near this PC"], ["dynamic", "Dynamic (SOCKS)", "A SOCKS proxy on this PC; traffic leaves through the SSH server"]] as [id, label, help]}
          <label class="kind" class:on={editing.kind === id}>
            <input type="radio" name="kind" value={id} bind:group={editing.kind} />
            <strong>{label}</strong>
            <span class="muted">{help}</span>
          </label>
        {/each}
      </div>

      <label class="field"><span>SSH server</span>
        <select value={editing.session_id ?? ""} onchange={(e) => useSession((e.target as HTMLSelectElement).value)}>
          <option value="">Type the server details below</option>
          {#each sshSessions as o (o.s.id)}<option value={o.s.id}>{o.label}</option>{/each}
        </select>
      </label>
      <div class="row">
        <label class="field grow"><span>Host</span><input bind:value={editing.ssh.host} /></label>
        <label class="field"><span>User</span><input bind:value={editing.ssh.user} /></label>
        <label class="field small"><span>Port</span><input type="number" bind:value={editing.ssh.port} /></label>
      </div>
      <label class="field"><span>Key file (optional)</span><input bind:value={editing.ssh.key_path} placeholder="Log in with a password or agent" /></label>

      <div class="row">
        <label class="field small"><span>{editing.kind === "remote" ? "Port on the SSH server" : "Port on this PC"}</span><input type="number" bind:value={editing.bind_port} /></label>
        {#if editing.kind !== "dynamic"}
          <label class="field grow"><span>{editing.kind === "remote" ? "Goes to host (seen from this PC)" : "Goes to host (seen from the SSH server)"}</span><input bind:value={editing.dest_host} /></label>
          <label class="field small"><span>Port</span><input type="number" bind:value={editing.dest_port} /></label>
        {/if}
      </div>
      <p class="route-preview">{describeTunnel(editing)}</p>

      <label class="check">
        <input
          type="checkbox"
          checked={editing.bind_address === "0.0.0.0"}
          onchange={(e) => editing && (editing.bind_address = (e.target as HTMLInputElement).checked ? "0.0.0.0" : editing.kind === "remote" ? "localhost" : "127.0.0.1")}
        />
        {editing.kind === "remote" ? "Let other computers reach the port on the SSH server" : "Let other computers on the network use this port"}
      </label>
      <label class="check"><input type="checkbox" bind:checked={editing.autostart} /> Start this tunnel when Cablebag starts</label>
      <label class="field"><span>Name</span><input bind:value={editing.name} placeholder="For example: Proxmox web interface" /></label>
    </div>
    {#snippet footer()}
      <button class="btn" onclick={() => (editing = null)}>Cancel</button>
      <button class="btn primary" onclick={save}>Save tunnel</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .tlist {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 1100px;
  }
  .tcard {
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--chrome);
  }
  .thead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 8px 12px;
    border-bottom: 1px solid var(--border);
  }
  .name {
    font-weight: 600;
  }
  .tag {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 3px;
    border: 1px solid var(--border);
    color: var(--muted);
  }
  .state {
    font-size: 12px;
    color: var(--muted);
    margin-right: 4px;
  }
  .state.running {
    color: var(--ok-text);
  }
  .state.failed {
    color: var(--danger-text);
  }
  .route {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
    padding: 10px 12px;
    background: var(--bg);
  }
  .stop {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .stop em {
    font-style: normal;
    font-size: 11px;
    color: var(--muted);
  }
  .stop .mono {
    font-size: 12px;
  }
  .arrow {
    width: 22px;
    height: 1px;
    background: var(--border-strong);
    position: relative;
  }
  .arrow::after {
    content: "";
    position: absolute;
    right: 0;
    top: -3px;
    border: 3.5px solid transparent;
    border-left: 5px solid var(--border-strong);
    border-right: none;
  }
  .msg {
    padding: 0 12px 10px;
    font-size: 12px;
    color: var(--danger-text);
    background: var(--bg);
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .kinds {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .kind {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 3px;
    cursor: pointer;
    font-size: 12px;
  }
  .kind.on {
    border-color: var(--accent);
    background: var(--selected);
  }
  .kind input {
    display: none;
  }
  .row {
    display: flex;
    gap: 10px;
  }
  .grow {
    flex: 1;
  }
  .small input {
    width: 110px;
  }
  .route-preview {
    margin: 0;
    padding: 8px 10px;
    border-radius: 3px;
    background: var(--panel);
    font-family: var(--font-mono);
    font-size: 12px;
  }
</style>

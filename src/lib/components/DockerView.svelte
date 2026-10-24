<script lang="ts">
  // The containers on one SSH server, like Portainer but inside Cablebag: grouped by compose project, with where
  // each project's compose file is, logs and a shell per container, and start, stop and restart. Docker is asked
  // over this tab's own SSH connection; no Docker port is opened.
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, Channel, type DockerContainer, type DockerListing } from "../api";
  import { app, newSession, type Tab } from "../state.svelte";
  import { ask } from "../ask.svelte";
  import { menu } from "../menu.svelte";
  import { containers } from "../docker.svelte";
  import { writeText } from "../clipboard";
  import Modal from "./Modal.svelte";
  import {
    Container, RefreshCw, RotateCw, ScrollText, TerminalSquare, Play, Square, Pause, FileCode, Ellipsis,
    ChevronRight, ExternalLink, Download, Copy, Search, X,
  } from "@lucide/svelte";
  import "./tools/tool.css";

  let { tab, visible }: { tab: Tab; visible: boolean } = $props();

  let message = $state("");
  let listing = $state<DockerListing | null>(null);
  let error = $state("");
  let loading = $state(false);
  let filter = $state("");
  let folded = $state<Record<string, boolean>>({});
  let busy = $state<Record<string, string>>({});
  let output = $state<{ title: string; text: string; ok: boolean } | null>(null);
  let viewing = $state<{ path: string; onServer: string; text: string } | null>(null);

  // svelte-ignore state_referenced_locally
  const server = tab.session!;
  const sudo = $derived(listing?.access === "sudo");

  onMount(() => {
    const un = listen<{ id: string; state: string; message: string }>("term-status", (e) => {
      if (e.payload.id === tab.id && e.payload.message) message = e.payload.message;
    });
    api.filesOpen(tab.id, $state.snapshot(server), new Channel<ArrayBuffer>()).catch((e) => {
      message = String(e);
      tab.status = "failed";
    });
    // While the tab is on screen the list follows the server.
    const timer = setInterval(() => {
      if (visible && tab.status === "connected" && document.visibilityState === "visible" && !loading) load(true);
    }, 10000);
    return () => {
      clearInterval(timer);
      un.then((f) => f());
    };
  });

  $effect(() => {
    if (tab.status === "connected" && !listing && !loading) load();
  });

  async function load(quiet = false) {
    loading = true;
    if (!quiet) error = "";
    try {
      listing = await api.dockerList(tab.id);
      error = "";
      containers.remember(server.id, listing.access === "direct" || listing.access === "sudo");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  interface Group {
    key: string;
    project: string;
    dir: string;
    files: string[];
    portainer: boolean;
    items: DockerContainer[];
  }

  const groups = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const list = (listing?.containers ?? []).filter(
      (c) => !q || [c.name, c.image, c.project, c.service, c.working_dir].some((v) => v.toLowerCase().includes(q)),
    );
    const out: Group[] = [];
    for (const c of list) {
      const key = c.project || "";
      let g = out.find((x) => x.key === key);
      if (!g) out.push((g = { key, project: c.project, dir: c.working_dir, files: [], portainer: c.portainer, items: [] }));
      for (const f of c.config_files) if (!g.files.includes(f)) g.files.push(f);
      g.items.push(c);
    }
    // Loose containers last.
    return out.sort((a, b) => (a.key === "" ? 1 : 0) - (b.key === "" ? 1 : 0) || a.key.localeCompare(b.key));
  });

  const running = $derived((listing?.containers ?? []).filter((c) => c.state === "running").length);

  function mark(c: DockerContainer): string {
    if (c.state === "running") return c.health === "unhealthy" ? "failed" : c.health === "starting" ? "connecting" : "connected";
    if (c.state === "restarting") return "connecting";
    if (c.state === "dead") return "failed";
    return "";
  }

  function since(iso: string): string {
    const t = Date.parse(iso);
    if (!t || t < 0) return "";
    const s = Math.max(0, (Date.now() - t) / 1000);
    if (s < 90) return "a minute";
    if (s < 5400) return `${Math.round(s / 60)} minutes`;
    if (s < 129600) return `${Math.round(s / 3600)} hours`;
    return `${Math.round(s / 86400)} days`;
  }

  function stateText(c: DockerContainer): string {
    const words: Record<string, string> = { running: "running", exited: "stopped", restarting: "restarting", paused: "paused", created: "created, not started", dead: "dead" };
    let t = words[c.state] ?? c.state;
    if (c.state === "running" && since(c.started)) t += ` for ${since(c.started)}`;
    if (c.health) t += `, ${c.health}`;
    if (c.restarts) t += `, restarted ${c.restarts} ${c.restarts === 1 ? "time" : "times"}`;
    return t;
  }

  /** A published web port opens as a web tab; ports that are usually https get https. */
  function openPort(port: number, privatePort: number) {
    const https = [443, 8443, 9443].includes(privatePort) || [443, 8443, 9443].includes(port);
    const s = newSession("browser");
    s.name = `${server.name || server.host}:${port}`;
    s.host = `${https ? "https" : "http"}://${server.host}:${port}`;
    s.folder_id = server.folder_id;
    if (server.data?.wireguard) s.data.wireguard = server.data.wireguard;
    app.openSession(s);
  }

  async function containerAction(c: DockerContainer, action: string) {
    if (action === "stop" && !(await ask.confirm("Stop container", `Stop ${c.name}?`, { okLabel: "Stop", danger: true }))) return;
    busy[c.id] = action;
    try {
      const r = await api.dockerContainer(tab.id, sudo, c.id, action);
      if (r.exit !== 0) output = { title: `${action} ${c.name}`, text: r.output, ok: false };
      else app.notify(`${c.name}: ${action === "stop" ? "stopped" : action === "start" ? "started" : action === "restart" ? "restarted" : action + "d"}`);
    } catch (e) {
      app.notify(String(e), "error");
    } finally {
      delete busy[c.id];
      load(true);
    }
  }

  const COMPOSE: Record<string, { label: string; ask?: string; done: string }> = {
    up: { label: "Apply the compose file", ask: "Start or recreate the containers of {p} as the compose file says now (docker compose up -d)?", done: "applied" },
    pull: { label: "Pull new images", done: "images pulled; apply the compose file to use them" },
    restart: { label: "Restart all", done: "restarted" },
    stop: { label: "Stop all", ask: "Stop every container of {p}?", done: "stopped" },
    start: { label: "Start all", done: "started" },
  };

  async function composeAction(g: Group, action: string) {
    const a = COMPOSE[action];
    if (a.ask && !(await ask.confirm(a.label, a.ask.replace("{p}", g.project), { okLabel: a.label, danger: action === "stop" }))) return;
    busy[`p:${g.key}`] = action;
    try {
      const r = await api.dockerCompose(tab.id, listing!.compose, sudo, g.project, g.dir, g.files, action);
      output = { title: `${g.project}: ${a.label.toLowerCase()}`, text: r.output || "(no output)", ok: r.exit === 0 };
      if (r.exit === 0) app.notify(`${g.project}: ${a.done}`);
    } catch (e) {
      app.notify(String(e), "error");
    } finally {
      delete busy[`p:${g.key}`];
      load(true);
    }
  }

  /** A compose file: in Cablebag's editor when it is a file on the server, read-only when it is Portainer's. */
  async function openFile(g: Group, path: string) {
    if (!g.portainer) {
      app.openEditor(tab.id, path);
      return;
    }
    const p = listing?.portainer;
    if (!p) {
      app.notify("This stack was made in Portainer, but Cablebag cannot find the Portainer container on this server to read it from.", "error");
      return;
    }
    try {
      const text = await api.dockerPortainerFile(tab.id, sudo, p.container, p.data, path);
      const inside = path.startsWith(p.data) ? `/data${path.slice(p.data.length)}` : path;
      viewing = { path: inside, onServer: `${p.data}${inside.slice("/data".length)}`, text };
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  function openPortainer() {
    const p = listing?.portainer;
    if (p?.url_port) openPort(p.url_port, p.https ? 9443 : 9000);
  }

  function projectMenu(e: MouseEvent, g: Group) {
    e.stopPropagation();
    const off = g.portainer ? "Made in Portainer: change and redeploy this stack there" : !g.dir ? "Docker does not know this project's folder" : "";
    const noCompose = listing?.compose === "none" ? "Docker Compose is not installed on this server" : "";
    menu.show(e, [
      ...g.files.map((f) => ({ label: `${g.portainer ? "View" : "Edit"} ${f.split("/").pop()}`, icon: FileCode, hint: g.files.length > 1 ? f : undefined, action: () => openFile(g, f) })),
      "sep",
      // Without the compose actions, one line says why instead of five greyed-out ones.
      ...(off || noCompose
        ? [{ label: off || noCompose, disabled: true }]
        : Object.entries(COMPOSE).map(([id, a]) => ({
            label: a.label,
            icon: id === "pull" ? Download : id === "stop" ? Square : id === "start" ? Play : RotateCw,
            danger: id === "stop",
            action: () => composeAction(g, id),
          }))),
      "sep",
      { label: "Copy the folder", icon: Copy, disabled: !g.dir, action: () => writeText(g.dir) },
      ...(g.portainer && listing?.portainer?.url_port ? [{ label: "Open Portainer", icon: ExternalLink, action: openPortainer }] : []),
    ]);
  }

  function containerMenu(e: MouseEvent, c: DockerContainer) {
    e.stopPropagation();
    const on = c.state === "running";
    menu.show(e, [
      { label: "Logs", icon: ScrollText, action: () => containers.logs(server, c, sudo) },
      { label: "Shell inside", icon: TerminalSquare, disabled: !on, action: () => containers.shell(server, c, sudo) },
      "sep",
      on
        ? { label: "Restart", icon: RotateCw, action: () => containerAction(c, "restart") }
        : { label: "Start", icon: Play, action: () => containerAction(c, c.state === "paused" ? "unpause" : "start") },
      ...(on ? [{ label: "Pause", icon: Pause, action: () => containerAction(c, "pause") }] : []),
      ...(on || c.state === "paused" || c.state === "restarting" ? [{ label: "Stop", icon: Square, danger: true, action: () => containerAction(c, "stop") }] : []),
      "sep",
      { label: "Copy the name", icon: Copy, action: () => writeText(c.name) },
      { label: "Copy the image", icon: Copy, action: () => writeText(c.image) },
    ]);
  }
</script>

<div class="tool-view docker" style:display={visible ? "block" : "none"}>
  <header>
    <Container size={20} />
    <div class="grow">
      <h2>Containers on {server.name || server.host}</h2>
      <p class="muted small">
        {#if listing && (listing.access === "direct" || listing.access === "sudo")}
          {running} of {listing.containers.length} running{listing.access === "sudo" ? ", through sudo" : ""}. Docker is asked over SSH; no Docker port is opened.
        {:else}
          {server.username ? `${server.username}@` : ""}{server.host}
        {/if}
      </p>
    </div>
    {#if listing?.portainer?.url_port}
      <button class="btn" onclick={openPortainer} title="Portainer's web interface on this server, in a web tab"><ExternalLink size={14} /> Portainer</button>
    {/if}
    <button class="btn" disabled={tab.status !== "connected" || loading} onclick={() => load()}><RefreshCw size={14} class={loading ? "spin" : ""} /> Refresh</button>
  </header>

  {#if tab.status !== "connected"}
    <div class="state">
      {#if tab.status === "connecting"}
        <p>Connecting to <strong>{server.host}</strong>...</p>
        {#if message}<p class="muted small">{message}</p>{/if}
      {:else}
        <p>{message || "Not connected."}</p>
        <button class="btn primary" onclick={() => app.restart(tab.id)}><RotateCw size={14} /> Connect again</button>
      {/if}
    </div>
  {:else if error}
    <p class="bad">{error}</p>
  {:else if !listing}
    <p class="muted">Asking Docker...</p>
  {:else if listing.access === "none"}
    <p class="muted empty">Docker is not installed on this server, or it is not on the PATH of {server.username || "this login"}.</p>
  {:else if listing.access === "denied"}
    <p class="muted empty">
      Docker is installed, but {server.username || "this login"} may not use it, and sudo wants a password. Add the user
      to the docker group (<code>sudo usermod -aG docker {server.username || "$USER"}</code>, then log in again), or log
      in as root.
    </p>
  {:else}
    <div class="find">
      <Search size={14} />
      <input bind:value={filter} placeholder="Find a container, image or project" spellcheck="false" />
      {#if filter}<button class="icon-btn" aria-label="Clear" onclick={() => (filter = "")}><X size={13} /></button>{/if}
    </div>
    {#if !groups.length}
      <p class="muted empty">{filter ? "Nothing matches." : "No containers on this server."}</p>
    {/if}
    {#each groups as g (g.key)}
      <section class="group">
        <div class="ghead">
          <button class="fold" aria-expanded={!folded[g.key]} onclick={() => (folded[g.key] = !folded[g.key])}>
            <span class="chev" class:open={!folded[g.key]}><ChevronRight size={13} /></span>
            <strong>{g.project || "Not in a compose project"}</strong>
            {#if g.portainer}<span class="tag" title="Made in Portainer: its compose file is inside Portainer's data">Portainer</span>{/if}
            <span class="muted small">{g.items.filter((c) => c.state === "running").length} of {g.items.length} running</span>
          </button>
          {#if g.project}
            {#if busy[`p:${g.key}`]}<span class="muted small">{COMPOSE[busy[`p:${g.key}`]].label}...</span>{/if}
            {#each g.files.slice(0, 1) as f}
              <button class="path mono" title={g.portainer ? `Inside Portainer: ${f}` : `Open ${f} in Cablebag's editor`} onclick={() => openFile(g, f)}>
                <FileCode size={13} />{f}
              </button>
            {/each}
            <button class="icon-btn" title="Compose file and actions" aria-label={`Actions for ${g.project}`} onclick={(e) => projectMenu(e, g)}><Ellipsis size={15} /></button>
          {/if}
        </div>
        {#if !folded[g.key]}
          <table class="grid">
            <tbody>
              {#each g.items as c (c.id)}
                <tr oncontextmenu={(e) => containerMenu(e, c)}>
                  <td class="st"><span class="smark {mark(c)}" role="img" aria-label={stateText(c)}></span></td>
                  <td class="nm">
                    <span class="name">{c.name}</span>
                    <span class="muted small mono img" title={c.image}>{c.image}</span>
                  </td>
                  <td class="muted small">{busy[c.id] ? `${busy[c.id]}...` : stateText(c)}</td>
                  <td class="ports">
                    {#each c.ports.filter((p) => p.public) as p}
                      <button class="chip" title={`Open port ${p.public} of the server (${p.private}/${p.proto} in the container) in a web tab`} onclick={() => openPort(p.public!, p.private)}>{p.public}{p.public !== p.private ? `:${p.private}` : ""}{p.proto !== "tcp" ? `/${p.proto}` : ""}</button>
                    {/each}
                  </td>
                  <td class="acts">
                    <button class="icon-btn" title="Logs" aria-label={`Logs of ${c.name}`} onclick={() => containers.logs(server, c, sudo)}><ScrollText size={14} /></button>
                    <button class="icon-btn" title="Shell inside" aria-label={`Shell inside ${c.name}`} disabled={c.state !== "running"} onclick={() => containers.shell(server, c, sudo)}><TerminalSquare size={14} /></button>
                    {#if c.state === "running"}
                      <button class="icon-btn" title="Restart" aria-label={`Restart ${c.name}`} disabled={!!busy[c.id]} onclick={() => containerAction(c, "restart")}><RotateCw size={14} /></button>
                    {:else}
                      <button class="icon-btn" title="Start" aria-label={`Start ${c.name}`} disabled={!!busy[c.id]} onclick={() => containerAction(c, c.state === "paused" ? "unpause" : "start")}><Play size={14} /></button>
                    {/if}
                    <button class="icon-btn" title="More" aria-label={`More for ${c.name}`} onclick={(e) => containerMenu(e, c)}><Ellipsis size={14} /></button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </section>
    {/each}
  {/if}
</div>

{#if output}
  <Modal title={output.title} width={720} warning={!output.ok} onclose={() => (output = null)}>
    <pre class="out">{output.text}</pre>
    {#snippet footer()}
      <button class="btn" onclick={() => writeText(output!.text)}><Copy size={14} /> Copy</button>
      <button class="btn primary" onclick={() => (output = null)}>Close</button>
    {/snippet}
  </Modal>
{/if}

{#if viewing}
  <Modal title={viewing.path.split("/").pop() ?? "Compose file"} width={820} onclose={() => (viewing = null)}>
    <p class="muted small view-note">
      Made in Portainer. Inside Portainer it is <span class="mono">{viewing.path}</span>; on the server
      <span class="mono">{viewing.onServer}</span> (only root can change it there). Change it in Portainer, so Portainer
      knows.
    </p>
    <pre class="out file">{viewing.text}</pre>
    {#snippet footer()}
      <button class="btn" onclick={() => writeText(viewing!.text)}><Copy size={14} /> Copy</button>
      {#if listing?.portainer?.url_port}<button class="btn" onclick={openPortainer}><ExternalLink size={14} /> Open Portainer</button>{/if}
      <button class="btn primary" onclick={() => (viewing = null)}>Close</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .state {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .state p {
    margin: 0;
  }
  .bad {
    color: var(--danger-text);
  }
  .find {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 420px;
    margin-bottom: 14px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--field);
    color: var(--muted);
  }
  .find input {
    flex: 1;
    border: none;
    background: none;
    padding: 6px 0;
  }
  .find input:focus {
    outline: none;
  }
  .group {
    margin-bottom: 16px;
  }
  .ghead {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    padding: 4px 0;
    border-bottom: 1px solid var(--border-strong);
  }
  .fold {
    display: flex;
    align-items: center;
    gap: 8px;
    background: none;
    border: none;
    padding: 2px 0;
    color: var(--text);
    cursor: pointer;
    font: inherit;
  }
  .chev {
    display: inline-flex;
    color: var(--muted);
    transition: transform 0.1s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .tag {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--panel-2);
    color: var(--muted);
  }
  .path {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    margin-left: auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: none;
    border: none;
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
  }
  .path:hover {
    color: var(--accent);
    text-decoration: underline;
  }
  .ghead :global(.icon-btn) {
    flex-shrink: 0;
  }
  .ghead > .icon-btn:only-of-type {
    margin-left: auto;
  }
  td.st {
    width: 18px;
    padding-top: 10px !important;
  }
  td.nm {
    width: 34%;
  }
  .name {
    display: block;
    font-weight: 600;
  }
  .img {
    display: block;
    max-width: 380px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  td.ports {
    white-space: nowrap;
  }
  .chip {
    margin: 0 4px 2px 0;
    padding: 1px 7px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel-2);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
    cursor: pointer;
  }
  .chip:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  td.acts {
    width: 1%;
    white-space: nowrap;
    text-align: right;
  }
  tr:hover td {
    background: var(--hover);
  }
  .out {
    margin: 0;
    max-height: 420px;
    overflow: auto;
    padding: 10px 12px;
    background: var(--field);
    border: 1px solid var(--border);
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre-wrap;
  }
  .out.file {
    max-height: 520px;
    white-space: pre;
  }
  .view-note {
    margin: 0 0 10px;
    line-height: 1.5;
  }
  code {
    font-family: var(--font-mono);
    font-size: 12px;
  }
</style>

<script lang="ts">
  // The containers on one SSH server, like Portainer but inside Cablebag. A short list on the left (name, and
  // whether it is fine), the details and the buttons of the container you pick on the right. Docker is asked over
  // this tab's own SSH connection.
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, Channel, type DockerContainer, type DockerListing } from "../api";
  import { app, newSession, type Tab } from "../state.svelte";
  import { ask } from "../ask.svelte";
  import { menu } from "../menu.svelte";
  import { containers, templates, blankSecrets, portainerFile } from "../docker.svelte";
  import { assistant } from "../assistant.svelte";
  import NewStackDialog from "./NewStackDialog.svelte";
  import MoveStackDialog from "./MoveStackDialog.svelte";
  import { writeText } from "../clipboard";
  import Modal from "./Modal.svelte";
  import {
    Container, RefreshCw, RotateCw, ScrollText, TerminalSquare, Play, Square, Pause, FileCode, ExternalLink, Download,
    Copy, Search, X, Plus, Layers, Trash2, Sparkles, Truck, KeyRound,
  } from "@lucide/svelte";
  import "./tools/tool.css";

  let { tab, visible }: { tab: Tab; visible: boolean } = $props();

  let message = $state("");
  let listing = $state<DockerListing | null>(null);
  let error = $state("");
  let loading = $state(false);
  let filter = $state("");
  let show = $state<"all" | "look" | "stopped">("all");
  let picked = $state("");
  let busy = $state<Record<string, string>>({});
  let output = $state<{ title: string; text: string; ok: boolean } | null>(null);
  let viewing = $state<{ path: string; onServer: string; text: string } | null>(null);
  let listEl = $state<HTMLDivElement>();
  let newStack = $state(false);
  let moving = $state("");
  let sudoNote = $state("");

  /** Docker as root through sudo with the login's password (the vault login, or asked once). */
  async function useSudoPassword() {
    sudoNote = "";
    await api.dockerUseSudoPassword(tab.id, $state.snapshot(server));
    containers.asRoot[tab.id] = true;
    listing = null;
    await load();
  }

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
      // Closed (not only reconnecting): the sudo password kept for it goes.
      if (!app.tabs.some((t) => t.id === tab.id)) {
        api.dockerUseSudoPassword(tab.id, null).catch(() => {});
        delete containers.asRoot[tab.id];
      }
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
      // Start on the container that needs a look, otherwise the first.
      if (!listing.containers.some((c) => c.id === picked)) picked = (listing.containers.find(needsLook) ?? listing.containers[0])?.id ?? "";
    } catch (e) {
      const text = String(e);
      // Cancelled or a password sudo refused: back to the view without sudo, which offers it again.
      if (containers.asRoot[tab.id] && /No password given|did not accept the password/.test(text)) {
        delete containers.asRoot[tab.id];
        sudoNote = /did not accept/.test(text) ? "sudo did not accept that password." : "No password given, so Docker is not used through sudo.";
        loading = false;
        return load();
      }
      error = text;
    } finally {
      loading = false;
    }
  }

  /** Not as it should be: unhealthy, restarting, dead, or restarted by Docker more than a couple of times. */
  function needsLook(c: DockerContainer): boolean {
    return c.health === "unhealthy" || c.state === "restarting" || c.state === "dead" || c.restarts >= 3;
  }
  const stopped = (c: DockerContainer) => c.state !== "running" && c.state !== "restarting";

  interface Group {
    key: string;
    project: string;
    portainer: boolean;
    items: DockerContainer[];
  }

  const all = $derived(listing?.containers ?? []);
  const lookCount = $derived(all.filter(needsLook).length);
  const stoppedCount = $derived(all.filter(stopped).length);
  const running = $derived(all.filter((c) => c.state === "running").length);

  const groups = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const list = all
      .filter((c) => (show === "look" ? needsLook(c) : show === "stopped" ? stopped(c) : true))
      .filter((c) => !q || [c.name, c.image, c.project, c.service].some((v) => v.toLowerCase().includes(q)));
    const out: Group[] = [];
    for (const c of list) {
      const key = c.project || `:${c.id}`;
      let g = out.find((x) => x.key === key);
      if (!g) out.push((g = { key, project: c.project, portainer: c.portainer, items: [] }));
      g.items.push(c);
    }
    return out.sort((a, b) => (a.project || a.items[0].name).localeCompare(b.project || b.items[0].name));
  });
  const flat = $derived(groups.flatMap((g) => g.items));

  /** Stacks in ~/stacks that have no containers (removed or never started): they can be started from here. */
  const dirOf = (f: string) => f.slice(0, f.lastIndexOf("/"));
  const idle = $derived.by(() => {
    if (show === "look") return [];
    const q = filter.trim().toLowerCase();
    return (listing?.stacks ?? [])
      .filter((f) => !all.some((c) => c.config_files.includes(f) || c.working_dir === dirOf(f)))
      .map((f) => ({ file: f, dir: dirOf(f), name: dirOf(f).split("/").pop()!.toLowerCase() }))
      .filter((s) => !q || s.name.includes(q));
  });
  const idlePicked = $derived(idle.find((s) => picked === `stack:${s.file}`));
  const current = $derived(all.find((c) => c.id === picked));
  const project = $derived(current?.project ? all.filter((c) => c.project === current.project) : []);

  function since(iso: string): string {
    const t = Date.parse(iso);
    if (!t || t < 0) return "";
    const s = Math.max(0, (Date.now() - t) / 1000);
    if (s < 90) return "a minute";
    if (s < 5400) return `${Math.round(s / 60)} minutes`;
    if (s < 129600) return `${Math.round(s / 3600)} hours`;
    return `${Math.round(s / 86400)} days`;
  }

  /** What the list says: how long it runs when all is well, otherwise what is wrong. */
  function short(c: DockerContainer): { text: string; bad: boolean } {
    if (busy[c.id]) return { text: `${busy[c.id]}...`, bad: false };
    if (c.state === "restarting") return { text: "restarting", bad: true };
    if (c.state === "dead") return { text: "dead", bad: true };
    if (c.state === "paused") return { text: "paused", bad: false };
    if (c.state !== "running") return { text: c.state === "created" ? "not started" : "stopped", bad: false };
    if (c.health === "unhealthy") return { text: "unhealthy", bad: true };
    if (c.restarts >= 3) return { text: `${c.restarts} restarts`, bad: true };
    return { text: since(c.started), bad: false };
  }

  function mark(c: DockerContainer): string {
    if (c.state === "running") return c.health === "unhealthy" || c.restarts >= 3 ? "warn" : "connected";
    if (c.state === "restarting") return "connecting";
    if (c.state === "dead") return "failed";
    return "";
  }

  function stateText(c: DockerContainer): string {
    const words: Record<string, string> = { running: "Running", exited: "Stopped", restarting: "Restarting", paused: "Paused", created: "Created, not started", dead: "Dead" };
    let t = words[c.state] ?? c.state;
    if (c.state === "running" && since(c.started)) t += ` for ${since(c.started)}`;
    if (c.health) t += `, ${c.health}`;
    return t;
  }

  /** The image without the usual registry and the :latest tag. */
  const imageName = (img: string) => img.replace(/^(docker\.io\/)?(library\/)?/, "").replace(/:latest$/, "");

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
    down: { label: "Remove stack", ask: "Remove the containers of {p}? Its volumes and its compose file stay.", done: "removed" },
  };

  async function composeAction(c: Pick<DockerContainer, "project" | "working_dir" | "config_files">, action: string) {
    const a = COMPOSE[action];
    if (a.ask && !(await ask.confirm(a.label, a.ask.replace("{p}", c.project), { okLabel: a.label, danger: action === "stop" }))) return;
    busy[`p:${c.project}`] = action;
    try {
      const r = await api.dockerCompose(tab.id, listing!.compose, sudo, c.project, c.working_dir, c.config_files, action);
      output = { title: `${c.project}: ${a.label.toLowerCase()}`, text: r.output || "(no output)", ok: r.exit === 0 };
      if (r.exit === 0) app.notify(`${c.project}: ${a.done}`);
    } catch (e) {
      app.notify(String(e), "error");
    } finally {
      delete busy[`p:${c.project}`];
      load(true);
    }
  }

  /** The services of the project a container belongs to, to recognise its compose file. */
  const servicesOf = (c: DockerContainer) => [...new Set(all.filter((x) => x.project === c.project).map((x) => x.service))];

  /** A compose file: in Cablebag's editor when it is a file on the server, read-only when it is Portainer's. */
  async function openFile(c: DockerContainer, path: string) {
    if (!c.portainer) {
      app.openEditor(tab.id, path);
      return;
    }
    try {
      const text = await portainerFile(tab.id, listing!, sudo, path, servicesOf(c));
      const p = listing?.portainer;
      const inside = p && path.startsWith(p.data) ? `/data${path.slice(p.data.length)}` : path;
      // On a server Portainer manages through its agent, the file is with that Portainer, not on this server.
      viewing = { path: inside, onServer: p ? `${p.data}${inside.slice("/data".length)}` : "", text };
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  /** A template from a running project (its compose file and .env) or from a container started without Compose. */
  async function saveTemplate(c: DockerContainer) {
    try {
      let compose = "";
      let env = "";
      if (c.project && c.config_files.length) {
        const f = c.config_files[0];
        if (c.portainer) {
          compose = await portainerFile(tab.id, listing!, sudo, f, servicesOf(c));
        } else {
          compose = (await api.readText(tab.id, f)).text;
          env = await api.readText(tab.id, `${c.working_dir}/.env`).then((r) => r.text, () => "");
        }
      } else {
        compose = await api.dockerContainerCompose(tab.id, sudo, c.id);
      }
      const n = await ask.text("Save as template", "Name of the template", c.project || c.name, "Save");
      if (!n?.trim()) return;
      // Passwords in the file become blanks: a template (and Sync) never keeps them.
      await templates.save({ id: crypto.randomUUID(), name: n.trim(), description: `From ${c.project || c.name} on ${server.name || server.host}`, compose: blankSecrets(compose), env: blankSecrets(env) || undefined });
      app.notify(`Saved template "${n.trim()}"; New stack offers it.`);
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  /** What to ask the assistant about a container that is not as it should be. */
  function question(c: DockerContainer): string {
    if (c.health === "unhealthy") return "Why is it unhealthy?";
    if (c.state === "restarting" || c.restarts >= 3) return "Why does it keep restarting?";
    if (c.state === "exited" || c.state === "dead") return "Why did it stop?";
    return "";
  }

  /** The assistant starts with the facts: the container's state, where it comes from, and its last log lines. */
  async function askAssistant(c: DockerContainer) {
    if (!assistant.hosts.length) {
      app.notify("Add an AI host under Settings, Assistant first.", "error");
      return;
    }
    const logs = await api.dockerLogs(tab.id, sudo, c.id, 100).catch((e) => `(could not read the logs: ${e})`);
    const from = c.project ? `compose project ${c.project}, service ${c.service}, file ${c.config_files[0] ?? "unknown"}${c.portainer ? " (made in Portainer)" : ""}` : "not started with Compose";
    const fence = "```";
    const text =
      `Container ${c.name} on this server: image ${c.image}, ${from}. ${stateText(c)}${c.restarts ? `, restarted ${c.restarts} times` : ""}.\n\n` +
      `Its last log lines:\n${fence}\n${logs.trim()}\n${fence}\n\n${question(c) || "Is everything all right with it?"}`;
    assistant.open = true;
    await tick();
    assistant.ask(tab.id, text);
  }

  function openPortainer() {
    const p = listing?.portainer;
    if (p?.url_port) openPort(p.url_port, p.https ? 9443 : 9000);
  }

  function containerMenu(e: MouseEvent, c: DockerContainer) {
    picked = c.id;
    const on = c.state === "running";
    menu.show(e, [
      { label: "Logs", icon: ScrollText, action: () => containers.logs(server, c, containers.prefix(tab.id, sudo)) },
      { label: "Shell inside", icon: TerminalSquare, disabled: !on, action: () => containers.shell(server, c, containers.prefix(tab.id, sudo)) },
      "sep",
      on
        ? { label: "Restart", icon: RotateCw, action: () => containerAction(c, "restart") }
        : { label: "Start", icon: Play, action: () => containerAction(c, c.state === "paused" ? "unpause" : "start") },
      ...(on || c.state === "paused" || c.state === "restarting" ? [{ label: "Stop", icon: Square, danger: true, action: () => containerAction(c, "stop") }] : []),
      "sep",
      { label: "Copy the name", icon: Copy, action: () => writeText(c.name) },
    ]);
  }

  /** Up and down walk the list; the details follow. */
  async function listKey(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const i = flat.findIndex((c) => c.id === picked);
    const next = flat[Math.max(0, Math.min(flat.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)))];
    if (!next) return;
    picked = next.id;
    await tick();
    listEl?.querySelector<HTMLElement>(`[data-id="${next.id}"]`)?.focus();
  }
</script>

<div class="dock" style:display={visible ? "flex" : "none"}>
  <header>
    <Container size={20} />
    <div class="title">
      <h2>{server.name || server.host}</h2>
      {#if listing && (listing.access === "direct" || listing.access === "sudo")}
        <span class="muted">{running} running{listing.access === "sudo" || containers.asRoot[tab.id] ? ", through sudo" : ""}</span>
        {#if lookCount}<span class="badge warn">{lookCount} {lookCount === 1 ? "needs" : "need"} a look</span>{/if}
      {/if}
    </div>
    <span class="grow"></span>
    {#if listing && (listing.access === "direct" || listing.access === "sudo") && listing.compose !== "none"}
      <button class="btn" onclick={() => (newStack = true)}><Plus size={14} /> New stack</button>
    {/if}
    <button class="btn" disabled={tab.status !== "connected" || loading} onclick={() => load()}><RefreshCw size={14} class={loading ? "spin" : ""} /> Refresh</button>
  </header>

  {#if tab.status !== "connected"}
    <div class="note">
      {#if tab.status === "connecting"}
        <p>Connecting to <strong>{server.host}</strong>...</p>
        {#if message}<p class="muted small">{message}</p>{/if}
      {:else}
        <p>{message || "Not connected."}</p>
        <button class="btn primary" onclick={() => app.restart(tab.id)}><RotateCw size={14} /> Connect again</button>
      {/if}
    </div>
  {:else if error}
    <p class="note bad">{error}</p>
  {:else if !listing}
    <p class="note muted">Asking Docker...</p>
  {:else if listing.access === "none"}
    <p class="note muted">Docker is not installed on this server, or it is not on the PATH of {server.username || "this login"}.</p>
  {:else if listing.access === "denied"}
    <p class="note muted">
      Docker is installed, but {server.username || "this login"} may not use it{listing.sudo_possible ? ", and sudo wants a password" : ""}.
      Add the user to the docker group (<code>sudo usermod -aG docker {server.username || "$USER"}</code>, then log in
      again), or log in as root.
    </p>
    {#if listing.sudo_possible}
      <p class="note">
        {#if sudoNote}<span class="bad">{sudoNote}</span><br />{/if}
        <button class="btn" onclick={useSudoPassword}><KeyRound size={14} /> Use sudo with your password</button>
        <span class="muted small">The password comes from the session's vault login, or Cablebag asks for it.</span>
      </p>
    {/if}
  {:else}
    <div class="body">
      <div class="left">
        <div class="tools">
          <div class="find">
            <Search size={14} />
            <input bind:value={filter} placeholder="Find" spellcheck="false" aria-label="Find a container" />
            {#if filter}<button class="icon-btn" aria-label="Clear" onclick={() => (filter = "")}><X size={13} /></button>{/if}
          </div>
          <div class="seg" role="radiogroup" aria-label="Show">
            <button role="radio" aria-checked={show === "all"} class:on={show === "all"} onclick={() => (show = "all")}>All</button>
            <button role="radio" aria-checked={show === "look"} class:on={show === "look"} onclick={() => (show = "look")}>Needs a look{lookCount ? ` (${lookCount})` : ""}</button>
            <button role="radio" aria-checked={show === "stopped"} class:on={show === "stopped"} onclick={() => (show = "stopped")}>Stopped{stoppedCount ? ` (${stoppedCount})` : ""}</button>
          </div>
        </div>
        <div class="list" bind:this={listEl} role="listbox" tabindex="-1" aria-label="Containers" onkeydown={listKey}>
          {#if !groups.length && !idle.length}
            <p class="muted empty">{filter || show !== "all" ? "Nothing here." : "No containers on this server."}</p>
          {/if}
          {#each groups as g (g.key)}
            {#if g.items.length > 1}
              <div class="glabel">{g.project}{#if g.portainer}<span class="tag">Portainer</span>{/if}</div>
            {/if}
            {#each g.items as c (c.id)}
              {@const s = short(c)}
              <div
                class="row"
                class:sub={g.items.length > 1}
                class:sel={c.id === picked}
                data-id={c.id}
                role="option"
                aria-selected={c.id === picked}
                tabindex={c.id === picked ? 0 : -1}
                onclick={() => (picked = c.id)}
                onkeydown={(e) => e.key === "Enter" && (picked = c.id)}
                oncontextmenu={(e) => containerMenu(e, c)}
              >
                <span class="smark {mark(c)}" aria-hidden="true"></span>
                <span class="nm">{c.name}</span>
                <span class="st" class:bad={s.bad}>{s.text}</span>
              </div>
            {/each}
          {/each}
          {#if idle.length}
            <div class="glabel">Not running</div>
            {#each idle as s (s.file)}
              <div
                class="row"
                class:sel={picked === `stack:${s.file}`}
                role="option"
                aria-selected={picked === `stack:${s.file}`}
                tabindex="-1"
                onclick={() => (picked = `stack:${s.file}`)}
                onkeydown={(e) => e.key === "Enter" && (picked = `stack:${s.file}`)}
              >
                <span class="smark" aria-hidden="true"></span>
                <span class="nm">{s.name}</span>
                <span class="st">no containers</span>
              </div>
            {/each}
          {/if}
        </div>
      </div>

      <aside class="right">
        {#if idlePicked}
          {@const s = idlePicked}
          <h3>{s.name}</h3>
          <p class="state">Not running: the stack has no containers now.</p>
          <div class="proj">
            <button class="link file" onclick={() => app.openEditor(tab.id, s.file)} title={s.file}><FileCode size={13} /> Edit {s.file.split("/").pop()}</button>
            <p class="muted small mono path">{s.file}</p>
            {#if busy[`p:${s.name}`]}<p class="muted small">{COMPOSE[busy[`p:${s.name}`]].label}...</p>{/if}
            <div class="acts">
              <button class="btn" disabled={!!busy[`p:${s.name}`] || listing.compose === "none"} onclick={() => composeAction({ project: s.name, working_dir: s.dir, config_files: [s.file] }, "up")}><Play size={14} /> Apply the compose file</button>
            </div>
          </div>
        {:else if current}
          {@const on = current.state === "running"}
          {@const web = current.ports.filter((p) => p.public)}
          <h3>{current.name}</h3>
          <p class="state" class:bad={short(current).bad}>
            {stateText(current)}{current.restarts ? `, restarted ${current.restarts} ${current.restarts === 1 ? "time" : "times"}` : ""}
          </p>
          <div class="acts">
            <button class="btn" onclick={() => containers.logs(server, current, containers.prefix(tab.id, sudo))}><ScrollText size={14} /> Logs</button>
            <button class="btn" disabled={!on} onclick={() => containers.shell(server, current, containers.prefix(tab.id, sudo))}><TerminalSquare size={14} /> Shell</button>
            {#if on}
              <button class="btn" disabled={!!busy[current.id]} onclick={() => containerAction(current, "restart")}><RotateCw size={14} /> Restart</button>
              <button class="btn" disabled={!!busy[current.id]} onclick={() => containerAction(current, "stop")}><Square size={14} /> Stop</button>
            {:else}
              <button class="btn" disabled={!!busy[current.id]} onclick={() => containerAction(current, current.state === "paused" ? "unpause" : "start")}><Play size={14} /> Start</button>
            {/if}
            {#if on}<button class="btn" disabled={!!busy[current.id]} onclick={() => containerAction(current, "pause")}><Pause size={14} /> Pause</button>{/if}
          </div>
          <div class="acts">
            <button class="btn" class:ask={!!question(current)} onclick={() => askAssistant(current)}><Sparkles size={14} /> {question(current) || "Ask the assistant"}</button>
          </div>

          <dl>
            <dt>Image</dt>
            <dd title={current.image}>{imageName(current.image)}</dd>
            {#if web.length}
              <dt>Web</dt>
              <dd class="links">
                {#each web as p}
                  <button class="link" onclick={() => openPort(p.public!, p.private)}>Open port {p.public}{p.proto !== "tcp" ? ` (${p.proto})` : ""}</button>
                {/each}
              </dd>
            {/if}
            {#if current.ports.some((p) => !p.public)}
              <dt>Inside only</dt>
              <dd class="muted">{current.ports.filter((p) => !p.public).map((p) => `${p.private}${p.proto !== "tcp" ? `/${p.proto}` : ""}`).join(", ")}</dd>
            {/if}
          </dl>

          {#if !current.project}
            <div class="proj">
              <p class="muted small">Not started with Compose.</p>
              <button class="btn" onclick={() => saveTemplate(current)}><Layers size={14} /> Save as template</button>
            </div>
          {/if}
          {#if current.project}
            <div class="proj">
              <h4>
                Project {current.project}
                {#if current.portainer}<span class="tag">Portainer</span>{/if}
              </h4>
              {#if project.length > 1}<p class="muted small">{project.filter((c) => c.state === "running").length} of {project.length} containers running</p>{/if}
              {#each current.config_files as f}
                <button class="link file" onclick={() => openFile(current, f)} title={f}>
                  <FileCode size={13} /> {current.portainer ? "View" : "Edit"} {f.split("/").pop()}
                </button>
                {#if !current.portainer}<p class="muted small mono path">{f}</p>{/if}
              {/each}
              {#if current.config_files.length && listing.compose !== "none"}
                <div class="acts"><button class="btn" onclick={() => (moving = current.project)}><Truck size={14} /> Move to another server...</button></div>
              {/if}
              {#if current.portainer}
                <p class="muted small">Made in Portainer: change and redeploy it there.</p>
                <div class="acts">
                  {#if listing.portainer?.url_port}<button class="btn" onclick={openPortainer}><ExternalLink size={14} /> Open in Portainer</button>{/if}
                  <button class="btn" onclick={() => saveTemplate(current)}><Layers size={14} /> Save as template</button>
                </div>
              {:else if current.working_dir && listing.compose !== "none"}
                {#if busy[`p:${current.project}`]}<p class="muted small">{COMPOSE[busy[`p:${current.project}`]].label}...</p>{/if}
                <div class="acts">
                  <button class="btn" disabled={!!busy[`p:${current.project}`]} onclick={() => composeAction(current, "up")}><Play size={14} /> Apply the compose file</button>
                  <button class="btn" disabled={!!busy[`p:${current.project}`]} onclick={() => composeAction(current, "pull")}><Download size={14} /> Pull new images</button>
                  {#if project.length > 1}
                    <button class="btn" disabled={!!busy[`p:${current.project}`]} onclick={() => composeAction(current, "restart")}><RotateCw size={14} /> Restart all</button>
                    <button class="btn" disabled={!!busy[`p:${current.project}`]} onclick={() => composeAction(current, "stop")}><Square size={14} /> Stop all</button>
                  {/if}
                  <button class="btn" onclick={() => saveTemplate(current)}><Layers size={14} /> Save as template</button>
                  <button class="btn danger-text" disabled={!!busy[`p:${current.project}`]} onclick={() => composeAction(current, "down")}><Trash2 size={14} /> Remove stack</button>
                </div>
              {/if}
            </div>
          {/if}
        {:else}
          <p class="muted">Pick a container on the left.</p>
        {/if}
      </aside>
    </div>
  {/if}
</div>

{#if newStack && listing}
  <NewStackDialog
    tab={tab.id}
    compose={listing.compose}
    {sudo}
    onclose={() => (newStack = false)}
    oncreated={async (name, text, ok) => {
      newStack = false;
      output = { title: `Stack ${name}`, text, ok };
      await load(true);
      const c = listing?.containers.find((x) => x.project === name);
      if (c) picked = c.id;
    }}
  />
{/if}

{#if moving && listing}
  <MoveStackDialog
    tab={tab.id}
    {listing}
    project={moving}
    items={listing.containers.filter((c) => c.project === moving)}
    onclose={() => (moving = "")}
    ondone={() => {
      moving = "";
      load(true);
    }}
  />
{/if}

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
      Made in Portainer. Inside Portainer it is <span class="mono">{viewing.path}</span>{#if viewing.onServer}; on the server
      <span class="mono">{viewing.onServer}</span> (only root can change it there){/if}. Change it in Portainer, so Portainer
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
  .dock {
    position: absolute;
    inset: 0;
    flex-direction: column;
    background: var(--bg);
    min-height: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 20px 12px;
    border-bottom: 1px solid var(--border);
  }
  header :global(svg) {
    color: var(--muted);
    flex-shrink: 0;
  }
  .title {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  .grow {
    flex: 1;
  }
  .badge {
    font-size: 12px;
    padding: 1px 8px;
    border-radius: 3px;
  }
  .badge.warn {
    background: color-mix(in srgb, var(--warn) 18%, transparent);
    color: var(--text);
  }
  .note {
    padding: 20px;
    max-width: 640px;
    line-height: 1.55;
  }
  .note p {
    margin: 0 0 8px;
  }
  .bad {
    color: var(--danger-text);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(260px, 1fr) minmax(320px, 1.1fr);
  }
  .left {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--border);
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
  }
  .find {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 140px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--field);
    color: var(--muted);
  }
  .find input {
    flex: 1;
    min-width: 0;
    border: none;
    background: none;
    padding: 5px 0;
  }
  .find input:focus {
    outline: none;
  }
  .seg {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 3px;
    overflow: hidden;
  }
  .seg button {
    padding: 4px 10px;
    border: none;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .seg button + button {
    border-left: 1px solid var(--border);
  }
  .seg button.on {
    background: var(--hover);
    color: var(--text);
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 8px 12px;
  }
  .empty {
    padding: 6px 8px;
  }
  .glabel {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 8px 3px;
    color: var(--muted);
    font-size: 12px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: 3px;
    cursor: default;
  }
  .row.sub {
    padding-left: 22px;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.sel {
    background: var(--selected, var(--hover));
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .row:focus-visible {
    outline: 1px solid var(--accent);
  }
  .nm {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .st {
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
  }
  .st.bad {
    color: var(--danger-text);
  }
  :global(.smark.warn) {
    background: var(--warn);
  }
  .tag {
    font-size: 11px;
    padding: 0 6px;
    border-radius: 3px;
    background: var(--panel-2);
    color: var(--muted);
    font-weight: normal;
  }
  .right {
    overflow: auto;
    padding: 18px 22px;
    min-width: 0;
  }
  h3 {
    margin: 0;
    font-size: 17px;
  }
  .state {
    margin: 2px 0 14px;
    color: var(--muted);
  }
  .state.bad {
    color: var(--danger-text);
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 16px;
  }
  dl {
    display: grid;
    grid-template-columns: 90px minmax(0, 1fr);
    gap: 6px 12px;
    margin: 0 0 16px;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
  }
  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
    text-align: left;
  }
  .link:hover {
    text-decoration: underline;
  }
  .link.file {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .ask {
    border-color: var(--warn);
  }
  .danger-text {
    color: var(--danger-text);
  }
  .proj {
    padding-top: 14px;
    border-top: 1px solid var(--border);
  }
  h4 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 6px;
    font-size: 14px;
  }
  .proj p {
    margin: 2px 0 10px;
  }
  .path {
    word-break: break-all;
  }
  .small {
    font-size: 12px;
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

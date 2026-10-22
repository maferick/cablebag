<script lang="ts">
  import { ask } from "../ask.svelte";
  import { app, newSession } from "../state.svelte";
  import { menu } from "../menu.svelte";
  import { kindInfo } from "../kinds";
  import { reach, reachKey } from "../reach.svelte";
  import { proxmox, isWindows } from "../proxmox.svelte";
  import { writeText } from "../clipboard";
  import { assistant } from "../assistant.svelte";
  import type { ProxmoxGuest, ProxmoxSource } from "../api";
  import { api, type Folder, type Session } from "../api";
  import {
    ChevronRight, Folder as FolderIcon, FolderOpen, Search, Plus, FolderPlus, Settings2, ListChecks, Camera, ListOrdered, Play, Pencil, Copy, Trash2, X, Activity,
    ServerCog, Server, Monitor, MonitorPlay, Boxes, Terminal, Globe, Save, Power, RotateCw, Square, RefreshCw, Settings,
  Shield, ShieldCheck, } from "@lucide/svelte";

  let filter = $state("");
  let open = $state<Record<string, boolean>>(loadOpen());
  let selected = $state<string | null>(null);

  function loadOpen(): Record<string, boolean> {
    try {
      return JSON.parse(localStorage.getItem("tree.open") || "{}");
    } catch {
      return {};
    }
  }

  function toggle(id: string) {
    open[id] = !open[id];
    try {
      localStorage.setItem("tree.open", JSON.stringify(open));
    } catch {}
  }

  const q = $derived(filter.trim().toLowerCase());

  function matches(s: Session) {
    if (!q) return true;
    return [s.name, s.host, s.username, s.comment].some((v) => v?.toLowerCase().includes(q));
  }

  function childFolders(parent: string | null): Folder[] {
    return app.folders.filter((f) => f.parent_id === parent).sort((a, b) => a.name.localeCompare(b.name));
  }

  function childSessions(folder: string | null): Session[] {
    return app.sessions.filter((s) => s.folder_id === folder && matches(s));
  }

  function hasMatch(folder: string): boolean {
    if (childSessions(folder).length) return true;
    return childFolders(folder).some((f) => hasMatch(f.id));
  }

  /** Sessions on screen: the top level plus everything inside open folders. */
  function shown(parent: string | null): Session[] {
    const inside = childFolders(parent).filter((f) => (q ? hasMatch(f.id) : open[f.id]));
    return [...childSessions(parent), ...inside.flatMap((f) => shown(f.id))];
  }

  function allIn(folder: string): Session[] {
    return [...app.sessions.filter((s) => s.folder_id === folder), ...childFolders(folder).flatMap((f) => allIn(f.id))];
  }

  const keysOf = (list: Session[]) => [...new Set(list.map(reachKey).filter((k): k is string => !!k))];

  $effect(() => {
    void app.settings["reach.mode"]; // a new way of checking checks again at once
    if (reach.enabled) reach.show(keysOf(shown(null)));
  });

  // Proxmox servers: live folders of virtual machines and containers.
  const pveKey = (id: string) => `pve:${id}`;

  function guestMatches(g: ProxmoxGuest) {
    if (!q) return true;
    return [g.name, String(g.vmid), g.node, ...g.ips, ...g.tags].some((v) => v.toLowerCase().includes(q));
  }

  $effect(() => {
    proxmox.showOpen(proxmox.sources.filter((s) => open[pveKey(s.id)] || (q && proxmox.listings[s.id])).map((s) => s.id));
  });

  function guestTitle(src: ProxmoxSource, g: ProxmoxGuest): string {
    const what = g.kind === "lxc" ? "Container" : isWindows(g) ? "Windows VM" : "VM";
    const lines = [`${what} ${g.vmid} on ${g.node} (${src.name})`, `State: ${g.status}`];
    if (g.ips.length) lines.push(`Address: ${g.ips.slice(0, 3).join(", ")}`);
    if (g.tags.length) lines.push(`Tags: ${g.tags.join(", ")}`);
    lines.push(g.status !== "running" ? "Double-click: start it" : !g.ips.length ? "Double-click: open the console" : `Double-click: open with ${isWindows(g) ? "RDP" : "SSH"}`);
    return lines.join("\n");
  }

  function guestMenu(e: MouseEvent, src: ProxmoxSource, g: ProxmoxGuest) {
    selected = `${src.id}:${g.node}:${g.vmid}`;
    const running = g.status === "running";
    const open = (kind: "ssh" | "rdp") => {
      const s = proxmox.session(src, g, kind);
      if (s) app.openSession(s);
    };
    const items: any[] = [
      { label: "Open with SSH", icon: Terminal, action: () => open("ssh") },
      { label: "Open with RDP", icon: Monitor, action: () => open("rdp") },
      ...(running ? [{ label: "Open the console", icon: MonitorPlay, action: () => proxmox.openConsole(src, g) }] : []),
      { label: "Open the console in the browser", icon: Globe, action: () => proxmox.openConsoleInBrowser(src, g) },
      {
        label: "Save as a session",
        icon: Save,
        action: () => {
          const s = proxmox.session(src, g, isWindows(g) ? "rdp" : "ssh");
          if (s) app.editing = s;
        },
      },
    ];
    if (g.ips.length) items.push({ label: `Copy address ${g.ips[0]}`, icon: Copy, action: () => writeText(g.ips[0]) });
    items.push(
      { label: "Snapshots", icon: Camera, action: () => (proxmox.dialog = { kind: "snapshots", src, g }) },
      { label: "Tasks of this guest", icon: ListOrdered, action: () => (proxmox.dialog = { kind: "tasks", src, g }) },
    );
    items.push("sep");
    if (running) {
      items.push(
        { label: "Shut down", icon: Power, action: () => proxmox.power(src, g, "shutdown") },
        { label: "Reboot", icon: RotateCw, action: () => proxmox.power(src, g, "reboot") },
        { label: "Force stop", icon: Square, danger: true, action: () => proxmox.power(src, g, "stop") },
      );
    } else {
      items.push({ label: "Start", icon: Play, action: () => proxmox.power(src, g, "start") });
    }
    menu.show(e, items);
  }

  function pveMenu(e: MouseEvent, src: ProxmoxSource) {
    e.stopPropagation();
    menu.show(e, [
      { label: "Refresh", icon: RefreshCw, action: () => proxmox.refresh(src.id) },
      { label: "Open the Proxmox web interface", icon: Globe, action: () => proxmox.openWeb(src, src.url, src.name) },
      ...proxmox.nodes(src).map((n) => ({ label: `Shell on the node ${n}`, icon: Terminal, action: () => proxmox.openNodeShell(src, n) })),
      { label: "Recent tasks", icon: ListOrdered, action: () => (proxmox.dialog = { kind: "tasks", src }) },
      { label: "Edit this Proxmox server", icon: Pencil, action: () => (app.proxmoxEditing = src) },
    ]);
  }

  function liveState(s: Session): string | null {
    const tabs = app.tabs.filter((t) => t.session?.id === s.id && s.id);
    if (!tabs.length) return null;
    if (tabs.some((t) => t.status === "connected")) return "connected";
    if (tabs.some((t) => t.status === "connecting")) return "connecting";
    return "closed";
  }

  function sessionMenu(e: MouseEvent, s: Session) {
    selected = s.id;
    menu.show(e, [
      { label: "Open", icon: Play, action: () => app.openSession(s) },
      { label: "Edit session", icon: Pencil, action: () => app.editSession(s) },
      {
        label: "Duplicate",
        icon: Copy,
        action: async () => {
          const c = JSON.parse(JSON.stringify(s)) as Session;
          c.id = "";
          c.name = `${s.name} (copy)`;
          await app.saveSession(c);
        },
      },
      "sep",
      {
        label: "Delete session",
        icon: Trash2,
        danger: true,
        action: async () => {
          if (await ask.confirm("Delete session", `Delete the session "${s.name}"?`, { okLabel: "Delete", danger: true }))
            await app.deleteSession(s.id);
        },
      },
    ]);
  }

  function folderMenu(e: MouseEvent, f: Folder | null) {
    menu.show(e, [
      {
        label: "New session here",
        icon: Plus,
        action: () => {
          const s = newSession("ssh");
          s.folder_id = f?.id ?? null;
          app.editing = s;
        },
      },
      { label: "New folder", icon: FolderPlus, action: () => newFolder(f?.id ?? null) },
      ...(f ? [] : [{ label: "Add a Proxmox server", icon: ServerCog, action: () => (app.proxmoxEditing = "new") }]),
      ...(f
        ? ([
            {
              label: "Open all sessions in this folder",
              icon: Play,
              action: () => app.sessions.filter((s) => s.folder_id === f.id).forEach((s) => app.openSession(s)),
            },
            {
              label: "Run a command on all its servers",
              icon: ListChecks,
              action: () => {
                app.runManyFolder = f.id;
                app.openTool("runmany", "Run on many servers");
              },
            },
            { label: "Rename folder", icon: Pencil, action: () => renameFolder(f) },
            { label: "Folder settings (user, key, vault login, jump host)", icon: Settings2, action: () => (app.folderEditing = f) },
            ((app.settings["ai.protected"] as string[] | undefined) ?? []).includes(f.id)
              ? { label: "Protected for the assistant (click to lift)", icon: ShieldCheck, action: () => assistant.setProtected(f.id, false) }
              : { label: "Protect from the assistant", icon: Shield, action: () => assistant.setProtected(f.id, true) },
            ...(reach.enabled
              ? [{ label: "Check which hosts answer", icon: Activity, action: () => reach.checkNow(keysOf(allIn(f.id))) }]
              : []),
            "sep",
            {
              label: "Delete folder",
              icon: Trash2,
              danger: true,
              action: async () => {
                const n = countIn(f.id);
                if (await ask.confirm("Delete folder", `Delete the folder "${f.name}"${n ? ` and the ${n} sessions inside it` : ""}?`, { okLabel: "Delete", danger: true })) {
                  await api.deleteFolder(f.id);
                  await app.reloadLibrary();
                }
              },
            },
          ] as const)
        : []),
    ]);
  }

  function countIn(id: string): number {
    return app.sessions.filter((s) => s.folder_id === id).length + childFolders(id).reduce((n, f) => n + countIn(f.id), 0);
  }

  async function newFolder(parent: string | null) {
    const name = await ask.text("New folder", "Name of the new folder", "", "Create");
    if (!name?.trim()) return;
    await api.saveFolder({ id: "", parent_id: parent, name: name.trim(), sort: 0 });
    if (parent) open[parent] = true;
    await app.reloadLibrary();
  }

  async function renameFolder(f: Folder) {
    const name = await ask.text("Rename folder", "New name", f.name, "Rename");
    if (!name?.trim()) return;
    await api.saveFolder({ ...f, name: name.trim() });
    await app.reloadLibrary();
  }

  // Drag a session onto a folder to move it.
  let dragId: string | null = null;
  async function dropOn(folder: string | null) {
    const s = app.sessions.find((x) => x.id === dragId);
    dragId = null;
    if (!s || s.folder_id === folder) return;
    await app.saveSession({ ...$state.snapshot(s), folder_id: folder } as Session);
  }
</script>

<div class="tree-wrap">
  <div class="search">
    <Search size={14} />
    <input bind:value={filter} placeholder="Find a session" />
    {#if filter}
      <button class="icon-btn" title="Clear" onclick={() => (filter = "")}><X size={13} /></button>
    {/if}
  </div>
  <div
    class="tree"
    role="tree"
    tabindex="-1"
    oncontextmenu={(e) => folderMenu(e, null)}
    ondragover={(e) => e.preventDefault()}
    ondrop={() => dropOn(null)}
  >
    {@render level(null, 0)}
    {#if !app.sessions.length}
      <div class="empty">
        <p>No saved sessions yet.</p>
        <button class="btn" onclick={() => app.editSession()}><Plus size={14} /> New session</button>
        <button class="btn" onclick={() => (app.importOpen = true)}>Import sessions</button>
      </div>
    {:else if q && !childSessions(null).length && !childFolders(null).some((f) => hasMatch(f.id))}
      <p class="empty muted">Nothing matches "{filter}".</p>
    {/if}
  </div>
</div>

{#snippet level(parent: string | null, depth: number)}
  {#each childFolders(parent) as f (f.id)}
    {#if !q || hasMatch(f.id)}
      {@const isOpen = !!q || open[f.id]}
      <div
        class="row folder"
        role="treeitem"
        tabindex="0"
        aria-selected="false"
        aria-expanded={isOpen}
        style:padding-left="{6 + depth * 14}px"
        onclick={() => toggle(f.id)}
        onkeydown={(e) => e.key === "Enter" && toggle(f.id)}
        oncontextmenu={(e) => folderMenu(e, f)}
        ondragover={(e) => e.preventDefault()}
        ondrop={(e) => {
          e.stopPropagation();
          dropOn(f.id);
        }}
      >
        <span class="chev" class:open={isOpen}><ChevronRight size={13} /></span>
        {#if isOpen}<FolderOpen size={15} class="ficon" />{:else}<FolderIcon size={15} class="ficon" />{/if}
        <span class="name">{f.name}</span>
        <span class="count">{countIn(f.id)}</span>
      </div>
      {#if isOpen}
        {@render level(f.id, depth + 1)}
      {/if}
    {/if}
  {/each}
  {#if parent === null}
    {@render pveFolders()}
  {/if}
  {#each childSessions(parent) as s (s.id)}
    {@const k = kindInfo(s.kind)}
    {@const live = liveState(s)}
    {@const rk = reach.enabled ? reachKey(s) : null}
    <div
      class="row session {live ? `open ${live}` : ''}"
      class:selected={selected === s.id}
      role="treeitem"
      tabindex="0"
      aria-selected={selected === s.id}
      draggable="true"
      style:padding-left="{24 + depth * 14}px"
      title={`${k.label}${s.host ? ` - ${s.username ? s.username + "@" : ""}${s.host}${s.port && s.port !== k.defaultPort ? ":" + s.port : ""}` : ""}${s.comment ? `\n${s.comment}` : ""}`}
      onclick={() => (selected = s.id)}
      ondblclick={() => app.openSession(s)}
      onkeydown={(e) => e.key === "Enter" && app.openSession(s)}
      oncontextmenu={(e) => sessionMenu(e, s)}
      ondragstart={() => (dragId = s.id)}
    >
      <span class="kicon" class:pending={!k.ready}>
        <k.icon size={14} />
        {#if rk}<span class="reach {reach.results[rk]?.state ?? 'checking'}" title={reach.describe(rk)}></span>{/if}
      </span>
      <span class="name">{s.name || s.host}</span>
    </div>
  {/each}
{/snippet}

{#snippet pveFolders()}
  {#each proxmox.sources as src (src.id)}
    {@const key = pveKey(src.id)}
    {@const l = proxmox.listings[src.id]}
    {@const guests = (l?.guests ?? []).filter(guestMatches)}
    {#if !q || guests.length}
      {@const isOpen = (!!q && guests.length > 0) || !!open[key]}
      <div
        class="row folder"
        role="treeitem"
        tabindex="0"
        aria-selected="false"
        aria-expanded={isOpen}
        style:padding-left="6px"
        title={`Proxmox: ${src.url}`}
        onclick={() => toggle(key)}
        onkeydown={(e) => e.key === "Enter" && toggle(key)}
        oncontextmenu={(e) => pveMenu(e, src)}
      >
        <span class="chev" class:open={isOpen}><ChevronRight size={13} /></span>
        <ServerCog size={15} class="ficon" />
        <span class="name">{src.name}</span>
        {#if l?.loading && !l.guests.length}
          <span class="count">loading</span>
        {:else if l?.guests.length}
          <span class="count" title="Running / all">{l.guests.filter((g) => g.status === "running").length}/{l.guests.length}</span>
        {/if}
      </div>
      {#if isOpen}
        {#if l?.error}
          <div class="row note bad" style:padding-left="38px" title={l.error}>
            <span class="name">{l.error}</span>
            <button class="icon-btn" title="Try again" onclick={() => proxmox.refresh(src.id)}><RefreshCw size={12} /></button>
          </div>
        {/if}
        {#each guests as g (`${g.node}:${g.vmid}`)}
          {@const gid = `${src.id}:${g.node}:${g.vmid}`}
          {@const Icon = g.kind === "lxc" ? Boxes : isWindows(g) ? Monitor : Server}
          <div
            class="row session"
            class:off={g.status !== "running"}
            class:selected={selected === gid}
            role="treeitem"
            tabindex="0"
            aria-selected={selected === gid}
            style:padding-left="38px"
            title={guestTitle(src, g)}
            onclick={() => (selected = gid)}
            ondblclick={() => proxmox.openDefault(src, g)}
            onkeydown={(e) => e.key === "Enter" && proxmox.openDefault(src, g)}
            oncontextmenu={(e) => guestMenu(e, src, g)}
          >
            <span class="kicon">
              <Icon size={14} />
              <span class="reach {g.status === 'running' ? 'ok' : g.status === 'stopped' ? 'down' : 'service'}"></span>
            </span>
            <span class="name">{g.name}</span>
            <span class="count">{g.vmid}</span>
          </div>
        {/each}
      {/if}
    {/if}
  {/each}
{/snippet}

<style>
  .row.off .name {
    color: var(--muted);
  }
  .note {
    font-size: 12px;
    color: var(--muted);
  }
  .note.bad {
    color: var(--danger);
  }
  .tree-wrap {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px;
    padding: 0 4px 0 8px;
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--faint);
  }
  .search input {
    flex: 1;
    border: none;
    background: none;
    box-shadow: none;
    padding: 5px 0;
  }
  .tree {
    flex: 1;
    overflow: auto;
    padding: 0 4px 12px;
    outline: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding-right: 8px;
    border-radius: 3px;
    cursor: default;
    white-space: nowrap;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--selected);
  }
  .folder {
    font-weight: 500;
  }
  .chev {
    display: inline-flex;
    color: var(--faint);
    transition: transform 0.12s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .folder :global(.ficon) {
    color: var(--muted);
    flex-shrink: 0;
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .count {
    font-size: 11px;
    color: var(--faint);
  }
  .kicon {
    position: relative;
    display: inline-flex;
    color: var(--muted);
  }
  .reach {
    position: absolute;
    right: -3px;
    bottom: -2px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    box-shadow: 0 0 0 1.5px var(--panel);
  }
  .reach.ok {
    background: var(--ok);
  }
  .reach.service {
    background: var(--warn);
  }
  .reach.down {
    background: var(--faint);
  }
  .reach.noname {
    background: var(--panel);
    box-shadow: inset 0 0 0 1.5px var(--faint), 0 0 0 1.5px var(--panel);
  }
  .reach.checking {
    display: none;
  }
  .kicon.pending {
    opacity: 0.55;
  }
  /* Open in a tab: a bar at the left edge, so it cannot be mistaken for the "host answers" dot. */
  .row.open {
    position: relative;
  }
  .row.open::before {
    content: "";
    position: absolute;
    left: 0;
    top: 3px;
    bottom: 3px;
    width: 3px;
    border-radius: 0 2px 2px 0;
    background: var(--accent);
  }
  .row.open.connecting::before {
    background: var(--warn);
  }
  .row.open.closed::before {
    background: var(--danger);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 12px;
  }
  .empty p {
    margin: 0;
  }
</style>

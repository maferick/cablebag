<script lang="ts">
  import { browserProfiles } from "../browserProfiles.svelte";
  import { ask } from "../ask.svelte";
  import { treeKeys } from "../keyNav";
  import { app, newSession } from "../state.svelte";
  import { menu } from "../menu.svelte";
  import { kindInfo } from "../kinds";
  import { reach, reachKey } from "../reach.svelte";
  import { proxmox, isWindows } from "../proxmox.svelte";
  import { writeText } from "../clipboard";
  import { assistant } from "../assistant.svelte";
  import { pins } from "../pins.svelte";
  import { webItems } from "../webMenu.svelte";
  import type { ProxmoxGuest, ProxmoxSource } from "../api";
  import { api, type Folder, type Session } from "../api";
  import {
    ChevronRight, Folder as FolderIcon, FolderOpen, Search, Plus, FolderPlus, Settings2, ListChecks, Camera, ListOrdered, Play, Pencil, Copy, Trash2, X, Activity,
    ServerCog, Server, Monitor, MonitorPlay, Boxes, Terminal, Globe, Save, Power, RotateCw, Square, RefreshCw, Settings,
  Shield, ShieldCheck, Pin, Upload, } from "@lucide/svelte";

  let filter = $state("");
  /** Servers: the session tree without folders of only web sessions. Web: the web sessions on their own. */
  let view = $state<"servers" | "web">(stored("tree.view") === "web" ? "web" : "servers");
  /** The web sessions by the folders they are in, or by site. */
  let webBy = $state<"folder" | "site">(stored("tree.webBy") === "site" ? "site" : "folder");

  function stored(k: string): string | null {
    try {
      return localStorage.getItem(k);
    } catch {
      return null;
    }
  }

  function remember(k: string, v: string) {
    try {
      localStorage.setItem(k, v);
    } catch {}
  }
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

  const isWeb = (s: Session) => s.kind === "browser";

  /** Folders with web sessions somewhere inside, and those with nothing else (hidden in the Servers view). */
  const webFolders = $derived.by(() => {
    const some = new Set<string>();
    const only = new Set<string>();
    const walk = (id: string): { web: number; other: number } => {
      let web = 0;
      let other = 0;
      for (const s of app.sessions) if (s.folder_id === id) isWeb(s) ? web++ : other++;
      for (const f of app.folders) {
        if (f.parent_id !== id) continue;
        const c = walk(f.id);
        web += c.web;
        other += c.other;
      }
      if (web) some.add(id);
      if (web && !other) only.add(id);
      return { web, other };
    };
    for (const f of app.folders) if (!f.parent_id) walk(f.id);
    return { some, only };
  });

  /** Folders as the Servers view shows them. */
  function serverFolders(parent: string | null): Folder[] {
    return childFolders(parent).filter((f) => !webFolders.only.has(f.id));
  }

  // ---- The Web view ----

  const webSessions = $derived(app.sessions.filter(isWeb));

  /** Folders with web sessions in them, and the web sessions directly in a folder. */
  const webSubfolders = (parent: string | null) => childFolders(parent).filter((f) => webFolders.some.has(f.id));
  const webIn = (parent: string | null) => webSessions.filter((s) => s.folder_id === parent);

  /** A folder that only holds one other folder is skipped (Chrome's "Bookmarks" and "Bookmarks bar"). */
  function through(f: Folder): Folder {
    let cur = f;
    for (let i = 0; i < 20; i++) {
      const sub = webSubfolders(cur.id);
      if (webIn(cur.id).length || sub.length !== 1) break;
      cur = sub[0];
    }
    return cur;
  }

  /** What the Web view shows at the top: the import's wrapper folder left out, loose sites apart. */
  const webTop = $derived.by(() => {
    let folders = webSubfolders(null).map(through);
    let loose = webIn(null);
    if (folders.length === 1 && !loose.length) {
      const only = folders[0];
      loose = webIn(only.id);
      folders = webSubfolders(only.id).map(through);
    }
    return { folders, loose };
  });

  const webCount = (id: string) => allIn(id).filter(isWeb).length;

  /** The site of an address, without www. */
  function siteOf(s: Session): string {
    try {
      return new URL(s.host.includes("://") ? s.host : `https://${s.host}`).hostname.replace(/^www\./, "").toLowerCase();
    } catch {
      return s.host.toLowerCase();
    }
  }

  /** By site: sites with two or more saved pages get a group; the rest stay together. */
  const bySite = $derived.by(() => {
    const groups = new Map<string, Session[]>();
    for (const s of webSessions) {
      const k = siteOf(s);
      groups.set(k, [...(groups.get(k) ?? []), s]);
    }
    const many = [...groups].filter(([, l]) => l.length > 1).sort(([a], [b]) => a.localeCompare(b));
    const single = [...groups].filter(([, l]) => l.length === 1).flatMap(([, l]) => l).sort((a, b) => (a.name || a.host).localeCompare(b.name || b.host));
    return { many, single };
  });

  /** The sites opened last, apart from the pinned ones. */
  const recent = $derived(
    webSessions
      .filter((s) => s.last_used > 0 && !pins.any(s.id))
      .sort((a, b) => b.last_used - a.last_used)
      .slice(0, 3),
  );

  const webMatches = $derived(q ? webSessions.filter(matches) : []);

  function ago(t: number): string {
    const m = Math.floor((Date.now() / 1000 - t) / 60);
    if (m < 1) return "now";
    if (m < 60) return `${m} min`;
    if (m < 60 * 24) return `${Math.floor(m / 60)} h`;
    return `${Math.floor(m / 60 / 24)} d`;
  }

  function setView(v: "servers" | "web") {
    view = v;
    remember("tree.view", v);
  }

  function setWebBy(v: "folder" | "site") {
    webBy = v;
    remember("tree.webBy", v);
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
      ...(isWeb(s) ? webItems(s) : [{ label: "Open", icon: Play, action: () => app.openSession(s) }]),
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
    <input bind:value={filter} placeholder={view === "web" ? "Find a site" : "Find a session"} aria-label={view === "web" ? "Find a site" : "Find a session"} />
    {#if filter}
      <button class="icon-btn" title="Clear" aria-label="Clear" onclick={() => (filter = "")}><X size={13} /></button>
    {/if}
  </div>
  <div class="views" role="radiogroup" aria-label="Show">
    <button role="radio" aria-checked={view === "servers"} class:on={view === "servers"} onclick={() => setView("servers")}>Servers</button>
    <button role="radio" aria-checked={view === "web"} class:on={view === "web"} onclick={() => setView("web")}>
      Web{#if webSessions.length}<span class="vcount">{webSessions.length}</span>{/if}
    </button>
  </div>
  {#if view === "web"}
    <div class="tree" role="tree" tabindex="-1" onkeydown={treeKeys} oncontextmenu={(e) => e.preventDefault()}>
      {@render web()}
    </div>
  {:else}
  <div
    class="tree"
    role="tree"
    tabindex="-1"
    onkeydown={treeKeys}
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
  {/if}
</div>

{#snippet level(parent: string | null, depth: number)}
  {#each serverFolders(parent) as f (f.id)}
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
        {#if f.data?.browser_profile && f.data.browser_profile !== "none"}
          <span class="ident" title="Web sessions in this folder use the browser profile {browserProfiles.name(f.data.browser_profile)}">
            <span class="idot" style:background={browserProfiles.color(f.data.browser_profile) ?? "var(--faint)"} aria-hidden="true"></span>{browserProfiles.name(f.data.browser_profile)}
          </span>
        {/if}
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
      {#if s.kind === "browser"}
        {@const ident = app.withFolderDefaults(s).data?.browser_profile}
        {#if ident && ident !== "none"}
          <span class="ident" title="Browser profile: {browserProfiles.name(ident)}">
            <span class="idot" style:background={browserProfiles.color(ident) ?? "var(--faint)"} aria-hidden="true"></span>{browserProfiles.name(ident)}
          </span>
        {/if}
      {/if}
    </div>
  {/each}
{/snippet}

{#snippet web()}
  {#if !webSessions.length}
    <div class="empty">
      <p>No web sessions yet. Bring in your bookmarks from Chrome, Edge or Firefox, or add a site.</p>
      <button class="btn" onclick={() => (app.importOpen = true)}><Upload size={14} /> Import bookmarks</button>
      <button class="btn" onclick={() => (app.editing = newSession("browser"))}><Plus size={14} /> New web session</button>
    </div>
  {:else if q}
    {#each webMatches as s (s.id)}
      {@render webRow(s, 0, app.folders.find((f) => f.id === s.folder_id)?.name ?? "")}
    {:else}
      <p class="empty muted">No site matches "{filter}".</p>
    {/each}
  {:else}
    {#if !pins.list.length}
      <p class="hintline"><Pin size={12} /> Right-click a site and choose Pin to keep it on the strip at the left, with its unread count.</p>
    {/if}
    {#if recent.length}
      <div class="cap">Recent</div>
      {#each recent as s (s.id)}
        {@render webRow(s, 0, ago(s.last_used))}
      {/each}
    {/if}
    <div class="cap">
      <span>Sites</span>
      <span class="by" role="radiogroup" aria-label="Group the sites">
        <button role="radio" aria-checked={webBy === "folder"} class:on={webBy === "folder"} onclick={() => setWebBy("folder")}>by folder</button>
        <button role="radio" aria-checked={webBy === "site"} class:on={webBy === "site"} onclick={() => setWebBy("site")}>by site</button>
      </span>
    </div>
    {#if webBy === "folder"}
      {#each webTop.folders as f (f.id)}
        {@render webFolder(f, 0)}
      {/each}
      {#if webTop.folders.length && webTop.loose.length}
        {@render group("web:loose", "Loose bookmarks", webTop.loose, 0)}
      {:else}
        {#each webTop.loose as s (s.id)}
          {@render webRow(s, 0)}
        {/each}
      {/if}
    {:else}
      {#each bySite.many as [site, list] (site)}
        {@render group(`site:${site}`, site, list, 0)}
      {/each}
      {#if bySite.many.length && bySite.single.length}
        {@render group("site:other", "Other sites", bySite.single, 0)}
      {:else}
        {#each bySite.single as s (s.id)}
          {@render webRow(s, 0)}
        {/each}
      {/if}
    {/if}
  {/if}
{/snippet}

{#snippet webFolder(f: Folder, depth: number)}
  {@const key = `web:${f.id}`}
  <div
    class="row folder"
    role="treeitem"
    tabindex="0"
    aria-selected="false"
    aria-expanded={!!open[key]}
    style:padding-left="{6 + depth * 14}px"
    onclick={() => toggle(key)}
    onkeydown={(e) => e.key === "Enter" && toggle(key)}
    oncontextmenu={(e) => folderMenu(e, f)}
    ondragover={(e) => e.preventDefault()}
    ondrop={(e) => {
      e.stopPropagation();
      dropOn(f.id);
    }}
  >
    <span class="chev" class:open={open[key]}><ChevronRight size={13} /></span>
    {#if open[key]}<FolderOpen size={15} class="ficon" />{:else}<FolderIcon size={15} class="ficon" />{/if}
    <span class="name">{f.name}</span>
    <span class="count">{webCount(f.id)}</span>
  </div>
  {#if open[key]}
    {#each webSubfolders(f.id).map(through) as sub (sub.id)}
      {@render webFolder(sub, depth + 1)}
    {/each}
    {#each webIn(f.id) as s (s.id)}
      {@render webRow(s, depth + 1)}
    {/each}
  {/if}
{/snippet}

{#snippet group(key: string, name: string, list: Session[], depth: number)}
  <div
    class="row folder"
    role="treeitem"
    tabindex="0"
    aria-selected="false"
    aria-expanded={!!open[key]}
    style:padding-left="{6 + depth * 14}px"
    onclick={() => toggle(key)}
    onkeydown={(e) => e.key === "Enter" && toggle(key)}
  >
    <span class="chev" class:open={open[key]}><ChevronRight size={13} /></span>
    {#if open[key]}<FolderOpen size={15} class="ficon" />{:else}<FolderIcon size={15} class="ficon" />{/if}
    <span class="name">{name}</span>
    <span class="count">{list.length}</span>
  </div>
  {#if open[key]}
    {#each list as s (s.id)}
      {@render webRow(s, depth + 1)}
    {/each}
  {/if}
{/snippet}

{#snippet webRow(s: Session, depth: number, note = "")}
  {@const live = liveState(s)}
  {@const ident = app.withFolderDefaults(s).data?.browser_profile}
  <div
    class="row session {live ? `open ${live}` : ''}"
    class:selected={selected === s.id}
    role="treeitem"
    tabindex="0"
    aria-selected={selected === s.id}
    draggable="true"
    style:padding-left="{24 + depth * 14}px"
    title={`${s.host}${s.comment ? `\n${s.comment}` : ""}`}
    onclick={() => (selected = s.id)}
    ondblclick={() => app.openSession(s)}
    onkeydown={(e) => e.key === "Enter" && app.openSession(s)}
    oncontextmenu={(e) => sessionMenu(e, s)}
    ondragstart={() => (dragId = s.id)}
  >
    <span class="kicon"><Globe size={14} /></span>
    <span class="name">{s.name || s.host}</span>
    {#if ident && ident !== "none" && browserProfiles.get(ident)}
      <span class="idot" style:background={browserProfiles.color(ident) ?? "var(--faint)"} title="Browser profile: {browserProfiles.name(ident)}"></span>
    {/if}
    {#if pins.any(s.id)}<span class="pinned" title="Pinned to the strip"><Pin size={11} /></span>{/if}
    {#if note}<span class="count note-r">{note}</span>{/if}
  </div>
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
            <button class="icon-btn" title="Try again" aria-label="Try again" onclick={() => proxmox.refresh(src.id)}><RefreshCw size={12} /></button>
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
  .ident {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 1;
    min-width: 0;
    max-width: 45%;
    margin-left: 6px;
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .idot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .views {
    display: grid;
    grid-template-columns: 1fr 1fr;
    margin: 0 8px 6px;
    border: 1px solid var(--border);
    border-radius: 3px;
    overflow: hidden;
  }
  .views button {
    border: none;
    border-radius: 0;
    background: none;
    padding: 3px 0;
    font-size: 12px;
    color: var(--muted);
  }
  .views button.on {
    background: var(--raised);
    color: var(--text);
    font-weight: 600;
  }
  .vcount {
    margin-left: 5px;
    font-weight: 400;
    color: var(--faint);
  }
  .cap {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 10px 6px 3px;
    font-size: 11px;
    color: var(--faint);
  }
  .by {
    display: inline-flex;
    gap: 2px;
  }
  .by button {
    border: none;
    background: none;
    padding: 0 4px;
    font-size: 11px;
    color: var(--faint);
  }
  .by button.on {
    color: var(--text);
    text-decoration: underline;
  }
  .hintline {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    margin: 4px 6px 6px;
    font-size: 11px;
    color: var(--muted);
  }
  .pinned {
    display: inline-flex;
    color: var(--faint);
  }
  .note-r {
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>

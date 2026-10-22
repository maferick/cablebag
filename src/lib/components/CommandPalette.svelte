<script lang="ts">
  import { tick } from "svelte";
  import { tiles } from "../tiles.svelte";
  import { app, SETTINGS_PAGES, type Tab } from "../state.svelte";
  import { kindInfo } from "../kinds";
  import { TOOLS, toolInfo } from "../tools";
  import { macros } from "../macros.svelte";
  import { workspaces } from "../workspaces.svelte";
  import { vault } from "../vault.svelte";
  import { proxmox, isWindows } from "../proxmox.svelte";
  import type { Session } from "../api";
  import {
    Search, Plus, TerminalSquare, X, Copy, RotateCw, Square, Columns2, Rows2, Grid2x2, Radio, Circle, CircleStop,
    PanelLeft, Sun, Moon, Download, LockOpen, Lock, RefreshCw, Settings, Play, AArrowUp, AArrowDown, ArrowRight, House, Info, Keyboard, LayoutGrid, History, Bug,
    Server, Monitor, Boxes, FolderTree, ZoomIn, ZoomOut,
  } from "@lucide/svelte";

  interface Item {
    group: string;
    label: string;
    detail?: string;
    /** Extra words the search also looks at. */
    words?: string;
    keys?: string;
    icon: any;
    run: () => void;
    edit?: () => void;
    /** Sorting within a group when nothing is typed. */
    order?: number;
  }

  const GROUPS = ["Connect", "Open tabs", "Sessions", "Proxmox", "Actions", "Tools", "Local terminals", "Macros", "Settings"];

  let query = $state("");
  let index = $state(0);
  let input = $state<HTMLInputElement>();
  let list = $state<HTMLDivElement>();
  let box = $state<HTMLDivElement>();
  let drop = $state<HTMLDivElement>();
  /** Where the list drops down: under the search box, right edges in line. */
  let at = $state({ top: 40, right: 8 });

  const title = (s: Session) => s.name || `${s.username ? s.username + "@" : ""}${s.host}`;

  function tabItem(t: Tab): Item {
    const icon = t.type === "home" ? House : t.type === "tool" ? (t.tool === "settings" ? Settings : toolInfo(t.tool!).icon) : t.session ? kindInfo(t.session.kind).icon : TerminalSquare;
    const state = t.status === "connected" ? "connected" : t.status === "connecting" ? "connecting" : t.status === "failed" ? "failed" : t.status === "closed" ? "closed" : "";
    return { group: "Open tabs", label: t.title, detail: [t.session?.host, state].filter(Boolean).join(" · "), icon, run: () => (app.activeId = t.id) };
  }

  function sessionItem(s: Session): Item {
    const k = kindInfo(s.kind);
    const folder = app.folderPath(s.folder_id);
    return {
      group: "Sessions",
      label: title(s),
      detail: [k.label, s.host && s.host !== s.name ? `${s.username ? s.username + "@" : ""}${s.host}` : "", folder].filter(Boolean).join(" · "),
      words: `${s.host} ${s.username} ${s.comment} ${k.id}`,
      icon: k.icon,
      run: () => app.openSession(s),
      edit: () => app.editSession(s),
      order: -s.last_used,
    };
  }

  /** Guests of the Proxmox servers whose folder has been loaded. */
  function guests(): Item[] {
    return proxmox.sources.flatMap((src) =>
      (proxmox.listings[src.id]?.guests ?? []).map((g) => ({
        group: "Proxmox",
        label: g.name,
        detail: [src.name, `${g.kind === "lxc" ? "container" : "VM"} ${g.vmid}`, g.status === "running" ? g.ips[0] : g.status].filter(Boolean).join(" · "),
        words: `${g.ips.join(" ")} ${g.tags.join(" ")} ${g.node} ${g.vmid}`,
        icon: g.kind === "lxc" ? Boxes : isWindows(g) ? Monitor : Server,
        run: () => proxmox.openDefault(src, g),
      })),
    );
  }

  function actions(): Item[] {
    const t = app.active;
    const real = t && t.type !== "home";
    const live = t && ["terminal", "rdp", "vnc", "files"].includes(t.type);
    const a: Item[] = [
      { group: "Actions", label: "New session", keys: "Ctrl+Shift+N", icon: Plus, run: () => app.editSession() },
      { group: "Actions", label: "New local terminal", keys: "Ctrl+Shift+T", icon: TerminalSquare, run: () => app.openLocal() },
    ];
    if (real) {
      a.push(
        { group: "Actions", label: "Close this tab", detail: t.title, keys: "Ctrl+Shift+W", icon: X, run: () => app.closeTab(t.id) },
        { group: "Actions", label: "Duplicate this tab", detail: t.title, keys: "Ctrl+Shift+D", icon: Copy, run: () => app.duplicate(t.id) },
      );
    }
    if (live) a.push({ group: "Actions", label: "Reconnect this tab", detail: t.title, icon: RotateCw, run: () => app.restart(t.id) });
    a.push(
      { group: "Actions", label: "Show one terminal", words: "split", icon: Square, run: () => app.setSplit("1") },
      { group: "Actions", label: "Show two terminals side by side", words: "split", icon: Columns2, run: () => app.setSplit("2v") },
      { group: "Actions", label: "Show two terminals above each other", words: "split", icon: Rows2, run: () => app.setSplit("2h") },
      { group: "Actions", label: "Show four terminals", words: "split", icon: Grid2x2, run: () => app.setSplit("4") },
      app.settings["ui.tiling"]
        ? { group: "Actions", label: "Show one tab at a time", words: "split tiling tiles", icon: Square, run: () => app.setLayout("single") }
        : { group: "Actions", label: "Tile all tabs", words: "split tiling tiles panes", icon: Grid2x2, run: () => app.setLayout("tiles") },
      { group: "Actions", label: "Arrange tabs as a grid", words: "split tiling tiles", icon: Grid2x2, run: () => app.setLayout("grid") },
      { group: "Actions", label: "Arrange tabs side by side", words: "split tiling tiles columns", icon: Columns2, run: () => app.setLayout("columns") },
      { group: "Actions", label: "Arrange tabs above each other", words: "split tiling tiles rows", icon: Rows2, run: () => app.setLayout("rows") },
      { group: "Actions", label: "Arrange with this tab as the main pane", words: "split tiling tiles master stack", icon: Columns2, run: () => app.setLayout("main") },
      { group: "Actions", label: "Equal sizes for all panes", words: "split tiling tiles balance even resize", icon: Grid2x2, run: () => tiles.equalize() },
      { group: "Actions", label: "Make this pane bigger", words: "split tiling tiles grow resize larger", icon: Grid2x2, run: () => tiles.grow(app.activeId, 0.05) },
      { group: "Actions", label: "Make this pane smaller", words: "split tiling tiles shrink resize", icon: Grid2x2, run: () => tiles.grow(app.activeId, -0.05) },
      { group: "Actions", label: "Turn the split of this pane", words: "split tiling tiles rotate orientation", icon: Grid2x2, run: () => tiles.rotate(app.activeId) },
    );
    // Send to all: only with two or more connected terminals (or to switch it off).
    if (app.multiExec || app.canSendToAll)
      a.push({
        group: "Actions",
        label: app.multiExec ? "Stop sending to all terminals" : "Send typing to all terminals",
        words: "multi execution broadcast",
        icon: Radio,
        run: () => (app.multiExec = !app.multiExec),
      });
    if (macros.recording) {
      a.push({ group: "Actions", label: "Stop recording and save the macro", icon: CircleStop, run: () => macros.stopRecording() });
    } else if (t?.type === "terminal") {
      a.push({ group: "Actions", label: "Record a macro in this terminal", icon: Circle, run: () => macros.startRecording(t.id) });
    }
    a.push(
      { group: "Actions", label: "Show Home", keys: "Ctrl+Shift+H", words: "start recent", icon: House, run: () => app.showHome() },
      {
        group: "Actions",
        label: app.filesOpen ? "Hide the remote files" : "Show the remote files",
        keys: "Ctrl+Shift+E",
        words: "sftp browser files column",
        icon: FolderTree,
        run: () => (app.filesOpen = !app.filesOpen),
      },
      { group: "Actions", label: "Larger interface", keys: "Ctrl+Shift+plus", words: "zoom size scale text", icon: ZoomIn, run: () => app.setScale(1) },
      { group: "Actions", label: "Smaller interface", keys: "Ctrl+Shift+minus", words: "zoom size scale text", icon: ZoomOut, run: () => app.setScale(-1) },
      {
        group: "Actions",
        label: app.settings["ui.sidebar"] ? "Hide the sidebar" : "Show the sidebar",
        icon: PanelLeft,
        run: () => app.setSetting("ui.sidebar", !app.settings["ui.sidebar"]),
      },
      {
        group: "Actions",
        label: app.dark ? "Switch to light" : "Switch to dark",
        words: "theme colours",
        icon: app.dark ? Sun : Moon,
        run: () => app.setSetting("ui.theme", app.dark ? "light" : "dark"),
      },
      { group: "Actions", label: "Larger terminal text", keys: "Ctrl+plus", words: "font zoom", icon: AArrowUp, run: () => app.setSetting("term.fontSize", Math.min(40, Number(app.settings["term.fontSize"]) + 1)) },
      { group: "Actions", label: "Smaller terminal text", keys: "Ctrl+minus", words: "font zoom", icon: AArrowDown, run: () => app.setSetting("term.fontSize", Math.max(7, Number(app.settings["term.fontSize"]) - 1)) },
      { group: "Actions", label: "Save the open tabs as a workspace", words: "workspace tabs set customer", icon: LayoutGrid, run: () => workspaces.saveCurrent() },
      ...(workspaces.last.length ? [{ group: "Actions", label: `Open the ${workspaces.last.length} tabs from last time`, words: "reopen restore previous tabs", icon: History, run: () => workspaces.openLast() }] : []),
      ...workspaces.list.map((w) => ({ group: "Workspaces", label: w.name, words: `workspace ${w.tabs.map((t) => t.title).join(" ")}`, icon: LayoutGrid, run: () => workspaces.open(w.tabs) })),
      { group: "Actions", label: "Keyboard shortcuts", words: "keys hotkeys ctrl alt tiling", icon: Keyboard, run: () => app.openSettings("keyboard") },
      { group: "Actions", label: "About Cablebag", words: "version licence license update changelog what's new data folder", icon: Info, run: () => app.openSettings("about") },
      { group: "Actions", label: "Report a problem", words: "bug issue error crash help github feedback wrong broken", icon: Bug, run: () => app.openReport() },
      { group: "Actions", label: "Import sessions", words: "mobaxterm putty winscp kitty ssh config rdp ini", icon: Download, run: () => (app.importOpen = true) },
    );
    if (vault.status.configured) {
      if (vault.status.unlocked) {
        a.push(
          { group: "Actions", label: "Sync the vault", words: "vaultwarden bitwarden", icon: RefreshCw, run: () => vault.sync() },
          { group: "Actions", label: "Lock the vault", words: "vaultwarden bitwarden", icon: Lock, run: () => vault.lock() },
        );
      } else {
        a.push({ group: "Actions", label: "Unlock the vault", words: "vaultwarden bitwarden", icon: LockOpen, run: () => vault.unlock() });
      }
    }
    return a;
  }

  /** The few actions shown before anything is typed; the rest answer to typing, or to ">". */
  function quickActions(): Item[] {
    const t = app.active;
    const real = t && t.type !== "home";
    const tiling = !!app.settings["ui.tiling"];
    const a: Item[] = [
      { group: "Actions", label: "New session", keys: "Ctrl+Shift+N", icon: Plus, run: () => app.editSession() },
      { group: "Actions", label: "New local terminal", keys: "Ctrl+Shift+T", icon: TerminalSquare, run: () => app.openLocal() },
    ];
    if (real) {
      a.push(
        { group: "Actions", label: "Close this tab", detail: t.title, keys: "Ctrl+Shift+W", icon: X, run: () => app.closeTab(t.id) },
        { group: "Actions", label: "Duplicate this tab", detail: t.title, keys: "Ctrl+Shift+D", icon: Copy, run: () => app.duplicate(t.id) },
      );
      if (["terminal", "rdp", "vnc", "files", "browser"].includes(t.type)) a.push({ group: "Actions", label: "Reconnect this tab", detail: t.title, icon: RotateCw, run: () => app.restart(t.id) });
    }
    a.push(
      tiling
        ? { group: "Actions", label: "Show one tab at a time", icon: Square, run: () => app.setLayout("single") }
        : { group: "Actions", label: "Tile all tabs", icon: Grid2x2, run: () => app.setLayout("tiles") },
    );
    return a.slice(0, 6);
  }

  function commands(): Item[] {
    return [
      ...actions(),
      ...TOOLS.map((x) => ({ group: "Tools", label: x.name, detail: x.description, icon: x.icon, run: () => app.openTool(x.id, x.name) })),
      ...app.shells.map((s) => ({ group: "Local terminals", label: s.name, icon: TerminalSquare, run: () => app.openLocal(s) })),
      ...macros.list.map((m) => ({ group: "Macros", label: m.name, detail: m.folder, keys: m.hotkey, icon: Play, run: () => macros.run(m) })),
      ...SETTINGS_PAGES.map((p) => ({ group: "Settings", label: `Settings: ${p.label}`, icon: Settings, keys: p.id === "general" ? "Ctrl+," : undefined, run: () => app.openSettings(p.id) })),
    ];
  }

  /** 0 = no match; higher is better. Every typed word has to appear somewhere. */
  function score(it: Item, q: string, words: string[]): number {
    const label = it.label.toLowerCase();
    const all = `${label} ${it.detail ?? ""} ${it.words ?? ""} ${it.group}`.toLowerCase();
    if (!words.every((w) => all.includes(w))) return 0;
    if (label === q) return 5;
    if (label.startsWith(q)) return 4;
    if (label.split(/[\s@.\-_/]+/).some((p) => p.startsWith(words[0]))) return 3;
    if (words.every((w) => label.includes(w))) return 2;
    return 1;
  }

  const looksLikeAddress = (q: string) => /[@.:]/.test(q) || /^(ssh|rdp|vnc|sftp)(\s|:\/\/)/i.test(q);

  const items = $derived.by((): Item[] => {
    const raw = query.trim();
    const onlyCommands = raw.startsWith(">");
    const q = (onlyCommands ? raw.slice(1) : raw).trim().toLowerCase();
    const tabs = app.tabs.filter((t) => t.type !== "home" && t.id !== app.activeId).map(tabItem);

    if (!q) {
      if (onlyCommands) return commands();
      const recent = app.sessions
        .filter((s) => s.last_used > 0)
        .sort((a, b) => b.last_used - a.last_used)
        .slice(0, 8)
        .map(sessionItem);
      return [...tabs, ...recent, ...quickActions()];
    }

    const words = q.split(/\s+/);
    const pool = onlyCommands ? commands() : [...tabs, ...app.sessions.map(sessionItem), ...guests(), ...commands()];
    const found = pool
      .map((it) => ({ it, s: score(it, q, words) }))
      .filter((x) => x.s > 0)
      .sort((a, b) => b.s - a.s || GROUPS.indexOf(a.it.group) - GROUPS.indexOf(b.it.group) || (a.it.order ?? 0) - (b.it.order ?? 0))
      .slice(0, 60)
      .map((x) => x.it);

    if (onlyCommands) return found;
    const connect: Item = {
      group: "Connect",
      label: `Connect to ${raw}`,
      detail: "SSH, or start with rdp, vnc or sftp",
      icon: ArrowRight,
      run: () => app.quickConnect(raw),
    };
    return looksLikeAddress(raw) || !found.length ? [connect, ...found] : [...found, connect];
  });

  /** Headers only when nothing is typed; while searching, each row names its group. */
  const grouped = $derived(!query.trim());

  $effect(() => {
    items;
    index = 0;
  });

  function place() {
    const r = box?.getBoundingClientRect();
    if (r) at = { top: r.bottom + 4, right: Math.max(8, window.innerWidth - r.right) };
  }

  // Ctrl+K (or a click) opens it: the list drops down and the box takes the typing.
  $effect(() => {
    if (app.paletteOpen) {
      place();
      tick().then(() => input?.focus());
    } else {
      query = "";
      input?.blur();
    }
  });

  function close() {
    app.paletteOpen = false;
  }

  function choose(it: Item | undefined, edit = false) {
    if (!it) return;
    close();
    if (edit && it.edit) it.edit();
    else it.run();
  }

  function move(step: number) {
    if (!items.length) return;
    index = (index + step + items.length) % items.length;
    tick().then(() => list?.querySelector(".row.on")?.scrollIntoView({ block: "nearest" }));
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") move(1);
    else if (e.key === "ArrowUp") move(-1);
    else if (e.key === "PageDown") move(8);
    else if (e.key === "PageUp") move(-8);
    else if (e.key === "Enter") choose(items[index], e.shiftKey);
    else if (e.key === "Escape") close();
    else return;
    e.preventDefault();
  }
</script>

<svelte:window
  onresize={place}
  onmousedown={(e) => {
    if (app.paletteOpen && !box?.contains(e.target as Node) && !drop?.contains(e.target as Node)) close();
  }}
/>

<div class="search" class:open={app.paletteOpen} bind:this={box}>
  <Search size={14} />
  <input
    bind:this={input}
    bind:value={query}
    onfocus={() => (app.paletteOpen = true)}
    onkeydown={onKey}
    placeholder="Find, or connect to user@host"
    aria-label="Find a session, tab or command, or connect to user@host"
    spellcheck="false"
    autocomplete="off"
  />
  {#if !app.paletteOpen}<kbd>Ctrl+K</kbd>{/if}
</div>

{#if app.paletteOpen}
  <div class="palette" bind:this={drop} role="dialog" aria-label="Search results" style:top="{at.top}px" style:right="{at.right}px">
    <div class="list" bind:this={list} role="listbox">
      {#each items as it, i (i)}
        {#if grouped && (i === 0 || items[i - 1].group !== it.group)}
          <div class="head">{it.group === "Sessions" ? "Recent sessions" : it.group}</div>
        {/if}
        <button
          class="row"
          class:on={i === index}
          role="option"
          aria-selected={i === index}
          onmousemove={() => (index = i)}
          onclick={(e) => choose(it, e.shiftKey)}
        >
          <span class="icon"><it.icon size={15} /></span>
          <span class="label">{it.label}</span>
          {#if it.detail}<span class="detail">{it.detail}</span>{/if}
          <span class="grow"></span>
          {#if it.keys}<kbd>{it.keys}</kbd>{/if}
          {#if !grouped}<span class="group">{it.group}</span>{/if}
        </button>
      {:else}
        <p class="empty">Nothing matches "{query}".</p>
      {/each}
    </div>
    <div class="foot">
      <span><kbd>Enter</kbd> open</span>
      <span><kbd>Shift+Enter</kbd> edit the session</span>
      <span><kbd>&gt;</kbd> all commands</span>
      <span><kbd>Esc</kbd> close</span>
    </div>
  </div>
{/if}

<style>
  .palette {
    position: fixed;
    z-index: 950;
    width: 660px;
    max-width: calc(100vw - 16px);
    max-height: 72vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--border-strong);
    border-radius: 3px;
    box-shadow: var(--shadow);
    overflow: hidden;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    width: clamp(170px, 24vw, 380px);
    height: 28px;
    padding: 0 6px 0 10px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--bg);
    color: var(--faint);
  }
  .search.open {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: none;
    background: none;
    padding: 0;
    color: var(--text);
    outline: none;
    box-shadow: none;
  }
  .search kbd {
    padding: 0 5px;
  }
  .list {
    overflow: auto;
    padding: 4px 0 6px;
  }
  .head {
    padding: 8px 14px 3px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 14px;
    border: none;
    border-radius: 0;
    background: none;
    text-align: left;
    color: var(--text);
  }
  .row.on {
    background: var(--selected);
  }
  .icon {
    display: flex;
    color: var(--muted);
  }
  .label {
    white-space: nowrap;
  }
  .detail {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
    font-size: 12px;
  }
  .grow {
    flex: 1;
  }
  .group {
    flex-shrink: 0;
    color: var(--faint);
    font-size: 11px;
  }
  kbd {
    flex-shrink: 0;
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--raised);
    color: var(--muted);
    font-family: var(--font-ui);
    font-size: 11px;
  }
  .empty {
    margin: 0;
    padding: 14px;
    color: var(--muted);
  }
  .foot {
    display: flex;
    gap: 16px;
    padding: 7px 14px;
    border-top: 1px solid var(--border);
    background: var(--panel);
    color: var(--faint);
    font-size: 11px;
  }
</style>

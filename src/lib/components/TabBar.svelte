<script lang="ts">
  // The one strip at the top: sidebar toggle, the tabs, the + menu, the search box, Layout and Settings.
  import { TAB_COLORS } from "../tabColors";
  import { tabKeys } from "../keyNav";
  import { app, statusWord, type Tab } from "../state.svelte";
  import { menu, type MenuEntry } from "../menu.svelte";
  import { tiles } from "../tiles.svelte";
  import { workspaces } from "../workspaces.svelte";
  import { kindInfo } from "../kinds";
  import { TOOLS, toolInfo } from "../tools";
  import CommandPalette from "./CommandPalette.svelte";
  import {
    Plus, X, Copy, RotateCw, Pencil, Palette, TerminalSquare, Save, ClipboardCopy, ChevronDown, PanelLeft, Settings, LayoutGrid,
    Square, LayoutDashboard, Grid2x2, Columns2, Rows2, Equal, Radio, Circle, Server,
  } from "@lucide/svelte";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { api } from "../api";
  import { bufferText } from "../terms";
  import { writeText } from "../clipboard";
  import { macros } from "../macros.svelte";

  async function saveOutput(t: Tab) {
    const stamp = new Date().toISOString().slice(0, 16).replace(/[:T]/g, "-");
    const path = await saveDialog({
      title: "Save terminal output",
      defaultPath: `${t.title.replace(/[^\w.@-]+/g, "_")}_${stamp}.txt`,
      filters: [{ name: "Text", extensions: ["txt", "log"] }],
    });
    if (!path) return;
    try {
      await api.writeTextFile(path, bufferText(t.id));
      app.notify(`Saved the output of "${t.title}"`);
    } catch (e) {
      app.notify(`Could not save: ${e}`, "error");
    }
  }

  const COLORS = TAB_COLORS;

  let renaming = $state<string | null>(null);
  let renameText = $state("");

  const shown = $derived(app.tabs.filter((t) => t.type !== "home"));

  function tabMenu(e: MouseEvent, t: Tab) {
    const term = t.type === "terminal";
    menu.show(e, [
      { label: "Rename tab", icon: Pencil, action: () => startRename(t) },
      { label: "Set tab colour", icon: Palette, action: () => colorMenu(e, t) },
      { label: "Duplicate tab", icon: Copy, action: () => app.duplicate(t.id), hint: "Ctrl+Shift+D" },
      { label: "Reconnect", icon: RotateCw, action: () => app.restart(t.id), disabled: t.type === "pending" || t.type === "tool" },
      { label: "Save terminal output", icon: Save, action: () => saveOutput(t), disabled: !term },
      { label: "Copy all terminal output", icon: ClipboardCopy, action: () => writeText(bufferText(t.id)), disabled: !term },
      "sep",
      {
        label: app.multiExec ? "Stop sending to all terminals" : "Send typing to all terminals",
        icon: Radio,
        disabled: !app.multiExec && !app.canSendToAll,
        hint: !app.multiExec && !app.canSendToAll ? "Needs two terminals" : undefined,
        action: () => (app.multiExec = !app.multiExec),
      },
      macros.recording
        ? { label: "Stop recording and save the macro", icon: Circle, action: () => macros.stopRecording() }
        : { label: "Record a macro in this terminal", icon: Circle, disabled: !term, action: () => macros.startRecording(t.id) },
      "sep",
      { label: "Close tab", icon: X, action: () => app.closeTab(t.id), hint: "Ctrl+Shift+W" },
      { label: "Close tabs to the left", action: () => app.closeOthers(t.id, "left") },
      { label: "Close tabs to the right", action: () => app.closeOthers(t.id, "right") },
      { label: "Close all other tabs", action: () => app.closeOthers(t.id, "others") },
      { label: "Close all tabs", danger: true, action: () => app.closeOthers(t.id, "all") },
    ]);
  }

  function colorMenu(e: MouseEvent, t: Tab) {
    setTimeout(() => menu.show(e, COLORS.map((c) => ({ label: c.name, action: () => (t.color = c.value || undefined) }))), 0);
  }

  function startRename(t: Tab) {
    renaming = t.id;
    renameText = t.title;
  }

  /** The + menu: everything new comes from here. */
  function plusMenu(e: MouseEvent, at?: HTMLElement) {
    const items: MenuEntry[] = [
      { label: "New session", icon: Plus, hint: "Ctrl+Shift+N", action: () => app.editSession() },
      { label: "Add a Proxmox server", icon: Server, action: () => (app.proxmoxEditing = "new") },
      "sep",
      ...app.shells.map((s, i) => ({ label: s.name, icon: TerminalSquare, hint: i === 0 ? "Ctrl+Shift+T" : undefined, action: () => app.openLocal(s) })),
      "sep",
      ...TOOLS.map((t) => ({ label: t.name, icon: t.icon, action: () => app.openTool(t.id, t.name) })),
      "sep",
      ...workspaces.list.map((w) => ({ label: `Workspace: ${w.name}`, icon: LayoutGrid, action: () => workspaces.open(w.tabs) })),
      { label: "Save the open tabs as a workspace", icon: LayoutGrid, action: () => workspaces.saveCurrent() },
    ];
    if (at) menu.showAt(at, items);
    else menu.show(e, items);
  }

  function layoutMenu(e: MouseEvent) {
    const tiling = !!app.settings["ui.tiling"];
    menu.showAt(e.currentTarget as HTMLElement, [
      { label: "One tab at a time", icon: Square, hint: !tiling && app.split === "1" ? "now" : undefined, action: () => app.setLayout("single") },
      { label: "Tile all tabs", icon: LayoutDashboard, hint: tiling ? "now" : undefined, action: () => app.setLayout("tiles") },
      { label: "Arrange as a grid", icon: Grid2x2, action: () => app.setLayout("grid") },
      { label: "Arrange side by side", icon: Columns2, action: () => app.setLayout("columns") },
      { label: "Arrange above each other", icon: Rows2, action: () => app.setLayout("rows") },
      { label: "Main pane with the others beside it", icon: PanelLeft, action: () => app.setLayout("main") },
      { label: "Equal sizes for all panes", icon: Equal, hint: "Ctrl+Alt+0", disabled: !tiling, action: () => tiles.equalize() },
      "sep",
      { label: "Two terminals side by side", icon: Columns2, hint: app.split === "2v" ? "now" : undefined, action: () => app.setSplit("2v") },
      { label: "Two terminals above each other", icon: Rows2, hint: app.split === "2h" ? "now" : undefined, action: () => app.setSplit("2h") },
      { label: "Four terminals", icon: Grid2x2, hint: app.split === "4" ? "now" : undefined, action: () => app.setSplit("4") },
    ]);
  }

  function iconOf(t: Tab) {
    if (t.type === "tool") return t.tool === "settings" ? Settings : toolInfo(t.tool!).icon;
    if (t.session) return kindInfo(t.session.kind).icon;
    return TerminalSquare;
  }

  const LayoutIcon = $derived(app.settings["ui.tiling"] ? LayoutDashboard : app.split === "2v" ? Columns2 : app.split === "2h" ? Rows2 : app.split === "4" ? Grid2x2 : Square);
  const layoutName = $derived(app.settings["ui.tiling"] ? "tiled" : app.split !== "1" ? "split" : "one tab at a time");
</script>

<div class="top">
  <button
    class="icon-btn side"
    class:on={app.settings["ui.sidebar"]}
    title={app.settings["ui.sidebar"] ? "Hide the sessions" : "Show the sessions"} aria-label={app.settings["ui.sidebar"] ? "Hide the sessions" : "Show the sessions"}
    onclick={() => app.setSetting("ui.sidebar", !app.settings["ui.sidebar"])}
  >
    <PanelLeft size={16} />
  </button>
  <div class="tabs" role="tablist" tabindex="-1" onkeydown={tabKeys}>
    {#each shown as t (t.id)}
      {@const Icon = iconOf(t)}
      <div
        class="tab"
        class:active={t.id === app.activeId}
        role="tab"
        tabindex="0"
        aria-selected={t.id === app.activeId}
        style:--tabc={t.color ?? "transparent"}
        onclick={() => (app.activeId = t.id)}
        onkeydown={(e) => e.key === "Enter" && (app.activeId = t.id)}
        onauxclick={(e) => e.button === 1 && app.closeTab(t.id)}
        oncontextmenu={(e) => tabMenu(e, t)}
        ondblclick={() => startRename(t)}
        title={t.session ? `${kindInfo(t.session.kind).label} ${t.session.host}` : t.title}
      >
        <span class="kicon">
          <Icon size={14} />
          {#if t.type !== "tool"}<span class="dot smark {t.status}" role="img" aria-label={statusWord(t.status)} title={statusWord(t.status)}></span>{/if}
        </span>
        {#if renaming === t.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="rename"
            bind:value={renameText}
            autofocus
            onblur={() => {
              if (renameText.trim()) t.title = renameText.trim();
              renaming = null;
            }}
            onkeydown={(e) => {
              if (e.key === "Enter") (e.target as HTMLInputElement).blur();
              if (e.key === "Escape") renaming = null;
            }}
          />
        {:else}
          <span class="title">{t.title}</span>{#if t.unread}<span class="badge" title="{t.unread} unread" aria-label="{t.unread} unread">{t.unread > 99 ? "99+" : t.unread}</span>{/if}{#if app.unsaved.has(t.id)}<span class="unsaved" title="Not saved yet"></span>{/if}
        {/if}
        <button
          class="close"
          title="Close tab (Ctrl+Shift+W)"
          onclick={(e) => {
            e.stopPropagation();
            app.closeTab(t.id);
          }}><X size={13} /></button
        >
      </div>
    {/each}
  </div>
  <div class="plus">
    <button class="icon-btn" title="New local terminal (Ctrl+Shift+T); right-click for more" aria-label="New local terminal (Ctrl+Shift+T); right-click for more" onclick={() => app.openLocal()} oncontextmenu={(e) => plusMenu(e)}>
      <Plus size={16} />
    </button>
    <button class="icon-btn drop" title="New session, local terminals and tools" aria-label="New session, local terminals and tools" onclick={(e) => plusMenu(e, e.currentTarget as HTMLElement)}>
      <ChevronDown size={13} />
    </button>
  </div>
  <span class="spacer"></span>
  <CommandPalette />
  <button class="icon-btn" title="Layout: {layoutName}" aria-label="Layout: {layoutName}" onclick={layoutMenu}>
    <LayoutIcon size={16} />
  </button>
  <button class="icon-btn" title="Settings (Ctrl+,)" aria-label="Settings (Ctrl+,)" onclick={() => app.toggleTool("settings", "Settings")}>
    <Settings size={16} />
  </button>
</div>

<style>
  .unsaved {
    width: 7px;
    height: 7px;
    margin-left: 6px;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
  }
  .top {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    min-height: 38px;
    padding: 0 8px 0 6px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
  }
  .top > :global(.icon-btn),
  .plus,
  .top > :global(.search) {
    align-self: center;
  }
  .side.on {
    color: var(--text);
  }
  .tabs {
    display: flex;
    align-items: flex-end;
    gap: 0;
    min-width: 0;
    overflow-x: auto;
    overflow-y: hidden;
    padding-top: 5px;
    scrollbar-width: none;
  }
  .tab {
    position: relative;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 32px;
    padding: 0 6px 0 11px;
    min-width: 90px;
    max-width: 220px;
    border-right: 1px solid var(--border);
    color: var(--muted);
    cursor: default;
    flex-shrink: 0;
  }
  .tab::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 3px;
    background: var(--tabc);
  }
  .tab:hover {
    background: var(--hover);
    color: var(--text);
  }
  .tab.active {
    background: var(--bg);
    color: var(--text);
    box-shadow: 0 1px 0 var(--bg);
  }
  .title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    flex-shrink: 0;
    min-width: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: #c42b1c;
    color: #fff;
    font-size: 10.5px;
    font-weight: 600;
    line-height: 16px;
    text-align: center;
  }
  .rename {
    width: 130px;
    padding: 2px 5px;
  }
  .close {
    display: inline-flex;
    border: none;
    background: none;
    padding: 3px;
    border-radius: 4px;
    color: var(--faint);
    opacity: 0;
  }
  .tab:hover .close,
  .tab.active .close {
    opacity: 1;
  }
  .close:hover {
    background: var(--hover);
    color: var(--text);
  }
  /* The same drawing as a row in the session tree: protocol icon, state dot on its corner. */
  .kicon {
    position: relative;
    display: inline-flex;
    flex-shrink: 0;
    color: var(--muted);
  }
  .tab.active .kicon {
    color: var(--text);
  }
  .dot {
    position: absolute;
    right: -3px;
    bottom: -2px;
    outline: 1.5px solid var(--chrome);
  }
  .dot.connecting {
    outline: none;
  }
  .tab.active .dot {
    outline-color: var(--bg);
  }
  .plus {
    display: flex;
    flex-shrink: 0;
    margin-left: 2px;
  }
  .plus .drop {
    padding-left: 1px;
    padding-right: 1px;
  }
  .spacer {
    flex: 1;
    min-width: 8px;
  }
</style>

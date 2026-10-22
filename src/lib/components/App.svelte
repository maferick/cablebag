<script lang="ts">
  import { workspaces } from "../workspaces.svelte";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import "../theme.css";
  import { app, type Tab, type TabStatus, type Transfer } from "../state.svelte";
  import { tiles, type Divider, type Rect } from "../tiles.svelte";
  import { Maximize2, Minimize2, X, EllipsisVertical, Plus, Minus, Equal, RotateCw, MoveHorizontal, MoveVertical, PanelLeft, Columns2, Rows2, ArrowLeft, ArrowRight, ArrowUp, ArrowDown } from "@lucide/svelte";
  import { menu, type MenuEntry } from "../menu.svelte";
  import type { PromptRequest } from "../api";
  import Sidebar from "./Sidebar.svelte";
  import TabBar from "./TabBar.svelte";
  import StatusBar from "./StatusBar.svelte";
  import FilesColumn from "./FilesColumn.svelte";
  import AssistantColumn from "./AssistantColumn.svelte";
  import { assistant } from "../assistant.svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import TerminalView from "./TerminalView.svelte";
  import HomeView from "./HomeView.svelte";
  import PendingView from "./PendingView.svelte";
  import RdpView from "./RdpView.svelte";
  import ToolView from "./tools/ToolView.svelte";
  import FilesView from "./FilesView.svelte";
  import BrowserView from "./BrowserView.svelte";
  import VncView from "./VncView.svelte";
  import { tunnels } from "../tunnels.svelte";
  import { vault } from "../vault.svelte";
  import { reach } from "../reach.svelte";
  import { proxmox } from "../proxmox.svelte";
  import { sync } from "../sync.svelte";
  import { update } from "../update.svelte";
  import type { TunnelStatus } from "../api";
  import PromptDialog from "./PromptDialog.svelte";
  import SessionEditor from "./SessionEditor.svelte";
  import ImportDialog from "./ImportDialog.svelte";
  import Modal from "./Modal.svelte";
  import ProxmoxSettings from "./ProxmoxSettings.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import AskDialog from "./AskDialog.svelte";
  import { ask } from "../ask.svelte";
  import { macros } from "../macros.svelte";
  import { letter } from "../shortcuts";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let ready = $state(false);

  // Tiling: every tab except Home gets a pane. Home (or a zoomed pane) fills the area on its own.
  const FULL: Rect = { x: 0, y: 0, w: 1, h: 1 };
  const tiling = $derived(!!app.settings["ui.tiling"]);
  const tiled = $derived(app.tiled);
  const layout = $derived(tiles.layout);
  const tilesShown = $derived(tiling && tiled.length > 0 && app.activeId !== "home");
  /** Pane headers, when there is more than one pane to tell apart. */
  const headed = $derived(tilesShown && tiled.length > 1);

  $effect(() => {
    if (tiling) tiles.sync(tiled, app.activeId);
  });

  // A conversation ends with its tab.
  $effect(() => {
    const open = new Set(app.tabs.map((t) => t.id));
    for (const id of Object.keys(assistant.convs)) if (!open.has(id)) assistant.forget(id);
  });

  // Interface size: the whole window zooms, like a browser page; the terminal font has its own size on top.
  $effect(() => {
    const scale = (Number(app.settings["ui.scale"]) || 100) / 100;
    getCurrentWebview().setZoom(scale).catch(() => {});
  });

  // Sync through git: a change here is sent a few seconds later.
  $effect(() => {
    void sync.print();
    if (ready) sync.changed();
  });

  /** The split view's place for a terminal (index in app.panes). */
  function splitPane(i: number): Rect {
    switch (app.split) {
      case "2v":
        return { x: i * 0.5, y: 0, w: 0.5, h: 1 };
      case "2h":
        return { x: 0, y: i * 0.5, w: 1, h: 0.5 };
      case "4":
        return { x: (i % 2) * 0.5, y: Math.floor(i / 2) * 0.5, w: 0.5, h: 0.5 };
      default:
        return FULL;
    }
  }

  function paneOf(t: Tab): Rect | null {
    if (!tilesShown) {
      if (!tiling && app.split !== "1") {
        const i = app.panes.indexOf(t.id);
        return i < 0 ? null : splitPane(i);
      }
      return app.activeId === t.id ? FULL : null;
    }
    if (tiles.zoomed) return tiles.zoomed === t.id ? FULL : null;
    return layout.rects.get(t.id) ?? null;
  }

  const pct = (v: number) => `${(v * 100).toFixed(4)}%`;
  const cellStyle = (r: Rect | null) => (r ? `left:${pct(r.x)};top:${pct(r.y)};width:${pct(r.w)};height:${pct(r.h)}` : "display:none");

  function dividerStyle(d: Divider): string {
    const a = d.area;
    return d.node.dir === "row"
      ? `left:calc(${pct(a.x + a.w * d.node.ratio)} - 3px);top:${pct(a.y)};height:${pct(a.h)}`
      : `top:calc(${pct(a.y + a.h * d.node.ratio)} - 3px);left:${pct(a.x)};width:${pct(a.w)}`;
  }

  let stage: HTMLDivElement;

  function dragDivider(e: PointerEvent, d: Divider) {
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const box = stage.getBoundingClientRect();
    const move = (ev: PointerEvent) => {
      const a = d.area;
      const r = d.node.dir === "row" ? ((ev.clientX - box.left) / box.width - a.x) / a.w : ((ev.clientY - box.top) / box.height - a.y) / a.h;
      tiles.setRatio(d.node, r);
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  }

  // Drag a pane's header onto another pane to swap them.
  let dragging = $state<string | null>(null);
  function dropOn(e: DragEvent, target: string) {
    e.preventDefault();
    if (dragging && dragging !== target) tiles.swap(dragging, target);
    dragging = null;
  }

  /** The right Alt key (AltGr) is held. */
  let altGr = false;

  /** How much one press of bigger or smaller changes a pane, as a share of the area. */
  const STEP = 0.05;

  function movePane(t: string, dir: "left" | "right" | "up" | "down") {
    const n = tiles.neighbour(t, dir);
    if (n) tiles.swap(t, n);
  }

  function paneMenu(t: Tab): MenuEntry[] {
    const many = tiled.length > 1;
    const can = (dir: "left" | "right" | "up" | "down") => !!tiles.neighbour(t.id, dir);
    return [
      { label: "Bigger", icon: Plus, hint: "Ctrl+Alt+=", disabled: !many, action: () => tiles.grow(t.id, STEP) },
      { label: "Smaller", icon: Minus, hint: "Ctrl+Alt+-", disabled: !many, action: () => tiles.grow(t.id, -STEP) },
      { label: "Wider", icon: MoveHorizontal, disabled: !many, action: () => tiles.grow(t.id, STEP, "row") },
      { label: "Narrower", icon: MoveHorizontal, disabled: !many, action: () => tiles.grow(t.id, -STEP, "row") },
      { label: "Taller", icon: MoveVertical, disabled: !many, action: () => tiles.grow(t.id, STEP, "col") },
      { label: "Lower", icon: MoveVertical, disabled: !many, action: () => tiles.grow(t.id, -STEP, "col") },
      { label: "Equal sizes for all panes", icon: Equal, hint: "Ctrl+Alt+0", disabled: !many, action: () => tiles.equalize() },
      "sep",
      { label: "Turn the split", icon: RotateCw, hint: "Ctrl+Alt+R", disabled: !many, action: () => tiles.rotate(t.id) },
      { label: "Make this the main pane", icon: PanelLeft, hint: "Ctrl+Alt+M", disabled: !many, action: () => tiles.makeMain(t.id) },
      { label: "Move left", icon: ArrowLeft, hint: "Ctrl+Alt+Shift+Left", disabled: !can("left"), action: () => movePane(t.id, "left") },
      { label: "Move right", icon: ArrowRight, hint: "Ctrl+Alt+Shift+Right", disabled: !can("right"), action: () => movePane(t.id, "right") },
      { label: "Move up", icon: ArrowUp, hint: "Ctrl+Alt+Shift+Up", disabled: !can("up"), action: () => movePane(t.id, "up") },
      { label: "Move down", icon: ArrowDown, hint: "Ctrl+Alt+Shift+Down", disabled: !can("down"), action: () => movePane(t.id, "down") },
      "sep",
      { label: tiles.next === "row" ? "Next tab opens beside this one (on)" : "Next tab opens beside this one", icon: Columns2, action: () => { app.activeId = t.id; tiles.next = tiles.next === "row" ? null : "row"; } },
      { label: tiles.next === "col" ? "Next tab opens below this one (on)" : "Next tab opens below this one", icon: Rows2, action: () => { app.activeId = t.id; tiles.next = tiles.next === "col" ? null : "col"; } },
      "sep",
      { label: tiles.zoomed === t.id ? "Back to the tiles" : "Fill the area", icon: tiles.zoomed === t.id ? Minimize2 : Maximize2, hint: "Ctrl+Alt+Enter", action: () => tiles.toggleZoom(t.id) },
      { label: "Close", icon: X, danger: true, action: () => app.closeTab(t.id) },
    ];
  }

  /**
   * Tiling keys, all on Ctrl+Alt: arrows move the focus (with Shift the pane), Enter zooms, = and - make the
   * pane bigger or smaller, 0 gives all panes the same size, R turns the split, M makes it the main pane.
   */
  function onTileKey(e: KeyboardEvent) {
    if (e.code === "AltRight") altGr = true;
    // AltGr arrives as Ctrl+Alt and types characters on many keyboards, so it is left alone. Windows reports
    // left Ctrl+Alt as AltGraph too on layouts such as US international, so only the right Alt key counts.
    if (!tilesShown || !e.ctrlKey || !e.altKey || e.metaKey || altGr) return;
    const dir = ({ ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down" } as const)[e.key as "ArrowLeft"];
    const id = app.activeId;
    if (dir) {
      if (e.shiftKey) movePane(id, dir);
      else app.activeId = tiles.neighbour(id, dir) ?? id;
    } else if (e.key === "Enter") {
      tiles.toggleZoom(id);
    } else if (e.code === "Equal" || e.code === "NumpadAdd" || (!e.code && (e.key === "=" || e.key === "+"))) {
      tiles.grow(id, STEP);
    } else if (e.code === "Minus" || e.code === "NumpadSubtract" || (!e.code && e.key === "-")) {
      tiles.grow(id, -STEP);
    } else if (e.code === "Digit0" || e.code === "Numpad0" || (!e.code && e.key === "0")) {
      tiles.equalize();
    } else if (letter(e, "R") && !e.shiftKey) {
      tiles.rotate(id);
    } else if (letter(e, "M") && !e.shiftKey) {
      tiles.makeMain(id);
    } else return;
    // Before the terminal or remote screen sees it.
    e.preventDefault();
    e.stopPropagation();
  }
  /** RDP and VNC panes carry the pane controls in their own bar. */
  const barred = (t: Tab) => t.type === "rdp" || t.type === "vnc";

  let sidebarWidth = $state(Number(localStorage.getItem("sidebar.width")) || 280);

  onMount(() => {
    const ro = new ResizeObserver(([e]) => {
      tiles.stage = { w: e.contentRect.width, h: e.contentRect.height };
      tiles.version++;
    });
    ro.observe(stage);
    return () => ro.disconnect();
  });

  onMount(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    app.systemDark = mq.matches;
    mq.addEventListener("change", (e) => (app.systemDark = e.matches));

    const unlisten = [
      listen<PromptRequest>("prompt", (e) => {
        app.prompts = [...app.prompts, e.payload];
      }),
      listen<{ id: string; state: TabStatus; message: string }>("term-status", (e) => {
        const t = app.tabs.find((x) => x.id === e.payload.id);
        if (!t) return;
        t.status = e.payload.state;
        // A lost SSH connection (not an ordinary end) may connect again by itself.
        if (e.payload.state === "connected") t.lost = undefined;
        else if (e.payload.state === "closed" && t.proto === "ssh" && e.payload.message) t.lost = e.payload.message;
        // Remote files on connecting: the session's own choice, otherwise the one in Settings.
        if (e.payload.state === "connected" && t.proto === "ssh" && !(t.id in app.filesFor)) {
          const own = t.session?.data?.files;
          app.filesFor[t.id] = own === "show" ? true : own === "hide" ? false : app.settings["ssh.autoFiles"] !== false;
        }
      }),
      listen<Transfer>("transfer", (e) => {
        const p = e.payload;
        app.transfers[`${p.tab}:${p.name}`] = p;
      }),
    ];

    // Ask before closing Cablebag while sessions are still connected.
    unlisten.push(
      getCurrentWindow().onCloseRequested(async (e) => {
        const open = app.tabs.filter((t) => t.status === "connected").length;
        if (open && app.settings["ui.confirmClose"] && !(await ask.confirm("Close Cablebag", `${open} session${open === 1 ? " is" : "s are"} still connected. Close Cablebag anyway?`, { okLabel: "Close", danger: true }))) {
          e.preventDefault();
        }
      }),
    );

    unlisten.push(listen<TunnelStatus>("tunnel-status", (e) => tunnels.onStatus(e.payload)));
    unlisten.push(listen("vault-changed", () => vault.refresh()));
    // A session got linked to the vault login it logged in with.
    unlisten.push(listen("library-changed", () => app.reloadLibrary()));
    // A sync brought changes from another PC.
    unlisten.push(
      listen("sync-applied", async () => {
        await app.load();
        await proxmox.load();
      }),
    );

    app
      .load()
      .then(() => tunnels.init())
      .then(() => vault.refresh())
      .then(() => proxmox.load())
      .finally(() => {
        ready = true;
        reach.start();
        proxmox.start();
        sync.start();
        update.start();
        assistant.load();
        workspaces.start();
      });
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });

  $effect(() => {
    document.documentElement.dataset.theme = app.dark ? "dark" : "light";
  });

  // What is open now is what opens again next time.
  $effect(() => {
    void app.tabs.map((t) => `${t.id}${t.title}`).join();
    workspaces.remember();
  });


  function onKey(e: KeyboardEvent) {
    const m = (e.ctrlKey || e.altKey) && macros.byHotkey(e);
    if (m) {
      e.preventDefault();
      macros.run(m);
      return;
    }
    if (!e.ctrlKey) return;
    // Ctrl+K belongs to the shell inside a terminal or remote screen (cut line in nano, kill line in
    // bash), so there only Ctrl+Shift+K opens the palette.
    if (letter(e, "K") && !e.altKey && (e.shiftKey || !(e.target as HTMLElement)?.closest?.(".xterm, .vnc canvas"))) {
      e.preventDefault();
      app.paletteOpen = !app.paletteOpen;
    } else if (e.key === "Tab") {
      e.preventDefault();
      app.cycle(e.shiftKey ? -1 : 1);
    } else if (e.shiftKey && letter(e, "T")) {
      e.preventDefault();
      app.openLocal();
    } else if (e.shiftKey && letter(e, "W")) {
      e.preventDefault();
      app.closeTab(app.activeId);
    } else if (e.shiftKey && letter(e, "D")) {
      e.preventDefault();
      app.duplicate(app.activeId);
    } else if (e.shiftKey && letter(e, "N")) {
      e.preventDefault();
      app.editSession();
    } else if (e.shiftKey && letter(e, "H")) {
      e.preventDefault();
      app.showHome();
    } else if (e.shiftKey && letter(e, "A")) {
      e.preventDefault();
      assistant.open = !assistant.open;
    } else if (e.shiftKey && letter(e, "E")) {
      e.preventDefault();
      app.filesOpen = !app.filesOpen;
    } else if (e.key === "," && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      app.openSettings();
    } else if (e.shiftKey && (e.code === "Equal" || e.code === "NumpadAdd" || e.key === "+")) {
      e.preventDefault();
      app.setScale(1);
    } else if (e.shiftKey && (e.code === "Minus" || e.code === "NumpadSubtract" || e.key === "_")) {
      e.preventDefault();
      app.setScale(-1);
    } else if (e.code === "Equal" || e.code === "NumpadAdd") {
      e.preventDefault();
      app.setSetting("term.fontSize", Math.min(40, Number(app.settings["term.fontSize"]) + 1));
    } else if (e.code === "Minus" || e.code === "NumpadSubtract") {
      e.preventDefault();
      app.setSetting("term.fontSize", Math.max(7, Number(app.settings["term.fontSize"]) - 1));
    }
  }

  function dragSidebar(e: PointerEvent) {
    const startX = e.clientX;
    const start = sidebarWidth;
    const move = (ev: PointerEvent) => (sidebarWidth = Math.max(200, Math.min(600, start + ev.clientX - startX)));
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      localStorage.setItem("sidebar.width", String(sidebarWidth));
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
</script>

<svelte:window onkeydown={onKey} onkeydowncapture={onTileKey} onkeyupcapture={(e) => e.code === "AltRight" && (altGr = false)} onblur={() => (altGr = false)} oncontextmenu={(e) => e.preventDefault()} />

<div class="app">
  <TabBar />
  <div class="main">
    {#if app.settings["ui.sidebar"]}
      <div class="side" style:width="{sidebarWidth}px">
        <Sidebar />
      </div>
      <div class="splitter" role="separator" aria-orientation="vertical" onpointerdown={dragSidebar}></div>
    {/if}
    <div class="work">
      {#if app.multiExec}
        <div class="banner send">
          <span><b>Send to all</b> Typing goes to all {app.tabs.filter((t) => t.type === "terminal" && t.status === "connected").length} connected terminals.</span>
          <button class="btn small" onclick={() => (app.multiExec = false)}>Stop sending to all</button>
        </div>
      {/if}
      {#if macros.recording}
        <div class="banner rec">
          <span><b>Recording</b> Everything you type in this terminal becomes a macro.</span>
          <button class="btn small" onclick={() => macros.stopRecording()}>Stop and save</button>
        </div>
      {/if}
      {#if macros.running}
        <div class="banner">
          <span>Running macro "{macros.running}"...</span>
          <button class="btn small" onclick={() => macros.stop()}>Stop</button>
        </div>
      {/if}
      <div class="stage" bind:this={stage}>
        {#if ready}
          {#each app.tabs as t (t.id)}
            {@const r = paneOf(t)}
            {@const shown = !!r}
            <div
              class="cell"
              class:pane={headed}
              class:focused={headed && app.activeId === t.id}
              class:splitfocus={!tiling && app.split !== "1" && app.activeId === t.id}
              class:target={!!dragging && dragging !== t.id}
              style={cellStyle(r)}
              role="presentation"
              onmousedown={() => (app.activeId = t.id)}
              ondragover={(e) => dragging && e.preventDefault()}
              ondrop={(e) => dropOn(e, t.id)}
            >
              {#if headed && shown && !barred(t)}
                <div
                  class="ph"
                  role="toolbar"
                  tabindex="-1"
                  draggable="true"
                  title="Drag onto another pane to swap them; double-click to zoom"
                  ondragstart={(e) => {
                    dragging = t.id;
                    e.dataTransfer?.setData("text/plain", t.title);
                  }}
                  ondragend={() => (dragging = null)}
                  ondblclick={() => tiles.toggleZoom(t.id)}
                  oncontextmenu={(e) => menu.show(e, paneMenu(t))}
                >
                  <span class="smark {t.status}"></span>
                  <span class="pt">{t.title}</span>
                  <button class="icon-btn" title="Size, place and more (or right-click the header)" onclick={(e) => { e.stopPropagation(); menu.showAt(e.currentTarget as HTMLElement, paneMenu(t)); }}>
                    <EllipsisVertical size={12} />
                  </button>
                  <button class="icon-btn" title={tiles.zoomed === t.id ? "Back to the tiles (Ctrl+Alt+Enter)" : "Fill the area (Ctrl+Alt+Enter)"} onclick={() => tiles.toggleZoom(t.id)}>
                    {#if tiles.zoomed === t.id}<Minimize2 size={12} />{:else}<Maximize2 size={12} />{/if}
                  </button>
                  <button class="icon-btn" title="Close" onclick={() => app.closeTab(t.id)}><X size={13} /></button>
                </div>
              {/if}
              <div class="body" class:headed={headed && shown && !barred(t)}>
                {#if t.type === "home"}
                  {#if shown}<HomeView />{/if}
                {:else if t.type === "terminal"}
                  {#key t.gen}
                    <TerminalView tab={t} visible={shown} />
                  {/key}
                {:else if t.type === "rdp"}
                  {#key t.gen}
                    <RdpView tab={t} visible={shown} pane={headed ? paneLead : undefined} paneEnd={headed ? paneEnd : undefined} />
                  {/key}
                {:else if t.type === "vnc"}
                  {#key t.gen}
                    <VncView tab={t} visible={shown} pane={headed ? paneLead : undefined} paneEnd={headed ? paneEnd : undefined} />
                  {/key}
                {:else if t.type === "browser"}
                  {#key t.gen}
                    <BrowserView tab={t} visible={shown} />
                  {/key}
                {:else if t.type === "files"}
                  {#key t.gen}
                    <FilesView tab={t} visible={shown} />
                  {/key}
                {:else if t.type === "tool"}
                  <ToolView tab={t} visible={shown} />
                {:else if shown}
                  <PendingView tab={t} />
                {/if}
              </div>
            </div>
          {/each}
          {#if tilesShown && !tiles.zoomed}
            {#each layout.dividers as d}
              <div class="divider {d.node.dir}" style={dividerStyle(d)} role="separator" title="Drag to resize; double-click for equal sizes" onpointerdown={(e) => dragDivider(e, d)} ondblclick={() => tiles.equalize(d.node)}></div>
            {/each}
          {/if}
        {/if}
      </div>
    </div>
    <FilesColumn />
    <AssistantColumn />
  </div>
  <StatusBar />
</div>

{#snippet paneLead(t: Tab)}
  <span
    class="lead"
    role="toolbar"
    tabindex="-1"
    draggable="true"
    title="Drag onto another pane to swap them; double-click to zoom"
    ondragstart={(e) => {
      dragging = t.id;
      e.dataTransfer?.setData("text/plain", t.title);
    }}
    ondragend={() => (dragging = null)}
    ondblclick={() => tiles.toggleZoom(t.id)}
    oncontextmenu={(e) => menu.show(e, paneMenu(t))}
  >
    <span class="smark {t.status}"></span>
    <span class="pt">{t.title}</span>
  </span>
{/snippet}

{#snippet paneEnd(t: Tab)}
  <span class="tail">
    <button class="icon-btn" title="Size, place and more (or right-click the name)" onclick={(e) => { e.stopPropagation(); menu.showAt(e.currentTarget as HTMLElement, paneMenu(t)); }}>
      <EllipsisVertical size={12} />
    </button>
    <button class="icon-btn" title={tiles.zoomed === t.id ? "Back to the tiles (Ctrl+Alt+Enter)" : "Fill the area (Ctrl+Alt+Enter)"} onclick={() => tiles.toggleZoom(t.id)}>
      {#if tiles.zoomed === t.id}<Minimize2 size={12} />{:else}<Maximize2 size={12} />{/if}
    </button>
    <button class="icon-btn" title="Close" onclick={() => app.closeTab(t.id)}><X size={13} /></button>
  </span>
{/snippet}

{#if app.editing}
  {#key app.editing}
    <SessionEditor session={app.editing} />
  {/key}
{/if}
{#if app.importOpen}
  <ImportDialog />
{/if}
{#if app.proxmoxEditing}
  {@const p = app.proxmoxEditing}
  <Modal title={p === "new" ? "Add a Proxmox server" : `Edit ${p.name}`} width={700} onclose={() => (app.proxmoxEditing = null)}>
    <ProxmoxSettings only={p} onDone={() => (app.proxmoxEditing = null)} />
  </Modal>
{/if}
{#each app.prompts as p (p.id)}
  <PromptDialog req={p} />
{/each}
<ContextMenu />
{#if ask.current}
  <AskDialog />
{/if}
{#if app.toast}
  <div class="toast" class:error={app.toast.kind === "error"} role="status">{app.toast.text}</div>
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .main {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .side {
    flex-shrink: 0;
    min-height: 0;
  }
  .splitter {
    width: 4px;
    margin-left: -2px;
    cursor: col-resize;
    z-index: 2;
  }
  .splitter:hover {
    background: var(--accent);
  }
  .work {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .cell {
    position: absolute;
  }
  .cell.pane {
    border: 1px solid var(--border);
  }
  .cell.focused {
    border-color: var(--accent);
  }
  .cell.splitfocus::after {
    content: "";
    position: absolute;
    inset: 0;
    border: 1px solid var(--accent);
    pointer-events: none;
    z-index: 3;
  }
  .cell.target {
    outline: 2px dashed var(--accent);
    outline-offset: -4px;
  }
  .body {
    position: absolute;
    inset: 0;
  }
  .body.headed {
    top: 24px;
  }
  .ph {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 24px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 4px 0 8px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    font-size: 12px;
    color: var(--muted);
    cursor: grab;
    user-select: none;
  }
  .cell.focused .ph {
    color: var(--text);
    background: var(--chrome-2);
    box-shadow: inset 0 -2px 0 var(--accent);
  }
  .pt {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ph :global(.icon-btn),
  .tail :global(.icon-btn) {
    padding: 2px;
  }
  .lead {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: grab;
  }
  .tail {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: 4px;
    padding-left: 6px;
    border-left: 1px solid var(--border);
  }
  .divider {
    position: absolute;
    z-index: 4;
  }
  .divider.row {
    width: 6px;
    cursor: col-resize;
  }
  .divider.col {
    height: 6px;
    cursor: row-resize;
  }
  .divider:hover {
    background: var(--border-strong);
  }
  .banner {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 28px;
    padding: 3px 12px;
    background: var(--chrome-2);
    color: var(--text);
    border-bottom: 1px solid var(--border);
  }
  .banner span {
    flex: 1;
  }
  .banner.send {
    background: var(--warn-soft);
  }
  .banner.send b {
    color: var(--warn-text);
    margin-right: 6px;
  }
  .banner.rec {
    background: var(--danger-soft);
  }
  .banner.rec b {
    color: var(--danger-text);
    margin-right: 6px;
  }
  .banner :global(.small) {
    padding: 2px 10px;
    font-size: 12px;
  }
  .toast {
    position: fixed;
    bottom: 18px;
    left: 50%;
    transform: translateX(-50%);
    max-width: 70vw;
    padding: 8px 14px;
    border-radius: 3px;
    border: 1px solid var(--border-strong);
    background: var(--chrome-2);
    color: var(--text);
    box-shadow: var(--shadow);
    z-index: 1100;
    user-select: text;
  }
  .toast.error {
    border-color: var(--danger);
    color: var(--danger-text);
  }
</style>

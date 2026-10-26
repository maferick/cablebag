import { ask } from "./ask.svelte";
import { SvelteSet } from "svelte/reactivity";
import { api, type Folder, type LocalShell, type PromptRequest, type ProxmoxSource, type Session } from "./api";
import { kindInfo } from "./kinds";
import { tiles } from "./tiles.svelte";

export type TabStatus = "idle" | "connecting" | "connected" | "closed" | "failed";

export interface LocalSpec {
  program: string;
  args: string[];
  cwd?: string;
  startup?: string;
}

export interface Tab {
  id: string;
  title: string;
  type: "home" | "terminal" | "rdp" | "vnc" | "files" | "browser" | "tool" | "docker" | "pending";
  proto: "ssh" | "sftp" | "ftp" | "telnet" | "serial" | "winrm" | "local" | "rdp" | "vnc" | "browser" | "none";
  session?: Session;
  local?: LocalSpec;
  status: TabStatus;
  /** For tool tabs: which tool. */
  tool?: string;
  color?: string;
  /** Raised to restart the terminal in the same tab. */
  gen: number;
  /** Raised to connect again inside the same terminal, keeping what it shows. */
  reconnect?: number;
  /** Why the connection was lost, when it was not an ordinary end (for connecting again by itself). */
  lost?: string;
  /** For an editor tab: the remote file, and the tab whose SFTP connection reads and writes it. */
  edit?: { tab: string; path: string };
  /** For a web tab: the unread count its page shows in the title. */
  unread?: number;
  /** For a web tab: the icon of the page it shows (a data URL). */
  icon?: string;
}

export interface Transfer {
  tab: string;
  name: string;
  done: number;
  total: number;
  finished: boolean;
}

export const DEFAULT_SETTINGS: Record<string, any> = {
  "ui.theme": "system",
  "ui.sidebar": true,
  "term.font": "Cascadia Mono",
  "term.fontSize": 14,
  "term.scrollback": 20000,
  "term.cursor": "block",
  "term.cursorBlink": true,
  "term.screenReader": false,
  "term.copyOnSelect": true,
  "term.rightClickPaste": true,
  "term.scheme": "graphite",
  "term.pasteWarning": true,
  "ssh.agent.pageant": false,
  "ssh.agent.openssh": false,
  "ssh.autoFiles": true,
  "ssh.autoReconnect": true,
  "tabs.reopen": "ask",
  "shell.default": "",
  "term.syntax": "keywords",
  "log.enabled": false,
  "log.dir": "",
  "log.kind": "text",
  "log.name": "&S_&H_&Y-&M-&D_&T.log",
  "ui.confirmClose": true,
  "ui.tiling": true,
  /** Size of everything in the window, in percent. */
  "ui.scale": 100,
  "update.auto": true,
  "vault.unlock": "hello",
  "reach.enabled": true,
  "reach.mode": "ping",
  "reach.interval": 120,
  macros: [],
  tunnels: [],
};

/** The interface sizes offered, in percent. */
export const SCALES = [90, 100, 110, 125, 150, 175];

/** A tab's state in words, for status marks that would otherwise say it only by shape and colour. */
export function statusWord(status: string): string {
  return { connected: "Connected", connecting: "Connecting", failed: "Failed", closed: "Closed" }[status] ?? "Not connected";
}

export type SplitMode = "1" | "2v" | "2h" | "4";

/** The pages of Settings, in groups for the side navigation. */
export const SETTINGS_PAGES = [
  { id: "general", label: "General", group: "Cablebag" },
  { id: "terminal", label: "Terminal", group: "Cablebag" },
  { id: "connections", label: "Connections", group: "Cablebag" },
  { id: "browsers", label: "Web tabs", group: "Cablebag" },
  { id: "vault", label: "Credential sources", group: "Logins and servers" },
  { id: "proxmox", label: "Proxmox servers", group: "Logins and servers" },
  { id: "passwords", label: "Passwords on this PC", group: "Logins and servers" },
  { id: "hosts", label: "Trusted servers", group: "Logins and servers" },
  { id: "assistant", label: "Assistant", group: "Assistant" },
  { id: "memory", label: "Assistant memory", group: "Assistant" },
  { id: "ailog", label: "Assistant log", group: "Assistant" },
  { id: "sync", label: "Sync", group: "Elsewhere" },
  { id: "keyboard", label: "Keyboard shortcuts", group: "Elsewhere" },
  { id: "about", label: "About Cablebag", group: "Elsewhere" },
];

/** Pages that were merged into others, so links to them still land in the right place. */
const SETTINGS_MOVED: Record<string, string> = { ssh: "connections", highlighting: "terminal", logging: "terminal" };

const uid = () => crypto.randomUUID();

/** What a folder can give the sessions inside it, and for which kinds of session. */
export const FOLDER_DEFAULTS: { key: string; label: string; kinds: string[] }[] = [
  { key: "username", label: "user name", kinds: ["ssh", "sftp", "rdp", "telnet", "ftp", "winrm"] },
  { key: "key_path", label: "key file", kinds: ["ssh", "sftp"] },
  { key: "vault_item", label: "vault login", kinds: ["ssh", "sftp", "rdp", "vnc", "telnet", "ftp", "winrm"] },
  { key: "jump_hosts", label: "jump hosts", kinds: ["ssh", "sftp"] },
  { key: "wireguard", label: "WireGuard network", kinds: ["ssh", "sftp", "rdp", "vnc", "telnet", "winrm", "browser"] },
  { key: "x11", label: "Linux windows on this PC", kinds: ["ssh"] },
  { key: "psm", label: "CyberArk PSM", kinds: ["ssh", "sftp", "rdp"] },
  { key: "gateway", label: "Remote Desktop Gateway", kinds: ["rdp"] },
  { key: "browser_profile", label: "browser profile", kinds: ["browser"] },
  { key: "tab_color", label: "tab colour", kinds: [] },
];

/** Defaults where "none" in a folder ends what a folder above gives (go directly, no PSM, the shared browser profile). */
const ENDS_WITH_NONE = ["wireguard", "psm", "browser_profile"];

/** A Remote Desktop Gateway counts when it names a server, or says "never" to end the one a folder gives. */
const filled = (v: unknown) =>
  Array.isArray(v)
    ? v.length > 0
    : v && typeof v === "object" && ("host" in v || "usage" in v)
      ? !!(v as any).host?.trim() || (v as any).usage === "never"
      : v !== undefined && v !== null && v !== "";

export function newSession(kind = "ssh"): Session {
  return {
    id: "",
    folder_id: null,
    name: "",
    kind,
    host: "",
    port: kindInfo(kind).defaultPort,
    username: "",
    data: {},
    comment: "",
    sort: 0,
    last_used: 0,
  };
}

class AppState {
  folders = $state<Folder[]>([]);
  sessions = $state<Session[]>([]);
  shells = $state<LocalShell[]>([]);
  settings = $state<Record<string, any>>({ ...DEFAULT_SETTINGS });
  tabs = $state<Tab[]>([{ id: "home", title: "Home", type: "home", proto: "none", status: "idle", gen: 0 }]);
  activeId = $state("home");
  prompts = $state<PromptRequest[]>([]);
  transfers = $state<Record<string, Transfer>>({});
  /** Per SSH tab: whether its remote files show beside the tab area. */
  filesFor = $state<Record<string, boolean>>({});

  /** The remote files of the active tab: shown or not. */
  get filesOpen(): boolean {
    return !!this.filesFor[this.activeId];
  }
  set filesOpen(v: boolean) {
    this.filesFor[this.activeId] = v;
  }
  /** Send to all: what is typed in a terminal goes to every connected terminal. */
  multiExec = $state(false);

  /** The terminals Send to all types into. */
  get liveTerminals(): number {
    return this.tabs.filter((t) => t.type === "terminal" && t.status === "connected").length;
  }

  /** Send to all only means something with two or more connected terminals. */
  get canSendToAll(): boolean {
    return this.liveTerminals >= 2;
  }
  /** Split view: up to four terminals side by side, apart from tiling. */
  split = $state<SplitMode>("1");
  /** Index of the first terminal tab shown when the view is split. */
  paneStart = $state(0);
  editing = $state<Session | null>(null);
  /** The page the Settings tab shows. */
  settingsPage = $state("general");
  importOpen = $state(false);
  /** The "Report a problem" window, with what it starts with (an error message, for instance), or closed. */
  reporting = $state<{ what: string } | null>(null);
  paletteOpen = $state(false);
  /** The Proxmox server dialog: "new", the server being edited, or closed. */
  proxmoxEditing = $state<ProxmoxSource | "new" | null>(null);
  folderEditing = $state<Folder | null>(null);
  /** Tabs with changes that are not saved yet (editors): closing them asks first. */
  unsaved = new SvelteSet<string>();
  /** Another dialog over the tabs (Proxmox snapshots or tasks), so remote screens step aside. */
  /** Dialogs of their own that are open (they hide the web pages and remote screens under them). */
  dialogs = $state(0);
  /** The folder "Run on many servers" starts with (from a folder's menu). */
  runManyFolder = $state<string | null>(null);
  toast = $state<{ text: string; kind: "info" | "error" } | null>(null);
  systemDark = $state(false);

  /** True while something is drawn over the tab area, so native RDP windows must step aside. */
  get overlay(): boolean {
    return !!(this.prompts.length || this.editing || this.importOpen || this.reporting || this.paletteOpen || this.proxmoxEditing || this.folderEditing || this.dialogs > 0 || ask.current);
  }

  get active(): Tab | undefined {
    return this.tabs.find((t) => t.id === this.activeId);
  }

  /** Tabs that get a pane when tiling: everything except Home. */
  get tiled(): string[] {
    return this.tabs.filter((t) => t.type !== "home").map((t) => t.id);
  }

  /** Terminal tabs shown at the same time in the split view, in screen order. */
  get panes(): string[] {
    const n = this.split === "1" ? 1 : this.split === "4" ? 4 : 2;
    const terms = this.tabs.filter((t) => t.type === "terminal");
    const active = terms.findIndex((t) => t.id === this.activeId);
    if (n === 1 || active < 0) return [this.activeId];
    let start = Math.min(this.paneStart, Math.max(0, terms.length - n));
    if (active < start || active >= start + n) start = Math.max(0, Math.min(active, terms.length - n));
    return terms.slice(start, start + n).map((t) => t.id);
  }

  /** Split view and tiling exclude each other: a split switches tiling off. */
  setSplit(mode: SplitMode) {
    const terms = this.tabs.filter((t) => t.type === "terminal");
    this.paneStart = Math.max(0, terms.findIndex((t) => t.id === this.activeId));
    this.split = mode;
    if (mode !== "1" && this.settings["ui.tiling"]) this.setSetting("ui.tiling", false);
  }

  /** One tab at a time, tiled, or tiled in a fresh arrangement. Tiling switches the split view off. */
  setLayout(how: "single" | "tiles" | "grid" | "columns" | "rows" | "main") {
    this.setSetting("ui.tiling", how !== "single");
    if (how !== "single") this.split = "1";
    if (how === "main") tiles.arrange("main", [...this.tiled].sort((a, b) => +(b === this.activeId) - +(a === this.activeId)));
    else if (how === "grid" || how === "columns" || how === "rows") tiles.arrange(how, this.tiled);
    if (how !== "single" && this.activeId === "home" && this.tiled.length) this.activeId = this.tiled[0];
  }

  get dark(): boolean {
    const t = this.settings["ui.theme"];
    return t === "dark" || (t === "system" && this.systemDark);
  }

  async load() {
    const [lib, settings, shells] = await Promise.all([api.library(), api.settings(), api.localShells()]);
    this.folders = lib.folders;
    this.sessions = lib.sessions;
    this.settings = { ...DEFAULT_SETTINGS, ...settings };
    this.shells = shells;
  }

  async reloadLibrary() {
    const lib = await api.library();
    this.folders = lib.folders;
    this.sessions = lib.sessions;
  }

  async setSetting(key: string, value: any) {
    this.settings[key] = value;
    await api.setSetting(key, value);
  }

  /** Report a problem, starting with what happened (an error message) when there is one. */
  openReport(what = "") {
    this.toast = null;
    this.paletteOpen = false;
    this.reporting = { what };
  }

  notify(text: string, kind: "info" | "error" = "info") {
    this.toast = { text, kind };
    const current = this.toast;
    setTimeout(() => {
      if (this.toast === current) this.toast = null;
    }, kind === "error" ? 7000 : 3500);
  }

  defaultShell(): LocalShell | undefined {
    const id = this.settings["shell.default"];
    return this.shells.find((s) => s.id === id) ?? this.shells[0];
  }

  private push(tab: Tab) {
    this.tabs.push(tab);
    this.activeId = tab.id;
  }

  /** At the start: web sessions set to keep running open in the background (Teams, for its notifications). */
  openKeptRunning() {
    const active = this.activeId;
    for (const s of this.sessions.filter((s) => s.kind === "browser" && s.data?.keep_running)) {
      if (!this.tabs.some((t) => t.session?.id === s.id)) this.openSession(s);
    }
    this.activeId = active;
  }

  openLocal(shell?: LocalShell, title?: string, cwd?: string) {
    const sh = shell ?? this.defaultShell();
    if (!sh) {
      this.notify("No local shell was found on this PC.", "error");
      return;
    }
    this.push({
      id: uid(),
      title: title ?? sh.name,
      type: "terminal",
      proto: "local",
      local: { program: sh.program, args: sh.args, cwd },
      status: "connecting",
      gen: 0,
    });
  }

  /** A local terminal as it was saved (last time's tabs, a workspace). */
  openLocalSpec(title: string, local: LocalSpec) {
    this.push({ id: uid(), title, type: "terminal", proto: "local", local: { ...local }, status: "connecting", gen: 0 });
  }

  openSession(s: Session) {
    s = this.withFolderDefaults(s);
    const title = s.name || (s.host ? `${s.username ? s.username + "@" : ""}${s.host}` : kindInfo(s.kind).label);
    const color = s.data?.tab_color || undefined;
    if (s.kind === "ssh") {
      this.push({ id: uid(), title, type: "terminal", proto: "ssh", session: s, status: "connecting", color, gen: 0 });
    } else if (s.kind === "telnet" || s.kind === "serial" || s.kind === "winrm") {
      this.push({ id: uid(), title, type: "terminal", proto: s.kind, session: s, status: "connecting", color, gen: 0 });
    } else if (s.kind === "ftp") {
      this.push({ id: uid(), title, type: "files", proto: "ftp", session: s, status: "connecting", color, gen: 0 });
    } else if (s.kind === "browser" && !s.host && s.data?.target) {
      // Imported by an older Cablebag, which kept the address apart.
      this.openSession({ ...s, host: s.data.target });
    } else if (s.kind === "browser") {
      // Without a colour of its own, the tab shows its browser profile's, so two tenants are told apart at a glance.
      const profile = ((this.settings["browser.profiles"] ?? []) as { id: string; color: string }[]).find((p) => p.id === s.data?.browser_profile);
      this.push({ id: uid(), title: s.name || s.host, type: "browser", proto: "browser", session: s, status: "connecting", color: color ?? (profile?.color || undefined), gen: 0 });
    } else if (s.kind === "file") {
      // A file or folder opens in its own Windows program; links in the web browser.
      const target = (s.host || s.data?.target || s.data?.path || "").trim();
      if (!target) return this.notify("This session has no path.", "error");
      api.fileOpen(target).catch((e) => this.notify(String(e), "error"));
    } else if (s.kind === "vnc") {
      this.push({ id: uid(), title, type: "vnc", proto: "vnc", session: s, status: "connecting", color, gen: 0 });
    } else if (s.kind === "sftp") {
      this.push({ id: uid(), title, type: "files", proto: "sftp", session: s, status: "connecting", color, gen: 0 });
    } else if (s.kind === "rdp") {
      this.push({ id: uid(), title, type: "rdp", proto: "rdp", session: s, status: "connecting", color, gen: 0 });
    } else if (s.kind === "wsl") {
      const distro = s.data?.distro || "";
      const args = distro ? ["-d", distro] : [];
      if (s.username) args.push("-u", s.username);
      args.push("--cd", s.data?.startup_dir || "~");
      this.push({
        id: uid(),
        title,
        type: "terminal",
        proto: "local",
        session: s,
        local: { program: "wsl.exe", args, startup: s.data?.startup_commands },
        status: "connecting",
        color,
        gen: 0,
      });
    } else if (s.kind === "shell") {
      const want = String(s.data?.shell || "").toLowerCase();
      const sh =
        this.shells.find((x) => x.id === want || x.name.toLowerCase() === want) ??
        (want.includes("cmd") ? this.shells.find((x) => x.id === "cmd") : undefined) ??
        (want.includes("power") ? this.shells.find((x) => x.id === "pwsh" || x.id === "powershell") : undefined) ??
        (want.includes("bash") ? this.shells.find((x) => x.id === "gitbash" || x.id.startsWith("wsl:")) : undefined) ??
        this.defaultShell();
      if (!sh) return this.notify("No local shell was found on this PC.", "error");
      this.push({
        id: uid(),
        title,
        type: "terminal",
        proto: "local",
        session: s,
        local: { program: sh.program, args: sh.args, cwd: s.data?.startup_dir, startup: s.data?.startup_commands },
        status: "connecting",
        color,
        gen: 0,
      });
    } else {
      this.push({ id: uid(), title, type: "pending", proto: "none", session: s, status: "idle", color, gen: 0 });
    }
    if (s.id) {
      const known = this.sessions.find((x) => x.id === s.id);
      if (known) known.last_used = Math.floor(Date.now() / 1000);
    }
  }

  /** The containers on an SSH server, in a tab with its own connection; switches to it when it is already open. */
  openContainers(s: Session) {
    s = this.withFolderDefaults(s);
    const open = this.tabs.find((t) => t.type === "docker" && (s.id ? t.session?.id === s.id : t.session?.host === s.host && t.session?.username === s.username));
    if (open) {
      this.activeId = open.id;
      return;
    }
    const name = s.name || s.host;
    this.push({ id: uid(), title: `Containers on ${name}`, type: "docker", proto: "ssh", session: s, status: "connecting", color: s.data?.tab_color || undefined, gen: 0 });
  }

  /** Opens a tool in its own tab, or switches to it when it is already open. */
  openTool(id: string, title: string) {
    const open = this.tabs.find((t) => t.type === "tool" && t.tool === id);
    if (open) {
      this.activeId = open.id;
      return;
    }
    this.push({ id: uid(), title, type: "tool", proto: "none", tool: id, status: "idle", gen: 0 });
  }

  /** Opens Settings in its own tab, at a page. */
  openSettings(page = "general") {
    this.settingsPage = SETTINGS_MOVED[page] ?? page;
    this.openTool("settings", "Settings");
  }

  /** The tab that was active when a tool was opened with its button, to go back to when the button closes it. */
  private toolFrom: Record<string, string> = {};

  /** For the buttons of Settings and Tunnels: the first press opens the tool, a press while it shows closes it. */
  toggleTool(id: string, title: string) {
    const open = this.tabs.find((t) => t.type === "tool" && t.tool === id);
    if (open && this.activeId === open.id) {
      const back = this.toolFrom[id];
      delete this.toolFrom[id];
      this.closeTab(open.id);
      if (back && this.tabs.some((t) => t.id === back)) this.activeId = back;
      return;
    }
    this.toolFrom[id] = this.activeId;
    if (id === "settings") this.openSettings();
    else this.openTool(id, title);
  }

  /** Interface size one step up (1) or down (-1). */
  setScale(step: number) {
    const cur = Number(this.settings["ui.scale"]) || 100;
    const i = SCALES.findIndex((v) => v >= cur);
    const next = SCALES[Math.min(SCALES.length - 1, Math.max(0, (i < 0 ? SCALES.length - 1 : i) + step))];
    this.setSetting("ui.scale", next);
  }

  /** Home shows when no tab is open; Ctrl+Shift+H or the palette bring it up any time. */
  showHome() {
    this.activeId = "home";
  }

  /** Parses the quick connect box: "user@host:port", "ssh host", "rdp host", a shell name... */
  quickConnect(text: string) {
    const t = text.trim();
    if (!t) return;
    const lower = t.toLowerCase();
    const shell = this.shells.find((s) => s.name.toLowerCase() === lower || s.id === lower);
    if (shell) return this.openLocal(shell);
    const saved = this.sessions.find((s) => s.name.toLowerCase() === lower);
    if (saved) return this.openSession(saved);
    let kind = "ssh";
    let rest = t;
    const m = t.match(/^(ssh|rdp|vnc|telnet|sftp|ftp|winrm)(:\/\/|\s+)(.+)$/i);
    if (m) {
      kind = m[1].toLowerCase();
      rest = m[3].trim();
    }
    const s = newSession(kind);
    const at = rest.lastIndexOf("@");
    if (at > 0) {
      s.username = rest.slice(0, at);
      rest = rest.slice(at + 1);
    }
    const pm = rest.match(/^\[?([^\]]+?)\]?(?::(\d+))?$/);
    s.host = pm ? pm[1] : rest;
    if (pm?.[2]) s.port = Number(pm[2]);
    this.openSession(s);
  }

  /** A remote file in Cablebag's editor (one tab per file). */
  openEditor(sourceTab: string, path: string) {
    const open = this.tabs.find((t) => t.edit?.tab === sourceTab && t.edit.path === path);
    if (open) {
      this.activeId = open.id;
      return;
    }
    const src = this.tabs.find((t) => t.id === sourceTab);
    this.push({ id: uid(), title: path.split("/").pop() || path, type: "tool", tool: "editor", proto: "none", edit: { tab: sourceTab, path }, status: "idle", color: src?.color, gen: 0 });
  }

  closeTab(id: string) {
    if (this.unsaved.has(id)) {
      const t = this.tabs.find((x) => x.id === id);
      ask
        .confirm("Close without saving", `${t?.title ?? "This file"} has changes that are not saved. Close it anyway?`, { okLabel: "Close without saving", danger: true })
        .then((ok) => {
          if (ok) {
            this.unsaved.delete(id);
            this.closeTab(id);
          }
        });
      return;
    }
    const i = this.tabs.findIndex((t) => t.id === id);
    if (i < 0 || this.tabs[i].type === "home") return;
    const tab = this.tabs[i];
    if (tab.type === "terminal" || tab.type === "files" || tab.type === "docker") api.close(id).catch(() => {});
    if (tab.type === "rdp") api.rdpClose(id).catch(() => {});
    if (tab.type === "browser") api.browserClose(id).catch(() => {});
    this.tabs.splice(i, 1);
    delete this.filesFor[id];
    if (this.activeId === id) {
      const next = this.tabs[Math.min(i, this.tabs.length - 1)];
      this.activeId = next?.id ?? "home";
    }
  }

  closeOthers(id: string, which: "others" | "left" | "right" | "all") {
    const i = this.tabs.findIndex((t) => t.id === id);
    const victims = this.tabs.filter((t, j) => {
      if (t.type === "home") return false;
      if (which === "all") return true;
      if (which === "others") return t.id !== id;
      if (which === "left") return j < i;
      return j > i;
    });
    victims.forEach((t) => this.closeTab(t.id));
  }

  duplicate(id: string) {
    const t = this.tabs.find((x) => x.id === id);
    if (!t || t.type === "home") return;
    const copy: Tab = { ...$state.snapshot(t), id: uid(), status: "connecting", gen: 0 } as Tab;
    const i = this.tabs.findIndex((x) => x.id === id);
    this.tabs.splice(i + 1, 0, copy);
    this.activeId = copy.id;
  }

  restart(id: string) {
    const t = this.tabs.find((x) => x.id === id);
    if (!t) return;
    // A terminal that has ended connects again in place, with its scrollback.
    if (t.type === "terminal" && (t.status === "closed" || t.status === "failed")) {
      t.status = "connecting";
      t.reconnect = (t.reconnect ?? 0) + 1;
      return;
    }
    if (t.type === "rdp") api.rdpClose(id).catch(() => {});
    else if (t.type !== "vnc") api.close(id).catch(() => {});
    t.status = "connecting";
    t.gen += 1;
  }

  /** Ctrl+Tab: through the open tabs; Home only counts when nothing else is open. */
  cycle(step: number) {
    const list = this.tabs.filter((t) => t.type !== "home");
    if (!list.length) return;
    const i = list.findIndex((t) => t.id === this.activeId);
    const n = list.length;
    this.activeId = list[i < 0 ? (step > 0 ? 0 : n - 1) : (i + step + n) % n].id;
  }

  async saveSession(s: Session) {
    const saved = await api.saveSession(s);
    await this.reloadLibrary();
    return saved;
  }

  async deleteSession(id: string) {
    await api.deleteSession(id);
    await this.reloadLibrary();
  }

  editSession(s?: Session) {
    this.editing = s ? (JSON.parse(JSON.stringify(s)) as Session) : newSession("ssh");
  }

  /** Whether a folder is the given folder or lies inside it. */
  inFolder(id: string | null, ancestor: string): boolean {
    let cur = this.folders.find((f) => f.id === id);
    while (cur) {
      if (cur.id === ancestor) return true;
      cur = this.folders.find((f) => f.id === cur!.parent_id);
    }
    return false;
  }

  /** The defaults a folder gives (its own over those of the folders above it), and which folder gives each. */
  folderDefaults(id: string | null): { values: Record<string, any>; from: Record<string, string> } {
    const chain: Folder[] = [];
    let cur = this.folders.find((f) => f.id === id);
    while (cur) {
      chain.unshift(cur);
      cur = this.folders.find((f) => f.id === cur!.parent_id);
    }
    const values: Record<string, any> = {};
    const from: Record<string, string> = {};
    for (const f of chain) {
      for (const { key } of FOLDER_DEFAULTS) {
        if (ENDS_WITH_NONE.includes(key) && f.data?.[key] === "none") {
          // A folder that goes directly ends the network (or PSM, or browser profile) a folder above gives.
          delete values[key];
          delete from[key];
        } else if (filled(f.data?.[key])) {
          values[key] = f.data![key];
          from[key] = f.name;
        }
      }
    }
    return { values, from };
  }

  /** The session as it connects: its own values, and its folders' defaults where it has none. */
  withFolderDefaults(s: Session): Session {
    const { values } = this.folderDefaults(s.folder_id);
    if (!Object.keys(values).length && s.data?.gateway?.usage !== "never") return s;
    const out: Session = { ...s, data: { ...(s.data ?? {}) } };
    for (const d of FOLDER_DEFAULTS) {
      if (!(d.key in values) || (d.kinds.length && !d.kinds.includes(s.kind))) continue;
      if (d.key === "username") {
        if (!out.username) out.username = values.username;
      } else if (!filled(out.data[d.key])) {
        out.data[d.key] = values[d.key];
      }
    }
    if (out.data.gateway?.usage === "never") delete out.data.gateway;
    return out;
  }

  folderPath(id: string | null): string {
    const parts: string[] = [];
    let cur = this.folders.find((f) => f.id === id);
    while (cur) {
      parts.unshift(cur.name);
      cur = this.folders.find((f) => f.id === cur!.parent_id);
    }
    return parts.join(" / ");
  }
}

export const app = new AppState();

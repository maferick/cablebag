// Tabs that can be opened again: the ones that were open when Cablebag closed last time (kept on this PC only),
// and named workspaces, sets of tabs saved by name that travel with Sync.
import { app, type Tab, type LocalSpec } from "./state.svelte";
import type { Session } from "./api";
import { ask } from "./ask.svelte";

export type TabSpec =
  | { kind: "session"; title: string; session: Session }
  | { kind: "local"; title: string; local: LocalSpec }
  | { kind: "tool"; title: string; tool: string };

export interface Workspace {
  id: string;
  name: string;
  tabs: TabSpec[];
}

class Workspaces {
  /** The tabs that were open last time, as read at the start (emptied once they are opened). */
  last = $state<TabSpec[]>([]);
  private ready = false;
  private timer: ReturnType<typeof setTimeout> | undefined;

  get list(): Workspace[] {
    return (app.settings["workspaces"] as Workspace[]) ?? [];
  }

  /** How a tab can be opened again; Home and tabs without a lasting form give null. */
  specOf(t: Tab): TabSpec | null {
    if (t.session) return { kind: "session", title: t.title, session: $state.snapshot(t.session) as Session };
    if (t.local) return { kind: "local", title: t.title, local: $state.snapshot(t.local) as LocalSpec };
    if (t.type === "tool" && t.tool && t.tool !== "editor") return { kind: "tool", title: t.title, tool: t.tool };
    return null;
  }

  current(): TabSpec[] {
    return app.tabs.map((t) => this.specOf(t)).filter((s): s is TabSpec => !!s);
  }

  /** Opens the tabs next to the ones already open. A saved session opens as it is now in the tree. */
  open(specs: TabSpec[]) {
    for (const s of specs) {
      if (s.kind === "session") {
        const now = s.session.id ? app.sessions.find((x) => x.id === s.session.id) : undefined;
        // A web session that keeps running is open already (it opened by itself at the start).
        if (now?.data?.keep_running && app.tabs.some((t) => t.session?.id === now.id)) continue;
        app.openSession(now ?? s.session);
      } else if (s.kind === "local") {
        app.openLocalSpec(s.title, s.local);
      } else {
        app.openTool(s.tool, s.title);
      }
    }
  }

  /** At the start: keep last time's tabs, and open them straight away when Settings says so. */
  start() {
    this.last = ((app.settings["tabs.last"] as TabSpec[]) ?? []).filter((s) => s && s.kind);
    this.ready = true;
    if (app.settings["tabs.reopen"] === "always" && this.last.length) this.openLast();
  }

  openLast() {
    const specs = this.last;
    this.last = [];
    this.open(specs);
  }

  /** After every change of the open tabs: what to open again next time (written a moment later). */
  remember() {
    if (!this.ready) return;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => app.setSetting("tabs.last", this.current()), 1000);
  }

  async saveCurrent() {
    const tabs = this.current();
    if (!tabs.length) return app.notify("Open the tabs for the workspace first.", "error");
    const name = (await ask.text("Save as a workspace", `A name for these ${tabs.length} tabs`, "", "Save"))?.trim();
    if (!name) return;
    const same = this.list.find((w) => w.name.toLowerCase() === name.toLowerCase());
    if (same && !(await ask.confirm("Replace the workspace", `A workspace called ${same.name} exists. Replace its tabs with these?`, { okLabel: "Replace" }))) return;
    const list = same ? this.list.map((w) => (w.id === same.id ? { ...w, tabs } : w)) : [...this.list, { id: crypto.randomUUID(), name, tabs }];
    await app.setSetting("workspaces", list);
    app.notify(`Workspace ${name} saved`);
  }

  async remove(w: Workspace) {
    if (!(await ask.confirm("Remove the workspace", `Remove the workspace ${w.name}? Its sessions stay in the tree.`, { okLabel: "Remove", danger: true }))) return;
    await app.setSetting("workspaces", this.list.filter((x) => x.id !== w.id));
  }
}

export const workspaces = new Workspaces();

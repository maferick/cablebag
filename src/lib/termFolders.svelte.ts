// The folder each SSH tab's terminal is in, for remote files that follow it. Learnt without typing anything into
// the terminal: from the shell when it reports its folder itself (OSC 7), otherwise by asking the server on a
// channel of its own after Enter was pressed.
import { api } from "./api";
import { app } from "./state.svelte";

class TermFolders {
  /** The last folder seen per tab. */
  dirs = $state<Record<string, string>>({});
  /** Tabs whose server cannot tell (not Linux), so nobody asks again. */
  unknown = $state<Record<string, true>>({});
  private reports = new Set<string>();
  private timers = new Map<string, ReturnType<typeof setTimeout>>();
  private busy = new Set<string>();

  get follow(): boolean {
    return app.settings["files.follow"] !== false;
  }

  setFollow(on: boolean) {
    app.setSetting("files.follow", on);
  }

  /** Everything typed into a terminal comes here: after Enter the folder may have changed. */
  typed(tab: string, data: string) {
    if (!data.includes("\r") || !this.wanted(tab)) return;
    clearTimeout(this.timers.get(tab));
    this.timers.set(tab, setTimeout(() => this.check(tab), 500));
  }

  /** OSC 7 from the shell: file://host/path, with the path URL-encoded. */
  reported(tab: string, data: string) {
    const m = /^file:\/\/[^/]*(\/.*)$/.exec(data);
    if (!m) return;
    let path = m[1];
    try {
      path = decodeURIComponent(path);
    } catch {
      /* keep it as sent */
    }
    this.reports.add(tab);
    this.dirs[tab] = path;
  }

  /** Asks the server where the tab's terminal is, unless the shell reports it or the server cannot tell. */
  async check(tab: string) {
    if (this.reports.has(tab) || this.unknown[tab] || this.busy.has(tab)) return;
    const t = app.tabs.find((x) => x.id === tab);
    if (t?.proto !== "ssh" || t.status !== "connected") return;
    this.busy.add(tab);
    try {
      const dir = await api.terminalFolder(tab);
      if (dir === null) this.unknown[tab] = true;
      else this.dirs[tab] = dir;
    } catch {
      /* not connected (any more); the next Enter asks again */
    } finally {
      this.busy.delete(tab);
    }
  }

  private wanted(tab: string): boolean {
    return this.follow && app.filesOpen && app.active?.id === tab && app.active.proto === "ssh";
  }
}

export const termFolders = new TermFolders();

/** A path as one word for a POSIX shell. */
export function shellQuote(path: string): string {
  return /^[\w./~-]+$/.test(path) ? path : `'${path.replace(/'/g, `'\\''`)}'`;
}

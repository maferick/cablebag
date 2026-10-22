// Updates come only from the official GitHub releases, signed with the Cablebag key (see updater.rs).
import { listen } from "@tauri-apps/api/event";
import { api, type UpdateInfo } from "./api";
import { app } from "./state.svelte";
import { ask } from "./ask.svelte";

class UpdateStore {
  current = $state("");
  /** The newer version found, if any. */
  found = $state<UpdateInfo | null>(null);
  checking = $state(false);
  installing = $state(false);
  progress = $state(0);
  lastChecked = $state(0);
  error = $state("");
  private started = false;

  async start() {
    if (this.started) return;
    this.started = true;
    this.current = await api.appVersion().catch(() => "");
    listen<{ done: number; total: number }>("update-progress", (e) => {
      this.progress = e.payload.total ? Math.round((e.payload.done / e.payload.total) * 100) : 0;
    });
    // A short while after the start, then every six hours.
    setTimeout(() => this.auto(), 15_000);
    setInterval(() => this.auto(), 6 * 60 * 60 * 1000);
  }

  private auto() {
    if (app.settings["update.auto"] !== false) this.check(true);
  }

  async check(quiet = false) {
    if (this.checking || this.installing) return;
    this.checking = true;
    try {
      const info = await api.updateCheck();
      this.error = "";
      this.found = info.version ? info : null;
      if (!quiet) app.notify(info.version ? `Cablebag ${info.version} is available.` : `Cablebag ${info.current} is the newest version.`);
    } catch (e) {
      this.error = String(e);
      if (!quiet) app.notify(this.error, "error");
    } finally {
      this.checking = false;
      this.lastChecked = Date.now();
    }
  }

  async install() {
    if (!this.found || this.installing) return;
    const open = app.tabs.filter((t) => t.status === "connected").length;
    const text =
      `Install Cablebag ${this.found.version} now? Cablebag closes, installs the update and starts again.` +
      (open ? ` ${open} connected session${open === 1 ? " is" : "s are"} closed.` : "");
    if (!(await ask.confirm("Install the update", text, { okLabel: "Install and restart" }))) return;
    this.installing = true;
    this.progress = 0;
    try {
      await api.updateInstall();
    } catch (e) {
      this.installing = false;
      this.error = String(e);
      app.notify(this.error, "error");
    }
  }
}

export const update = new UpdateStore();

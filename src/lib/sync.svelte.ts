// Sync through git: syncs at start, a few seconds after a change, and every few minutes.
import { api, type SyncResult } from "./api";
import { app } from "./state.svelte";
import { proxmox } from "./proxmox.svelte";

class SyncStore {
  busy = $state(false);
  last = $state<SyncResult | null>(null);
  private timer: ReturnType<typeof setTimeout> | undefined;
  private started = false;
  private lastPrint = "";

  get on(): boolean {
    return !!app.settings["sync.auto"] && !!app.settings["sync.repo"];
  }

  /** What is synced, to notice changes: sessions without when they were last used. */
  print(): string {
    return JSON.stringify([
      app.folders,
      app.sessions.map(({ last_used, ...s }) => s),
      app.settings["macros"],
      app.settings["tunnels"],
      proxmox.sources.map(({ has_secret, ...s }) => s),
      app.settings["ai.memory"],
    ]);
  }

  start() {
    if (this.started) return;
    this.started = true;
    this.last = (app.settings["sync.last"] as SyncResult) ?? null;
    this.lastPrint = this.print();
    if (this.on) this.now(true);
    setInterval(() => {
      if (this.on && document.visibilityState === "visible") this.now(true);
    }, 5 * 60 * 1000);
  }

  /** Called when synced data may have changed; syncs a few seconds after the last change. */
  changed() {
    if (!this.on || this.busy) return;
    const p = this.print();
    if (p === this.lastPrint) return;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.now(true), 6000);
  }

  async now(quiet = false) {
    if (this.busy) return;
    this.busy = true;
    try {
      this.last = await api.syncNow();
      if (!quiet) app.notify(this.summary(this.last));
    } catch (e) {
      this.last = { at: Math.floor(Date.now() / 1000), received: 0, sent: 0, conflicts: 0, sessions: 0, folders: 0, error: String(e) };
      if (!quiet) app.notify(String(e), "error");
    } finally {
      this.busy = false;
      this.lastPrint = this.print();
    }
  }

  summary(r: SyncResult): string {
    if (r.error) return `Sync failed: ${r.error}`;
    const parts = [];
    if (r.received) parts.push(`${r.received} change${r.received === 1 ? "" : "s"} received`);
    if (r.sent) parts.push(`${r.sent} sent`);
    if (r.conflicts) parts.push(`${r.conflicts} changed on both sides, this PC's version kept`);
    return `${parts.length ? parts.join(", ") : "Up to date"}: ${r.sessions} sessions in ${r.folders} folders.`;
  }
}

export const sync = new SyncStore();

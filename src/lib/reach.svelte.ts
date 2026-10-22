// Reachability dots: whether the host of a saved session answers, checked for the sessions on screen.
import { invoke } from "@tauri-apps/api/core";
import type { Session } from "./api";
import { kindInfo } from "./kinds";
import { app } from "./state.svelte";

/** ok: answers; service: host answers ping but the port does not; down: no answer; noname: name does not resolve. */
export type ReachState = "ok" | "service" | "down" | "noname";

export interface ReachResult {
  state: ReachState;
  ms: number | null;
  at: number;
}

const NETWORK_KINDS = new Set(["ssh", "sftp", "rdp", "vnc", "telnet", "ftp", "mosh", "rsh"]);

/** The host:port to check for a session, or null when this PC cannot reach it directly (local, or through a gateway). */
export function reachKey(s: Session): string | null {
  if (!NETWORK_KINDS.has(s.kind) || !s.host?.trim()) return null;
  const d = app.withFolderDefaults(s).data ?? {};
  // Through WireGuard the address means something else than from this PC (it may even be a machine here).
  if (d.wireguard && d.wireguard !== "none") return null;
  if ((d.jump_hosts ?? []).some((h: any) => h?.host?.trim())) return null;
  if (d.ssh_gateway?.host?.trim()) return null;
  if (d.gateway?.host?.trim() && (d.gateway.usage ?? "always") === "always") return null;
  return `${s.host.trim().toLowerCase()}:${s.port || kindInfo(s.kind).defaultPort}`;
}

class Reachability {
  results = $state<Record<string, ReachResult>>({});
  private visible = new Set<string>();
  private busy = new Set<string>();
  private soon: ReturnType<typeof setTimeout> | undefined;
  private started = false;
  private mode = "";

  get enabled(): boolean {
    return app.settings["reach.enabled"] !== false;
  }

  start() {
    if (this.started) return;
    this.started = true;
    setInterval(() => this.check(), 15000);
    document.addEventListener("visibilitychange", () => this.check());
  }

  /** The tree reports which hosts are on screen; ones without a recent answer are checked shortly. */
  show(keys: string[]) {
    this.visible = new Set(keys);
    clearTimeout(this.soon);
    this.soon = setTimeout(() => this.check(), 400);
  }

  /** Checks these hosts now, whatever their last result. */
  checkNow(keys: string[]) {
    return this.check(keys);
  }

  private async check(force?: string[]) {
    if (!this.enabled || document.visibilityState === "hidden") return;
    // Results from the other way of checking mean something else, so they are dropped.
    if (app.settings["reach.mode"] !== this.mode) {
      this.mode = app.settings["reach.mode"];
      this.results = {};
    }
    const every = Math.max(30, Number(app.settings["reach.interval"]) || 120) * 1000;
    const now = Date.now();
    const due = (force ?? [...this.visible]).filter((k) => !this.busy.has(k) && (force || !this.results[k] || now - this.results[k].at >= every));
    if (!due.length) return;
    due.forEach((k) => this.busy.add(k));
    const portFirst = app.settings["reach.mode"] === "port";
    try {
      const targets = due.map((key) => {
        const i = key.lastIndexOf(":");
        return { key, host: key.slice(0, i), port: Number(key.slice(i + 1)) };
      });
      const res = await invoke<{ key: string; state: string; ms: number | null }[]>("net_reach", { targets, portFirst });
      const at = Date.now();
      for (const r of res) {
        const state: ReachState = r.state === "open" ? "ok" : r.state === "up" ? (portFirst ? "service" : "ok") : r.state === "unknown-name" ? "noname" : "down";
        this.results[r.key] = { state, ms: r.ms, at };
      }
    } catch {
      // Not fatal: the dots simply stay as they were.
    } finally {
      due.forEach((k) => this.busy.delete(k));
    }
  }

  /** Text for the dot's tooltip. */
  describe(key: string): string {
    const r = this.results[key];
    if (!r) return "Checking whether the host answers...";
    const port = key.slice(key.lastIndexOf(":") + 1);
    const when = new Date(r.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    const ms = r.ms !== null ? ` in ${r.ms} ms` : "";
    switch (r.state) {
      case "ok":
        return `Answers${ms} (checked ${when})`;
      case "service":
        return `The host answers ping${ms}, but nothing answers on port ${port} (checked ${when})`;
      case "noname":
        return `The name does not resolve from this PC (checked ${when})`;
      default:
        return `No answer from this PC: off, or needs a VPN (checked ${when})`;
    }
  }
}

export const reach = new Reachability();

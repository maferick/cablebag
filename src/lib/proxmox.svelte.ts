// Proxmox servers as live folders in the sessions tree: their guests, state and addresses.
import { openUrl } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import { api, type ProxmoxGuest, type ProxmoxSnapshot, type ProxmoxSource, type ProxmoxTask, type Session } from "./api";
import { ask } from "./ask.svelte";
import { app, newSession } from "./state.svelte";

interface Listing {
  guests: ProxmoxGuest[];
  loading: boolean;
  error: string;
  at: number;
}

const ACTIONS: Record<string, { label: string; doing: string; confirm?: string }> = {
  start: { label: "Start", doing: "Starting" },
  shutdown: { label: "Shut down", doing: "Shutting down", confirm: "Shut down" },
  reboot: { label: "Reboot", doing: "Rebooting", confirm: "Reboot" },
  stop: { label: "Force stop", doing: "Stopping", confirm: "Force stop" },
};

export const isWindows = (g: ProxmoxGuest) => g.os.startsWith("w");

class ProxmoxStore {
  sources = $state<ProxmoxSource[]>([]);
  listings = $state<Record<string, Listing>>({});
  private open = new Set<string>();
  private started = false;

  async load() {
    this.sources = await api.proxmoxSources();
  }

  start() {
    if (this.started) return;
    this.started = true;
    // Open folders follow the servers every 30 seconds while Cablebag is on screen.
    setInterval(() => {
      if (document.visibilityState === "visible") this.open.forEach((id) => this.refresh(id, true));
    }, 30000);
  }

  /** The tree tells which Proxmox folders are open; a folder without a recent listing loads now. */
  showOpen(ids: string[]) {
    this.open = new Set(ids);
    for (const id of ids) {
      const l = this.listings[id];
      if (!l || (!l.loading && Date.now() - l.at > 30000)) this.refresh(id);
    }
  }

  async refresh(id: string, quiet = false) {
    const cur = this.listings[id];
    if (cur?.loading) return;
    this.listings[id] = { guests: cur?.guests ?? [], loading: true, error: quiet ? (cur?.error ?? "") : "", at: cur?.at ?? 0 };
    try {
      const guests = await api.proxmoxList(id);
      this.listings[id] = { guests, loading: false, error: "", at: Date.now() };
    } catch (e) {
      this.listings[id] = { guests: cur?.guests ?? [], loading: false, error: String(e), at: Date.now() };
    }
  }

  source(id: string) {
    return this.sources.find((s) => s.id === id);
  }

  /** A session to reach the guest with, or null (with a message) when it has no known address. */
  session(src: ProxmoxSource, g: ProxmoxGuest, kind: "ssh" | "rdp"): Session | null {
    const ip = g.ips[0];
    if (!ip) {
      app.notify(
        g.status !== "running"
          ? `${g.name} is not running.`
          : g.kind === "qemu"
            ? `${g.name} does not report an address: switch on the QEMU guest agent in the VM's options and install it in the VM.`
            : `${g.name} does not report an address.`,
        "error",
      );
      return null;
    }
    const s = newSession(kind);
    s.name = g.name;
    s.host = ip;
    if (kind === "ssh") {
      s.username = src.ssh_user || "root";
      if (src.key_path) s.data.key_path = src.key_path;
    } else {
      s.username = src.rdp_user || "";
    }
    s.comment = `${g.kind === "lxc" ? "Container" : "VM"} ${g.vmid} on ${g.node} (${src.name})`;
    return s;
  }

  /** Double-click: Windows over RDP, the rest over SSH; a guest that is off is offered a start. */
  async openDefault(src: ProxmoxSource, g: ProxmoxGuest) {
    if (g.status !== "running") {
      if (await ask.confirm(`Start ${g.name}`, `${g.name} is ${g.status}. Start it?`, { okLabel: "Start" })) await this.power(src, g, "start", true);
      return;
    }
    // Without a known address the console still works.
    if (!g.ips.length) return this.openConsole(src, g);
    const s = this.session(src, g, isWindows(g) ? "rdp" : "ssh");
    if (s) app.openSession(s);
  }

  /** The guest's own screen in a tab: Proxmox's console over VNC. */
  openConsole(src: ProxmoxSource, g: ProxmoxGuest) {
    if (g.status !== "running") {
      app.notify(`${g.name} is not running, so it has no console.`, "error");
      return;
    }
    const s = newSession("vnc");
    s.name = `${g.name} (console)`;
    s.host = src.url.replace(/^https?:\/\//, "").replace(/:\d+$/, "");
    s.port = 0;
    s.comment = `Console of ${g.kind === "lxc" ? "container" : "VM"} ${g.vmid} on ${g.node} (${src.name})`;
    s.data.proxmox = { source: src.id, node: g.node, kind: g.kind, vmid: g.vmid, os: g.os };
    app.openSession(s);
  }

  consoleUrl(src: ProxmoxSource, g: ProxmoxGuest): string {
    const q = new URLSearchParams({ console: g.kind === "lxc" ? "lxc" : "kvm", novnc: "1", vmid: String(g.vmid), vmname: g.name, node: g.node, resize: "scale" });
    return `${src.url}/?${q}`;
  }

  openConsoleInBrowser(src: ProxmoxSource, g: ProxmoxGuest) {
    openUrl(this.consoleUrl(src, g)).catch((e) => app.notify(String(e), "error"));
  }

  async power(src: ProxmoxSource, g: ProxmoxGuest, action: string, confirmed = false) {
    const a = ACTIONS[action];
    if (!a) return;
    if (a.confirm && !confirmed) {
      const warn = action === "stop" ? " This is like pulling the plug: unsaved work in it is lost." : "";
      if (!(await ask.confirm(a.label, `${a.confirm} ${g.name} (${g.kind === "lxc" ? "container" : "VM"} ${g.vmid} on ${g.node})?${warn}`, { okLabel: a.label, danger: action !== "reboot" })))
        return;
    }
    try {
      await api.proxmoxPower(src.id, g, action);
      app.notify(`${a.doing} ${g.name}...`);
      setTimeout(() => this.refresh(src.id, true), 3000);
      setTimeout(() => this.refresh(src.id, true), 12000);
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  actions = ACTIONS;

  /** The snapshot or task dialog that is open. */
  dialog = $state<{ kind: "snapshots" | "tasks"; src: ProxmoxSource; g?: ProxmoxGuest } | null>(null);

  snapshots(src: ProxmoxSource, g: ProxmoxGuest) {
    return invoke<ProxmoxSnapshot[]>("proxmox_snapshots", { id: src.id, node: g.node, kind: g.kind, vmid: g.vmid });
  }

  /** A snapshot name Proxmox accepts, from the date and time. */
  snapshotName() {
    const d = new Date();
    const p = (n: number) => String(n).padStart(2, "0");
    return `cablebag_${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}_${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
  }

  /** Takes, rolls back or deletes a snapshot and waits for Proxmox to finish. Returns "OK" or the error. */
  async snapshot(src: ProxmoxSource, g: { node: string; kind: string; vmid: number }, action: "create" | "rollback" | "delete", name: string, description = "", ram = false) {
    const upid = await invoke<string>("proxmox_snapshot", { id: src.id, node: g.node, kind: g.kind, vmid: g.vmid, action, name, description, ram });
    return upid ? this.waitTask(src.id, g.node, upid) : "OK";
  }

  /** Follows a Proxmox task until it ends (at most ten minutes). */
  async waitTask(sourceId: string, node: string, upid: string): Promise<string> {
    const until = Date.now() + 10 * 60_000;
    while (Date.now() < until) {
      const [running, exit] = await invoke<[boolean, string]>("proxmox_task_status", { id: sourceId, node, upid });
      if (!running) return exit || "OK";
      await new Promise((r) => setTimeout(r, 1000));
    }
    return "still running after ten minutes; see the task log";
  }

  tasks(src: ProxmoxSource, vmid?: number) {
    return invoke<ProxmoxTask[]>("proxmox_tasks", { id: src.id, vmid: vmid ?? null });
  }

  taskLog(src: ProxmoxSource, t: ProxmoxTask) {
    return invoke<string>("proxmox_task_log", { id: src.id, node: t.node, upid: t.upid });
  }

  /** A shell on a node: your saved session for it, or root on its address. */
  async openNodeShell(src: ProxmoxSource, node: string) {
    try {
      app.openSession(await invoke<Session>("proxmox_node_session", { id: src.id, node }));
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  nodes(src: ProxmoxSource): string[] {
    return [...new Set((this.listings[src.id]?.guests ?? []).map((g) => g.node))].sort();
  }
}

export const proxmox = new ProxmoxStore();

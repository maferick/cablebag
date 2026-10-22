import { api, type TunnelDef, type TunnelStatus } from "./api";
import { app } from "./state.svelte";

class TunnelStore {
  status = $state<Record<string, TunnelStatus>>({});

  get list(): TunnelDef[] {
    return (app.settings["tunnels"] as TunnelDef[]) ?? [];
  }

  async save(list: TunnelDef[]) {
    await app.setSetting("tunnels", list);
  }

  async upsert(def: TunnelDef) {
    const list = this.list.some((t) => t.id === def.id) ? this.list.map((t) => (t.id === def.id ? def : t)) : [...this.list, def];
    await this.save(list);
  }

  async remove(id: string) {
    await this.stop(id);
    await this.save(this.list.filter((t) => t.id !== id));
  }

  state(id: string): TunnelStatus["state"] {
    return this.status[id]?.state ?? "stopped";
  }

  async start(def: TunnelDef) {
    try {
      await api.tunnelStart($state.snapshot(def) as TunnelDef);
    } catch {
      /* the failure arrives as a status event with the message */
    }
  }

  async stop(id: string) {
    await api.tunnelStop(id).catch(() => {});
  }

  async startAll() {
    await Promise.all(this.list.filter((t) => this.state(t.id) !== "running").map((t) => this.start(t)));
  }

  async stopAll() {
    await Promise.all(this.list.map((t) => this.stop(t.id)));
  }

  /** Called once after the settings are loaded. */
  async init() {
    for (const s of await api.tunnelStatuses()) this.status[s.id] = s;
    for (const t of this.list) if (t.autostart && this.state(t.id) !== "running") this.start(t);
  }

  onStatus(s: TunnelStatus) {
    this.status[s.id] = s;
  }
}

export const tunnels = new TunnelStore();

export function describeTunnel(t: TunnelDef): string {
  const ssh = `${t.ssh.user ? t.ssh.user + "@" : ""}${t.ssh.host}`;
  if (t.kind === "local") return `This PC port ${t.bind_port} → ${ssh} → ${t.dest_host}:${t.dest_port}`;
  if (t.kind === "remote") return `${ssh} port ${t.bind_port} → this PC → ${t.dest_host}:${t.dest_port}`;
  return `SOCKS proxy on this PC port ${t.bind_port} → out through ${ssh}`;
}

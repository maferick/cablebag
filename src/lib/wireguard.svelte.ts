// WireGuard networks that Cablebag runs itself, one per profile, for the sessions that go through them.
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, type WireGuardInfo, type WireGuardProfile } from "./api";
import { app } from "./state.svelte";

class WireGuardStore {
  list = $state<WireGuardInfo[]>([]);

  async load() {
    this.list = await api.wireguardProfiles().catch(() => []);
  }

  name(id: string): string {
    if (id === "none") return "directly from this PC";
    return this.list.find((p) => p.id === id)?.name ?? "a WireGuard network that is gone";
  }

  /** Asks for a .conf file and adds it as a network; its id, or null when nothing was added. */
  async importFile(): Promise<string | null> {
    const f = await openDialog({ title: "Choose a WireGuard configuration", filters: [{ name: "WireGuard", extensions: ["conf"] }, { name: "All files", extensions: ["*"] }] });
    if (typeof f !== "string") return null;
    try {
      const p = await api.wireguardImport(f);
      await this.load();
      app.notify(`WireGuard network ${p.name} added; its private key is kept in Cablebag's vault`);
      return p.id;
    } catch (e) {
      app.notify(String(e), "error");
      return null;
    }
  }

  /** What Sync carries: the profiles without their keys or state. */
  synced(): WireGuardProfile[] {
    // The public key follows from the private key, which only some PCs have: it would differ on every sync.
    return this.list.map(({ has_key, has_preshared, own_public_key, status, ...p }) => p);
  }
}

export const wireguard = new WireGuardStore();

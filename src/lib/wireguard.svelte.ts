// WireGuard networks that Cablebag runs itself, one per profile, for the sessions that go through them.
import { api, type WireGuardInfo, type WireGuardProfile } from "./api";

class WireGuardStore {
  list = $state<WireGuardInfo[]>([]);

  async load() {
    this.list = await api.wireguardProfiles().catch(() => []);
  }

  name(id: string): string {
    if (id === "none") return "directly from this PC";
    return this.list.find((p) => p.id === id)?.name ?? "a WireGuard network that is gone";
  }

  /** What Sync carries: the profiles without their keys or state. */
  synced(): WireGuardProfile[] {
    // The public key follows from the private key, which only some PCs have: it would differ on every sync.
    return this.list.map(({ has_key, has_preshared, own_public_key, status, ...p }) => p);
  }
}

export const wireguard = new WireGuardStore();

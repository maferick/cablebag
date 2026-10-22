import { api, type VaultItem, type VaultStatus } from "./api";
import { app } from "./state.svelte";

class VaultStore {
  status = $state<VaultStatus>({ configured: false, unlocked: false, remembered: false, hello: false, server: "", email: "", items: 0, synced: 0 });
  items = $state<VaultItem[]>([]);
  busy = $state(false);

  async refresh() {
    this.status = await api.vaultStatus();
    this.items = this.status.unlocked ? await api.vaultItems() : [];
  }

  async unlock() {
    this.busy = true;
    try {
      await api.vaultUnlock();
      await this.refresh();
      app.notify(`Vault unlocked: ${this.items.length} logins`);
      return true;
    } catch (e) {
      app.notify(String(e), "error");
      return false;
    } finally {
      this.busy = false;
    }
  }

  async sync() {
    this.busy = true;
    try {
      await api.vaultSync();
      await this.refresh();
      app.notify("Vault synced");
    } catch (e) {
      app.notify(String(e), "error");
    } finally {
      this.busy = false;
    }
  }

  async lock() {
    await api.vaultLock();
    await this.refresh();
  }

  item(id: string | undefined): VaultItem | undefined {
    return id ? this.items.find((i) => i.id === id) : undefined;
  }
}

export const vault = new VaultStore();

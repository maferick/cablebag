import { api, type SourceConfig, type SourceKind, type SourceStatus, type VaultItem } from "./api";
import { app } from "./state.svelte";

/** The names of the kinds of credential source, as the window shows them. */
export const SOURCE_KINDS: { kind: SourceKind; label: string }[] = [
  { kind: "vaultwarden", label: "Vaultwarden or Bitwarden" },
  { kind: "cyberark", label: "CyberArk (Privilege Cloud or PVWA)" },
  { kind: "delinea", label: "Delinea Secret Server" },
  { kind: "protonpass", label: "Proton Pass" },
];

/** The credential sources and the logins of the open ones. */
class VaultStore {
  sources = $state<SourceStatus[]>([]);
  items = $state<VaultItem[]>([]);
  /** Windows Hello can be used on this PC. */
  hello = $state(false);
  /** Ids of the sources being unlocked or synced. */
  busyIds = $state<string[]>([]);

  /** One line for the whole set, as the single vault had: any set up, any open. */
  status = $derived({
    configured: this.sources.some((s) => s.configured),
    unlocked: this.sources.some((s) => s.unlocked),
    allUnlocked: this.sources.length > 0 && this.sources.filter((s) => s.configured).every((s) => s.unlocked),
    open: this.sources.filter((s) => s.unlocked).length,
    count: this.sources.filter((s) => s.configured).length,
  });

  get busy() {
    return this.busyIds.length > 0;
  }

  async refresh() {
    this.sources = await api.sources();
    this.items = this.status.unlocked ? await api.vaultItems() : [];
  }

  async loadHello() {
    this.hello = await api.sourcesHello();
  }

  source(id: string | undefined): SourceStatus | undefined {
    return id ? this.sources.find((s) => s.id === id) : undefined;
  }

  /** The first source that is set up: the one a connect opens when none is open. */
  get first(): SourceStatus | undefined {
    return this.sources.find((s) => s.configured);
  }

  private async run(id: string, what: () => Promise<void>) {
    this.busyIds = [...this.busyIds, id];
    try {
      await what();
      return true;
    } catch (e) {
      app.notify(String(e), "error");
      return false;
    } finally {
      this.busyIds = this.busyIds.filter((b) => b !== id);
      await this.refresh().catch(() => {});
    }
  }

  /** Unlocks one source, or the first one set up. */
  async unlock(id?: string) {
    const s = id ? this.source(id) : this.first;
    if (!s) return false;
    const ok = await this.run(s.id, () => api.sourceUnlock(s.id));
    if (ok) {
      const n = this.source(s.id)?.items ?? 0;
      app.notify(`${s.name} ${s.kind === "vaultwarden" ? "unlocked" : s.kind === "protonpass" ? "opened" : "signed in"}: ${n} logins`);
    }
    return ok;
  }

  /** Syncs one source, or every open one. */
  async sync(id?: string) {
    const ids = id ? [id] : this.sources.filter((s) => s.unlocked).map((s) => s.id);
    let ok = true;
    for (const i of ids) ok = (await this.run(i, () => api.sourceSync(i))) && ok;
    if (ok && ids.length) app.notify(ids.length === 1 ? `${this.source(ids[0])?.name ?? "Vault"} synced` : "Credential sources synced");
  }

  /** Locks (signs out of) one source, or every open one. */
  async lock(id?: string) {
    const ids = id ? [id] : this.sources.filter((s) => s.unlocked).map((s) => s.id);
    for (const i of ids) await this.run(i, () => api.sourceLock(i));
  }

  async checkIn(item: string) {
    const src = this.sources.find((s) => s.checked_out.some((c) => c.id === item));
    await this.run(src?.id ?? item, () => api.sourceCheckIn(item));
  }

  async save(source: SourceConfig) {
    const saved = await api.sourceSave(source);
    await this.refresh();
    return saved;
  }

  async remove(id: string) {
    await api.sourceDelete(id);
    await this.refresh();
  }

  async reorder(ids: string[]) {
    await api.sourcesReorder(ids);
    await this.refresh();
  }

  item(id: string | undefined): VaultItem | undefined {
    return id ? this.items.find((i) => i.id === id) : undefined;
  }

  /** The source a reference points to: "source:item", or the first vault for a bare id. */
  sourceOf(ref: string | undefined): SourceStatus | undefined {
    if (!ref) return undefined;
    const m = /^([A-Za-z0-9_-]+):/.exec(ref);
    if (m) return this.source(m[1]);
    return this.source("vaultwarden") ?? this.sources.find((s) => s.kind === "vaultwarden");
  }
}

export const vault = new VaultStore();

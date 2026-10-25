// Browser profiles: named sets of cookies and logins for web tabs, so a second Azure tenant, Teams or other
// account can be signed in next to the first. Kept in the setting browser.profiles, which Sync carries.
import { api, type Folder, type Session } from "./api";
import { ask } from "./ask.svelte";
import { TAB_COLORS } from "./tabColors";
import { app } from "./state.svelte";

export type BrowserProfile = {
  id: string;
  name: string;
  color: string;
  /** Its web tabs show no Windows notifications. */
  muted?: boolean;
};

/** The value for Default, Cablebag's own cookies and logins (it also ends what a folder above gives). */
export const SHARED = "none";
/** The value for a private tab: nothing is kept after it closes. */
export const PRIVATE = "private";

class BrowserProfiles {
  get list(): BrowserProfile[] {
    return (app.settings["browser.profiles"] as BrowserProfile[] | undefined) ?? [];
  }

  get(id: string | undefined): BrowserProfile | undefined {
    return this.list.find((p) => p.id === id);
  }

  /** Plain words: Default is Cablebag's own cookies and logins, a private window keeps nothing. */
  name(id: string | undefined): string {
    if (!id || id === SHARED) return "Default";
    if (id === PRIVATE) return "Private window";
    return this.get(id)?.name ?? "a browser profile that is gone";
  }

  /** A tab colour no other profile has yet, so two tenants never look alike. */
  freeColor(): string {
    const used = new Set(this.list.map((p) => p.color));
    const colors = TAB_COLORS.map((c) => c.value).filter(Boolean);
    return colors.find((c) => !used.has(c)) ?? colors[this.list.length % colors.length];
  }

  /** A new profile from just a name (suggested from the folder or site); its colour is picked. Null when cancelled. */
  async askNew(suggest = ""): Promise<string | null> {
    const name = await ask.text("New browser profile", "Name, such as the customer or tenant", suggest, "Create");
    if (!name?.trim()) return null;
    const id = await this.save({ id: "", name: name.trim(), color: this.freeColor() });
    app.notify(`Browser profile ${name.trim()} made`);
    return id;
  }

  /** The profile a saved web session always opens in; null follows its folder. */
  async setForSession(s: Session, id: string | null) {
    const data = { ...($state.snapshot(s.data) ?? {}) };
    if (id === null) delete data.browser_profile;
    else data.browser_profile = id;
    await app.saveSession({ ...($state.snapshot(s) as Session), data });
  }

  /** The profile of a folder's web sessions (and its subfolders'); null follows the folder above. */
  async setForFolder(f: Folder, id: string | null) {
    const data = { ...($state.snapshot(f.data) ?? {}) };
    if (id === null) delete data.browser_profile;
    else data.browser_profile = id;
    await api.saveFolder({ ...($state.snapshot(f) as Folder), data });
    await app.reloadLibrary();
  }

  /** The profile a web tab uses, with its name and colour; null for other tabs and the shared profile. */
  ofTab(tab: { type: string; session?: { data?: Record<string, any> } }): { name: string; color: string } | null {
    if (tab.type !== "browser") return null;
    const id = tab.session?.data?.browser_profile as string | undefined;
    if (id === PRIVATE) return { name: "Private", color: "#7b7a74" };
    const p = this.get(id);
    return p ? { name: p.name, color: p.color || "#7b7a74" } : null;
  }

  /** The colour a tab of this profile gets when the session sets none of its own. */
  color(id: string | undefined): string | undefined {
    return this.get(id)?.color || undefined;
  }

  async save(p: BrowserProfile): Promise<string> {
    const id = p.id || crypto.randomUUID();
    const next = { ...p, id, name: p.name.trim() || "Browser profile" };
    const list = this.list.some((x) => x.id === id) ? this.list.map((x) => (x.id === id ? next : x)) : [...this.list, next];
    await app.setSetting("browser.profiles", list);
    return id;
  }

  /** The open tabs that use the profile (directly or through their folder). */
  openTabs(id: string) {
    return app.tabs.filter((t) => t.type === "browser" && t.session?.data?.browser_profile === id);
  }

  /** Closes the profile's tabs and waits a moment: WebView2 lets go of the profile's files only when it has closed. */
  private async closeTabs(id: string) {
    const tabs = this.openTabs(id);
    for (const t of tabs) app.closeTab(t.id);
    if (tabs.length) await new Promise((r) => setTimeout(r, 2000));
  }

  /** Signs out of everything in the profile: its tabs close, and its cookies and logins go. */
  async clear(id: string) {
    await this.closeTabs(id);
    await api.browserProfileForget(id);
  }

  /** Deletes the profile: sessions and folders that used it go back to Default. */
  async remove(id: string) {
    await this.closeTabs(id);
    for (const s of app.sessions.filter((s) => s.data?.browser_profile === id)) {
      const data = { ...s.data };
      delete data.browser_profile;
      await api.saveSession({ ...$state.snapshot(s), data });
    }
    for (const f of app.folders.filter((f) => f.data?.browser_profile === id)) {
      const data = { ...f.data };
      delete data.browser_profile;
      await api.saveFolder({ ...$state.snapshot(f), data });
    }
    await app.reloadLibrary();
    await app.setSetting("browser.profiles", this.list.filter((p) => p.id !== id));
    await api.browserProfileForget(id);
  }
}

export const browserProfiles = new BrowserProfiles();

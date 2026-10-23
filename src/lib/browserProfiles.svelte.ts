// Browser profiles: named sets of cookies and logins for web tabs, so a second Azure tenant, Teams or other
// account can be signed in next to the first. Kept in the setting browser.profiles, which Sync carries.
import { api } from "./api";
import { app } from "./state.svelte";

export type BrowserProfile = {
  id: string;
  name: string;
  color: string;
  /** Its web tabs show no Windows notifications. */
  muted?: boolean;
};

/** The value for "Cablebag's shared profile" (it also ends what a folder above gives). */
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

  name(id: string | undefined): string {
    if (!id || id === SHARED) return "Cablebag's shared profile";
    if (id === PRIVATE) return "Private (nothing kept)";
    return this.get(id)?.name ?? "a browser profile that is gone";
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

  /** Deletes the profile: sessions and folders that used it go back to the shared profile. */
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

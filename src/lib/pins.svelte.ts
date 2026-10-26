// Pinned sites: web sessions on the strip at the left edge, each with the browser profile it opens in (the same site
// can be pinned once per profile, such as Teams for two tenants). A pinned site opens when Cablebag starts and keeps
// running, so its unread count stays live. Kept in the setting browser.pins, which Sync carries.
import type { Session } from "./api";
import { browserProfiles, PRIVATE, SHARED } from "./browserProfiles.svelte";
import { app, type Tab } from "./state.svelte";

export type Pin = {
  id: string;
  /** The web session. */
  session: string;
  /** The browser profile it opens in: a profile's id, or SHARED. */
  profile: string;
  /** Its place on the strip. */
  sort: number;
};

/** A profile value as a tab or pin compares it: a profile that is gone, or none, is the shared one. */
export function profileKey(id: string | undefined): string {
  if (id === PRIVATE) return PRIVATE;
  return id && browserProfiles.get(id) ? id : SHARED;
}

/** The profile a web session opens in: its own, its folder's, or the shared one. */
export function profileOf(s: Session): string {
  return profileKey(app.withFolderDefaults(s).data?.browser_profile);
}

/** The profile a web tab uses. */
export function profileOfTab(t: Tab): string {
  return profileKey(t.session?.data?.browser_profile);
}

/** Whether a page is on the same site as a session's address: the same host, or one under the same domain
 * (sits-it.xurrent.com and app.xurrent.com; not login.microsoftonline.com for teams.cloud.microsoft). */
export function sameSite(page: string, address: string): boolean {
  const host = (u: string) => {
    try {
      return new URL(/^[a-z]+:\/\//i.test(u) ? u : `https://${u}`).hostname.toLowerCase();
    } catch {
      return "";
    }
  };
  const a = host(page);
  const b = host(address);
  if (!a || !b) return false;
  if (a === b) return true;
  if (/^[\d.]+$|:/.test(b)) return false;
  const base = b.split(".").slice(-2).join(".");
  return a === base || a.endsWith(`.${base}`);
}

/** Opens a web session in a given profile (once; the session keeps its own). */
export function openIn(s: Session, profile: string, keepRunning = false) {
  const data: Record<string, any> = { ...($state.snapshot(s.data) ?? {}), browser_profile: profile };
  if (keepRunning) data.keep_running = true;
  app.openSession({ ...$state.snapshot(s), data } as Session);
}

class Pins {
  get list(): Pin[] {
    const all = (app.settings["browser.pins"] as Pin[] | undefined) ?? [];
    // Pins of sessions that were deleted (here or on another PC) are left out, not shown as empty squares.
    return all.filter((p) => app.sessions.some((s) => s.id === p.session)).sort((a, b) => a.sort - b.sort);
  }

  private async store(list: Pin[]) {
    await app.setSetting("browser.pins", list.map((p, i) => ({ ...p, sort: i })));
  }

  find(session: string, profile: string): Pin | undefined {
    return this.list.find((p) => p.session === session && p.profile === profile);
  }

  /** Whether the session is pinned in any profile. */
  any(session: string): boolean {
    return this.list.some((p) => p.session === session);
  }

  session(p: Pin): Session | undefined {
    return app.sessions.find((s) => s.id === p.session);
  }

  /** The open tab of a pin: the session in the pin's profile. */
  tab(p: Pin): Tab | undefined {
    return app.tabs.find((t) => t.type === "browser" && t.session?.id === p.session && profileOfTab(t) === p.profile);
  }

  /** Whether a web tab is a pinned site (it then keeps running). */
  covers(t: Tab): boolean {
    return !!t.session?.id && this.list.some((p) => p.session === t.session!.id && p.profile === profileOfTab(t));
  }

  async add(s: Session, profile: string) {
    if (this.find(s.id, profile)) return;
    const all = (app.settings["browser.pins"] as Pin[] | undefined) ?? [];
    await this.store([...all.sort((a, b) => a.sort - b.sort), { id: crypto.randomUUID(), session: s.id, profile, sort: all.length }]);
  }

  async remove(id: string) {
    await this.store(((app.settings["browser.pins"] as Pin[] | undefined) ?? []).filter((p) => p.id !== id).sort((a, b) => a.sort - b.sort));
  }

  /** The icon a pin shows: the last one its site's page had (a data URL), or none yet. */
  icon(p: Pin): string | undefined {
    return this.siteIcon(p.session);
  }

  /** The remembered icon of a web session's site. */
  siteIcon(session: string | undefined): string | undefined {
    return session ? (app.settings["browser.siteIcons"] as Record<string, string> | undefined)?.[session] : undefined;
  }

  /** Remembers a site's icon (setting browser.siteIcons, this PC only), so its pin has it before the site opens
   * again. Only an icon of the site's own pages counts: on a sign-in page elsewhere (Microsoft's, for Teams) the
   * tab shows that page's icon, but the pin keeps the site's. */
  async setIcon(s: Session, page: string, data: string) {
    if (!sameSite(page, s.host)) return;
    const all = (app.settings["browser.siteIcons"] as Record<string, string> | undefined) ?? {};
    if (all[s.id] === data) return;
    // Sessions that were deleted take their icons with them.
    const kept = Object.fromEntries(Object.entries(all).filter(([id]) => app.sessions.some((x) => x.id === id)));
    await app.setSetting("browser.siteIcons", { ...kept, [s.id]: data });
  }

  /** Moves a pin to the place of another (dragging on the strip). */
  async move(id: string, before: string | null) {
    const list = this.list;
    const p = list.find((x) => x.id === id);
    if (!p || id === before) return;
    const rest = list.filter((x) => x.id !== id);
    const at = before ? rest.findIndex((x) => x.id === before) : rest.length;
    rest.splice(at < 0 ? rest.length : at, 0, p);
    await this.store(rest);
  }

  /** A click on the strip: to the pin's tab, or open it. */
  show(p: Pin) {
    const t = this.tab(p);
    if (t) {
      app.activeId = t.id;
      return;
    }
    const s = this.session(p);
    if (s) openIn(s, p.profile, true);
  }

  /** At the start: every pinned site opens in the background. */
  openAll() {
    const active = app.activeId;
    for (const p of this.list) {
      const s = this.session(p);
      if (s && !this.tab(p)) openIn(s, p.profile, true);
    }
    app.activeId = active;
  }
}

export const pins = new Pins();

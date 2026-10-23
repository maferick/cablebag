// The menu of a web session (in the session tree and on the pinned-sites strip): open it in another browser profile
// once, pin it, or choose the profile it always opens in.
import type { Session } from "./api";
import { app } from "./state.svelte";
import { menu, type MenuEntry } from "./menu.svelte";
import { browserProfiles, PRIVATE, SHARED } from "./browserProfiles.svelte";
import { openIn, pins, profileOf } from "./pins.svelte";
import { ExternalLink, Pin, PinOff, Star, Users } from "@lucide/svelte";

/** A profile's name as a menu shows it. */
function named(id: string): string {
  if (id === SHARED) return "Shared profile";
  if (id === PRIVATE) return "Private tab";
  return browserProfiles.name(id);
}

/** "Open in...": the other profiles, the shared one and a private tab. */
export function openInMenu(s: Session, current: string): MenuEntry[] {
  const ids = [...browserProfiles.list.map((p) => p.id), SHARED, PRIVATE].filter((id) => id !== current);
  return ids.map((id) => ({ label: named(id), action: () => openIn(s, id) }));
}

/** "Always open in...": the profile the session keeps (its own, or its folder's). */
function alwaysMenu(s: Session): MenuEntry[] {
  const own = s.data?.browser_profile as string | undefined;
  const folder = app.folderDefaults(s.folder_id);
  const save = async (id: string | null) => {
    const data = { ...($state.snapshot(s.data) ?? {}) };
    if (id === null) delete data.browser_profile;
    else data.browser_profile = id;
    await app.saveSession({ ...$state.snapshot(s), data } as Session);
  };
  const items: MenuEntry[] = [...browserProfiles.list.map((p) => p.id), SHARED].map((id) => ({
    label: named(id),
    hint: own === id ? "now" : undefined,
    action: () => save(id),
  }));
  if (folder.values.browser_profile) {
    items.push("sep", { label: `Like the folder ${folder.from.browser_profile} (${named(folder.values.browser_profile)})`, hint: own ? undefined : "now", action: () => save(null) });
  }
  return items;
}

/** The web part of a session's menu. */
export function webItems(s: Session): MenuEntry[] {
  const profile = profileOf(s);
  const pin = pins.find(s.id, profile);
  return [
    { label: "Open", icon: ExternalLink, hint: profile === SHARED ? undefined : named(profile), action: () => app.openSession(s) },
    { label: "Open in...", icon: Users, action: () => menu.again(openInMenu(s, profile)) },
    pin
      ? { label: "Unpin", icon: PinOff, action: () => pins.remove(pin.id) }
      : { label: "Pin to the strip", icon: Pin, hint: profile === SHARED ? undefined : named(profile), action: () => pins.add(s, profile) },
    { label: "Always open in...", icon: Star, action: () => menu.again(alwaysMenu(s)) },
  ];
}

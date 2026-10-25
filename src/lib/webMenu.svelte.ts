// The menus of web sessions and browser profiles (session tree, Web view, pinned-sites strip, address bar): open in
// another profile once, pin, choose the profile a session or a whole folder always uses, and make a new profile
// right where it is needed.
import type { Folder, Session } from "./api";
import { app } from "./state.svelte";
import { menu, type MenuEntry } from "./menu.svelte";
import { browserProfiles, PRIVATE, SHARED } from "./browserProfiles.svelte";
import { openIn, pins, profileOf } from "./pins.svelte";
import { ExternalLink, Pin, PinOff, Plus, Star, Users, FolderCog } from "@lucide/svelte";

/** A profile's name as a menu shows it. */
export const named = (id: string) => browserProfiles.name(id);

/** "Open in...": the other profiles, Default and a private window, and a new profile made on the spot. */
export function openInMenu(s: Session, current: string): MenuEntry[] {
  const ids = [...browserProfiles.list.map((p) => p.id), SHARED, PRIVATE].filter((id) => id !== current);
  return [
    ...ids.map((id) => ({ label: named(id), action: () => openIn(s, id) })),
    "sep",
    {
      label: "New profile...",
      icon: Plus,
      action: async () => {
        const id = await browserProfiles.askNew(s.name || "");
        if (id) openIn(s, id);
      },
    },
  ];
}

/**
 * The profiles to choose from for something that keeps one (a session, a folder): every profile with "now" at the
 * one in use, Default, "like the folder above" when there is one to follow, and a new profile.
 */
function choose(own: string | undefined, inherited: { id?: string; from?: string }, suggest: string, pick: (id: string | null) => void): MenuEntry[] {
  // With nothing chosen and nothing to follow, Default is what it uses now.
  const now = own ?? (inherited.id ? undefined : SHARED);
  const items: MenuEntry[] = [...browserProfiles.list.map((p) => p.id), SHARED].map((id) => ({
    label: named(id),
    hint: now === id ? "now" : undefined,
    action: () => pick(id),
  }));
  if (inherited.id) items.push("sep", { label: `Like ${inherited.from} (${named(inherited.id)})`, hint: own ? undefined : "now", action: () => pick(null) });
  items.push("sep", {
    label: "New profile...",
    icon: Plus,
    action: async () => {
      const id = await browserProfiles.askNew(suggest);
      if (id) pick(id);
    },
  });
  return items;
}

/** "Always open in...": the profile a saved web session keeps (its own, or its folder's). */
export function alwaysMenu(s: Session, after?: () => void): MenuEntry[] {
  const folder = app.folderDefaults(s.folder_id);
  const parent = app.folders.find((f) => f.id === s.folder_id);
  return choose(
    s.data?.browser_profile,
    { id: folder.values.browser_profile, from: `the folder ${folder.from.browser_profile}` },
    parent?.name ?? s.name ?? "",
    async (id) => {
      await browserProfiles.setForSession(s, id);
      after?.();
    },
  );
}

/** The profile of a folder's web sessions, and of its subfolders that choose none themselves. */
export function folderMenu(f: Folder, after?: () => void): MenuEntry[] {
  const above = app.folderDefaults(f.parent_id);
  return choose(
    f.data?.browser_profile,
    { id: above.values.browser_profile, from: `the folder ${above.from.browser_profile}` },
    f.name,
    async (id) => {
      await browserProfiles.setForFolder(f, id);
      after?.();
    },
  );
}

/** The menu item that opens folderMenu, for a folder's right-click menu. */
export function folderItem(f: Folder): MenuEntry {
  const own = f.data?.browser_profile as string | undefined;
  return { label: "Browser profile for this folder...", icon: FolderCog, hint: own ? named(own) : undefined, action: () => menu.again(folderMenu(f)) };
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

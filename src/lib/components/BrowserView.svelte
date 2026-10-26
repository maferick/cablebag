<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "../api";
  import { app, type Tab } from "../state.svelte";
  import { menu, type MenuEntry } from "../menu.svelte";
  import { tiles } from "../tiles.svelte";
  import { browserProfiles, PRIVATE, SHARED } from "../browserProfiles.svelte";
  import { unreadFromTitle } from "../unread";
  import { pins } from "../pins.svelte";
  import { alwaysMenu, folderMenu } from "../webMenu.svelte";
  import { vault } from "../vault.svelte";
  import { ask } from "../ask.svelte";
  import FillLoginDialog from "./FillLoginDialog.svelte";
  import { ArrowLeft, ArrowRight, RotateCw, ExternalLink, Globe, ChevronDown, KeyRound, Plus, Star, FolderCog } from "@lucide/svelte";

  let { tab, visible }: { tab: Tab; visible: boolean } = $props();
  let area: HTMLDivElement;
  const start = untrack(() => tab.session?.host ?? "");
  let address = $state(start);
  let current = $state(start);
  let loading = $state(true);
  let error = $state("");
  let opened = $state(false);
  let asleep = $state(false);
  let hiddenAt = Date.now();
  const keepRunning = untrack(() => !!tab.session?.data?.keep_running);
  // Whose cookies and logins this tab uses: shown next to the address, so two tenants are not mixed up.
  // A profile that was deleted (or has not arrived through Sync yet) means the shared one, not a new empty one.
  const profileId = untrack(() => {
    const id = tab.session?.data?.browser_profile as string | undefined;
    return id === PRIVATE || browserProfiles.get(id) ? id : undefined;
  });
  const profile = $derived(browserProfiles.ofTab(tab));

  /** Sign-in pages: their address only works in the profile that started the sign-in. */
  const SIGN_IN_HOSTS = /(^|\.)(login\.microsoftonline\.com|login\.microsoft\.com|login\.live\.com|accounts\.google\.com|login\.windows\.net)$/i;

  /** Opens this page again in another profile (or privately, or in the shared one) as a tab of its own; from a
   * sign-in page, the session's own address, so the other profile starts its own sign-in. */
  function otherProfiles(): MenuEntry[] {
    const page = () => {
      try {
        return current && !SIGN_IN_HOSTS.test(new URL(current).hostname) ? current : start;
      } catch {
        return start;
      }
    };
    const openIn = (id: string) => tab.session && app.openSession({ ...$state.snapshot(tab.session), id: "", name: "", host: page(), data: { ...$state.snapshot(tab.session.data), browser_profile: id } });
    const items: MenuEntry[] = browserProfiles.list.filter((p) => p.id !== profileId).map((p) => ({ label: `Open this page in ${p.name}`, action: () => openIn(p.id) }));
    if (profileId !== PRIVATE) items.push({ label: "Open this page in a private window", action: () => openIn(PRIVATE) });
    if (profileId) items.push({ label: "Open this page in Default", action: () => openIn(SHARED) });
    // The saved session (and its folder) this tab came from: make the choice stick, right here.
    const saved = tab.session?.id ? app.sessions.find((s) => s.id === tab.session!.id) : undefined;
    const folder = saved?.folder_id ? app.folders.find((f) => f.id === saved.folder_id) : undefined;
    items.push("sep", {
      label: "New profile for this page...",
      icon: Plus,
      action: async () => {
        const id = await browserProfiles.askNew(folder?.name ?? "");
        if (id) openIn(id);
      },
    });
    // A tab keeps the profile it opened with: after the choice it opens again, in the profile chosen.
    const reopen = () => {
      const now = app.sessions.find((s) => s.id === saved?.id);
      if (!now) return;
      app.openSession(now);
      app.closeTab(tab.id);
    };
    if (saved) items.push({ label: "Always open this session in...", icon: Star, action: () => menu.again(alwaysMenu(saved, reopen)) });
    if (folder) items.push({ label: `For the whole folder ${folder.name}...`, icon: FolderCog, action: () => menu.again(folderMenu(folder, reopen)) });
    items.push("sep", { label: "Manage browser profiles", action: () => app.openSettings("browsers") });
    return items;
  }

  // ---- Filling a login from the credential sources (only when asked, with the login the person picks) ----

  /** The page's site while "Choose another login" is open. */
  let pickFor = $state<string | null>(null);
  /** The page is not encrypted (http), as the last menu found. */
  let plain = false;
  let asking = false;

  /** Fills a login; `confirmedHost` when the person confirmed one not saved for this site (it must still be there). */
  async function fill(id: string, name: string, confirmedHost?: string) {
    try {
      await invoke("browser_fill", { id: tab.id, item: id, confirmedHost: confirmedHost ?? null });
      app.notify(plain ? `Filled ${name}. This page is not encrypted (http): what you send can be read on the way.` : `Filled ${name}`);
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  async function fillMenu(el: HTMLElement) {
    if (asking) return;
    const configured = vault.sources.filter((s) => s.configured);
    if (!configured.length) {
      menu.showAt(el, [{ label: "Set up a credential source to fill logins from...", action: () => app.openSettings("vault") }]);
      return;
    }
    let found: { host: string; plain: boolean; choices: { id: string; name: string; username: string; source: string; fits: boolean }[] };
    asking = true;
    try {
      found = await invoke("browser_fill_choices", { id: tab.id });
    } catch (e) {
      app.notify(String(e), "error");
      return;
    } finally {
      asking = false;
    }
    if (!el.isConnected) return;
    plain = found.plain;
    const several = configured.length > 1;
    const label = (c: { name: string; username: string }) => (c.username ? `${c.name} · ${c.username}` : c.name);
    const items: MenuEntry[] = found.choices
      .filter((c) => c.fits)
      .map((c) => ({ label: label(c), hint: several ? c.source : undefined, action: () => fill(c.id, c.name) }));
    const named = found.choices.filter((c) => !c.fits);
    if (named.length) {
      if (items.length) items.push("sep");
      for (const c of named)
        items.push({
          label: label(c),
          hint: "not saved for this site",
          action: async () => {
            if (await ask.confirm(`Fill ${c.name}?`, `This login is not saved for ${found.host}. Fill it into this page on ${found.host} anyway?`, { okLabel: "Fill" })) fill(c.id, c.name, found.host);
          },
        });
    }
    if (!items.length) items.push({ label: `No saved login for ${found.host}`, disabled: true });
    const locked = configured.filter((s) => !s.unlocked);
    if (locked.length) items.push("sep", ...locked.map((s) => ({ label: `Open ${s.name} for more logins`, action: () => vault.unlock(s.id) })));
    items.push("sep", { label: "Choose another login...", action: () => (pickFor = found.host) });
    menu.showAt(el, items);
  }

  // The page is a web view of its own over this area; it steps aside for dialogs and menus.
  const show = $derived(visible && !app.overlay && !menu.open);

  function rect() {
    const r = area.getBoundingClientRect();
    return { x: r.left, y: r.top, w: r.width, h: r.height };
  }

  $effect(() => {
    void tiles.version;
    if (!opened) return;
    api.browserVisible(tab.id, show).catch(() => {});
    if (show) requestAnimationFrame(() => api.browserBounds(tab.id, rect()).catch(() => {}));
  });

  onMount(() => {
    const ro = new ResizeObserver(() => {
      if (opened && area.offsetWidth) api.browserBounds(tab.id, rect()).catch(() => {});
    });
    ro.observe(area);
    const un = listen<{ id: string; url: string; title: string; loading: boolean }>("browser-event", (e) => {
      if (e.payload.id !== tab.id) return;
      current = e.payload.url;
      if (document.activeElement?.id !== `addr-${tab.id}`) address = e.payload.url;
      loading = e.payload.loading;
      if (e.payload.title) {
        tab.title = e.payload.title;
        tab.unread = unreadFromTitle(e.payload.title);
      }
    });
    // The page's icon: on the tab always, and for the pin when the page is on the site itself (a saved session).
    const unIcon = listen<{ id: string; data: string; url: string }>("browser-icon", (e) => {
      if (e.payload.id !== tab.id) return;
      tab.icon = e.payload.data;
      if (tab.session?.id) pins.setIcon(tab.session, e.payload.url, e.payload.data);
    });
    // A link that opens a new tab: a tab of its own here, with this tab's profile and network.
    const unNew = listen<{ from: string; url: string }>("browser-new-tab", (e) => {
      if (e.payload.from !== tab.id || !tab.session) return;
      app.openSession({ ...$state.snapshot(tab.session), id: "", name: "", host: e.payload.url });
    });
    // The page's notifications (Teams, chat): as Windows notifications with the profile's name, unless do not
    // disturb is on or the profile is muted. Clicking one brings this tab to the front.
    // At most a few per tab in a short while, so a page cannot flood the screen.
    let noted: number[] = [];
    const unNote = listen<{ id: string; key: number; title: string; body: string; origin: string }>("browser-notification", (e) => {
      if (e.payload.id !== tab.id) return;
      if (app.settings["browser.notify"] === false || browserProfiles.get(profileId)?.muted) return;
      const now = Date.now();
      noted = noted.filter((t) => now - t < 30_000);
      if (noted.length >= 5) return;
      noted.push(now);
      const title = profile ? `${profile.name}: ${e.payload.title}` : e.payload.title || tab.title;
      invoke("browser_notify", { id: tab.id, key: e.payload.key, title, body: e.payload.body, origin: e.payload.origin }).catch(() => {});
    });
    const unClick = listen<{ id: string }>("browser-notification-click", (e) => {
      if (e.payload.id === tab.id && app.tabs.some((t) => t.id === tab.id)) app.activeId = tab.id;
    });
    const unDl = listen<{ id: string; path: string; success: boolean }>("browser-download", (e) => {
      if (e.payload.id !== tab.id) return;
      const name = e.payload.path.split(/[\\/]/).pop() || "The file";
      if (e.payload.success) app.notify(`Downloaded ${name} to ${e.payload.path.slice(0, e.payload.path.length - name.length - 1) || "Downloads"}`);
      else app.notify(`Downloading ${name} did not finish`, "error");
    });
    open(start);
    // A tab not looked at for the time set in Settings goes to sleep: its page is closed to free memory, and opens
    // again where it was when the tab is shown. Tabs kept running or pinned (Teams, for its notifications) and private ones
    // (they would lose their login) stay awake.
    const sleeper = setInterval(() => {
      const minutes = Number(app.settings["browser.sleepMinutes"]) || 0;
      if (!minutes || keepRunning || pins.covers(tab) || profileId === PRIVATE || visible || asleep || !opened) return;
      if (Date.now() - hiddenAt >= minutes * 60_000) {
        asleep = true;
        opened = false;
        tab.status = "idle";
        api.browserClose(tab.id).catch(() => {});
      }
    }, 30_000);
    return () => {
      clearInterval(sleeper);
      ro.disconnect();
      for (const u of [un, unNew, unDl, unNote, unClick, unIcon]) u.then((f) => f());
    };
  });

  function open(url: string) {
    api
      .browserOpen(tab.id, url, tab.session?.data?.wireguard, profileId, profile?.name, rect())
      .then(() => {
        // Made while the tab may not have its size yet: place it now (the effect above keeps it placed).
        opened = true;
        tab.status = "connected";
        api.browserBounds(tab.id, rect()).catch(() => {});
      })
      .catch((e) => {
        error = String(e);
        tab.status = "failed";
      });
  }

  // When it was last hidden, for going to sleep; a sleeping tab wakes when it is shown.
  $effect(() => {
    if (!visible) hiddenAt = Date.now();
    else if (untrack(() => asleep)) {
      asleep = false;
      open(untrack(() => current) || start);
    }
  });

  onDestroy(() => api.browserClose(tab.id).catch(() => {}));

  const go = (what: string, url?: string) => api.browserCommand(tab.id, what, url).catch((e) => app.notify(String(e), "error"));
</script>

<div class="browser" style:display={visible ? "flex" : "none"}>
  <div class="bar" class:profiled={!!profile} style:--pc={profile?.color}>
    <button class="icon-btn" title="Back" aria-label="Back" onclick={() => go("back")}><ArrowLeft size={15} /></button>
    <button class="icon-btn" title="Forward" aria-label="Forward" onclick={() => go("forward")}><ArrowRight size={15} /></button>
    <button class="icon-btn" class:spin={loading} title="Reload" aria-label="Reload" onclick={() => go("reload")}><RotateCw size={15} /></button>
    <form
      class="addr"
      onsubmit={(e) => {
        e.preventDefault();
        go("go", address);
      }}
    >
      <Globe size={13} />
      <input id={`addr-${tab.id}`} bind:value={address} spellcheck="false" autocomplete="off" aria-label="Address" />
    </form>
    <button class="icon-btn" title="Fill a login from your credential sources" aria-label="Fill a login from your credential sources" onclick={(e) => fillMenu(e.currentTarget as HTMLElement)}><KeyRound size={15} /></button>
    <!-- Always there: it is where you find out an account is the wrong one, and where you make a profile. -->
    <button
      class="profile"
      title={profile
        ? `This tab uses the cookies and logins of ${profile.name}. Click for another profile, or to keep one for this session or folder.`
        : "This tab uses the Default cookies and logins. Click to open it in a profile, make one, or keep one for this session or folder."}
      onclick={(e) => menu.showAt(e.currentTarget as HTMLElement, otherProfiles())}
    >
      <span class="dot" style:background={profile?.color || "var(--faint)"} aria-hidden="true"></span>{profile?.name ?? "Default"}
      <ChevronDown size={12} />
    </button>
    <button class="icon-btn" title="Open in the web browser on this PC" aria-label="Open in the web browser on this PC" onclick={() => openUrl(current)}><ExternalLink size={15} /></button>
  </div>
  <!-- A frame in the profile's colour around the page, so its identity shows wherever you look. -->
  <div class="frame" class:profiled={!!profile} style:--pc={profile?.color}>
    <div class="area" bind:this={area}>
      {#if error}
        <div class="state"><p>{error}</p></div>
      {:else if asleep}
        <div class="state"><p class="muted">Asleep to save memory; it opens again in a moment.</p></div>
      {:else if !show && visible}
        <div class="state"><p class="muted">The page is hidden while a dialog is open.</p></div>
      {/if}
    </div>
  </div>
</div>

{#if pickFor}
  <FillLoginDialog
    host={pickFor}
    onclose={() => (pickFor = null)}
    onpick={(id, name) => {
      const host = pickFor ?? "";
      pickFor = null;
      fill(id, name, host);
    }}
  />
{/if}

<style>
  .browser {
    position: absolute;
    inset: 0;
    flex-direction: column;
    background: var(--bg);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .addr {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--raised);
    color: var(--faint);
  }
  .addr input {
    flex: 1;
    min-width: 0;
    border: none;
    background: none;
    box-shadow: none;
    padding: 4px 0;
  }
  .bar.profiled {
    border-left: 4px solid var(--pc);
    background: color-mix(in srgb, var(--pc) 14%, var(--panel));
  }
  .profile {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 220px;
    padding: 3px 6px 3px 8px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--raised);
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }
  .profile:hover {
    background: var(--hover);
  }
  .profile :global(svg) {
    flex-shrink: 0;
    color: var(--muted);
  }
  .frame {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .frame.profiled {
    padding: 0 3px 3px;
    background: var(--pc);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .spin :global(svg) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .area {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: #fff;
  }
  .state {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
    background: var(--bg);
    text-align: center;
  }
</style>

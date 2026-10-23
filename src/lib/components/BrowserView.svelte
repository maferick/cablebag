<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "../api";
  import { app, type Tab } from "../state.svelte";
  import { menu } from "../menu.svelte";
  import { tiles } from "../tiles.svelte";
  import { browserProfiles, PRIVATE } from "../browserProfiles.svelte";
  import { unreadFromTitle } from "../unread";
  import { ArrowLeft, ArrowRight, RotateCw, ExternalLink, Globe } from "@lucide/svelte";

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
  const profile = $derived(profileId && profileId !== "none" ? { name: browserProfiles.name(profileId), color: profileId === PRIVATE ? "" : browserProfiles.color(profileId) } : null);

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
    // again where it was when the tab is shown. Tabs kept running (Teams, for its notifications) and private ones
    // (they would lose their login) stay awake.
    const sleeper = setInterval(() => {
      const minutes = Number(app.settings["browser.sleepMinutes"]) || 0;
      if (!minutes || keepRunning || profileId === PRIVATE || visible || asleep || !opened) return;
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
      for (const u of [un, unNew, unDl, unNote, unClick]) u.then((f) => f());
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
  <div class="bar">
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
    {#if profile}
      <span class="profile" title="This tab uses the cookies and logins of {profile.name}">
        <span class="dot" style:background={profile.color || "var(--faint)"} aria-hidden="true"></span>{profile.name}
      </span>
    {/if}
    <button class="icon-btn" title="Open in the web browser on this PC" aria-label="Open in the web browser on this PC" onclick={() => openUrl(current)}><ExternalLink size={15} /></button>
  </div>
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
  .profile {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 200px;
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: 3px;
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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

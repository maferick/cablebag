<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "../api";
  import { app, type Tab } from "../state.svelte";
  import { menu } from "../menu.svelte";
  import { tiles } from "../tiles.svelte";
  import { ArrowLeft, ArrowRight, RotateCw, ExternalLink, Globe } from "@lucide/svelte";

  let { tab, visible }: { tab: Tab; visible: boolean } = $props();
  let area: HTMLDivElement;
  const start = untrack(() => tab.session?.host ?? "");
  let address = $state(start);
  let current = $state(start);
  let loading = $state(true);
  let error = $state("");
  let opened = $state(false);

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
      if (e.payload.title) tab.title = e.payload.title;
    });
    api
      .browserOpen(tab.id, tab.session?.host ?? "", tab.session?.data?.wireguard, rect())
      .then(() => {
        // Made while the tab may not have its size yet: place it now (the effect below keeps it placed).
        opened = true;
        tab.status = "connected";
        api.browserBounds(tab.id, rect()).catch(() => {});
      })
      .catch((e) => {
        error = String(e);
        tab.status = "failed";
      });
    return () => {
      ro.disconnect();
      un.then((f) => f());
    };
  });

  onDestroy(() => api.browserClose(tab.id).catch(() => {}));

  const go = (what: string, url?: string) => api.browserCommand(tab.id, what, url).catch((e) => app.notify(String(e), "error"));
</script>

<div class="browser" style:display={visible ? "flex" : "none"}>
  <div class="bar">
    <button class="icon-btn" title="Back" onclick={() => go("back")}><ArrowLeft size={15} /></button>
    <button class="icon-btn" title="Forward" onclick={() => go("forward")}><ArrowRight size={15} /></button>
    <button class="icon-btn" class:spin={loading} title="Reload" onclick={() => go("reload")}><RotateCw size={15} /></button>
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
    <button class="icon-btn" title="Open in the web browser on this PC" onclick={() => openUrl(current)}><ExternalLink size={15} /></button>
  </div>
  <div class="area" bind:this={area}>
    {#if error}
      <div class="state"><p>{error}</p></div>
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

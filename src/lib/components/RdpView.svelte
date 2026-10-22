<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api } from "../api";
  import { app, type Tab } from "../state.svelte";
  import { menu } from "../menu.svelte";
  import { tiles } from "../tiles.svelte";
  import { Keyboard, RotateCw, Unplug, Monitor } from "@lucide/svelte";

  /** In tiling the bar doubles as the pane header: pane is its name part, paneEnd its buttons. */
  let { tab, visible, pane, paneEnd }: { tab: Tab; visible: boolean; pane?: Snippet<[Tab]>; paneEnd?: Snippet<[Tab]> } = $props();
  let area: HTMLDivElement;
  let message = $state("");
  let log = $state<string[]>([]);
  let opened = false;
  // The native window is shown once the control has something to show: a Windows prompt or the desktop.
  let nativeUp = $state(false);

  function rect() {
    const r = area.getBoundingClientRect();
    const d = window.devicePixelRatio || 1;
    return { x: Math.round(r.left * d), y: Math.round(r.top * d), w: Math.round(r.width * d), h: Math.round(r.height * d) };
  }

  const show = $derived(
    visible && !app.overlay && !menu.open && (tab.status === "connected" || (tab.status === "connecting" && nativeUp)),
  );

  $effect(() => {
    void tiles.version;
    if (opened) api.rdpVisible(tab.id, show).catch(() => {});
    if (show) requestAnimationFrame(() => api.rdpBounds(tab.id, rect()));
  });

  onMount(() => {
    const ro = new ResizeObserver(() => {
      if (opened && area.offsetWidth) api.rdpBounds(tab.id, rect()).catch(() => {});
    });
    ro.observe(area);
    const un = listen<{ id: string; event: string; text: string }>("rdp-event", (e) => {
      if (e.payload.id !== tab.id) return;
      if (e.payload.event === "log" && e.payload.text) log = [...log.slice(-6), e.payload.text];
      // A click on the remote screen: this pane becomes the active one.
      if (e.payload.event === "activated" && app.activeId !== tab.id) app.activeId = tab.id;
      if (e.payload.event === "disconnected") {
        message = e.payload.text || "The remote desktop session ended.";
        nativeUp = false;
      }
      if (["auth-warning", "connected", "login", "logon-error"].includes(e.payload.event)) nativeUp = true;
    });
    message = "";
    api
      .rdpOpen(tab.id, $state.snapshot(tab.session!), rect(), Math.round((window.devicePixelRatio || 1) * 100))
      .then(() => (opened = true))
      .catch((e) => {
        message = String(e);
        tab.status = "failed";
      });
    return () => {
      ro.disconnect();
      un.then((f) => f());
    };
  });

  onDestroy(() => api.rdpClose(tab.id).catch(() => {}));

  const s = $derived(tab.session!);
</script>

<div class="rdp" style:display={visible ? "flex" : "none"}>
  <div class="bar">
    {#if pane}
      {@render pane(tab)}
    {:else}
      <Monitor size={14} />
      <span class="where">{s.username ? `${s.username} on ` : ""}{s.host}</span>
      <span class="spacer"></span>
    {/if}
    <button class="btn small" disabled={tab.status !== "connected"} onclick={() => api.rdpCommand(tab.id, "cad")} title="Send Ctrl+Alt+Del to the remote computer">
      <Keyboard size={14} /><span class="lbl">Ctrl+Alt+Del</span>
    </button>
    <button class="btn small" onclick={() => app.restart(tab.id)} title="Connect again"><RotateCw size={14} /><span class="lbl">Reconnect</span></button>
    <button class="btn small" disabled={tab.status !== "connected"} onclick={() => api.rdpCommand(tab.id, "disconnect")} title="End the remote session">
      <Unplug size={14} /><span class="lbl">Disconnect</span>
    </button>
    {#if paneEnd}{@render paneEnd(tab)}{/if}
  </div>
  <div class="area" bind:this={area}>
    {#if tab.status !== "connected" && !show}
      <div class="state">
        {#if tab.status === "connecting"}
          <p>Connecting to <strong>{s.host}</strong>...</p>
        {:else}
          <p>{message || "Not connected."}</p>
          <button class="btn primary" onclick={() => app.restart(tab.id)}><RotateCw size={14} /> Connect again</button>
        {/if}
        {#each log as l}<p class="log mono">{l}</p>{/each}
      </div>
    {:else if !show && visible}
      <div class="state"><p class="muted">The remote screen is hidden while a dialog is open.</p></div>
    {/if}
  </div>
</div>

<style>
  .rdp {
    position: absolute;
    inset: 0;
    container-type: inline-size;
    flex-direction: column;
    background: var(--bg);
  }
  .bar {
    white-space: nowrap;
    overflow: hidden;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted);
  }
  .where {
    color: var(--text);
  }
  .spacer {
    flex: 1;
  }
  .small {
    padding: 3px 9px;
    font-size: 12px;
  }
  .btn.small {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
  }
  /* In a narrow pane only the icons stay; the pane header already names the tab. */
  @container (max-width: 640px) {
    .lbl,
    .where {
      display: none;
    }
  }

  .area {
    position: relative;
    flex: 1;
    min-height: 0;
    background: #1d1c1a;
  }
  .state {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: #ebe7df;
    text-align: center;
    padding: 20px;
  }
  .log {
    margin: 0;
    font-size: 11px;
    color: #a39e93;
  }
</style>

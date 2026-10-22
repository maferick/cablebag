<script lang="ts">
  // The line at the bottom: what is running and which modes are on. Every item is a button.
  import { onMount } from "svelte";
  import { app } from "../state.svelte";
  import { menu } from "../menu.svelte";
  import { tunnels } from "../tunnels.svelte";
  import { vault } from "../vault.svelte";
  import { sync } from "../sync.svelte";
  import { macros } from "../macros.svelte";
  import { update } from "../update.svelte";
  import { Radio, Circle, CircleStop, Waypoints, Lock, LockOpen, RefreshCw, Settings, Cable, FolderTree, ArrowDownToLine } from "@lucide/svelte";

  let now = $state(Date.now());
  onMount(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  const connected = $derived(app.tabs.filter((t) => t.status === "connected").length);
  const running = $derived(Object.values(tunnels.status).filter((s) => s.state === "running").length);
  const transfers = $derived(Object.values(app.transfers).filter((t) => !t.finished));

  const vaultWord = $derived(!vault.status.configured ? "not set up" : vault.status.unlocked ? "unlocked" : "locked");

  function ago(at: number): string {
    const min = Math.floor((now / 1000 - at) / 60);
    if (min < 1) return "just now";
    if (min < 60) return `${min} min ago`;
    const h = Math.floor(min / 60);
    return h < 24 ? `${h} h ago` : new Date(at * 1000).toLocaleDateString();
  }

  const syncWord = $derived(
    sync.busy ? "syncing..." : !app.settings["sync.repo"] ? "not set up" : sync.last?.error ? "failed" : sync.last ? ago(sync.last.at) : sync.on ? "waiting" : "off",
  );

  function vaultMenu(e: MouseEvent) {
    if (!vault.status.configured) return app.openSettings("vault");
    menu.showAt(e.currentTarget as HTMLElement, [
      vault.status.unlocked
        ? { label: "Lock the vault", icon: Lock, action: () => vault.lock() }
        : { label: "Unlock the vault", icon: LockOpen, action: () => vault.unlock() },
      { label: "Sync the vault", icon: RefreshCw, disabled: !vault.status.unlocked, action: () => vault.sync() },
      { label: "Vault settings", icon: Settings, action: () => app.openSettings("vault") },
    ]);
  }

  function syncMenu(e: MouseEvent) {
    if (!app.settings["sync.repo"]) return app.openSettings("sync");
    menu.showAt(e.currentTarget as HTMLElement, [
      { label: "Sync now", icon: RefreshCw, disabled: sync.busy, action: () => sync.now() },
      ...(sync.last?.error ? [{ label: `Last error: ${sync.last.error.slice(0, 80)}`, disabled: true }] : []),
      { label: "Sync settings", icon: Settings, action: () => app.openSettings("sync") },
    ]);
  }

  function toggleRecord() {
    if (macros.recording) return macros.stopRecording();
    const t = app.active;
    if (!t || t.type !== "terminal") return app.notify("Open a terminal tab first; the macro records what you type in it.", "error");
    macros.startRecording(t.id);
  }
</script>

<footer class="status">
  <button class="item" title="Find an open tab" onclick={() => (app.paletteOpen = true)}>
    <span class="dot" class:on={connected > 0}></span>{connected} connected
  </button>
  <button class="item" title="Port forwarding over SSH" onclick={() => app.openTool("tunnels", "Tunnels")}>
    <Waypoints size={13} />Tunnels: {running ? `${running} running` : "none"}
  </button>
  <button class="item" class:warn={vault.status.configured && !vault.status.unlocked} title="Passwords from Vaultwarden" onclick={vaultMenu}>
    {#if vault.status.unlocked}<LockOpen size={13} />{:else}<Lock size={13} />{/if}Vault: {vaultWord}
  </button>
  <button class="item" class:bad={!!sync.last?.error} title={sync.last?.error ? `Sync failed: ${sync.last.error}` : "Sessions synced through git"} onclick={syncMenu}>
    <RefreshCw size={13} />Sync: {syncWord}
  </button>
  <button class="item" class:on={app.filesOpen} title="Show or hide the remote files of this SSH tab (Ctrl+Shift+E). Whether they show when a session connects: Settings, General." onclick={() => (app.filesOpen = !app.filesOpen)}>
    <FolderTree size={13} />Remote files: {app.filesOpen ? "shown" : "hidden"}
  </button>
  {#each transfers as t (t.tab + t.name)}
    <span class="item transfer" title={t.name}>
      <Cable size={13} /><span class="tn">{t.name}</span>
      <span class="bar"><span style:width="{t.total ? Math.round((t.done / t.total) * 100) : 0}%"></span></span>
    </span>
  {/each}
  <span class="spacer"></span>
  {#if update.installing}
    <span class="item on"><ArrowDownToLine size={13} />Updating to {update.found?.version}: {update.progress}%</span>
  {:else if update.found}
    <button class="item on" title={update.found.notes || "A new version of Cablebag from its GitHub page"} onclick={() => update.install()}>
      <ArrowDownToLine size={13} />Update to {update.found.version}
    </button>
  {/if}
  <button class="item" class:send={app.multiExec} title="Type into all connected terminals at once" onclick={() => (app.multiExec = !app.multiExec)}>
    <Radio size={13} />Send to all: {app.multiExec ? "on" : "off"}
  </button>
  <button class="item" class:rec={!!macros.recording} title={macros.recording ? "Stop recording and save the macro" : "Record what you type in this terminal as a macro"} onclick={toggleRecord}>
    {#if macros.recording}<CircleStop size={13} />Recording: stop{:else}<Circle size={13} />Record{/if}
  </button>
  <button class="item" title="Interface size (Ctrl+Shift+plus and minus)" onclick={() => app.openSettings("general")}>
    {app.settings["ui.scale"] ?? 100}%
  </button>
</footer>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 24px;
    padding: 0 6px;
    background: var(--panel);
    border-top: 1px solid var(--border);
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
  }
  .item {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 20px;
    padding: 0 7px;
    border: none;
    border-radius: 4px;
    background: none;
    color: inherit;
    font-size: 12px;
  }
  button.item:hover {
    background: var(--hover);
    color: var(--text);
  }
  .item.on {
    color: var(--text);
  }
  .item.send {
    color: var(--warn-text);
    font-weight: 600;
  }
  .item.rec {
    color: var(--danger-text);
    font-weight: 600;
  }
  .item.warn {
    color: var(--warn-text);
  }
  .item.bad {
    color: var(--danger-text);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--border-strong);
  }
  .dot.on {
    background: var(--ok);
  }
  .spacer {
    flex: 1;
  }
  .tn {
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar {
    width: 70px;
    height: 5px;
    border-radius: 3px;
    background: var(--border);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
</style>

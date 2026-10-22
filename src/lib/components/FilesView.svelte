<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, Channel } from "../api";
  import { app, type Tab } from "../state.svelte";
  import FileBrowser from "./FileBrowser.svelte";
  import { FolderLock, RotateCw } from "@lucide/svelte";

  let { tab, visible }: { tab: Tab; visible: boolean } = $props();
  let message = $state("");

  onMount(() => {
    const un = listen<{ id: string; state: string; message: string }>("term-status", (e) => {
      if (e.payload.id === tab.id && e.payload.message) message = e.payload.message;
    });
    api.filesOpen(tab.id, $state.snapshot(tab.session!), new Channel<ArrayBuffer>()).catch((e) => {
      message = String(e);
      tab.status = "failed";
    });
    return () => {
      un.then((f) => f());
    };
  });

  const s = $derived(tab.session!);
</script>

<div class="files" style:display={visible ? "flex" : "none"}>
  {#if tab.status === "connected"}
    <FileBrowser {tab} />
  {:else}
    <div class="state">
      <FolderLock size={26} />
      {#if tab.status === "connecting"}
        <p>Connecting to <strong>{s.host}</strong>...</p>
        {#if message}<p class="muted small">{message}</p>{/if}
      {:else}
        <p>{message || "Not connected."}</p>
        <button class="btn primary" onclick={() => app.restart(tab.id)}><RotateCw size={14} /> Connect again</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .files {
    position: absolute;
    inset: 0;
    flex-direction: column;
    background: var(--bg);
    padding: 4px 8px 0;
  }
  .state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: var(--muted);
  }
  .state :global(svg) {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
</style>

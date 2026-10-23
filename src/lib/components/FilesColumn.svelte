<script lang="ts">
  // Remote files beside the tab area: the files of the active SSH tab. Folded to a strip when hidden or when
  // the active tab is not an SSH session, so it is always there to find.
  import { app } from "../state.svelte";
  import FileBrowser from "./FileBrowser.svelte";
  import { FolderTree, X } from "@lucide/svelte";

  const ssh = $derived(app.active?.proto === "ssh" ? app.active : undefined);
  const shown = $derived(app.filesOpen && !!ssh);

  let width = $state(Number(localStorage.getItem("files.width")) || 360);

  function drag(e: PointerEvent) {
    const startX = e.clientX;
    const start = width;
    const move = (ev: PointerEvent) => (width = Math.max(240, Math.min(900, start - (ev.clientX - startX))));
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      localStorage.setItem("files.width", String(width));
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
</script>

{#if shown && ssh}
  <div class="splitter" role="separator" aria-orientation="vertical" onpointerdown={drag}></div>
  <aside class="files" style:width="{width}px">
    <header>
      <FolderTree size={13} />
      <span class="t">Remote files of {ssh.title}</span>
      <button class="icon-btn" title="Hide the remote files (Ctrl+Shift+E)" aria-label="Hide the remote files (Ctrl+Shift+E)" onclick={() => (app.filesOpen = false)}><X size={13} /></button>
    </header>
    {#if ssh.status === "connected"}
      {#key ssh.id + ":" + ssh.gen}
        <FileBrowser tab={ssh} />
      {/key}
    {:else}
      <p class="note muted">Waiting for the SSH connection of "{ssh.title}"...</p>
    {/if}
  </aside>
{:else}
  <button
    class="strip"
    class:idle={!ssh}
    title={ssh ? "Show the remote files of this SSH tab (Ctrl+Shift+E)" : "Remote files: open an SSH tab to browse its files here"}
    onclick={() => (ssh ? (app.filesOpen = true) : app.notify("Open an SSH session to browse its files here."))}
  >
    <FolderTree size={13} />
    <span>Remote files</span>
  </button>
{/if}

<style>
  .splitter {
    width: 4px;
    margin-right: -2px;
    cursor: col-resize;
    z-index: 2;
  }
  .splitter:hover {
    background: var(--accent);
  }
  .files {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--panel);
    border-left: 1px solid var(--border);
  }
  header {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 4px 0 10px;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
    font-size: 12px;
  }
  .t {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .note {
    padding: 10px 12px;
    margin: 0;
  }
  .strip {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 24px;
    padding: 10px 0;
    border: none;
    border-left: 1px solid var(--border);
    border-radius: 0;
    background: var(--panel);
    color: var(--muted);
    writing-mode: vertical-rl;
    font-size: 12px;
  }
  .strip:hover {
    color: var(--text);
    background: var(--hover);
  }
  .strip.idle {
    color: var(--faint);
  }
</style>

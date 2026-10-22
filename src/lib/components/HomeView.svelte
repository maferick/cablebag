<script lang="ts">
  import { app } from "../state.svelte";
  import { kindInfo } from "../kinds";
  import Logo from "./Logo.svelte";
  import { workspaces } from "../workspaces.svelte";
  import { TerminalSquare, Plus, Clock, History, LayoutGrid, X } from "@lucide/svelte";

  const lastNames = $derived(workspaces.last.map((s) => s.title));

  const recent = $derived(
    [...app.sessions].filter((s) => s.last_used > 0).sort((a, b) => b.last_used - a.last_used).slice(0, 10),
  );
</script>

<div class="home">
  <div class="inner">
    <h1><Logo height={44} /></h1>
    <p class="muted lead">
      Open a session from the tree, or type a session name, a host like <span class="mono">user@server</span> or a
      command in the box at the top (<kbd>Ctrl+K</kbd>).
    </p>

    {#if workspaces.last.length && app.settings["tabs.reopen"] !== "never"}
      <button class="last" onclick={() => workspaces.openLast()}>
        <History size={16} />
        <span class="txt">
          <span class="nm">Open the {workspaces.last.length} tabs from last time</span>
          <span class="muted small">{lastNames.slice(0, 6).join(", ")}{lastNames.length > 6 ? ", ..." : ""}</span>
        </span>
      </button>
    {/if}

    <div class="cols">
      <section>
        <h2>Start</h2>
        {#each app.shells.slice(0, 6) as sh}
          <button class="item" onclick={() => app.openLocal(sh)}>
            <TerminalSquare size={15} />
            <span class="nm">{sh.name}</span>
          </button>
        {/each}
        <button class="item" onclick={() => app.editSession()}>
          <Plus size={15} />
          <span class="nm">New session</span>
        </button>
      </section>
      <section>
        <h2>Recent sessions</h2>
        {#if recent.length}
          {#each recent as s (s.id)}
            {@const k = kindInfo(s.kind)}
            <button class="item" onclick={() => app.openSession(s)}>
              <k.icon size={15} />
              <span class="nm">{s.name}</span>
              <span class="muted small">{app.folderPath(s.folder_id)}</span>
            </button>
          {/each}
        {:else}
          <p class="muted empty"><Clock size={14} /> Sessions you open appear here.</p>
        {/if}
      </section>
    </div>

    <section class="ws">
        <h2>Workspaces</h2>
        {#each workspaces.list as w (w.id)}
          <div class="wrow">
            <button class="item" onclick={() => workspaces.open(w.tabs)}>
              <LayoutGrid size={15} />
              <span class="nm">{w.name}</span>
              <span class="muted small">{w.tabs.length} tabs</span>
            </button>
            <button class="icon-btn" title="Remove the workspace" onclick={() => workspaces.remove(w)}><X size={13} /></button>
          </div>
        {:else}
          <p class="muted empty">A workspace opens a set of tabs in one go, such as everything for one customer.</p>
        {/each}
        <button class="item" onclick={() => workspaces.saveCurrent()}>
          <Plus size={15} />
          <span class="nm">Save the open tabs as a workspace</span>
        </button>
    </section>
  </div>
</div>

<style>
  .home {
    position: absolute;
    inset: 0;
    overflow: auto;
    background: var(--bg);
  }
  .inner {
    max-width: 820px;
    margin: 0 auto;
    padding: 48px 32px;
  }
  h1 {
    margin: 0;
    font-size: 26px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .lead {
    margin: 14px 0 18px;
  }
  kbd {
    padding: 0 5px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--raised);
    font-family: var(--font-ui);
    font-size: 12px;
  }
  .last {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    margin-top: 8px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--panel);
    color: var(--text);
    text-align: left;
  }
  .last:hover {
    border-color: var(--accent);
  }
  .last :global(svg) {
    color: var(--accent);
    flex-shrink: 0;
  }
  .last .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .ws {
    margin-top: 26px;
    max-width: 50%;
  }
  .wrow {
    display: flex;
    align-items: center;
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 32px;
    margin-top: 26px;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--faint);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 10px;
    border: none;
    background: none;
    border-radius: 3px;
    text-align: left;
    color: var(--text);
  }
  .item :global(svg) {
    color: var(--muted);
    flex-shrink: 0;
  }
  .item:hover {
    background: var(--hover);
  }
  .nm {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .small {
    font-size: 11px;
  }
  .empty {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
  }
  @media (max-width: 700px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
</style>

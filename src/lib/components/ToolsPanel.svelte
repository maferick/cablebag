<script lang="ts">
  import { app } from "../state.svelte";
  import { TOOLS } from "../tools";
  import { tunnels } from "../tunnels.svelte";

  const runningTunnels = $derived(Object.values(tunnels.status).filter((s) => s.state === "running").length);
</script>

<div class="tools">
  <section>
    {#each TOOLS as t}
      <!-- As the Settings and Tunnels buttons: a second press closes the tool and goes back to where you were. -->
      {@const shown = app.active?.type === "tool" && app.active.tool === t.id}
      <button class="tool" class:on={shown} aria-pressed={shown} onclick={() => app.toggleTool(t.id, t.name)} title={shown ? `Close ${t.name}` : t.description}>
        <t.icon size={15} />
        <span>{t.name}</span>
        {#if t.id === "tunnels" && runningTunnels}<em class="live">{runningTunnels} running</em>{/if}
      </button>
    {/each}
  </section>
</div>

<style>
  .tools {
    overflow: auto;
    padding: 0 6px 8px;
  }
  .tool {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    border: none;
    background: none;
    border-radius: 3px;
    text-align: left;
  }
  button.tool :global(svg) {
    color: var(--muted);
  }
  button.tool:hover {
    background: var(--hover);
  }
  button.tool.on {
    background: var(--hover);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .tool span {
    flex: 1;
  }
  em {
    font-style: normal;
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--panel-2);
  }
  em.live {
    color: var(--ok);
  }
</style>

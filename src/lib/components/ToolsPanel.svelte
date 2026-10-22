<script lang="ts">
  import { app } from "../state.svelte";
  import { TOOLS } from "../tools";
  import { tunnels } from "../tunnels.svelte";

  const runningTunnels = $derived(Object.values(tunnels.status).filter((s) => s.state === "running").length);
</script>

<div class="tools">
  <section>
    {#each TOOLS as t}
      <button class="tool" onclick={() => app.openTool(t.id, t.name)} title={t.description}>
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

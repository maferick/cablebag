<script lang="ts">
  // Which WireGuard network a session (or the sessions of a folder) goes through.
  import { wireguard } from "../wireguard.svelte";
  import { app } from "../state.svelte";

  /** inherited: the profile id a folder above gives, with that folder's name. */
  let { value = $bindable(""), inherited, from = "" }: { value?: string; inherited?: string; from?: string } = $props();
</script>

<div class="wgp">
  <select bind:value>
    <option value="">{inherited ? `As ${from}: ${wireguard.name(inherited)}` : "Directly from this PC"}</option>
    {#if inherited}<option value="none">Directly from this PC</option>{/if}
    {#each wireguard.list as p (p.id)}<option value={p.id}>WireGuard: {p.name}</option>{/each}
    {#if value && value !== "none" && !wireguard.list.some((p) => p.id === value)}<option value={value}>A WireGuard network that is gone</option>{/if}
  </select>
  {#if !wireguard.list.length}
    <button class="link" onclick={() => app.openTool("tunnels", "Tunnels")}>Add a WireGuard network under Tunnels</button>
  {/if}
</div>

<style>
  .wgp {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .wgp select {
    flex: 1;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent-text, var(--accent));
    cursor: pointer;
    font-size: 12px;
    white-space: nowrap;
  }
</style>

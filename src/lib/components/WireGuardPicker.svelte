<script lang="ts">
  // Which WireGuard network a session (or the sessions of a folder) goes through, with a new one made or imported
  // right here.
  import { wireguard } from "../wireguard.svelte";
  import type { WireGuardInfo } from "../api";
  import WireGuardEditor from "./WireGuardEditor.svelte";
  import { FileUp, Plus, Pencil } from "@lucide/svelte";

  /** inherited: the profile id a folder above gives, with that folder's name. */
  // No fallback for value: most sessions have no network set, and binding undefined to a prop with a fallback is
  // an error in Svelte that stops the whole window from updating.
  let { value = $bindable(), inherited, from = "" }: { value?: string; inherited?: string; from?: string } = $props();

  /** The network in the editor: undefined = closed, null = a new one. */
  let editing = $state<WireGuardInfo | null | undefined>(undefined);
  const chosen = $derived(wireguard.list.find((p) => p.id === value));

  async function importConf() {
    const id = await wireguard.importFile();
    if (id) value = id;
  }
</script>

<div class="wgp">
  <select value={value ?? ""} onchange={(e) => (value = e.currentTarget.value)}>
    <option value="">{inherited ? `As ${from}: ${wireguard.name(inherited)}` : "Directly from this PC"}</option>
    {#if inherited}<option value="none">Directly from this PC</option>{/if}
    {#each wireguard.list as p (p.id)}<option value={p.id}>WireGuard: {p.name}</option>{/each}
    {#if value && value !== "none" && !chosen}<option value={value}>A WireGuard network that is gone</option>{/if}
  </select>
  {#if chosen}
    <button class="btn" title="Change the settings of {chosen.name}" onclick={() => (editing = chosen)}><Pencil size={14} /> Edit</button>
  {/if}
  <button class="btn" title="Add the .conf file you use in the WireGuard app" onclick={importConf}><FileUp size={14} /> Import .conf</button>
  <button class="btn" title="Fill in a WireGuard network yourself" onclick={() => (editing = null)}><Plus size={14} /> New</button>
</div>

{#if editing !== undefined}
  <WireGuardEditor profile={editing} onclose={() => (editing = undefined)} onsaved={(id) => (value = id)} />
{/if}

<style>
  .wgp {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .wgp select {
    flex: 1;
    min-width: 0;
  }
  .wgp .btn {
    white-space: nowrap;
  }
</style>

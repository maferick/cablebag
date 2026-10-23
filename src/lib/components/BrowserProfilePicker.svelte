<script lang="ts">
  // Which browser profile (cookies and logins) a web session, or the web sessions of a folder, use; with a new one
  // made right here.
  import { browserProfiles, SHARED, PRIVATE, type BrowserProfile } from "../browserProfiles.svelte";
  import BrowserProfileEditor from "./BrowserProfileEditor.svelte";
  import { Plus, Pencil } from "@lucide/svelte";

  /** inherited: the profile a folder above gives, with that folder's name. No fallback for value (see WireGuardPicker). */
  let { value = $bindable(), inherited, from = "" }: { value?: string; inherited?: string; from?: string } = $props();

  /** The profile in the editor: undefined = closed, null = a new one. */
  let editing = $state<BrowserProfile | null | undefined>(undefined);
  const chosen = $derived(browserProfiles.get(value));
</script>

<div class="bpp">
  <select value={value ?? ""} onchange={(e) => (value = e.currentTarget.value)}>
    <option value="">{inherited ? `${browserProfiles.name(inherited)} (from ${from})` : "Cablebag's shared profile"}</option>
    {#if inherited}<option value={SHARED}>Cablebag's shared profile</option>{/if}
    {#each browserProfiles.list as p (p.id)}<option value={p.id}>Profile: {p.name}</option>{/each}
    <option value={PRIVATE}>Private (nothing kept once the last private tab closes)</option>
    {#if value && value !== SHARED && value !== PRIVATE && !chosen}<option value={value}>A browser profile that is gone</option>{/if}
  </select>
  {#if chosen}
    <button class="btn" title="Change the name or colour of {chosen.name}" onclick={() => (editing = chosen)}><Pencil size={14} /> Edit</button>
  {/if}
  <button class="btn" title="Make a browser profile with its own cookies and logins" onclick={() => (editing = null)}><Plus size={14} /> New</button>
</div>

{#if editing !== undefined}
  <BrowserProfileEditor profile={editing} onclose={() => (editing = undefined)} onsaved={(id) => (value = id)} />
{/if}

<style>
  .bpp {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .bpp select {
    flex: 1;
    min-width: 0;
  }
  .bpp .btn {
    white-space: nowrap;
  }
</style>

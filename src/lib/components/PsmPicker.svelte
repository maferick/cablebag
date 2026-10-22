<script lang="ts">
  // Whether a session (or the sessions of a folder) connects directly or through CyberArk PSM: SSH through PSM
  // for SSH, Remote Desktop through the PSM server, as set up in the CyberArk source.
  import { vault } from "../vault.svelte";
  import { app } from "../state.svelte";

  /** inherited: the CyberArk source id a folder above gives, with that folder's name. */
  // No fallback for value: binding undefined to a prop with a fallback stops the whole window from updating.
  let { value = $bindable(), inherited, from = "" }: { value?: string; inherited?: string; from?: string } = $props();

  const cyberark = $derived(vault.sources.filter((s) => s.kind === "cyberark"));
  const name = (id: string | undefined) => vault.source(id)?.name ?? "a CyberArk source that is gone";
  const chosen = $derived(cyberark.find((s) => s.id === value));
</script>

{#if cyberark.length || value || inherited}
  <div class="psm">
    <select value={value ?? ""} onchange={(e) => (value = e.currentTarget.value)}>
      <option value="">{inherited && inherited !== "none" ? `As ${from}: through PSM of ${name(inherited)}` : "Directly"}</option>
      {#if inherited && inherited !== "none"}<option value="none">Directly</option>{/if}
      {#each cyberark as s (s.id)}<option value={s.id}>Through CyberArk PSM: {s.name}</option>{/each}
      {#if value && value !== "none" && !chosen}<option value={value}>{name(value)}</option>{/if}
    </select>
  </div>
  {#if chosen}
    <p class="muted small">
      PSM logs in to the target with the account CyberArk holds, and records the session when your company has that on.
      The user name above is the target user; leave it empty to take the one of the linked CyberArk login.
    </p>
  {/if}
{:else}
  <p class="muted small">
    Add a CyberArk source under <button class="link" onclick={() => app.openSettings("vault")}>Credential sources</button> to connect through PSM.
  </p>
{/if}

<style>
  .psm {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .psm select {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .link {
    border: none;
    background: none;
    padding: 0;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }
</style>

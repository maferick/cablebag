<script lang="ts">
  import { vault } from "../vault.svelte";
  import { app } from "../state.svelte";
  import { KeyRound, Search, X, LockOpen } from "@lucide/svelte";

  let { itemId = $bindable(), hint = "" }: { itemId?: string; hint?: string } = $props();

  let query = $state("");
  let open = $state(false);
  let box = $state<HTMLDivElement>();

  const linked = $derived(vault.item(itemId));
  const q = $derived(query.trim().toLowerCase());
  const matches = $derived(
    vault.items
      .filter((i) => !q || [i.name, i.username, i.folder, i.organization, ...i.uris].some((v) => v.toLowerCase().includes(q)))
      .slice(0, 60),
  );

  function startSearch() {
    query = hint;
    open = true;
  }
</script>

<div class="vp" bind:this={box}>
  {#if !vault.status.configured}
    <p class="muted small">Set up the vault under Settings, Vault, to take logins from Vaultwarden.</p>
    <button class="btn" onclick={() => app.openSettings("vault")}>Set up the vault</button>
  {:else if itemId}
    <div class="linked">
      <KeyRound size={15} />
      {#if linked}
        <span class="name">{linked.name}</span>
        <span class="muted">{linked.username}</span>
      {:else if vault.status.unlocked}
        <span class="name warn">This vault item no longer exists</span>
      {:else}
        <span class="name">Linked to a vault item</span>
        <span class="muted">unlock the vault to see which</span>
      {/if}
      <span class="grow"></span>
      <button class="icon-btn" title="Remove the link" onclick={() => (itemId = "")}><X size={14} /></button>
    </div>
  {:else if !vault.status.unlocked}
    <button class="btn" disabled={vault.busy} onclick={async () => (await vault.unlock()) && startSearch()}>
      <LockOpen size={14} /> {vault.busy ? "Unlocking..." : "Unlock the vault to choose a login"}
    </button>
  {:else if !open}
    <button class="btn" onclick={startSearch}><KeyRound size={14} /> Choose a login from the vault</button>
  {:else}
    <div class="search">
      <Search size={14} />
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={query} placeholder="Name, user or address" autofocus onkeydown={(e) => e.key === "Escape" && (open = false)} />
      <button class="icon-btn" title="Close" onclick={() => (open = false)}><X size={14} /></button>
    </div>
    <div class="list">
      {#each matches as i (i.id)}
        <button
          class="item"
          onclick={() => {
            itemId = i.id;
            open = false;
          }}
        >
          <span class="name">{i.name}</span>
          <span class="muted">{i.username}</span>
          <span class="where">{[i.organization, i.folder].filter(Boolean).join(" / ") || i.uris[0] || ""}</span>
        </button>
      {:else}
        <p class="muted small pad">No logins match "{query}".</p>
      {/each}
    </div>
  {/if}
</div>

<style>
  .vp {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-start;
  }
  .linked {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--raised);
  }
  .linked :global(svg) {
    color: var(--muted);
  }
  .name {
    font-weight: 500;
  }
  .warn {
    color: var(--danger);
  }
  .grow {
    flex: 1;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    color: var(--faint);
  }
  .search input {
    flex: 1;
  }
  .list {
    width: 100%;
    max-height: 220px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--raised);
  }
  .item {
    display: grid;
    grid-template-columns: 1.3fr 1fr 1fr;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border: none;
    background: none;
    text-align: left;
  }
  .item:hover {
    background: var(--selected);
  }
  .item span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .where {
    color: var(--faint);
    font-size: 12px;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .pad {
    padding: 8px 10px;
  }
</style>

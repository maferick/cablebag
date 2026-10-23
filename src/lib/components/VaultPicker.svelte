<script lang="ts">
  import { vault } from "../vault.svelte";
  import { app } from "../state.svelte";
  import { api, type VaultItem } from "../api";
  import { KeyRound, Search, X, LockOpen } from "@lucide/svelte";

  let { itemId = $bindable(), hint = "" }: { itemId?: string; hint?: string } = $props();

  let query = $state("");
  let open = $state(false);
  /** What the servers of sources that did not load everything found for the query. */
  let fromServer = $state<VaultItem[]>([]);
  let searching = $state(false);

  const linked = $derived(vault.item(itemId) ?? fromServer.find((i) => i.id === itemId));
  const linkedSource = $derived(vault.sourceOf(itemId));
  const configured = $derived(vault.sources.filter((s) => s.configured));
  const openSources = $derived(configured.filter((s) => s.unlocked));
  const locked = $derived(configured.filter((s) => !s.unlocked));
  const q = $derived(query.trim().toLowerCase());
  const fits = (i: VaultItem) => !q || [i.name, i.username, i.folder, i.organization, ...i.uris].some((v) => v.toLowerCase().includes(q));
  /** The matches, grouped by source in the order of the sources. */
  const groups = $derived(
    openSources
      .map((s) => {
        const local = vault.items.filter((i) => i.source === s.id && fits(i));
        const extra = s.partial ? fromServer.filter((i) => i.source === s.id && !local.some((l) => l.id === i.id)) : [];
        return { source: s, items: [...local, ...extra].slice(0, 60) };
      })
      .filter((g) => g.items.length > 0),
  );

  // Sources that did not load every login are asked when the words change (after a short pause).
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const words = q;
    if (!open || !words || !openSources.some((s) => s.partial)) return;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      searching = true;
      try {
        fromServer = await api.vaultSearch(words);
      } catch {
        fromServer = [];
      } finally {
        searching = false;
      }
    }, 350);
    return () => clearTimeout(timer);
  });

  function startSearch() {
    query = hint;
    open = true;
  }
</script>

<div class="vp">
  {#if !configured.length}
    <p class="muted small">Add a credential source under Settings, Credential sources, to take logins from Vaultwarden, CyberArk or Secret Server.</p>
    <button class="btn" onclick={() => app.openSettings("vault")}>Set up a credential source</button>
  {:else if itemId}
    <div class="linked">
      <KeyRound size={15} />
      {#if linked}
        <span class="name">{linked.name}</span>
        <span class="muted">{linked.username}</span>
        {#if configured.length > 1 && linkedSource}<span class="where">{linkedSource.name}</span>{/if}
      {:else if !linkedSource}
        <span class="name warn">The credential source of this login is no longer set up</span>
      {:else if linkedSource.unlocked && !linkedSource.partial}
        <span class="name warn">This login no longer exists in {linkedSource.name}</span>
      {:else}
        <span class="name">Linked to a login in {linkedSource.name}</span>
        <span class="muted">{linkedSource.unlocked ? "" : `${linkedSource.kind === "vaultwarden" ? "unlock" : "sign in to"} it to see which`}</span>
      {/if}
      <span class="grow"></span>
      <button class="icon-btn" title="Remove the link" aria-label="Remove the link" onclick={() => (itemId = "")}><X size={14} /></button>
    </div>
  {:else if !openSources.length}
    <div class="btns">
      {#each locked as s (s.id)}
        <button class="btn" disabled={vault.busyIds.includes(s.id)} onclick={async () => (await vault.unlock(s.id)) && startSearch()}>
          <LockOpen size={14} />
          {vault.busyIds.includes(s.id) ? "Opening..." : `${s.kind === "vaultwarden" ? "Unlock" : "Sign in to"} ${s.name} to choose a login`}
        </button>
      {/each}
    </div>
  {:else if !open}
    <button class="btn" onclick={startSearch}><KeyRound size={14} /> Choose a login{configured.length > 1 ? "" : ` from ${openSources[0].name}`}</button>
  {:else}
    <div class="search">
      <Search size={14} />
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={query} placeholder="Name, user or address" autofocus onkeydown={(e) => { if (e.key === "Escape") { e.preventDefault(); open = false; } }} />
      <button class="icon-btn" title="Close" aria-label="Close" onclick={() => (open = false)}><X size={14} /></button>
    </div>
    <div class="list">
      {#each groups as g (g.source.id)}
        {#if openSources.length > 1}<div class="group">{g.source.name}</div>{/if}
        {#each g.items as i (i.id)}
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
        {/each}
      {:else}
        <p class="muted small pad">{searching ? "Searching..." : `No logins match "${query}".`}</p>
      {/each}
      {#if locked.length}
        <div class="more">
          {#each locked as s (s.id)}
            <button class="link" disabled={vault.busyIds.includes(s.id)} onclick={() => vault.unlock(s.id)}>
              <LockOpen size={12} /> {s.kind === "vaultwarden" ? "Unlock" : "Sign in to"} {s.name}
            </button>
          {/each}
        </div>
      {/if}
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
  .btns {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
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
    max-height: 260px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--raised);
  }
  .group {
    position: sticky;
    top: 0;
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    background: var(--raised);
    border-bottom: 1px solid var(--border);
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
  .more {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    padding: 6px 10px;
    border-top: 1px solid var(--border);
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: none;
    background: none;
    padding: 0;
    color: var(--accent);
    font-size: 12px;
    cursor: pointer;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .pad {
    padding: 8px 10px;
  }
</style>

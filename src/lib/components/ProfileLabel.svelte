<script lang="ts">
  // The browser profile a web tab uses, as a small label in its colour: on the tab and on the pane header, so
  // two tabs of the same site in different tenants never look alike.
  import { browserProfiles } from "../browserProfiles.svelte";
  import type { Tab } from "../state.svelte";

  let { tab }: { tab: Tab } = $props();
  const ident = $derived(browserProfiles.ofTab(tab));
</script>

{#if ident}<span class="plabel" style:--pc={ident.color} title="Browser profile: {ident.name}">{ident.name}</span>{/if}

<style>
  .plabel {
    flex-shrink: 0;
    max-width: 110px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 5px;
    border-radius: 3px;
    border-left: 3px solid var(--pc);
    background: color-mix(in srgb, var(--pc) 22%, transparent);
    color: var(--text);
    font-size: 11px;
    font-weight: 600;
    line-height: 16px;
  }
</style>

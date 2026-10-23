<script lang="ts">
  // Rarely used settings in a fold at the end of a section. It opens by itself while something inside differs from
  // its default (`changed`), so a changed setting is never out of sight.
  import type { Snippet } from "svelte";
  import { ChevronRight } from "@lucide/svelte";
  let { label = "More", changed = false, children }: { label?: string; changed?: boolean; children: Snippet } = $props();
  let open = $state(false);
  $effect(() => {
    if (changed) open = true;
  });
</script>

<div class="more">
  <button type="button" class="toggle" aria-expanded={open} onclick={() => (open = !open)}>
    <span class="chev" class:open><ChevronRight size={13} /></span>{label}
  </button>
  {#if open}<div class="body">{@render children()}</div>{/if}
</div>

<style>
  .more {
    border-top: 1px solid var(--border);
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 8px 14px;
    border: none;
    background: none;
    color: var(--muted);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .toggle:hover {
    color: var(--text);
    background: var(--hover);
  }
  .chev {
    display: inline-flex;
    transition: transform 0.12s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .body {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
  }
</style>

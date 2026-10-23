<script lang="ts">
  // A small (i) after a label: the longer explanation shows only when asked for (hover for a tooltip, click to keep
  // it open), so a page reads short once you know it.
  import { Info } from "@lucide/svelte";

  let { text, label = "More about this" }: { text: string; label?: string } = $props();
  let open = $state(false);
</script>

<button class="info" class:open type="button" title={text} aria-label={label} aria-expanded={open} onclick={() => (open = !open)}><Info size={12} /></button>
{#if open}<span class="text">{text}</span>{/if}

<style>
  .info {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    margin-left: 4px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--faint);
    vertical-align: -3px;
    cursor: pointer;
  }
  .info:hover,
  .info.open {
    color: var(--text);
    background: var(--hover);
  }
  .text {
    display: block;
    margin-top: 2px;
    font-size: 12px;
    color: var(--muted);
    line-height: 1.45;
  }
</style>

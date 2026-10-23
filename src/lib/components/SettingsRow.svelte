<script lang="ts">
  // One setting: its name and a short explanation on the left, the control on the right. With `stack` the
  // control takes the full width under the name (lists, long fields). `info` is the longer explanation, behind an
  // (i) after the name.
  import type { Snippet } from "svelte";
  import Info from "./Info.svelte";
  let { label = "", help = "", info = "", stack = false, children }: { label?: string; help?: string; info?: string; stack?: boolean; children: Snippet } = $props();
</script>

<div class="row" class:stack>
  {#if label || help}
    <div class="lbl">
      {#if label}<span class="name">{label}{#if info}<Info text={info} label={`More about ${label}`} />{/if}</span>{/if}
      {#if help}<span class="help">{help}</span>{/if}
    </div>
  {/if}
  <div class="ctl">{@render children()}</div>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: minmax(220px, 1fr) minmax(240px, 400px);
    align-items: center;
    gap: 6px 24px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
  }
  .row:last-child {
    border-bottom: none;
  }
  .row.stack {
    grid-template-columns: 1fr;
  }
  .lbl {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .name {
    font-size: 13px;
  }
  .help {
    font-size: 12px;
    color: var(--muted);
    line-height: 1.45;
  }
  .ctl {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row:not(.stack) .ctl {
    align-items: stretch;
  }
  /* A lone button or switch keeps its own size. */
  .ctl > :global(.btn),
  .ctl > :global(input.switch) {
    align-self: flex-start;
  }
  .ctl :global(input:not([type="checkbox"]):not([type="radio"])),
  .ctl :global(select) {
    width: 100%;
  }
  @media (max-width: 900px) {
    .row {
      grid-template-columns: 1fr;
    }
  }
</style>

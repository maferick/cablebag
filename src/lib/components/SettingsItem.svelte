<script lang="ts">
  // One thing in a list on a Settings page (a server, a profile, a saved password): a mark (icon or colour dot),
  // its name, one line of detail, and its buttons on the right. The same row everywhere, so every list reads alike.
  import type { Snippet } from "svelte";
  let { name, detail = "", mono = false, mark, actions }: { name: string; detail?: string; mono?: boolean; mark?: Snippet; actions?: Snippet } = $props();
</script>

<div class="item">
  {#if mark}<span class="mark">{@render mark()}</span>{/if}
  <div class="txt">
    <div class="name" class:mono>{name}</div>
    {#if detail}<div class="detail" title={detail}>{detail}</div>{/if}
  </div>
  {#if actions}<div class="acts">{@render actions()}</div>{/if}
</div>

<style>
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 8px 14px;
    border-bottom: 1px solid var(--border);
  }
  .item:last-child {
    border-bottom: none;
  }
  .mark {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--muted);
  }
  .txt {
    flex: 1;
    min-width: 0;
  }
  .name {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name.mono {
    font-family: var(--font-mono);
    font-weight: 400;
  }
  .detail {
    font-size: 12px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
</style>

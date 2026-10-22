<script lang="ts">
  // A group of settings on a Settings page: a card with a heading, an optional line about it, and rows.
  import type { Snippet } from "svelte";
  let { title, desc = "", children }: { title: string; desc?: string; children: Snippet } = $props();
</script>

<section class="sec">
  <header>
    <h3>{title}</h3>
    {#if desc}<p>{desc}</p>{/if}
  </header>
  <div class="rows">{@render children()}</div>
</section>

<style>
  .sec {
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--chrome);
  }
  header {
    padding: 10px 14px 8px;
    border-bottom: 1px solid var(--border);
  }
  h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }
  header p {
    margin: 3px 0 0;
    font-size: 12px;
    color: var(--muted);
    line-height: 1.45;
  }
  .rows {
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
</style>

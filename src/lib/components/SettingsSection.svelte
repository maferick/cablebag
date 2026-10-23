<script lang="ts">
  // A group of settings on a Settings page: a card with a heading, an optional line about it (`info` for the
  // longer explanation behind an (i)), and rows. `find` holds extra words "Find a setting" answers to with this
  // section, besides its title.
  import type { Snippet } from "svelte";
  import Info from "./Info.svelte";
  let { title, desc = "", info = "", find = "", children }: { title: string; desc?: string; info?: string; find?: string; children: Snippet } = $props();
</script>

<section class="sec" data-find={`${title} ${desc} ${find}`.toLowerCase()}>
  <header>
    <h3>{title}{#if info}<Info text={info} label={`More about ${title}`} />{/if}</h3>
    {#if desc}<p>{desc}</p>{/if}
  </header>
  <div class="rows">{@render children()}</div>
</section>

<style>
  .sec {
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--chrome);
    scroll-margin-top: 12px;
  }
  .sec:global(.found) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
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

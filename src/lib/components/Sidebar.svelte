<script lang="ts">
  // The session tree takes the height and is never replaced; Macros and Tools fold open at the bottom.
  import SessionTree from "./SessionTree.svelte";
  import ToolsPanel from "./ToolsPanel.svelte";
  import MacrosPanel from "./MacrosPanel.svelte";
  import { macros } from "../macros.svelte";
  import { ChevronRight } from "@lucide/svelte";

  const load = (k: string) => {
    try {
      return localStorage.getItem(k) === "1";
    } catch {
      return false;
    }
  };
  let open = $state({ macros: load("side.macros"), tools: load("side.tools") });

  function toggle(k: "macros" | "tools") {
    open[k] = !open[k];
    try {
      localStorage.setItem(`side.${k}`, open[k] ? "1" : "0");
    } catch {
      /* only a convenience */
    }
  }
</script>

<aside class="sidebar">
  <div class="tree">
    <SessionTree />
  </div>
  <section class="fold" class:open={open.macros}>
    <button class="fh" onclick={() => toggle("macros")} aria-expanded={open.macros}>
      <span class="chev"><ChevronRight size={13} /></span>Macros<span class="count">{macros.list.length}</span>
    </button>
    {#if open.macros}<div class="fb"><MacrosPanel /></div>{/if}
  </section>
  <section class="fold" class:open={open.tools}>
    <button class="fh" onclick={() => toggle("tools")} aria-expanded={open.tools}>
      <span class="chev"><ChevronRight size={13} /></span>Tools
    </button>
    {#if open.tools}<div class="fb"><ToolsPanel /></div>{/if}
  </section>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--panel);
    border-right: 1px solid var(--border);
  }
  .tree {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 120px;
  }
  .fold {
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .fold.open {
    max-height: 33%;
  }
  .fh {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 10px 0 6px;
    border: none;
    border-radius: 0;
    background: none;
    color: var(--muted);
    font-size: 12px;
    font-weight: 600;
    text-align: left;
  }
  .fh:hover {
    color: var(--text);
    background: var(--hover);
  }
  .chev {
    display: inline-flex;
    transition: transform 0.12s;
  }
  .fold.open .chev {
    transform: rotate(90deg);
  }
  .count {
    margin-left: auto;
    font-weight: 400;
    color: var(--faint);
  }
  .fb {
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
</style>

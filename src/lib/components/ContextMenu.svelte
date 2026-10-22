<script lang="ts">
  import { menu } from "../menu.svelte";

  let el = $state<HTMLDivElement>();
  let pos = $state({ x: 0, y: 0 });

  $effect(() => {
    if (!menu.open || !el) return;
    const r = el.getBoundingClientRect();
    pos = {
      x: Math.min(menu.x, window.innerWidth - r.width - 6),
      y: Math.min(menu.y, window.innerHeight - r.height - 6),
    };
  });
</script>

<svelte:window
  onmousedown={(e) => {
    if (menu.open && el && !el.contains(e.target as Node)) menu.close();
  }}
  onkeydown={(e) => {
    if (e.key === "Escape") menu.close();
  }}
  onblur={() => menu.close()}
/>

{#if menu.open}
  <div class="menu" bind:this={el} style:left="{pos.x || menu.x}px" style:top="{pos.y || menu.y}px" role="menu">
    {#each menu.items as item}
      {#if item === "sep"}
        <div class="sep"></div>
      {:else}
        <button
          class="item"
          class:danger={item.danger}
          disabled={item.disabled}
          role="menuitem"
          onclick={() => {
            menu.close();
            item.action?.();
          }}
        >
          <span class="ic">{#if item.icon}<item.icon size={14} />{/if}</span>
          <span class="lbl">{item.label}</span>
          {#if item.hint}<span class="hint">{item.hint}</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 1000;
    min-width: 200px;
    padding: 4px;
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: 3px;
    box-shadow: var(--shadow);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px 6px 8px;
    border: none;
    background: none;
    border-radius: 3px;
    text-align: left;
  }
  .item:hover:not(:disabled) {
    background: var(--selected);
  }
  .item:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .item.danger {
    color: var(--danger);
  }
  .ic {
    width: 16px;
    display: inline-flex;
    color: var(--muted);
  }
  .danger .ic {
    color: var(--danger);
  }
  .lbl {
    flex: 1;
  }
  .hint {
    color: var(--faint);
    font-size: 11px;
  }
  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }
</style>

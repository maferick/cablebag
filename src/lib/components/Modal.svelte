<script lang="ts">
  import { X } from "@lucide/svelte";
  import type { Snippet } from "svelte";

  let {
    title,
    width = 520,
    warning = false,
    onclose,
    children,
    footer,
  }: {
    title: string;
    width?: number;
    warning?: boolean;
    onclose?: () => void;
    children: Snippet;
    footer?: Snippet;
  } = $props();
</script>

<div class="backdrop" role="presentation">
  <div class="modal" class:warning style:width="{width}px" role="dialog" aria-modal="true" aria-label={title}>
    <header>
      <h2>{title}</h2>
      {#if onclose}
        <button class="icon-btn" title="Close" onclick={onclose}><X size={16} /></button>
      {/if}
    </header>
    <div class="body">
      {@render children()}
    </div>
    {#if footer}
      <footer>{@render footer()}</footer>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(20, 19, 17, 0.42);
  }
  .modal {
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 40px);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 3px;
    box-shadow: var(--shadow);
  }
  .modal.warning {
    border-top: 4px solid var(--danger);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 12px 8px 18px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .body {
    padding: 4px 18px 16px;
    overflow: auto;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid var(--border);
    background: var(--panel);
    border-radius: 0 0 10px 10px;
  }
</style>

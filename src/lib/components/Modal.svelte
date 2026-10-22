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

  // Keyboard: Escape closes the dialog on top (a field that uses Escape itself calls preventDefault), Tab stays
  // inside it, and focus goes into the dialog when it opens and back where it was when it closes.
  let box = $state<HTMLDivElement>();
  const me = Symbol("modal");

  const focusable = () =>
    [...(box?.querySelectorAll<HTMLElement>('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])') ?? [])].filter(
      (el) => !(el as HTMLButtonElement).disabled && el.offsetParent !== null,
    );

  onMount(() => {
    const before = document.activeElement as HTMLElement | null;
    stack.push(me);
    // After the content has had its own chance to focus a field.
    const t = setTimeout(() => {
      if (box && !box.contains(document.activeElement)) {
        const fields = focusable();
        (fields.find((el) => el.matches("input, select, textarea")) ?? fields.find((el) => !el.matches(".close")) ?? box).focus();
      }
    }, 60);
    return () => {
      clearTimeout(t);
      stack.splice(stack.indexOf(me), 1);
      if (before?.isConnected) before.focus();
    };
  });

  function onKey(e: KeyboardEvent) {
    if (stack[stack.length - 1] !== me || e.defaultPrevented) return;
    if (e.key === "Escape" && onclose) {
      e.preventDefault();
      onclose();
    } else if (e.key === "Tab") {
      const fields = focusable();
      if (!fields.length) return;
      const i = fields.indexOf(document.activeElement as HTMLElement);
      if (e.shiftKey && i <= 0) {
        e.preventDefault();
        fields[fields.length - 1].focus();
      } else if (!e.shiftKey && (i === -1 || i === fields.length - 1)) {
        e.preventDefault();
        fields[0].focus();
      }
    }
  }
</script>

<script lang="ts" module>
  import { onMount } from "svelte";
  /** Open dialogs, the newest last: only that one answers the keyboard. */
  const stack: symbol[] = [];
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" role="presentation">
  <div class="modal" class:warning style:width="{width}px" role="dialog" aria-modal="true" aria-label={title} tabindex="-1" bind:this={box}>
    <header>
      <h2>{title}</h2>
      {#if onclose}
        <button class="icon-btn close" title="Close" aria-label="Close" onclick={onclose}><X size={16} /></button>
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

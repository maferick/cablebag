<script lang="ts">
  import Modal from "./Modal.svelte";
  import { ask } from "../ask.svelte";

  const req = $derived(ask.current!);
  let value = $state(ask.current?.input?.value ?? "");
  let field = $state<HTMLInputElement>();
  let okButton = $state<HTMLButtonElement>();

  $effect(() => {
    setTimeout(() => {
      if (field) {
        field.focus();
        field.select();
      } else okButton?.focus();
    }, 20);
  });

  function ok() {
    ask.answer(req.input ? value.trim() || null : true);
  }
</script>


<Modal title={req.title} width={440} warning={req.danger} onclose={() => ask.answer(req.input ? null : false)}>
  {#if req.message}<p class="msg">{req.message}</p>{/if}
  {#if req.input}
    <form
      onsubmit={(e) => {
        e.preventDefault();
        ok();
      }}
    >
      <label class="field"><span>{req.input.label}</span><input bind:this={field} bind:value /></label>
    </form>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={() => ask.answer(req.input ? null : false)}>Cancel</button>
    <button class="btn" class:primary={!req.danger} class:danger={req.danger} bind:this={okButton} onclick={ok}>{req.okLabel}</button>
  {/snippet}
</Modal>

<style>
  .msg {
    margin: 0 0 6px;
    line-height: 1.5;
    white-space: pre-wrap;
  }
</style>

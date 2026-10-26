<script lang="ts">
  import Modal from "./Modal.svelte";
  import { ask } from "../ask.svelte";

  // The question stays readable while the box closes (ask.current is already empty then).
  let last = ask.current!;
  const req = $derived.by(() => (last = ask.current ?? last));
  const typed = $derived(!!(req.input || req.inputs));
  let value = $state(ask.current?.input?.value ?? "");
  let values = $state<Record<string, string>>(Object.fromEntries((ask.current?.inputs ?? []).map((f) => [f.name, f.value])));
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

  // Enter in any of the lines answers, as in a dialog with one line.
  function enter(e: KeyboardEvent) {
    if (e.key !== "Enter") return;
    e.preventDefault();
    ok();
  }

  function ok() {
    if (req.inputs) ask.answer(JSON.stringify($state.snapshot(values)));
    else ask.answer(req.input ? value.trim() || null : true);
  }
</script>


<Modal title={req.title} width={440} warning={req.danger} onclose={() => ask.answer(typed ? null : false)}>
  {#if req.message}<p class="msg">{req.message}</p>{/if}
  {#if req.input || req.inputs}
    <form
      onsubmit={(e) => {
        e.preventDefault();
        ok();
      }}
    >
      {#if req.inputs}
        {#each req.inputs as f, i (f.name)}
          {#if i === 0}
            <label class="field"><span>{f.label}</span><input bind:this={field} bind:value={values[f.name]} onkeydown={enter} spellcheck="false" autocomplete="off" /></label>
          {:else}
            <label class="field"><span>{f.label}</span><input bind:value={values[f.name]} onkeydown={enter} spellcheck="false" autocomplete="off" /></label>
          {/if}
        {/each}
      {:else if req.input}
        <label class="field"><span>{req.input.label}</span><input bind:this={field} bind:value /></label>
      {/if}
    </form>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={() => ask.answer(typed ? null : false)}>Cancel</button>
    <button class="btn" class:primary={!req.danger} class:danger={req.danger} bind:this={okButton} onclick={ok}>{req.okLabel}</button>
  {/snippet}
</Modal>

<style>
  .msg {
    margin: 0 0 6px;
    line-height: 1.5;
    white-space: pre-wrap;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
</style>

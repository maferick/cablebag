<script lang="ts">
  import Modal from "./Modal.svelte";
  import { api, type PromptRequest } from "../api";
  import { app } from "../state.svelte";
  import { writeText } from "../clipboard";
  import { Copy } from "@lucide/svelte";

  let { req }: { req: PromptRequest } = $props();
  let values = $state<Record<string, any>>({});
  let first = $state<HTMLInputElement>();

  $effect(() => {
    const v: Record<string, any> = {};
    for (const f of req.fields) v[f.name] = f.kind === "choice" ? ((f.value as any[])[0]?.id ?? "") : f.value;
    values = v;
    setTimeout(() => first?.focus(), 30);
  });

  function answer(button: string) {
    api.promptAnswer(req.id, { button, values: $state.snapshot(values) });
    app.prompts = app.prompts.filter((p) => p.id !== req.id);
  }

  const primary = $derived(req.buttons.find((b) => b.style === "primary")?.id ?? req.buttons[0]?.id);
</script>

<Modal title={req.title} warning={req.warning} width={req.details.length ? 600 : 460} onclose={() => answer("cancel")}>
  {#if req.message}
    <p class="msg">{req.message}</p>
  {/if}
  {#if req.details.length}
    <dl>
      {#each req.details as [k, v]}
        <dt>{k}</dt>
        <dd class="mono">
          <span>{v}</span>
          <button class="icon-btn" title="Copy" onclick={() => writeText(v)}><Copy size={13} /></button>
        </dd>
      {/each}
    </dl>
  {/if}
  <form
    onsubmit={(e) => {
      e.preventDefault();
      if (primary) answer(primary);
    }}
  >
    {#each req.fields as f, i}
      {#if f.kind === "choice"}
        <div class="choices" role="radiogroup" aria-label={f.label}>
          {#each f.value as o (o.id)}
            <label class="choice" class:on={values[f.name] === o.id}>
              <input type="radio" name={f.name} value={o.id} bind:group={values[f.name]} />
              <span>{o.label}</span>
            </label>
          {/each}
        </div>
      {:else if f.kind === "checkbox"}
        <label class="check"><input type="checkbox" bind:checked={values[f.name]} /> {f.label}</label>
      {:else if i === 0}
        <label class="field"><span>{f.label}</span>
          <input bind:this={first} type={f.kind === "password" ? "password" : "text"} bind:value={values[f.name]} autocomplete="off" />
        </label>
      {:else}
        <label class="field"><span>{f.label}</span>
          <input type={f.kind === "password" ? "password" : "text"} bind:value={values[f.name]} autocomplete="off" />
        </label>
      {/if}
    {/each}
    <button type="submit" hidden aria-hidden="true"></button>
  </form>
  {#snippet footer()}
    {#each [...req.buttons].reverse() as b}
      <button class="btn" class:primary={b.style === "primary"} class:danger={b.style === "danger"} onclick={() => answer(b.id)}>{b.label}</button>
    {/each}
  {/snippet}
</Modal>

<style>
  .choices {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 300px;
    overflow: auto;
    margin-bottom: 8px;
  }
  .choice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 3px;
    cursor: pointer;
  }
  .choice:hover {
    background: var(--hover);
  }
  .choice.on {
    background: var(--selected);
  }
  .msg {
    margin: 0 0 12px;
    line-height: 1.5;
    white-space: pre-wrap;
    user-select: text;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 14px;
    margin: 0 0 12px;
    padding: 10px 12px;
    background: var(--panel);
    border-radius: 3px;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    word-break: break-all;
    user-select: text;
    font-size: 12px;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
</style>

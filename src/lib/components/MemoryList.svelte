<script lang="ts">
  // The assistant's memory for one place (a session, a folder, general) or for everything, editable.
  import { memory, type Memory } from "../memory.svelte";
  import { Pencil, Trash2, Plus, Check, X } from "@lucide/svelte";

  let {
    scope,
    target = "",
    all = false,
    filter = "",
  }: { scope?: Memory["scope"]; target?: string; /** Every memory, grouped by where it belongs. */ all?: boolean; /** Only those holding these words. */ filter?: string } = $props();

  const words = $derived(filter.trim().toLowerCase().split(/\s+/).filter(Boolean));
  const list = $derived(
    (all ? [...memory.all].sort((a, b) => memory.label(a).localeCompare(memory.label(b)) || b.at - a.at) : scope ? memory.of(scope, target) : []).filter((m) =>
      words.every((w) => `${m.text} ${memory.label(m)}`.toLowerCase().includes(w)),
    ),
  );
  let editing = $state<string | null>(null);
  let text = $state("");
  let adding = $state("");

  async function saveEdit(m: Memory) {
    await memory.update(m.id, text);
    editing = null;
  }

  async function add() {
    if (!scope || !adding.trim()) return;
    await memory.add(scope, target, adding, "you");
    adding = "";
  }

  const when = (t: number) => new Date(t * 1000).toLocaleDateString();
</script>

{#if list.length}
  <ul class="mem">
    {#each list as m, i (m.id)}
      {#if all && (i === 0 || memory.label(list[i - 1]) !== memory.label(m))}
        <li class="group">{memory.label(m)}</li>
      {/if}
      <li>
        {#if editing === m.id}
          <input bind:value={text} onkeydown={(e) => e.key === "Enter" && saveEdit(m)} />
          <button class="icon-btn" title="Save" aria-label="Save" onclick={() => saveEdit(m)}><Check size={13} /></button>
          <button class="icon-btn" title="Cancel" aria-label="Cancel" onclick={() => (editing = null)}><X size={13} /></button>
        {:else}
          <span class="t">{m.text}</span>
          <span class="meta">{m.by === "assistant" ? "by the assistant" : "by you"}, {when(m.at)}</span>
          <button class="icon-btn" title="Edit" aria-label="Edit" onclick={() => ((editing = m.id), (text = m.text))}><Pencil size={13} /></button>
          <button class="icon-btn" title="Forget" aria-label="Forget" onclick={() => memory.remove(m.id)}><Trash2 size={13} /></button>
        {/if}
      </li>
    {/each}
  </ul>
{:else if words.length}
  <p class="muted small">Nothing remembered matches "{filter}".</p>
{:else}
  <p class="muted small">Nothing remembered yet. The assistant adds what it learns while it works; you can add facts yourself too.</p>
{/if}
{#if scope}
  <div class="add">
    <input bind:value={adding} placeholder="Add a fact, for example: the app runs in Docker under /opt/app" onkeydown={(e) => e.key === "Enter" && add()} />
    <button class="btn" disabled={!adding.trim()} onclick={add}><Plus size={13} /> Add</button>
  </div>
{/if}

<style>
  .mem {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .mem li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 10px;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }
  .mem li:last-child {
    border-bottom: none;
  }
  .mem li.group {
    padding-top: 6px;
    font-weight: 600;
    color: var(--muted);
    background: var(--chrome);
  }
  .t {
    flex: 1;
    line-height: 1.4;
  }
  .meta {
    color: var(--faint);
    white-space: nowrap;
  }
  .mem input {
    flex: 1;
  }
  .add {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  .add input {
    flex: 1;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
</style>

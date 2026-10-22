<script lang="ts">
  import { ask } from "../ask.svelte";
  import { app } from "../state.svelte";
  import { macros, describe, type Macro } from "../macros.svelte";
  import { menu } from "../menu.svelte";
  import MacroEditor from "./MacroEditor.svelte";
  import { Play, Circle, Plus, Pencil, Copy, Trash2, Keyboard } from "@lucide/svelte";

  let editing = $state<Macro | null>(null);

  function record() {
    const t = app.active;
    if (!t || t.type !== "terminal") {
      app.notify("Open a terminal tab first; the macro records what you type in it.", "error");
      return;
    }
    macros.startRecording(t.id);
  }

  function newMacro() {
    editing = { id: crypto.randomUUID(), name: `Macro ${macros.list.length + 1}`, steps: [{ type: "text", text: "" }] };
  }

  async function remove(m: Macro) {
    if (await ask.confirm("Delete macro", `Delete the macro "${m.name}"?`, { okLabel: "Delete", danger: true }))
      await macros.save(macros.list.filter((x) => x.id !== m.id));
  }

  async function duplicate(m: Macro) {
    await macros.save([...macros.list, { ...JSON.parse(JSON.stringify(m)), id: crypto.randomUUID(), name: `${m.name} (copy)`, hotkey: "" }]);
  }

  function rowMenu(e: MouseEvent, m: Macro) {
    menu.show(e, [
      { label: "Run in the active terminal", icon: Play, action: () => macros.run(m) },
      { label: "Edit", icon: Pencil, action: () => (editing = JSON.parse(JSON.stringify(m))) },
      { label: "Duplicate", icon: Copy, action: () => duplicate(m) },
      "sep",
      { label: "Delete", icon: Trash2, danger: true, action: () => remove(m) },
    ]);
  }
</script>

<div class="macros">
  <div class="actions">
    <button class="btn" onclick={record} disabled={!!macros.recording}><Circle size={14} /> Record</button>
    <button class="btn" onclick={newMacro}><Plus size={14} /> New</button>
  </div>
  {#if !macros.list.length}
    <p class="muted empty">
      No macros yet. Record one: press Record, type in a terminal, then press Stop. Run a macro with a click, or give
      it a hotkey.
    </p>
  {/if}
  <div class="list">
    {#each macros.list as m (m.id)}
      <div
        class="row"
        role="button"
        tabindex="0"
        title={m.steps.map(describe).join("\n")}
        onclick={() => macros.run(m)}
        onkeydown={(e) => e.key === "Enter" && macros.run(m)}
        oncontextmenu={(e) => rowMenu(e, m)}
      >
        <Play size={13} />
        <span class="name">{m.name}</span>
        {#if m.hotkey}<span class="key"><Keyboard size={11} /> {m.hotkey}</span>{/if}
      </div>
    {/each}
  </div>
</div>

{#if editing}
  {#key editing.id}
    <MacroEditor macro={editing} onclose={() => (editing = null)} />
  {/key}
{/if}

<style>
  .macros {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }
  .actions {
    display: flex;
    gap: 6px;
    padding: 8px;
  }
  .empty {
    padding: 4px 12px;
    line-height: 1.5;
    margin: 0;
  }
  .list {
    overflow: auto;
    padding: 0 4px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 3px;
    cursor: default;
    color: var(--muted);
  }
  .row:hover {
    background: var(--hover);
    color: var(--text);
  }
  .name {
    flex: 1;
    color: var(--text);
  }
  .key {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--panel-2);
  }
</style>

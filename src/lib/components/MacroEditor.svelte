<script lang="ts">
  import { untrack } from "svelte";
  import Modal from "./Modal.svelte";
  import { macros, blanks, type Macro, type MacroStep } from "../macros.svelte";
  import { app } from "../state.svelte";
  import { Plus, Trash2, ArrowUp, ArrowDown } from "@lucide/svelte";

  let { macro, onclose }: { macro: Macro; onclose: () => void } = $props();
  const initial = untrack(() => macro);
  let m = $state<Macro>(JSON.parse(JSON.stringify(initial)));
  let capturing = $state(false);
  const asked = $derived(blanks(m));
  // Folders by their path, for "Show for".
  const folders = $derived(
    app.folders
      .map((f) => ({ id: f.id, path: app.folderPath(f.id) }))
      .sort((a, b) => a.path.localeCompare(b.path)),
  );

  // Text steps are edited with real line breaks; the terminal receives Enter as a carriage return.
  const shown = (t: string) => t.replace(/\r/g, "\n");
  const stored = (t: string) => t.replace(/\r?\n/g, "\r");

  function add(type: MacroStep["type"]) {
    const s: MacroStep =
      type === "text" ? { type, text: "" } : type === "sleep" ? { type, ms: 500 } : { type, pattern: "\\$ $", timeout: 30000 };
    m.steps = [...m.steps, s];
  }

  function move(i: number, d: number) {
    const s = [...m.steps];
    const [x] = s.splice(i, 1);
    s.splice(i + d, 0, x);
    m.steps = s;
  }

  function scaleSleeps(f: number) {
    m.steps = m.steps.map((s) => (s.type === "sleep" ? { ...s, ms: Math.max(10, Math.round(s.ms * f)) } : s));
  }

  function captureKey(e: KeyboardEvent) {
    if (!capturing) return;
    e.preventDefault();
    if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;
    if (e.key === "Escape") {
      capturing = false;
      return;
    }
    if (!e.ctrlKey && !e.altKey) return;
    m.hotkey = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.key.length === 1 ? e.key.toUpperCase() : e.key]
      .filter(Boolean)
      .join("+");
    capturing = false;
  }

  async function save() {
    const list = macros.list.some((x) => x.id === m.id)
      ? macros.list.map((x) => (x.id === m.id ? ($state.snapshot(m) as Macro) : x))
      : [...macros.list, $state.snapshot(m) as Macro];
    await macros.save(list);
    onclose();
  }
</script>

<svelte:window onkeydown={captureKey} />

<Modal title="Edit macro" width={660} {onclose}>
  <div class="top">
    <label class="field grow"><span>Name</span><input bind:value={m.name} /></label>
    <label class="field"><span>Hotkey</span>
      <button class="btn hk" onclick={() => (capturing = true)}>{capturing ? "Press Ctrl or Alt + a key..." : m.hotkey || "None"}</button>
    </label>
    {#if m.hotkey}<button class="btn clear" onclick={() => (m.hotkey = "")}>No hotkey</button>{/if}
  </div>
  <label class="field for"><span>Show for</span>
    <select value={m.folder ?? ""} onchange={(e) => (m.folder = (e.target as HTMLSelectElement).value || undefined)}>
      <option value="">Every session</option>
      {#each folders as f (f.id)}<option value={f.id}>The sessions in {f.path}</option>{/each}
    </select>
  </label>

  <div class="steps">
    {#each m.steps as s, i}
      <div class="step">
        <span class="n">{i + 1}</span>
        {#if s.type === "text"}
          <span class="kind">Type</span>
          <textarea rows="2" value={shown(s.text)} oninput={(e) => (s.text = stored((e.target as HTMLTextAreaElement).value))}></textarea>
        {:else if s.type === "sleep"}
          <span class="kind">Pause</span>
          <input type="number" min="10" bind:value={s.ms} /> <span class="muted">milliseconds</span>
        {:else}
          <span class="kind">Wait for</span>
          <input class="grow" bind:value={s.pattern} placeholder="Text or regular expression" />
          <input type="number" class="tiny" bind:value={s.timeout} title="Give up after this many milliseconds" />
        {/if}
        <span class="tools">
          <button class="icon-btn" title="Move up" aria-label="Move up" disabled={i === 0} onclick={() => move(i, -1)}><ArrowUp size={13} /></button>
          <button class="icon-btn" title="Move down" aria-label="Move down" disabled={i === m.steps.length - 1} onclick={() => move(i, 1)}><ArrowDown size={13} /></button>
          <button class="icon-btn" title="Remove" aria-label="Remove" onclick={() => (m.steps = m.steps.filter((_, x) => x !== i))}><Trash2 size={13} /></button>
        </span>
      </div>
    {/each}
  </div>
  <div class="adds">
    <button class="btn" onclick={() => add("text")}><Plus size={13} /> Text</button>
    <button class="btn" onclick={() => add("sleep")}><Plus size={13} /> Pause</button>
    <button class="btn" onclick={() => add("wait")}><Plus size={13} /> Wait for text</button>
    <span class="grow"></span>
    <button class="btn" onclick={() => scaleSleeps(0.5)}>Halve all pauses</button>
    <button class="btn" onclick={() => scaleSleeps(2)}>Double all pauses</button>
  </div>
  <p class="muted small">
    A line break in a text step presses Enter. "Wait for text" waits until the terminal shows it, for example a
    prompt. Write <code>{"{{name}}"}</code> for a blank that Cablebag asks for when the macro runs;
    <code>{"{{host}}"}</code>, <code>{"{{user}}"}</code> and <code>{"{{session}}"}</code> it fills in itself.
    {#if asked.length}<br />This macro asks for: {asked.join(", ")}.{/if}
  </p>
  {#snippet footer()}
    <button class="btn" onclick={onclose}>Cancel</button>
    <button class="btn primary" onclick={save}>Save macro</button>
  {/snippet}
</Modal>

<style>
  .top {
    display: flex;
    gap: 10px;
    align-items: flex-end;
    margin-bottom: 14px;
  }
  .grow {
    flex: 1;
  }
  .hk {
    min-width: 200px;
  }
  .for {
    margin: -4px 0 14px;
  }
  code {
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .steps {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 340px;
    overflow: auto;
  }
  .step {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .n {
    width: 18px;
    color: var(--faint);
    font-size: 11px;
  }
  .kind {
    width: 62px;
    color: var(--muted);
  }
  .step textarea {
    flex: 1;
    font-family: var(--font-mono);
    resize: vertical;
  }
  .step input[type="number"] {
    width: 100px;
  }
  .tiny {
    width: 90px;
  }
  .tools {
    display: flex;
    margin-left: auto;
  }
  .adds {
    display: flex;
    gap: 6px;
    margin-top: 10px;
  }
  .small {
    font-size: 12px;
    margin: 10px 0 0;
  }
</style>

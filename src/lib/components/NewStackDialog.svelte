<script lang="ts">
  // A new stack of Cablebag's own: a compose file (from scratch or a template, with its {{blanks}} filled in) that
  // goes to ~/stacks/<name> on the server and is started with docker compose up -d.
  import Modal from "./Modal.svelte";
  import { api } from "../api";
  import { app } from "../state.svelte";
  import { ask } from "../ask.svelte";
  import { blankNamesIn, fillBlanks } from "../macros.svelte";
  import { templates, blankSecrets, secretName, type StackTemplate } from "../docker.svelte";
  import { FilePlus, Trash2, Layers } from "@lucide/svelte";

  let {
    tab,
    compose,
    sudo,
    onclose,
    oncreated,
  }: { tab: string; compose: string; sudo: boolean; onclose: () => void; oncreated: (name: string, output: string, ok: boolean) => void } = $props();

  const EXAMPLE = "services:\n  app:\n    image: nginx:alpine\n    restart: unless-stopped\n    ports:\n      - \"{{port}}:80\"\n";

  let chosen = $state<string>("");
  let name = $state("");
  let file = $state(EXAMPLE);
  let env = $state("");
  let values = $state<Record<string, string>>({});
  let busy = $state(false);
  let error = $state("");

  const slug = (s: string) => s.toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 63);
  const names = $derived(blankNamesIn(file, env));
  const missing = $derived(names.filter((n) => !values[n]?.trim()));

  function pick(t: StackTemplate | null) {
    chosen = t?.id ?? "";
    file = t ? t.compose : EXAMPLE;
    env = t?.env ?? "";
    if (t && !name) name = slug(t.name);
    error = "";
  }

  async function removeTemplate(t: StackTemplate) {
    if (await ask.confirm("Delete template", `Delete the template "${t.name}"? Stacks made from it stay.`, { okLabel: "Delete", danger: true })) {
      await templates.remove(t.id);
      if (chosen === t.id) pick(null);
    }
  }

  async function saveTemplate() {
    const n = await ask.text("Save as template", "Name of the template", name || "My stack", "Save");
    if (!n?.trim()) return;
    // Passwords typed into the file become blanks, so the template (and Sync) never holds them.
    await templates.save({ id: crypto.randomUUID(), name: n.trim(), compose: blankSecrets(file), env: blankSecrets(env) || undefined });
    app.notify(`Saved template "${n.trim()}"`);
  }

  async function create() {
    error = "";
    const n = slug(name);
    if (!n) return (error = "Give the stack a name.");
    if (missing.length) return (error = `Fill in ${missing.join(", ")}.`);
    busy = true;
    try {
      const r = await api.dockerCreateStack(tab, compose, sudo, n, fillBlanks(file, values), fillBlanks(env, values));
      oncreated(n, `Files in ${r.dir}\n\n${r.output.trim() || "(no output)"}`, r.exit === 0);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title="New stack" width={980} {onclose}>
  <div class="grid">
    <nav class="tpl" aria-label="Start from">
      <button class="item" class:on={chosen === ""} onclick={() => pick(null)}><FilePlus size={14} /> <span>Empty</span></button>
      {#if templates.list.length}<div class="head">Templates</div>{/if}
      {#each templates.list as t (t.id)}
        <div class="item" class:on={chosen === t.id} role="button" tabindex="0" onclick={() => pick(t)} onkeydown={(e) => e.key === "Enter" && pick(t)}>
          <Layers size={14} />
          <span class="nm" title={t.description ?? t.name}>{t.name}</span>
          <button class="icon-btn del" title="Delete template" aria-label={`Delete template ${t.name}`} onclick={(e) => { e.stopPropagation(); removeTemplate(t); }}><Trash2 size={13} /></button>
        </div>
      {/each}
      {#if !templates.list.length}
        <p class="muted small hint">Save a stack as a template here, or a running one from its details.</p>
      {/if}
    </nav>
    <div class="form">
      <label class="field"><span>Name</span><input bind:value={name} placeholder="my-app" spellcheck="false" /></label>
      {#if slug(name)}<p class="muted small where">Its files go to <span class="mono">~/stacks/{slug(name)}</span> on the server.</p>{/if}
      <label class="field"><span>Compose file</span><textarea class="mono" rows="14" bind:value={file} spellcheck="false"></textarea></label>
      <label class="field"><span>.env (optional)</span><textarea class="mono" rows="3" bind:value={env} spellcheck="false" placeholder="TZ=Europe/Amsterdam"></textarea></label>
      {#if names.length}
        <div class="blanks">
          {#each names as n (n)}
            <label class="field"><span>{n}</span>
              {#if secretName(n)}<input type="password" bind:value={values[n]} autocomplete="new-password" />{:else}<input bind:value={values[n]} spellcheck="false" autocomplete="off" />{/if}
            </label>
          {/each}
        </div>
      {/if}
      <p class="muted small">Write <code>{"{{name}}"}</code> for something to fill in when the stack is made, as in macros.</p>
      {#if error}<p class="bad">{error}</p>{/if}
    </div>
  </div>
  {#snippet footer()}
    <button class="btn" onclick={saveTemplate} disabled={!file.trim()}>Save as template</button>
    <span class="grow"></span>
    <button class="btn" onclick={onclose}>Cancel</button>
    <button class="btn primary" disabled={busy} onclick={create}>{busy ? "Starting..." : "Create and start"}</button>
  {/snippet}
</Modal>

<style>
  .grid {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 18px;
  }
  .tpl {
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-right: 1px solid var(--border);
    padding-right: 12px;
  }
  .head {
    margin: 10px 8px 4px;
    font-size: 12px;
    color: var(--muted);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: none;
    border-radius: 3px;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .item:hover {
    background: var(--hover);
  }
  .item.on {
    background: var(--hover);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .nm {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .del {
    opacity: 0;
  }
  .item:hover .del,
  .item:focus-within .del {
    opacity: 1;
  }
  .hint {
    margin: 10px 8px;
    line-height: 1.5;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field > span {
    font-size: 12px;
    color: var(--muted);
  }
  textarea {
    resize: vertical;
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .where {
    margin: -6px 0 0;
  }
  .blanks {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 10px;
  }
  .small {
    font-size: 12px;
  }
  p {
    margin: 0;
  }
  .bad {
    color: var(--danger-text);
  }
  .grow {
    flex: 1;
  }
  code {
    font-family: var(--font-mono);
    font-size: 11px;
  }
</style>

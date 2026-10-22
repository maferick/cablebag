<script lang="ts">
  // Settings, Assistant: the AI hosts and the log of what was sent to them.
  import { invoke } from "@tauri-apps/api/core";
  import { assistant, type AiHost, type AiModel, type AiLogEntry } from "../assistant.svelte";
  import { ask } from "../ask.svelte";
  import { app } from "../state.svelte";
  import VaultPicker from "./VaultPicker.svelte";
  import { Plus, Pencil, Trash2, RefreshCw, ChevronRight } from "@lucide/svelte";

  let editing = $state<(AiHost & { key: string; keyFrom: "vault" | "pc" | "none" }) | null>(null);
  let models = $state<Record<string, AiModel[]>>({});
  let testing = $state<string | null>(null);
  let testError = $state<Record<string, string>>({});
  let log = $state<AiLogEntry[]>([]);
  let openLog = $state<number | null>(null);
  let error = $state("");

  $effect(() => {
    assistant.load();
    loadLog();
  });

  async function loadLog() {
    log = await invoke<AiLogEntry[]>("ai_log", { limit: 100 }).catch(() => []);
  }

  function blank() {
    editing = { id: "", name: "", base_url: "", vault_item: "", model: "", vision_model: "", protected_ok: false, key: "", keyFrom: "vault" };
    error = "";
  }

  function edit(h: AiHost) {
    editing = { ...h, key: "", keyFrom: h.vault_item ? "vault" : h.has_key ? "pc" : "none" };
    error = "";
  }

  async function save() {
    if (!editing) return;
    const { key, keyFrom, has_key, ...host } = editing;
    if (keyFrom !== "vault") host.vault_item = "";
    try {
      const saved = await invoke<AiHost>("ai_save_host", { host, key: keyFrom === "pc" && key ? key : null });
      editing = null;
      await assistant.load();
      test(saved.id);
    } catch (e) {
      error = String(e);
    }
  }

  async function remove(h: AiHost) {
    if (!(await ask.confirm("Remove AI host", `Remove "${h.name}"? A key kept on this PC for it is removed too.`, { okLabel: "Remove", danger: true }))) return;
    await invoke("ai_delete_host", { id: h.id });
    await assistant.load();
  }

  async function test(id: string) {
    testing = id;
    testError[id] = "";
    try {
      models[id] = await invoke<AiModel[]>("ai_models", { id });
      const h = assistant.hosts.find((x) => x.id === id);
      // Pick a model when none is chosen yet: the first one that can call tools.
      if (h && !h.model && models[id].length) {
        const first = models[id].find((m) => m.tools) ?? models[id][0];
        await setModel(h, "model", first.id);
        if (!h.vision_model && first.vision) await setModel(h, "vision_model", first.id);
      }
    } catch (e) {
      testError[id] = String(e);
    } finally {
      testing = null;
    }
  }

  async function setModel(h: AiHost, field: "model" | "vision_model", value: string) {
    const { has_key, ...host } = { ...h, [field]: value };
    await invoke("ai_save_host", { host, key: null });
    await assistant.load();
  }

  const can = (v: boolean | null, yes: string) => (v === true ? yes : v === false ? `no ${yes.toLowerCase()}` : "");
  const time = (t: number) => new Date(t * 1000).toLocaleString();
</script>

<p class="muted small">
  Any service with an OpenAI-compatible API works: LiteLLM, vLLM, Ollama, a cloud API. Before anything is sent,
  Cablebag replaces passwords it knows and text that looks like a secret; the log below shows exactly what went out.
</p>

{#each assistant.hosts as h (h.id)}
  <div class="host">
    <div class="hhead">
      <strong>{h.name}</strong>
      <span class="muted small mono">{h.base_url}</span>
      <span class="grow"></span>
      <button class="btn" disabled={testing === h.id} onclick={() => test(h.id)}><RefreshCw size={13} /> {testing === h.id ? "Testing..." : "Test"}</button>
      <button class="icon-btn" title="Edit" onclick={() => edit(h)}><Pencil size={14} /></button>
      <button class="icon-btn" title="Remove" onclick={() => remove(h)}><Trash2 size={14} /></button>
    </div>
    <div class="hbody">
      <span class="muted small">Key: {h.vault_item ? "from the vault" : h.has_key ? "kept on this PC" : "none"}{h.protected_ok ? " · may see protected folders" : ""}</span>
      {#if testError[h.id]}<p class="err small">{testError[h.id]}</p>{/if}
      <div class="row">
        <label class="field grow"><span>Model for questions and commands</span>
          <select value={h.model} onchange={(e) => setModel(h, "model", (e.target as HTMLSelectElement).value)}>
            {#if !models[h.id]}<option value={h.model}>{h.model || "Press Test to load the models"}</option>{/if}
            {#each models[h.id] ?? [] as m}<option value={m.id}>{m.id}{m.tools === false ? " (no tools)" : ""}</option>{/each}
          </select>
        </label>
        <label class="field grow"><span>Model for screens (Remote Desktop, VNC)</span>
          <select value={h.vision_model} onchange={(e) => setModel(h, "vision_model", (e.target as HTMLSelectElement).value)}>
            <option value="">None</option>
            {#if !models[h.id] && h.vision_model}<option value={h.vision_model}>{h.vision_model}</option>{/if}
            {#each models[h.id] ?? [] as m}<option value={m.id}>{m.id}{m.vision === false ? " (no images)" : ""}</option>{/each}
          </select>
        </label>
      </div>
      {#if models[h.id]}
        <ul class="models small">
          {#each models[h.id] as m}
            <li><span class="mono">{m.id}</span> <span class="muted">{[can(m.tools, "Tools"), can(m.vision, "Images"), m.context ? `${Math.round(m.context / 1000)}k context` : ""].filter(Boolean).join(" · ") || "the host does not say what it can do"}</span></li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
{/each}

{#if editing}
  <div class="host form">
    <div class="row">
      <label class="field grow"><span>Name</span><input bind:value={editing.name} placeholder="Work LLM" /></label>
      <label class="field grow2"><span>Address of the API</span><input bind:value={editing.base_url} placeholder="https://llm.example.com/v1" /></label>
    </div>
    <div class="field"><span>Key</span>
      <div class="radios">
        <label class="check"><input type="radio" bind:group={editing.keyFrom} value="vault" /> From the vault</label>
        <label class="check"><input type="radio" bind:group={editing.keyFrom} value="pc" /> Kept on this PC</label>
        <label class="check"><input type="radio" bind:group={editing.keyFrom} value="none" /> No key</label>
      </div>
    </div>
    {#if editing.keyFrom === "vault"}
      <VaultPicker bind:itemId={editing.vault_item} hint={editing.name} />
      <p class="muted small">The password of the vault item is used as the key.</p>
    {:else if editing.keyFrom === "pc"}
      <label class="field"><span>API key</span><input type="password" bind:value={editing.key} placeholder={editing.has_key ? "Kept; type a new one to replace it" : "Paste the key"} /></label>
    {/if}
    <label class="check"><input type="checkbox" bind:checked={editing.protected_ok} /> May see sessions in protected folders (for example your company's own AI host)</label>
    {#if error}<p class="err small">{error}</p>{/if}
    <div class="row end">
      <button class="btn" onclick={() => (editing = null)}>Cancel</button>
      <button class="btn primary" disabled={!editing.name.trim() || !editing.base_url.trim()} onclick={save}>Save and test</button>
    </div>
  </div>
{:else}
  <div><button class="btn" onclick={blank}><Plus size={14} /> Add an AI host</button></div>
{/if}

<h4>What was sent</h4>
{#if !log.length}
  <p class="muted small">Nothing yet. Every question, with the screen text that went with it, and every answer is listed here.</p>
{:else}
  <div class="log">
    {#each log as l, i}
      <button class="lrow" onclick={() => (openLog = openLog === i ? null : i)}>
        <span class="chev" class:open={openLog === i}><ChevronRight size={12} /></span>
        <span class="lt">{time(l.at)}</span>
        <span class="ld {l.direction}">{l.direction === "sent" ? "Sent" : l.direction === "received" ? "Answer" : "Error"}</span>
        <span class="la">{l.host} · {l.about}</span>
      </button>
      {#if openLog === i}<pre class="ltext">{l.text}</pre>{/if}
    {/each}
  </div>
{/if}

<style>
  .host {
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .hhead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 6px 6px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--chrome);
  }
  .hbody,
  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px 12px;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: flex-end;
  }
  .row.end {
    justify-content: flex-end;
  }
  .grow {
    flex: 1;
  }
  .grow2 {
    flex: 2;
  }
  .radios {
    display: flex;
    gap: 18px;
  }
  .models {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .small {
    font-size: 12px;
  }
  .err {
    color: var(--danger-text);
    margin: 0;
  }
  h4 {
    margin: 12px 0 0;
    font-size: 13px;
  }
  .log {
    border: 1px solid var(--border);
    border-radius: 3px;
    max-height: 360px;
    overflow: auto;
  }
  .lrow {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    border: none;
    border-bottom: 1px solid var(--border);
    background: none;
    text-align: left;
    font-size: 12px;
  }
  .lrow:hover {
    background: var(--hover);
  }
  .chev {
    display: inline-flex;
    color: var(--muted);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .lt {
    color: var(--muted);
    white-space: nowrap;
  }
  .ld {
    width: 52px;
  }
  .ld.error {
    color: var(--danger-text);
  }
  .la {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ltext {
    margin: 0;
    padding: 8px 10px;
    max-height: 260px;
    overflow: auto;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    font-family: var(--font-mono);
    font-size: 11px;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }
</style>

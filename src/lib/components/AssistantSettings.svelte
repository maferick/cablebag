<script lang="ts">
  // Settings, Assistant: the AI hosts, and what the assistant may do by itself (its memory and log have pages of
  // their own: Assistant memory in SettingsDialog, AssistantLog).
  import { invoke } from "@tauri-apps/api/core";
  import { assistant, type AiHost, type AiModel } from "../assistant.svelte";
  import { ask } from "../ask.svelte";
  import { app } from "../state.svelte";
  import VaultPicker from "./VaultPicker.svelte";
  import SettingsSection from "./SettingsSection.svelte";
  import SettingsRow from "./SettingsRow.svelte";
  import SettingsMore from "./SettingsMore.svelte";
  import { Plus, Pencil, Trash2, RefreshCw, X } from "@lucide/svelte";

  let editing = $state<(AiHost & { key: string; keyFrom: "vault" | "pc" | "none" }) | null>(null);
  let models = $state<Record<string, AiModel[]>>({});
  let testing = $state<string | null>(null);
  let testError = $state<Record<string, string>>({});
  let error = $state("");
  const protectedFolders = $derived(app.folders.filter((f) => ((app.settings["ai.protected"] as string[] | undefined) ?? []).includes(f.id)));

  $effect(() => {
    assistant.load();
  });

  /** How much goes to the AI and how far it may go on one question; under More, with the unit in words. */
  const LIMITS = [
    { key: "ai.screenLines", label: "Terminal text sent with a question", unit: "lines", info: "The last lines of the terminal you are in.", def: 300 },
    { key: "ai.maxSteps", label: "Commands per question", unit: "commands", info: "It stops and explains where it is when it reaches this.", def: 25 },
    { key: "ai.maxMinutes", label: "Time per question", unit: "minutes", info: "Also a stop with an explanation.", def: 15 },
    { key: "ai.outputKB", label: "Command output it reads", unit: "KB per command", info: "You always see all of the output in the card.", def: 48 },
    { key: "ai.memoryChars", label: "Memory that goes along", unit: "characters", info: "What it remembers about this connection, its folder and in general, newest first.", def: 6000 },
    { key: "ai.contextChars", label: "Conversation size", unit: "characters", info: "Above this, older questions lose their terminal text and the oldest turns are left out. Raise it for models with a large context.", def: 120000 },
  ];
  const limitsChanged = $derived(LIMITS.some((l) => app.settings[l.key] !== undefined && Number(app.settings[l.key]) !== l.def));

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
</script>

<SettingsSection title="AI hosts" desc="Any service with an OpenAI-compatible API: LiteLLM, vLLM, Ollama or a cloud API." info="Before anything is sent, Cablebag hides the passwords it knows and text that looks like a secret." find="host model openai litellm ollama vllm key">
  <div class="inner">
{#each assistant.hosts as h (h.id)}
  <div class="host">
    <div class="hhead">
      <strong>{h.name}</strong>
      <span class="muted small mono">{h.base_url}</span>
      <span class="grow"></span>
      <button class="btn" disabled={testing === h.id} onclick={() => test(h.id)}><RefreshCw size={13} /> {testing === h.id ? "Testing..." : "Test"}</button>
      <button class="icon-btn" title="Edit" aria-label="Edit" onclick={() => edit(h)}><Pencil size={14} /></button>
      <button class="icon-btn" title="Remove" aria-label="Remove" onclick={() => remove(h)}><Trash2 size={14} /></button>
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
  </div>
</SettingsSection>

<SettingsSection title="What it may do by itself" desc="For every server and screen it works on." find="mode ask free dangerous protected folders always allowed rules snapshot limits commands minutes">
  <div class="modes" role="radiogroup" aria-label="What the assistant may do by itself">
    {#each [{ id: "ask", name: "Read by itself, ask before every change", help: "Looking around (status, logs, listings) runs at once. Anything that changes something waits for Run it, Always or Refuse." }, { id: "free", name: "Do what is needed, ask only before dangerous commands", help: "Changes, root and actions on a screen run without asking. Dangerous commands, such as rm -rf / or wiping a disk, still wait for you." }] as m}
      <label class="mode" class:on={assistant.mode === m.id}>
        <input type="radio" name="ai-mode" value={m.id} checked={assistant.mode === m.id} onchange={() => app.setSetting("ai.mode", m.id)} />
        <span><span class="name">{m.name}</span><span class="help">{m.help}</span></span>
      </label>
    {/each}
  </div>
  <SettingsRow label="Snapshot of a Proxmox guest before the first change" info="When the assistant works inside a VM or container from its console tab: taken before the first command in a conversation that changes something, so you can roll back (the guest's Snapshots in the tree). With 'ask only before dangerous commands' it is taken without asking.">
    <select value={app.settings["ai.snapshot"] ?? "ask"} onchange={(e) => app.setSetting("ai.snapshot", (e.target as HTMLSelectElement).value)}>
      <option value="ask">Offer it</option>
      <option value="always">Always take one</option>
      <option value="never">Don't</option>
    </select>
  </SettingsRow>
  <SettingsRow label={`Protected folders (${protectedFolders.length})`} info="In a protected folder every command asks first, dangerous commands are refused, and only AI hosts that may see protected folders are used." stack>
    {#if protectedFolders.length}
      <ul class="plain">
        {#each protectedFolders as f (f.id)}
          <li><span>{app.folderPath(f.id)}</span><button class="icon-btn" title="Lift the protection" aria-label="Lift the protection" onclick={() => assistant.setProtected(f.id, false)}><X size={13} /></button></li>
        {/each}
      </ul>
    {:else}
      <p class="muted small">None yet. Right-click a folder in the session tree and choose to protect it from the assistant.</p>
    {/if}
  </SettingsRow>
  <SettingsRow label={`Always allowed (${assistant.rules.length})`} info="Kinds of commands you answered Always for: they run without asking on that server, never in a protected folder and never when dangerous." stack>
    {#if assistant.rules.length}
      <ul class="plain">
        {#each assistant.rules as r (r.kind + r.host)}
          <li><span><span class="mono">{r.kind}</span> on {r.host}</span><button class="icon-btn" title="Ask again from now on" aria-label="Ask again from now on" onclick={() => assistant.removeRule(r)}><X size={13} /></button></li>
        {/each}
      </ul>
    {:else}
      <p class="muted small">None yet. When the assistant asks before a change, Always adds that kind of command here.</p>
    {/if}
  </SettingsRow>
  <SettingsMore label="Limits per question" changed={limitsChanged}>
    {#each LIMITS as l}
      <SettingsRow label={l.label} info={l.info}>
        <div class="unit">
          <input type="number" min="1" value={app.settings[l.key] ?? l.def} onchange={(e) => app.setSetting(l.key, Number((e.target as HTMLInputElement).value) || l.def)} />
          <span class="muted small">{l.unit}</span>
        </div>
      </SettingsRow>
    {/each}
  </SettingsMore>
</SettingsSection>

<style>
  .unit {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .unit input {
    flex: 1;
  }
  .unit span {
    white-space: nowrap;
  }
  .modes {
    display: flex;
    flex-direction: column;
  }
  .mode {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
    cursor: pointer;
  }
  .mode:last-child {
    border-bottom: none;
  }
  .mode.on {
    background: var(--selected);
  }
  .mode input {
    margin-top: 2px;
  }
  .mode > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .mode .name {
    font-size: 13px;
  }
  .mode .help {
    font-size: 12px;
    color: var(--muted);
    line-height: 1.45;
  }
  .inner {
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
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
  p.small {
    margin: 0;
  }
  .err {
    color: var(--danger-text);
    margin: 0;
  }
  .plain {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .plain li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 4px 3px 10px;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }
  .plain li:last-child {
    border-bottom: none;
  }
</style>

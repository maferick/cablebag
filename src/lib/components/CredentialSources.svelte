<script lang="ts">
  // Settings, Credential sources: the vaults and password managers logins come from, any number of each kind.
  import { api, type SourceConfig, type SourceKind, type SourceStatus } from "../api";
  import { app } from "../state.svelte";
  import { ask } from "../ask.svelte";
  import { vault, SOURCE_KINDS } from "../vault.svelte";
  import SettingsSection from "./SettingsSection.svelte";
  import SettingsRow from "./SettingsRow.svelte";
  import CyberArkFields from "./sources/CyberArkFields.svelte";
  import DelineaFields from "./sources/DelineaFields.svelte";
  import ProtonPassFields from "./sources/ProtonPassFields.svelte";
  import { Plus, Pencil, Trash2, Lock, LockOpen, RefreshCw, ArrowUp, Undo2 } from "@lucide/svelte";

  let editing = $state<SourceConfig | null>(null);
  let error = $state("");

  const kindLabel = (k: SourceKind) => SOURCE_KINDS.find((x) => x.kind === k)?.label ?? k;
  const opens = (s: SourceStatus) => (s.kind === "vaultwarden" ? "Unlock" : "Sign in");
  const closes = (s: SourceStatus) => (s.kind === "vaultwarden" ? "Lock" : "Sign out");

  function line(s: SourceStatus): string {
    if (!s.configured) return `${kindLabel(s.kind)}: not set up completely yet.`;
    const where = s.url.replace(/^https:\/\//, "");
    const who = s.account ? `${s.account} on ${where}` : where;
    if (!s.unlocked) return `${who} · ${s.kind === "vaultwarden" ? "locked" : "signed out"}${s.remembered ? ", this PC keeps the key" : ""}`;
    const when = s.synced ? `, loaded ${new Date(s.synced * 1000).toLocaleTimeString()}` : "";
    const count = s.partial ? `the first ${s.items} logins (searching asks the server for the rest)` : `${s.items} logins`;
    return [`${who} · ${count}${when}`, s.detail].filter(Boolean).join(" · ");
  }

  function add(kind: SourceKind) {
    error = "";
    editing = { id: "", name: "", kind, url: "", account: "", options: {} };
  }

  function edit(s: SourceStatus) {
    error = "";
    editing = { id: s.id, name: s.name, kind: s.kind, url: s.url, account: s.account, options: { ...(s.options ?? {}) } };
  }

  async function save() {
    if (!editing) return;
    if (!editing.url.trim() && editing.kind !== "protonpass") {
      error = "Fill in the address of the server.";
      return;
    }
    if (editing.kind === "vaultwarden" && !editing.account.trim()) {
      error = "Fill in the email address of the vault account.";
      return;
    }
    try {
      const saved = await vault.save($state.snapshot(editing) as SourceConfig);
      // Where pass-cli is stays a setting of this PC (the backend keeps it out of the synced list).
      if (editing.kind === "protonpass") app.settings[`protonpass.cli.${saved.id}`] = String(editing.options?.cli ?? "").trim().replace(/^"|"$/g, "");
      app.notify(`${saved.name} saved`);
      editing = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function remove(s: SourceStatus) {
    const ok = await ask.confirm(
      `Remove ${s.name}?`,
      "Sessions linked to its logins keep the link, but it no longer works. What this PC kept for it is removed.",
      { okLabel: "Remove", danger: true },
    );
    if (ok) await vault.remove(s.id);
  }

  async function up(i: number) {
    const ids = vault.sources.map((s) => s.id);
    [ids[i - 1], ids[i]] = [ids[i], ids[i - 1]];
    await vault.reorder(ids);
  }
</script>

<SettingsSection title="Sources" desc="Sessions, folders, Proxmox servers, WireGuard networks and AI hosts can take their login from these. What is opened stays in memory only; passwords never go into the sync.">
  {#each vault.sources as s, i (s.id)}
    <SettingsRow label={s.name} help={line(s)}>
      <div class="btns end">
        {#if i > 0}<button class="icon-btn" title="Move up: the first source opens when a connect needs a login and none is open" aria-label="Move up: the first source opens when a connect needs a login and none is open" onclick={() => up(i)}><ArrowUp size={14} /></button>{/if}
        {#if s.configured}
          {#if s.unlocked}
            <button class="btn" disabled={vault.busyIds.includes(s.id)} title="Load the logins again" onclick={() => vault.sync(s.id)}><RefreshCw size={14} /></button>
            <button class="btn" onclick={() => vault.lock(s.id)}><Lock size={14} /> {closes(s)}</button>
          {:else}
            <button class="btn primary" disabled={vault.busyIds.includes(s.id)} onclick={() => vault.unlock(s.id)}
              ><LockOpen size={14} /> {vault.busyIds.includes(s.id) ? "Opening..." : opens(s)}</button
            >
          {/if}
        {/if}
        <button class="btn" onclick={() => edit(s)}><Pencil size={13} /> Edit</button>
        <button class="icon-btn" title="Remove" aria-label="Remove" onclick={() => remove(s)}><Trash2 size={14} /></button>
      </div>
    </SettingsRow>
    {#each s.checked_out as c (c.id)}
      <SettingsRow label={`Checked out: ${c.name}`} help="Checked out to you from this source. Check it in when you are done, so others can use it and it can be changed.">
        <button class="btn" onclick={() => vault.checkIn(c.id)}><Undo2 size={14} /> Check in</button>
      </SettingsRow>
    {/each}
  {:else}
    <SettingsRow help="No source yet. Add one below: a Vaultwarden or Bitwarden vault, CyberArk, Delinea Secret Server or Proton Pass.">
      <span></span>
    </SettingsRow>
  {/each}
  {#if !editing}
    <SettingsRow label="Add a source" stack>
      <div class="btns">
        {#each SOURCE_KINDS as k (k.kind)}
          <button class="btn" onclick={() => add(k.kind)}><Plus size={14} /> {k.label}</button>
        {/each}
      </div>
    </SettingsRow>
  {/if}
</SettingsSection>

{#if editing}
  <SettingsSection title={editing.id ? `Edit ${editing.name}` : `Add ${kindLabel(editing.kind)}`}>
    <SettingsRow label="Name" help="How the source shows in lists, such as Work or Home.">
      <input bind:value={editing.name} placeholder={kindLabel(editing.kind)} />
    </SettingsRow>
    {#if editing.kind === "vaultwarden"}
      <SettingsRow label="Server" help="Your Vaultwarden address, or https://vault.bitwarden.com (or .eu) for Bitwarden itself.">
        <input bind:value={editing.url} placeholder="https://vault.example.com" />
      </SettingsRow>
      <SettingsRow label="Email address of the account">
        <input bind:value={editing.account} />
      </SettingsRow>
    {:else if editing.kind === "cyberark"}
      <CyberArkFields bind:source={editing} />
    {:else if editing.kind === "delinea"}
      <DelineaFields bind:source={editing} />
    {:else if editing.kind === "protonpass"}
      <ProtonPassFields bind:source={editing} />
    {/if}
    {#if error}
      <SettingsRow><p class="bad">{error}</p></SettingsRow>
    {/if}
    <SettingsRow>
      <div class="btns end">
        <button class="btn" onclick={() => (editing = null)}>Cancel</button>
        <button class="btn primary" onclick={save}>Save</button>
      </div>
    </SettingsRow>
  </SettingsSection>
{/if}

<SettingsSection title="Opening after Cablebag starts">
  <SettingsRow
    label="Vaults and sign-ins this PC may keep"
    help={app.settings["vault.unlock"] === "password"
      ? "Safest: nothing about your sources stays on this PC; every start asks for the master password or sign-in."
      : app.settings["vault.unlock"] === "hello"
        ? "This PC keeps the vault key or sign-in, readable only by your Windows account; Windows Hello confirms it is you once per start."
        : "This PC keeps the vault key or sign-in, readable only by your Windows account, and uses it without asking: anyone using your Windows account can open them."}
  >
    <select
      value={app.settings["vault.unlock"]}
      onchange={async (e) => {
        const v = (e.target as HTMLSelectElement).value;
        await app.setSetting("vault.unlock", v);
        if (v === "password") for (const s of vault.sources) await api.sourceForget(s.id);
        await vault.refresh();
      }}
    >
      <option value="hello" disabled={!vault.hello}>Windows Hello (PIN, fingerprint or face){vault.hello ? "" : ", not set up here"}</option>
      <option value="remember">Without asking</option>
      <option value="password">Ask every time</option>
    </select>
  </SettingsRow>
</SettingsSection>

<style>
  .btns {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .end {
    justify-content: flex-end;
  }
  .bad {
    margin: 0;
    color: var(--danger-text);
    font-size: 12px;
  }
</style>

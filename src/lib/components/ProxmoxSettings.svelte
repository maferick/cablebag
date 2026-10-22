<script lang="ts">
  import { untrack } from "svelte";
  import { api, type ProxmoxSource } from "../api";
  import { app } from "../state.svelte";
  import { ask } from "../ask.svelte";
  import { proxmox } from "../proxmox.svelte";
  import { vault } from "../vault.svelte";
  import VaultPicker from "./VaultPicker.svelte";
  import WireGuardPicker from "./WireGuardPicker.svelte";
  import { Plus, Pencil, Trash2, PlugZap, Server, KeyRound } from "@lucide/svelte";

  let {
    only,
    onDone,
  }: {
    /** Show just the form for this server ("new" for a new one) instead of the whole list. */
    only?: ProxmoxSource | "new";
    onDone?: () => void;
  } = $props();

  const blank = (): ProxmoxSource => ({ id: "", name: "", url: "", token_id: "", vault_item: "", fingerprint: "", ssh_user: "root", key_path: "", rdp_user: "" });

  let editing = $state<ProxmoxSource | null>(null);
  let secretFrom = $state<"vault" | "here">("here");
  let secret = $state("");
  let busy = $state(false);
  let result = $state<{ ok: boolean; text: string } | null>(null);

  function edit(s?: ProxmoxSource) {
    editing = s ? { ...$state.snapshot(s) } : blank();
    secretFrom = editing.vault_item ? "vault" : vault.status.configured && !s ? "vault" : "here";
    secret = "";
    result = null;
  }

  async function save(): Promise<ProxmoxSource | null> {
    if (!editing) return null;
    const e = editing;
    if (!e.url.trim()) {
      result = { ok: false, text: "Fill in the address of the Proxmox server." };
      return null;
    }
    if (secretFrom === "vault") {
      if (!e.vault_item) {
        result = { ok: false, text: "Choose the vault item that holds the token." };
        return null;
      }
    } else {
      e.vault_item = "";
      if (!e.token_id.includes("!")) {
        result = { ok: false, text: "The API token ID looks like user@realm!name, for example root@pam!cablebag." };
        return null;
      }
    }
    if (!e.name.trim()) e.name = e.url.replace(/^https?:\/\//, "").replace(/:\d+$/, "");
    try {
      const saved = await api.proxmoxSaveSource(e, secretFrom === "here" ? secret : undefined);
      await proxmox.load();
      editing = { ...saved };
      secret = "";
      return saved;
    } catch (err) {
      result = { ok: false, text: String(err) };
      return null;
    }
  }

  async function test() {
    busy = true;
    result = null;
    try {
      const s = await save();
      if (!s) return;
      const guests = await api.proxmoxList(s.id);
      const running = guests.filter((g) => g.status === "running").length;
      result = { ok: true, text: `Connected: ${guests.length} virtual machines and containers, ${running} running.` };
      proxmox.listings[s.id] = { guests, loading: false, error: "", at: Date.now() };
    } catch (err) {
      result = { ok: false, text: String(err) };
    } finally {
      busy = false;
    }
  }

  async function remove(s: ProxmoxSource) {
    if (!(await ask.confirm("Remove Proxmox server", `Remove "${s.name}" from Cablebag? Its guests leave the sessions tree; nothing changes on the server.`, { okLabel: "Remove", danger: true })))
      return;
    await api.proxmoxDeleteSource(s.id);
    delete proxmox.listings[s.id];
    await proxmox.load();
    if (editing?.id === s.id) editing = null;
  }

  $effect.pre(() => {
    if (only && !editing) untrack(() => edit(only === "new" ? undefined : only));
  });

  function close() {
    editing = null;
    onDone?.();
  }

  const where = (s: ProxmoxSource) =>
    s.vault_item ? `Token from the vault${vault.item(s.vault_item) ? `: ${vault.item(s.vault_item)!.name}` : ""}` : s.has_secret ? "Token secret kept on this PC" : "Token secret asked when connecting";
</script>

{#if !only}
<p class="muted small">
  The virtual machines and containers of these servers show up as a folder in the sessions tree, with SSH, RDP, the
  console and start and stop right from the list. You can also add one from New session, or with a right-click in the
  sessions tree.
</p>
{/if}

{#each only ? [] : proxmox.sources as s (s.id)}
  {#if editing?.id !== s.id}
    <div class="src">
      <Server size={16} />
      <div class="grow">
        <div class="name">{s.name}</div>
        <div class="muted small">{s.url} · {s.token_id || "token ID from the vault item"} · {where(s)}</div>
      </div>
      <button class="btn small" onclick={() => edit(s)}><Pencil size={13} /> Edit</button>
      <button class="icon-btn" title="Remove" onclick={() => remove(s)}><Trash2 size={14} /></button>
    </div>
  {/if}
{/each}

{#if editing}
  <div class="form" class:bare={!!only}>
    {#if !only}<strong class="sub">{editing.id ? "Edit Proxmox server" : "Add a Proxmox server"}</strong>{/if}
    <div class="row2">
      <label class="field grow"><span>Address</span><input bind:value={editing.url} placeholder="pve.lan or https://10.0.0.5:8006" /></label>
      <label class="field"><span>Name in the tree</span><input bind:value={editing.name} placeholder="pve" /></label>
    </div>

    <div class="field"><span>Network (the server, and the sessions of its guests)</span>
      <WireGuardPicker bind:value={editing.wireguard} />
    </div>

    <div class="field">
      <span>API token</span>
      <div class="choice">
        <label class="check"><input type="radio" bind:group={secretFrom} value="vault" /> From a credential source (user name = token ID, password = secret)</label>
        <label class="check"><input type="radio" bind:group={secretFrom} value="here" /> Type it here, kept encrypted on this PC</label>
      </div>
    </div>
    {#if secretFrom === "vault"}
      <VaultPicker bind:itemId={editing.vault_item} hint={editing.name || editing.url} />
      <label class="field"><span>Token ID (leave empty to take the vault item's user name)</span><input bind:value={editing.token_id} placeholder="root@pam!cablebag" /></label>
    {:else}
      <div class="row2">
        <label class="field grow"><span>Token ID</span><input bind:value={editing.token_id} placeholder="root@pam!cablebag" /></label>
        <label class="field grow"
          ><span>Secret</span><input type="password" bind:value={secret} placeholder={editing.id && proxmox.sources.find((x) => x.id === editing!.id)?.has_secret ? "Leave empty to keep the saved one" : "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"} autocomplete="off" /></label
        >
      </div>
    {/if}
    <p class="muted small">
      Make one under Datacenter, Permissions, API Tokens. To see the guests the token needs the PVEAuditor role on
      /; to start and stop them also PVEVMUser (or VM.PowerMgmt). With "Privilege Separation" on, give the
      token itself these roles.
    </p>

    <strong class="sub">Opening a guest</strong>
    <div class="row2">
      <label class="field"><span>SSH user</span><input bind:value={editing.ssh_user} placeholder="root" /></label>
      <label class="field grow"><span>SSH key file (optional)</span><input bind:value={editing.key_path} placeholder="C:\Users\you\.ssh\id_ed25519" /></label>
      <label class="field"><span>RDP user (optional)</span><input bind:value={editing.rdp_user} placeholder="administrator" /></label>
    </div>
    {#if editing.fingerprint}
      <p class="muted small"><KeyRound size={12} /> Trusted certificate <span class="mono fp">{editing.fingerprint}</span></p>
    {/if}

    {#if result}<p class="result" class:bad={!result.ok}>{result.text}</p>{/if}
    <div class="buttons">
      <button class="btn" onclick={close}>Cancel</button>
      <button class="btn" disabled={busy} onclick={test}><PlugZap size={14} /> {busy ? "Connecting..." : "Save and test"}</button>
      <button
        class="btn primary"
        disabled={busy}
        onclick={async () => {
          if (await save()) {
            close();
            app.notify("Proxmox server saved");
          }
        }}>Save</button
      >
    </div>
  </div>
{:else if !only}
  <div><button class="btn" onclick={() => edit()}><Plus size={14} /> Add a Proxmox server</button></div>
{/if}

<style>
  .src {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .src :global(svg) {
    color: var(--muted);
    flex-shrink: 0;
  }
  .name {
    font-weight: 600;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .form.bare {
    padding: 0;
    border: none;
    background: none;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--border-strong);
    border-radius: 3px;
    background: var(--raised);
  }
  .row2 {
    display: flex;
    gap: 10px;
  }
  .choice {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .fp {
    word-break: break-all;
  }
  .result {
    margin: 0;
    padding: 7px 10px;
    border-radius: 3px;
    background: var(--hover);
    color: var(--ok);
  }
  .result.bad {
    color: var(--danger);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .btn.small {
    padding: 3px 9px;
    font-size: 12px;
  }
</style>

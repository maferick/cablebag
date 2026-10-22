<script lang="ts">
  // One WireGuard network, new or existing: opened from a session's or folder's Network choice and from Tunnels.
  import { untrack } from "svelte";
  import { api, type WireGuardInfo, type WireGuardProfile } from "../api";
  import { wireguard } from "../wireguard.svelte";
  import { app } from "../state.svelte";
  import Modal from "./Modal.svelte";
  import VaultPicker from "./VaultPicker.svelte";
  import { Copy, KeyRound } from "@lucide/svelte";

  /** profile: the network to edit, or null for a new one; onsaved gets its id. */
  let { profile, onclose, onsaved }: { profile: WireGuardInfo | null; onclose: () => void; onsaved?: (id: string) => void } = $props();

  let info = $state<WireGuardInfo | null>(untrack(() => profile));
  let editing = $state<WireGuardProfile>(
    untrack(() => {
      if (!profile) return { id: "", name: "", address: "", dns: "", mtu: 0, public_key: "", endpoint: "", allowed_ips: "", keepalive: 25, vault_item: "" };
      const { has_key, has_preshared, own_public_key, status, ...p } = profile;
      return p;
    }),
  );
  let privateKey = $state("");
  let presharedKey = $state("");
  /** The public key that goes with a newly made private key, to give to the server. */
  let newPublic = $state("");

  async function showPublic() {
    if (!info) return;
    try {
      const pub = await api.wireguardPublicKey(info.id);
      info = { ...info, own_public_key: pub };
      await wireguard.load();
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  async function makeKeys() {
    const [priv, pub] = await api.wireguardKeypair();
    privateKey = priv;
    newPublic = pub;
  }

  async function save() {
    try {
      // Empty key fields keep the keys Cablebag has.
      const p = await api.wireguardSave($state.snapshot(editing) as WireGuardProfile, privateKey || undefined, presharedKey || undefined);
      await wireguard.load();
      onsaved?.(p.id);
      onclose();
    } catch (e) {
      app.notify(String(e), "error");
    }
  }
</script>

<Modal title={info ? `Edit ${info.name}` : "New WireGuard network"} width={640} {onclose}>
  <div class="form">
    <label class="field"><span>Name</span><input bind:value={editing.name} placeholder="For example: Customer A" /></label>
    <div class="row">
      <label class="field grow"><span>Server (endpoint)</span><input bind:value={editing.endpoint} placeholder="vpn.example.com:51820" /></label>
      <label class="field grow"><span>Address of this side in the tunnel</span><input bind:value={editing.address} placeholder="10.8.0.2/32" /></label>
    </div>
    <label class="field"><span>Public key of the server</span><input class="mono" bind:value={editing.public_key} placeholder="44 characters, ending in =" /></label>
    <label class="field"><span>Private key of this side</span>
      <div class="with-btn">
        <input class="mono" type="password" bind:value={privateKey} placeholder={info?.has_key ? "Kept in Cablebag's vault; type a new one to replace it" : "From the [Interface] of the .conf"} />
        <button class="btn" onclick={makeKeys} title="Make a new key pair, for a peer of Cablebag's own"><KeyRound size={14} /> New key pair</button>
      </div>
    </label>
    {#if !newPublic && !info?.own_public_key && info?.vault_item && editing.vault_item === info.vault_item}
      <div class="field"><span>Public key of this side, for the server's peer list</span>
        <div><button class="btn" onclick={showPublic}><KeyRound size={14} /> Show the public key of the key in the vault</button></div>
      </div>
    {/if}
    {#if !newPublic && info?.own_public_key && info.vault_item && editing.vault_item === info.vault_item}
      <div><button class="link" onclick={showPublic}>Read the key in the vault again (after it changed there)</button></div>
    {/if}
    {#if newPublic || info?.own_public_key}
      {@const pub = newPublic || info?.own_public_key || ""}
      <div class="field"><span>{newPublic ? "Public key to add as a peer on the server (save to keep the new key)" : "Public key of this side, for the server's peer list"}</span>
        <div class="with-btn">
          <input class="mono" readonly value={pub} />
          <button class="btn" onclick={() => navigator.clipboard.writeText(pub).then(() => app.notify("Public key copied"))}><Copy size={14} /> Copy</button>
        </div>
      </div>
    {/if}
    <div class="field"><span>Or the private key from the vault (the item's password)</span>
      <VaultPicker bind:itemId={editing.vault_item} />
    </div>
    <div class="row">
      <label class="field grow"><span>DNS servers in the tunnel</span><input bind:value={editing.dns} placeholder="None: names are looked up as usual" /></label>
      <label class="field small"><span>Keepalive (s)</span><input type="number" bind:value={editing.keepalive} /></label>
      <label class="field small"><span>MTU</span><input type="number" value={editing.mtu || ""} oninput={(e) => (editing.mtu = Number((e.target as HTMLInputElement).value) || 0)} placeholder="1420" /></label>
    </div>
    <label class="field"><span>Networks behind the server (shown only)</span><input bind:value={editing.allowed_ips} placeholder="10.0.0.0/24" /></label>
    <label class="field"><span>Preshared key (optional)</span>
      <input class="mono" type="password" bind:value={presharedKey} placeholder={info?.has_preshared ? "Kept in Cablebag's vault; type a new one to replace it" : "None"} />
    </label>
    <p class="muted small">
      Using the same .conf as the WireGuard app works, but not with both switched on at once: the server talks to one
      of them only. For both at the same time, make a new key pair here and add its public key as a peer on the server.
    </p>
  </div>
  {#snippet footer()}
    <button class="btn" onclick={onclose}>Cancel</button>
    <button class="btn primary" onclick={save}>Save</button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .row {
    display: flex;
    gap: 10px;
  }
  .grow {
    flex: 1;
  }
  .small input {
    width: 100px;
  }
  .with-btn {
    display: flex;
    gap: 8px;
  }
  .with-btn input {
    flex: 1;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent-text, var(--accent));
    cursor: pointer;
    font-size: 12px;
  }
</style>

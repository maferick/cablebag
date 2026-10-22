<script lang="ts">
  // WireGuard networks that Cablebag runs itself: one per profile, only for the sessions that choose it, so two
  // networks with the same addresses can be used at the same time. No adapter or routes in Windows.
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { api, type WireGuardInfo, type WireGuardProfile } from "../../api";
  import { wireguard } from "../../wireguard.svelte";
  import { app } from "../../state.svelte";
  import { ask } from "../../ask.svelte";
  import Modal from "../Modal.svelte";
  import VaultPicker from "../VaultPicker.svelte";
  import { Plus, Pencil, Trash2, FileUp, Activity, Copy, KeyRound } from "@lucide/svelte";

  let editing = $state<WireGuardProfile | null>(null);
  let editingInfo = $state<WireGuardInfo | null>(null);
  let privateKey = $state("");
  let presharedKey = $state("");
  /** The public key that goes with a newly made private key, to give to the server. */
  let newPublic = $state("");
  let testing = $state<Record<string, string>>({});

  onMount(() => {
    wireguard.load();
    // Handshakes, data and open connections follow while the page is open.
    const t = setInterval(() => wireguard.load(), 2000);
    return () => clearInterval(t);
  });

  function blank(): WireGuardProfile {
    return { id: "", name: "", address: "", dns: "", mtu: 0, public_key: "", endpoint: "", allowed_ips: "", keepalive: 25, vault_item: "" };
  }

  function edit(p: WireGuardInfo | null) {
    editingInfo = p;
    if (p) {
      const { has_key, has_preshared, own_public_key, status, ...profile } = p;
      editing = profile;
    } else {
      editing = blank();
    }
    privateKey = "";
    presharedKey = "";
    newPublic = "";
  }

  async function makeKeys() {
    const [priv, pub] = await api.wireguardKeypair();
    privateKey = priv;
    newPublic = pub;
  }

  async function save() {
    if (!editing) return;
    try {
      // Empty key fields keep the keys Cablebag has.
      await api.wireguardSave($state.snapshot(editing) as WireGuardProfile, privateKey || undefined, presharedKey || undefined);
      editing = null;
      await wireguard.load();
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  async function importConf() {
    const f = await openDialog({ title: "Choose a WireGuard configuration", filters: [{ name: "WireGuard", extensions: ["conf"] }, { name: "All files", extensions: ["*"] }] });
    if (typeof f !== "string") return;
    try {
      const p = await api.wireguardImport(f);
      await wireguard.load();
      app.notify(`WireGuard network ${p.name} added; its private key is kept in Cablebag's vault`);
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  async function remove(p: WireGuardInfo) {
    const using = app.sessions.filter((s) => s.data?.wireguard === p.id).length + app.folders.filter((f) => f.data?.wireguard === p.id).length;
    const also = using ? ` ${using} session${using === 1 ? "" : "s"} or folder${using === 1 ? "" : "s"} use it and will connect directly again.` : "";
    if (await ask.confirm("Delete WireGuard network", `Delete ${p.name} and its keys?${also}`, { okLabel: "Delete", danger: true })) {
      await api.wireguardDelete(p.id);
      await wireguard.load();
    }
  }

  async function test(p: WireGuardInfo) {
    testing[p.id] = "Shaking hands...";
    try {
      await api.wireguardTest(p.id);
      testing[p.id] = "The server answered.";
    } catch (e) {
      testing[p.id] = String(e);
    }
    await wireguard.load();
  }

  function ago(s: number | null | undefined): string {
    if (s === null || s === undefined) return "no handshake yet";
    if (s < 5) return "handshake just now";
    if (s < 120) return `handshake ${s} seconds ago`;
    return `handshake ${Math.round(s / 60)} minutes ago`;
  }

  function size(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }

  const users = (id: string) => app.sessions.filter((s) => s.data?.wireguard === id).length;
</script>

<section class="wg">
  <div class="sechead">
    <div>
      <h3>WireGuard networks</h3>
      <p class="muted">
        Cablebag runs these itself, only for the sessions that choose one, so two networks with the same addresses
        work at the same time. Nothing is changed in Windows.
      </p>
    </div>
    <span class="grow"></span>
    <button class="btn" onclick={importConf}><FileUp size={14} /> Import .conf</button>
    <button class="btn" onclick={() => edit(null)}><Plus size={14} /> New WireGuard network</button>
  </div>

  {#if !wireguard.list.length}
    <div class="empty">
      <p class="muted">
        No WireGuard networks yet. Import the .conf file you use in the WireGuard app, then choose the network in a
        session or a folder under Network.
      </p>
    </div>
  {:else}
    <div class="tlist">
      {#each wireguard.list as p (p.id)}
        {@const st = p.status}
        <div class="tcard">
          <div class="thead">
            <span class="smark {st ? (st.handshake !== null ? 'connected' : 'connecting') : 'closed'}"></span>
            <strong class="name">{p.name}</strong>
            {#if users(p.id)}<span class="tag">{users(p.id)} session{users(p.id) === 1 ? "" : "s"}</span>{/if}
            {#if !p.has_key && !p.vault_item}<span class="tag warn">no private key</span>{/if}
            <span class="grow"></span>
            <span class="state" class:running={st?.handshake !== null && !!st}>
              {#if st}{st.connections} open, {ago(st.handshake)}, {size(st.sent)} sent, {size(st.received)} received{:else}Starts when a session needs it{/if}
            </span>
            <button class="btn" onclick={() => test(p)} title="Start the tunnel and wait for the server's answer"><Activity size={14} /> Test</button>
            <button class="icon-btn" title="Edit" onclick={() => edit(p)}><Pencil size={15} /></button>
            <button class="icon-btn" title="Delete" onclick={() => remove(p)}><Trash2 size={15} /></button>
          </div>
          <div class="route">
            <span class="stop"><em>This side in the tunnel</em><span class="mono">{p.address}</span></span>
            <span class="arrow" aria-hidden="true"></span>
            <span class="stop"><em>Server</em><span class="mono">{p.endpoint}</span></span>
            {#if p.allowed_ips}
              <span class="arrow" aria-hidden="true"></span>
              <span class="stop"><em>Reaches</em><span class="mono">{p.allowed_ips}</span></span>
            {/if}
          </div>
          {#if st?.warning}<div class="msg">{st.warning}</div>{/if}
          {#if testing[p.id]}<div class="msg" class:ok={testing[p.id] === "The server answered."}>{testing[p.id]}</div>{/if}
        </div>
      {/each}
    </div>
  {/if}
</section>

{#if editing}
  <Modal title={editingInfo ? `Edit ${editingInfo.name}` : "New WireGuard network"} width={640} onclose={() => (editing = null)}>
    <div class="form">
      <label class="field"><span>Name</span><input bind:value={editing.name} placeholder="For example: Customer A" /></label>
      <div class="row">
        <label class="field grow"><span>Server (endpoint)</span><input bind:value={editing.endpoint} placeholder="vpn.example.com:51820" /></label>
        <label class="field grow"><span>Address of this side in the tunnel</span><input bind:value={editing.address} placeholder="10.8.0.2/32" /></label>
      </div>
      <label class="field"><span>Public key of the server</span><input class="mono" bind:value={editing.public_key} placeholder="44 characters, ending in =" /></label>
      <label class="field"><span>Private key of this side</span>
        <div class="with-btn">
          <input class="mono" type="password" bind:value={privateKey} placeholder={editingInfo?.has_key ? "Kept in Cablebag's vault; type a new one to replace it" : "From the [Interface] of the .conf"} />
          <button class="btn" onclick={makeKeys} title="Make a new key pair, for a peer of Cablebag's own"><KeyRound size={14} /> New key pair</button>
        </div>
      </label>
      {#if newPublic || editingInfo?.own_public_key}
        {@const pub = newPublic || editingInfo?.own_public_key || ""}
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
        <label class="field small"><span>MTU</span><input type="number" value={editing.mtu || ""} oninput={(e) => editing && (editing.mtu = Number((e.target as HTMLInputElement).value) || 0)} placeholder="1420" /></label>
      </div>
      <label class="field"><span>Networks behind the server (shown only)</span><input bind:value={editing.allowed_ips} placeholder="10.0.0.0/24" /></label>
      <label class="field"><span>Preshared key (optional)</span>
        <input class="mono" type="password" bind:value={presharedKey} placeholder={editingInfo?.has_preshared ? "Kept in Cablebag's vault; type a new one to replace it" : "None"} />
      </label>
      <p class="muted small">
        Using the same .conf as the WireGuard app works, but not with both switched on at once: the server talks to
        one of them only. For both at the same time, make a new key pair here and add its public key as a peer on the
        server.
      </p>
    </div>
    {#snippet footer()}
      <button class="btn" onclick={() => (editing = null)}>Cancel</button>
      <button class="btn primary" onclick={save}>Save</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .wg {
    margin-top: 28px;
    max-width: 1100px;
  }
  .sechead {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-bottom: 10px;
  }
  .sechead h3 {
    margin: 0 0 4px;
    font-size: 14px;
  }
  .sechead p {
    margin: 0;
    max-width: 640px;
  }
  .grow {
    flex: 1;
  }
  .empty p {
    margin: 0;
  }
  .tlist {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .tcard {
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--chrome);
  }
  .thead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 8px 12px;
    border-bottom: 1px solid var(--border);
  }
  .name {
    font-weight: 600;
  }
  .tag {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 3px;
    border: 1px solid var(--border);
    color: var(--muted);
  }
  .tag.warn {
    color: var(--danger-text);
  }
  .state {
    font-size: 12px;
    color: var(--muted);
    margin-right: 4px;
  }
  .state.running {
    color: var(--ok-text);
  }
  .route {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
    padding: 10px 12px;
    background: var(--bg);
  }
  .stop {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .stop em {
    font-style: normal;
    font-size: 11px;
    color: var(--muted);
  }
  .stop .mono {
    font-size: 12px;
  }
  .arrow {
    width: 22px;
    height: 1px;
    background: var(--border-strong);
    position: relative;
  }
  .arrow::after {
    content: "";
    position: absolute;
    right: 0;
    top: -3px;
    border: 3.5px solid transparent;
    border-left: 5px solid var(--border-strong);
    border-right: none;
  }
  .msg {
    padding: 0 12px 10px;
    font-size: 12px;
    color: var(--danger-text);
    background: var(--bg);
  }
  .msg.ok {
    color: var(--ok-text);
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .row {
    display: flex;
    gap: 10px;
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
</style>

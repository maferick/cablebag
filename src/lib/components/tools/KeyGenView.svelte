<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { api, type KeyInfo } from "../../api";
  import { app } from "../../state.svelte";
  import { writeText } from "../../clipboard";
  import { KeyRound, Copy, Save, Upload, FolderOpen, Sparkles } from "@lucide/svelte";
  import "./tool.css";

  let kind = $state("ed25519");
  let comment = $state("");
  let key = $state<KeyInfo | null>(null);
  let busy = $state(false);
  let pass1 = $state("");
  let pass2 = $state("");
  let savedTo = $state("");
  let target = $state("");
  let loadPass = $state("");
  let defaultDir = "";

  const sshSessions = $derived(
    app.sessions
      .filter((s) => s.kind === "ssh")
      .map((s) => ({ s, label: `${app.folderPath(s.folder_id) ? app.folderPath(s.folder_id) + " / " : ""}${s.name}` }))
      .sort((a, b) => a.label.localeCompare(b.label)),
  );

  onMount(async () => {
    defaultDir = await api.keyDefaultDir();
    comment = `cablebag-${new Date().toISOString().slice(0, 10)}`;
  });

  onDestroy(() => {
    if (key) api.keyForget(key.id);
  });

  async function generate() {
    busy = true;
    try {
      if (key) api.keyForget(key.id);
      key = await api.keyGenerate(kind, comment);
      savedTo = "";
    } catch (e) {
      app.notify(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function load() {
    const f = await openDialog({ title: "Open an existing private key" });
    if (!f || Array.isArray(f)) return;
    try {
      if (key) api.keyForget(key.id);
      key = await api.keyLoad(f, loadPass || undefined);
      comment = key.comment;
      savedTo = "";
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  async function save() {
    if (!key) return;
    if (pass1 !== pass2) return app.notify("The two passphrases are not the same.", "error");
    const name = kind === "ed25519" ? "id_ed25519" : kind.startsWith("rsa") ? "id_rsa" : "id_ecdsa";
    const path = await saveDialog({ title: "Save the private key", defaultPath: `${defaultDir}\\${name}_cablebag` });
    if (!path) return;
    try {
      const [priv, pub] = await api.keySave(key.id, path, pass1, comment);
      savedTo = `${priv} and ${pub}`;
      app.notify("Key saved");
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  async function install() {
    const s = app.sessions.find((x) => x.id === target);
    if (!key || !s) return;
    busy = true;
    try {
      await api.keyInstall(key.id, $state.snapshot(s));
      app.notify(`The public key is now accepted by ${s.name}`);
    } catch (e) {
      app.notify(String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<div class="tool-view">
  <header>
    <KeyRound size={20} />
    <div>
      <h2>SSH key generator</h2>
      <p class="muted">Create a key pair, save it, and let a server accept it, so you log in without a password.</p>
    </div>
  </header>

  <section>
    <h3>1. Create a key</h3>
    <div class="controls">
      <label class="field"><span>Type</span>
        <select bind:value={kind}>
          <option value="ed25519">Ed25519 (recommended)</option>
          <option value="ecdsa-256">ECDSA 256</option>
          <option value="ecdsa-384">ECDSA 384</option>
          <option value="rsa-3072">RSA 3072 (older servers)</option>
          <option value="rsa-4096">RSA 4096 (older servers)</option>
        </select>
      </label>
      <label class="field"><span>Comment</span><input bind:value={comment} style="width: 240px" /></label>
      <button class="btn primary" onclick={generate} disabled={busy}><Sparkles size={14} /> {busy && !key ? "Creating..." : "Create key"}</button>
      <span class="muted">or</span>
      <label class="field"><span>Passphrase of an existing key</span><input type="password" bind:value={loadPass} style="width: 180px" /></label>
      <button class="btn" onclick={load}><FolderOpen size={14} /> Open existing key</button>
    </div>
  </section>

  {#if key}
    <section>
      <h3>Public key</h3>
      <div class="pub mono">{key.public_key}</div>
      <div class="controls">
        <button class="btn" onclick={() => key && writeText(key.public_key)}><Copy size={14} /> Copy public key</button>
        <span class="muted small">{key.algorithm} · {key.fingerprint}</span>
      </div>
    </section>

    <section>
      <h3>2. Save it</h3>
      <div class="controls">
        <label class="field"><span>Passphrase (optional)</span><input type="password" bind:value={pass1} style="width: 200px" /></label>
        <label class="field"><span>Passphrase again</span><input type="password" bind:value={pass2} style="width: 200px" /></label>
        <button class="btn" onclick={save}><Save size={14} /> Save key</button>
      </div>
      {#if savedTo}<p class="muted small">Saved as {savedTo}. Use the private key file in your session settings.</p>{/if}
    </section>

    <section>
      <h3>3. Put it on a server</h3>
      <div class="controls">
        <label class="field"><span>Server</span>
          <select bind:value={target} style="min-width: 320px">
            <option value="">Choose a saved SSH session</option>
            {#each sshSessions as o (o.s.id)}<option value={o.s.id}>{o.label}</option>{/each}
          </select>
        </label>
        <button class="btn" onclick={install} disabled={!target || busy}><Upload size={14} /> Add to the server</button>
      </div>
      <p class="muted small">Cablebag logs in once the usual way and adds the public key to ~/.ssh/authorized_keys, without duplicates.</p>
    </section>
  {/if}
</div>

<style>
  section {
    margin-bottom: 22px;
    max-width: 900px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 13px;
  }
  .pub {
    padding: 8px 10px;
    margin-bottom: 8px;
    border-radius: 3px;
    background: var(--panel);
    font-size: 12px;
    word-break: break-all;
    user-select: text;
  }
</style>

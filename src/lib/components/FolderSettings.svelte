<script lang="ts">
  // A folder's settings: what the sessions inside it (and in its subfolders) use when they have no value of their
  // own, such as one customer's user name, key, vault login and jump host.
  import Modal from "./Modal.svelte";
  import VaultPicker from "./VaultPicker.svelte";
  import WireGuardPicker from "./WireGuardPicker.svelte";
  import { wireguard } from "../wireguard.svelte";
  import { app, FOLDER_DEFAULTS } from "../state.svelte";
  import { api, type Folder } from "../api";
  import { TAB_COLORS } from "../tabColors";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { FolderOpen, Plus, Trash2 } from "@lucide/svelte";

  let { folder }: { folder: Folder } = $props();
  // A copy to edit: the folder itself changes only on Save (one dialog per folder, see App).
  // svelte-ignore state_referenced_locally
  let d = $state<Record<string, any>>({ jump_hosts: [], ...structuredClone($state.snapshot(folder.data ?? {})) });
  // What a folder above this one already gives, shown as the value when this folder leaves it empty.
  const above = $derived(app.folderDefaults(folder.parent_id));
  const count = $derived(app.sessions.filter((s) => app.inFolder(s.folder_id, folder.id)).length);

  async function browseKey() {
    const f = await openDialog({ title: "Choose a private key file", filters: [{ name: "Keys", extensions: ["*"] }] });
    if (f && !Array.isArray(f)) d.key_path = f;
  }

  async function save() {
    const data: Record<string, any> = {};
    for (const k of FOLDER_DEFAULTS) {
      const v = d[k.key];
      if (k.key === "jump_hosts") {
        const list = (v ?? []).filter((j: any) => j.host?.trim());
        if (list.length) data.jump_hosts = list;
      } else if (v !== undefined && v !== "" && v !== null) data[k.key] = v;
    }
    await api.saveFolder({ ...$state.snapshot(folder), data } as Folder);
    await app.reloadLibrary();
    app.folderEditing = null;
    app.notify(`Settings of ${folder.name} saved`);
  }

  const from = (key: string) => (above.from[key] ? `From ${above.from[key]}: ${key === "jump_hosts" ? above.values[key].map((j: any) => j.host).join(", ") : key === "wireguard" ? wireguard.name(above.values[key]) : above.values[key]}` : "");
</script>

<Modal title={`Folder ${folder.name}`} width={620} onclose={() => (app.folderEditing = null)}>
  <p class="muted lead">
    {count === 1 ? "The session" : `The ${count} sessions`} in this folder and its subfolders use these when they
    have no value of their own. A subfolder can set its own.
  </p>
  <div class="form">
    <label class="field"><span>User name</span><input bind:value={d.username} placeholder={from("username") || "None"} /></label>
    <label class="field"><span>Private key file (SSH)</span>
      <div class="with-btn">
        <input bind:value={d.key_path} placeholder={from("key_path") || "None"} />
        <button class="btn" onclick={browseKey}><FolderOpen size={14} /> Browse</button>
      </div>
    </label>
    <div class="field"><span>Login from the vault</span>
      <VaultPicker bind:itemId={d.vault_item} />
      {#if !d.vault_item && from("vault_item")}<span class="muted small">{from("vault_item")}</span>{/if}
    </div>
    <div class="field"><span>Network (SSH, RDP, VNC, telnet, web)</span>
      <WireGuardPicker bind:value={d.wireguard} inherited={above.values.wireguard} from={above.from.wireguard ?? ""} />
    </div>
    <div class="field">
      <span>Jump hosts (SSH)</span>
      {#each d.jump_hosts as j, i}
        <div class="jrow">
          <input bind:value={j.host} placeholder="Gateway host" />
          <input bind:value={j.user} placeholder="User" class="small" />
          <input type="number" bind:value={j.port} class="tiny" />
          <button class="icon-btn" title="Remove" aria-label="Remove" onclick={() => (d.jump_hosts = d.jump_hosts.filter((_: unknown, x: number) => x !== i))}><Trash2 size={14} /></button>
        </div>
      {/each}
      <div>
        <button class="btn" onclick={() => (d.jump_hosts = [...d.jump_hosts, { host: "", port: 22, user: "", key_path: "" }])}><Plus size={14} /> Add jump host</button>
        {#if !d.jump_hosts.length && from("jump_hosts")}<span class="muted small">{from("jump_hosts")}</span>{/if}
      </div>
    </div>
    <label class="check" title="SSH sessions in this folder show the windows of Linux programs on this PC, unless a session says otherwise.">
      <input type="checkbox" checked={!!(d.x11 ?? above.values.x11)} onchange={(e) => (d.x11 = (e.target as HTMLInputElement).checked)} />
      Show Linux windows on this PC (X11, SSH){#if d.x11 === undefined && from("x11")}<span class="muted small">From {above.from.x11}</span>{/if}
    </label>
    <label class="field"><span>Tab colour</span>
      <select bind:value={d.tab_color}>
        {#each TAB_COLORS as c}<option value={c.value}>{c.value ? c.name : above.values.tab_color ? `Same as above` : c.name}</option>{/each}
      </select>
    </label>
  </div>
  {#snippet footer()}
    <button class="btn" onclick={() => (app.folderEditing = null)}>Cancel</button>
    <button class="btn primary" onclick={save}>Save</button>
  {/snippet}
</Modal>

<style>
  .lead {
    margin: 0 0 14px;
    line-height: 1.5;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field > span:first-child {
    font-size: 12px;
    color: var(--muted);
  }
  .with-btn {
    display: flex;
    gap: 6px;
  }
  .with-btn input {
    flex: 1;
  }
  .jrow {
    display: flex;
    gap: 6px;
    margin-bottom: 6px;
  }
  .jrow input {
    flex: 1;
  }
  .jrow .small {
    flex: 0 0 120px;
  }
  .jrow .tiny {
    flex: 0 0 70px;
  }
  .small {
    font-size: 12px;
    margin-left: 8px;
  }
</style>

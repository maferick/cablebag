<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "../api";
  import { app } from "../state.svelte";
  import { sync } from "../sync.svelte";
  import { vault } from "../vault.svelte";
  import VaultPicker from "./VaultPicker.svelte";
  import { RefreshCw, ExternalLink } from "@lucide/svelte";

  const KINDS = [
    { id: "auto", label: "Find out from the address" },
    { id: "gitea", label: "Gitea or Forgejo" },
    { id: "github", label: "GitHub" },
    { id: "gitlab", label: "GitLab" },
    { id: "git", label: "Any other git repository (uses git on this PC)" },
  ];

  let kind = $state<string>(app.settings["sync.kind"] || "auto");
  let url = $state<string>(app.settings["sync.url"] || "");
  let repo = $state<string>(app.settings["sync.repo"] || "");
  let vaultItem = $state<string>(app.settings["sync.vault_item"] || "");
  let tokenFrom = $state<"vault" | "here">(app.settings["sync.vault_item"] || !app.settings["sync.url"] ? "vault" : "here");
  let token = $state("");
  /** The vaults whose account key can encrypt the sync. */
  const keyVaults = $derived(vault.sources.filter((s) => s.kind === "vaultwarden" && s.configured));
  let encrypt = $state<string>(app.settings["sync.encrypt"] || (vault.sources.some((s) => s.kind === "vaultwarden" && s.configured) ? "vault" : "passphrase"));
  let passphrase = $state("");
  let hasToken = $state(false);
  let hasPassphrase = $state(false);

  onMount(async () => {
    hasToken = await api.syncHasToken();
    hasPassphrase = await api.syncHasPassphrase();
  });

  const plainGit = $derived(kind === "git" || (kind === "auto" && /^(git@|ssh:\/\/)|\.git$/.test(url.trim())));

  async function save(): Promise<boolean> {
    try {
      await api.syncConfigure({
        kind,
        url,
        repo: plainGit ? "" : repo,
        vaultItem: plainGit || tokenFrom !== "vault" ? "" : vaultItem,
        token: !plainGit && tokenFrom === "here" ? token : undefined,
        encrypt,
        passphrase: encrypt === "passphrase" ? passphrase : undefined,
      });
      Object.assign(app.settings, {
        "sync.kind": kind,
        "sync.url": url.trim(),
        "sync.repo": plainGit ? "" : repo.trim(),
        "sync.vault_item": plainGit || tokenFrom !== "vault" ? "" : vaultItem,
        "sync.encrypt": encrypt,
      });
      token = "";
      passphrase = "";
      hasToken = await api.syncHasToken();
      hasPassphrase = await api.syncHasPassphrase();
      return true;
    } catch (e) {
      app.notify(String(e), "error");
      return false;
    }
  }

  const when = (t: number) => new Date(t * 1000).toLocaleString([], { dateStyle: "short", timeStyle: "short" });
  const tokenOk = $derived(plainGit || (tokenFrom === "vault" ? !!vaultItem : hasToken || !!token.trim()));
  const keyOk = $derived(encrypt !== "passphrase" || hasPassphrase || passphrase.length >= 8);
  const ready = $derived(!!url.trim() && (plainGit || repo.includes("/")) && tokenOk && keyOk);
  const webUrl = $derived(
    app.settings["sync.repo"] ? `${String(app.settings["sync.url"]).replace(/\/$/, "")}/${app.settings["sync.repo"]}` : /^https?:\/\//.test(String(app.settings["sync.url"] ?? "")) ? String(app.settings["sync.url"]).replace(/\.git$/, "") : "",
  );
</script>

<p class="muted small">
  Keeps your sessions, folders, macros, tunnels and Proxmox servers in a private git repository, so every PC with
  Cablebag has the same set and the repository keeps the history of every change. Passwords never go there: they stay
  in the vault or encrypted on each PC.
</p>

<div class="row2">
  <label class="field"><span>Git server</span>
    <select bind:value={kind}>
      {#each KINDS as k}<option value={k.id}>{k.label}</option>{/each}
    </select>
  </label>
  <label class="field grow"
    ><span>{plainGit ? "Repository address (https://... or git@...)" : "Server address"}</span><input
      bind:value={url}
      placeholder={plainGit ? "git@bitbucket.org:you/cablebag-sessions.git" : "https://gitea.example.com or https://github.com"}
    /></label
  >
</div>
{#if !plainGit}
  <label class="field"><span>Repository (made private if it does not exist)</span><input bind:value={repo} placeholder="owner/cablebag-sessions" /></label>

  <div class="field">
    <span>Access token, with read and write on repositories (GitHub: a fine-grained token with Contents; GitLab: api scope)</span>
    <div class="choice">
      <label class="check"><input type="radio" bind:group={tokenFrom} value="vault" /> From a credential source (password = token)</label>
      <label class="check"><input type="radio" bind:group={tokenFrom} value="here" /> Type it here, kept encrypted on this PC</label>
    </div>
  </div>
  {#if tokenFrom === "vault"}
    <VaultPicker bind:itemId={vaultItem} hint="git" />
  {:else}
    <input type="password" bind:value={token} placeholder={hasToken ? "Leave empty to keep the saved token" : "Paste the token"} autocomplete="off" />
  {/if}
{:else}
  <p class="muted small">Cablebag uses the git program on this PC and the sign-in it already has (Git Credential Manager or an SSH key). The repository has to exist.</p>
{/if}

<label class="field"><span>Encryption of the files in the repository</span>
  <select bind:value={encrypt}>
    <option value="vault" disabled={!keyVaults.length}>Encrypted with a key from your vault (the same on every PC with this vault){keyVaults.length ? "" : ", once a Vaultwarden source is set up"}</option>
    <option value="passphrase">Encrypted with a passphrase (type the same one on every PC)</option>
    <option value="off">Not encrypted (readable in the repository)</option>
  </select>
</label>
{#if encrypt === "vault" && keyVaults.length > 1}
  <label class="field"><span>The vault whose key it is (choose the same one on every PC)</span>
    <select value={app.settings["sync.keySource"] || keyVaults[0].id} onchange={(e) => app.setSetting("sync.keySource", (e.target as HTMLSelectElement).value)}>
      {#each keyVaults as v (v.id)}<option value={v.id}>{v.name} ({v.account})</option>{/each}
    </select>
  </label>
{/if}
{#if encrypt === "passphrase"}
  <input type="password" bind:value={passphrase} placeholder={hasPassphrase ? "Leave empty to keep the saved passphrase" : "At least 8 characters"} autocomplete="off" />
{/if}
<p class="muted small">
  {#if encrypt === "off"}
    Anyone who can read the repository sees your hosts, user names and folders.
  {:else}
    Each item is encrypted on its own (AES-256-GCM); only random item ids stay readable, so the history still shows when
    something changed but not what. {encrypt === "vault" ? "The key never leaves Cablebag and the vault server does not know it." : "Without the passphrase nobody can read the files, you neither: keep it in the vault."}
  {/if}
</p>

<label class="check">
  <input type="checkbox" checked={!!app.settings["sync.auto"]} onchange={(e) => app.setSetting("sync.auto", (e.target as HTMLInputElement).checked)} />
  Sync by itself: when Cablebag starts, a few seconds after a change, and every five minutes
</label>

<div class="buttons">
  <button class="btn" disabled={!ready} onclick={save}>Save</button>
  <button
    class="btn primary"
    disabled={!ready || sync.busy}
    onclick={async () => {
      if (await save()) sync.now();
    }}><RefreshCw size={14} /> {sync.busy ? "Syncing..." : "Save and sync now"}</button
  >
  {#if webUrl}
    <button class="btn" onclick={() => openUrl(webUrl)}><ExternalLink size={14} /> Open the repository</button>
  {/if}
</div>

{#if sync.last}
  <p class="state" class:bad={!!sync.last.error}>{when(sync.last.at)}: {sync.summary(sync.last)}</p>
  {#if sync.last.conflict_list?.length}
    <div class="conflicts">
      <p>Changed on this PC and on another one since the last sync. The version named here is kept; change the item again if the other one was right.</p>
      <ul>
        {#each sync.last.conflict_list as c}
          <li><span class="muted">{c.what}</span> <strong>{c.name}</strong>: {c.kept}'s version kept</li>
        {/each}
      </ul>
    </div>
  {/if}
{/if}

<style>
  .row2 {
    display: flex;
    gap: 10px;
  }
  .grow {
    flex: 1;
  }
  .choice {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .state {
    margin: 0;
    padding: 8px 10px;
    border-radius: 3px;
    background: var(--hover);
  }
  .conflicts {
    margin-top: 8px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 3px;
    font-size: 12px;
    line-height: 1.5;
  }
  .conflicts p {
    margin: 0 0 4px;
  }
  .conflicts ul {
    margin: 0;
    padding-left: 18px;
  }
  .state.bad {
    color: var(--danger);
  }
</style>

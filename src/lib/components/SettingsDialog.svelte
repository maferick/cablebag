<script lang="ts">
  // Settings opens as a tab, so the terminal stays in view while fonts and colours change.
  import { app, SETTINGS_PAGES, SCALES } from "../state.svelte";
  import { proxmox } from "../proxmox.svelte";
  import ProxmoxSettings from "./ProxmoxSettings.svelte";
  import SyncSettings from "./SyncSettings.svelte";
  import { api, type KnownHost, type SecretInfo } from "../api";
  import { SCHEMES } from "../schemes";
  import { BUILTIN_SYNTAX, type SyntaxSet } from "../highlight";
  import { vault } from "../vault.svelte";
  import { update } from "../update.svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { Trash2, Eye, FolderOpen, Lock, LockOpen, RefreshCw } from "@lucide/svelte";

  const COLOURS = ["red", "yellow", "green", "cyan", "blue", "magenta", "underline"];
  const COLOUR_NAMES: Record<string, string> = {
    red: "Red", yellow: "Yellow", green: "Green", cyan: "Cyan", blue: "Blue", magenta: "Magenta", underline: "Underlined",
  };

  function custom(): SyntaxSet {
    return (app.settings["term.customSyntax"] as SyntaxSet) ?? { id: "custom", name: "My own words", rules: [] };
  }

  function patternsFor(color: string): string {
    return custom().rules.find((r) => r.color === color)?.patterns.join(", ") ?? "";
  }

  function setPatterns(color: string, text: string) {
    const c = custom();
    const rules = c.rules.filter((r) => r.color !== color);
    const patterns = text.split(",").map((p) => p.trim()).filter(Boolean);
    if (patterns.length) rules.push({ color, patterns });
    app.setSetting("term.customSyntax", { ...c, rules });
  }

  async function chooseLogDir() {
    const d = await openDialog({ directory: true, title: "Folder for terminal logs" });
    if (d && !Array.isArray(d)) app.setSetting("log.dir", d);
  }

  async function enableLogging(on: boolean) {
    if (on && !app.settings["log.dir"]) await app.setSetting("log.dir", await api.defaultLogDir());
    await app.setSetting("log.enabled", on);
  }

  const pages = SETTINGS_PAGES;
  let page = $derived(app.settingsPage);
  let hosts = $state<KnownHost[]>([]);
  let secrets = $state<SecretInfo[]>([]);
  let revealed = $state<Record<string, string>>({});
  let dataFolder = $state("");
  let vwServer = $state("");
  let vwEmail = $state("");

  async function saveVault() {
    await api.vaultConfigure(vwServer, vwEmail);
    await vault.refresh();
    app.notify("Vault settings saved");
  }

  $effect(() => {
    if (page === "hosts") api.knownHosts().then((h) => (hosts = h));
    if (page === "passwords") api.secrets().then((s) => (secrets = s));
    if (page === "general") api.dataFolder().then((d) => (dataFolder = d));
    if (page === "proxmox") proxmox.load();
    if (page === "vault")
      vault.refresh().then(() => {
        vwServer = vault.status.server || "";
        vwEmail = vault.status.email;
      });
  });

  const set = (k: string) => (e: Event) => {
    const t = e.target as HTMLInputElement;
    app.setSetting(k, t.type === "checkbox" ? t.checked : t.type === "number" ? Number(t.value) : t.value);
  };

  function date(t: number) {
    return new Date(t * 1000).toLocaleDateString();
  }
</script>

<div class="settings">
  <div class="layout">
    <nav>
      {#each pages as p}
        <button class:on={page === p.id} onclick={() => (app.settingsPage = p.id)}>{p.label}</button>
      {/each}
    </nav>
    <div class="page">
      {#if page === "general"}
        <label class="field"><span>Theme</span>
          <select value={app.settings["ui.theme"]} onchange={set("ui.theme")}>
            <option value="system">Same as Windows</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </label>
        <label class="field"><span>Interface size (Ctrl+Shift+plus and minus)</span>
          <select value={String(app.settings["ui.scale"] ?? 100)} onchange={(e) => app.setSetting("ui.scale", Number((e.target as HTMLSelectElement).value))}>
            {#each SCALES as v}<option value={String(v)}>{v}%{v === 100 ? " (normal)" : ""}</option>{/each}
          </select>
        </label>
        <p class="muted small">Makes everything in the window larger or smaller. The terminal text has its own size on the Terminal page (Ctrl+plus and minus) on top of this.</p>
        <label class="field"><span>Default local shell (the + button)</span>
          <select value={app.settings["shell.default"]} onchange={set("shell.default")}>
            <option value="">First available</option>
            {#each app.shells as s}<option value={s.id}>{s.name}</option>{/each}
          </select>
        </label>
        <label class="check"><input type="checkbox" checked={app.settings["ui.confirmClose"]} onchange={set("ui.confirmClose")} /> Ask before closing Cablebag while sessions are connected</label>
        <label class="check"><input type="checkbox" checked={app.settings["reach.enabled"]} onchange={set("reach.enabled")} /> Show which hosts answer, with a dot on each session in the tree</label>
        <div class="row2">
          <label class="field grow"><span>How to check</span>
            <select value={app.settings["reach.mode"]} onchange={set("reach.mode")} disabled={!app.settings["reach.enabled"]}>
              <option value="ping">Ping first; try the port only when ping gets no answer</option>
              <option value="port">Always try the port (also shows a stopped service)</option>
            </select>
          </label>
          <label class="field"><span>Every (seconds)</span><input type="number" min="30" max="3600" value={app.settings["reach.interval"]} onchange={set("reach.interval")} disabled={!app.settings["reach.enabled"]} /></label>
        </div>
        <p class="muted small">Only sessions in open folders are checked, and only sessions Cablebag can reach directly: not through a jump host or gateway. Trying the port leaves a line in the server's log each time (for example "connection closed" in the SSH log), which tools like fail2ban may count against this PC; ping does not.</p>
        <div class="field"><span>Sessions</span>
          <div><button class="btn" onclick={() => (app.importOpen = true)}>Import sessions</button></div>
        </div>
        <div class="field"><span>Updates</span>
          <div class="upd">
            <span>
              Cablebag {update.current}{#if update.found}: version {update.found.version} is available{:else if update.lastChecked && !update.error}, the newest version{/if}
            </span>
            {#if update.found}
              <button class="btn primary" disabled={update.installing} onclick={() => update.install()}>Install and restart</button>
            {/if}
            <button class="btn" disabled={update.checking} onclick={() => update.check()}>{update.checking ? "Checking..." : "Check now"}</button>
          </div>
        </div>
        {#if update.error}<p class="small bad">{update.error}</p>{/if}
        <label class="check"><input type="checkbox" checked={app.settings["update.auto"] !== false} onchange={set("update.auto")} /> Check for updates by itself (at the start and every six hours)</label>
        <p class="muted small">Updates come only from the official page, github.com/maferick/cablebag, and are installed only when they carry the Cablebag signature.</p>
        <p class="muted small">Cablebag keeps its database in <span class="mono">{dataFolder}</span></p>
      {:else if page === "terminal"}
        <div class="row2">
          <label class="field grow"><span>Font</span><input value={app.settings["term.font"]} onchange={set("term.font")} /></label>
          <label class="field"><span>Size</span><input type="number" min="7" max="40" value={app.settings["term.fontSize"]} onchange={set("term.fontSize")} /></label>
        </div>
        <label class="field"><span>Colour scheme</span>
          <select value={app.settings["term.scheme"]} onchange={set("term.scheme")}>
            {#each SCHEMES as s}<option value={s.id}>{s.name}</option>{/each}
          </select>
        </label>
        <div class="row2">
          <label class="field grow"><span>Cursor</span>
            <select value={app.settings["term.cursor"]} onchange={set("term.cursor")}>
              <option value="block">Block</option>
              <option value="underline">Underline</option>
              <option value="bar">Line</option>
            </select>
          </label>
          <label class="field grow"><span>Lines kept in scrollback</span><input type="number" value={app.settings["term.scrollback"]} onchange={set("term.scrollback")} /></label>
        </div>
        <label class="check"><input type="checkbox" checked={app.settings["term.cursorBlink"]} onchange={set("term.cursorBlink")} /> Blinking cursor</label>
        <label class="check"><input type="checkbox" checked={app.settings["term.copyOnSelect"]} onchange={set("term.copyOnSelect")} /> Copy text as soon as it is selected</label>
        <label class="check"><input type="checkbox" checked={app.settings["term.rightClickPaste"]} onchange={set("term.rightClickPaste")} /> Paste with the right mouse button</label>
        <label class="check"><input type="checkbox" checked={app.settings["term.pasteWarning"]} onchange={set("term.pasteWarning")} /> Ask before pasting several lines at once</label>
        <p class="muted small">Font and scheme changes apply to open terminals straight away; cursor blink and scrollback apply to new tabs.</p>
      {:else if page === "highlighting"}
        <label class="field"><span>Colour these words in terminal output</span>
          <select value={app.settings["term.syntax"]} onchange={set("term.syntax")}>
            {#each BUILTIN_SYNTAX as s}<option value={s.id}>{s.name}</option>{/each}
            <option value="custom">My own words (below)</option>
          </select>
        </label>
        <p class="muted small">Sessions can choose their own set in the session editor. Programs that colour their own output, and full-screen programs such as vim, are left alone.</p>
        <strong class="sub">My own words</strong>
        {#each COLOURS as c}
          <label class="field"><span>{COLOUR_NAMES[c]}</span>
            <input value={patternsFor(c)} onchange={(e) => setPatterns(c, (e.target as HTMLInputElement).value)} placeholder="Words separated by commas" />
          </label>
        {/each}
        <label class="check"><input type="checkbox" checked={!!custom().regex} onchange={(e) => app.setSetting("term.customSyntax", { ...custom(), regex: (e.target as HTMLInputElement).checked })} /> Treat the words as regular expressions</label>
        <label class="check"><input type="checkbox" checked={!!custom().caseSensitive} onchange={(e) => app.setSetting("term.customSyntax", { ...custom(), caseSensitive: (e.target as HTMLInputElement).checked })} /> Match upper and lower case exactly</label>
        <p class="muted small">Changes apply to terminals opened afterwards.</p>
      {:else if page === "logging"}
        <label class="check"><input type="checkbox" checked={app.settings["log.enabled"]} onchange={(e) => enableLogging((e.target as HTMLInputElement).checked)} /> Write the output of every terminal to a log file</label>
        <label class="field"><span>Folder</span>
          <div class="with-btn">
            <input value={app.settings["log.dir"]} onchange={set("log.dir")} placeholder="Documents\Cablebag logs" />
            <button class="btn" onclick={chooseLogDir}><FolderOpen size={14} /> Browse</button>
          </div>
        </label>
        <label class="field"><span>What to write</span>
          <select value={app.settings["log.kind"]} onchange={set("log.kind")}>
            <option value="text">The text on screen</option>
            <option value="timestamps">The text on screen, each line with date and time</option>
            <option value="raw">Everything, including control codes</option>
          </select>
        </label>
        <label class="field"><span>File name</span><input value={app.settings["log.name"]} onchange={set("log.name")} /></label>
        <p class="muted small">
          In the file name, &amp;S becomes the session type, &amp;H the host, &amp;U the user, &amp;P the port, &amp;Y, &amp;M and &amp;D the date and &amp;T the time.
          Logging starts with terminals opened after you switch it on.
        </p>
      {:else if page === "ssh"}
        <label class="check"><input type="checkbox" checked={app.settings["ssh.agent.pageant"]} onchange={set("ssh.agent.pageant")} /> Try keys from PuTTY Pageant</label>
        <label class="check"><input type="checkbox" checked={app.settings["ssh.agent.openssh"]} onchange={set("ssh.agent.openssh")} /> Try keys from the Windows OpenSSH agent</label>
        <p class="muted small">Agent keys are tried before the key file and password of a session.</p>
        <label class="field"><span>Remote files when an SSH session connects</span>
          <select value={app.settings["ssh.autoFiles"] === false ? "hide" : "show"} onchange={(e) => app.setSetting("ssh.autoFiles", (e.target as HTMLSelectElement).value === "show")}>
            <option value="show">Show</option>
            <option value="hide">Don't show</option>
          </select>
        </label>
        <p class="muted small">A session can choose differently (its editor, Remote files). Ctrl+Shift+E shows or hides them for the tab you are in.</p>
      {:else if page === "vault"}
        <p class="muted small">
          Sessions and Proxmox servers can take their login from your Vaultwarden vault. The decrypted vault stays in memory
          only; what this PC keeps between starts depends on the choice below.
        </p>
        <label class="field"><span>Server</span><input bind:value={vwServer} placeholder="https://vault.example.com" /></label>
        <label class="field"><span>Email address of the account</span><input bind:value={vwEmail} /></label>
        <div><button class="btn" onclick={saveVault} disabled={!vwServer.trim() || !vwEmail.trim()}>Save</button></div>
        <label class="field"><span>When Cablebag starts</span>
          <select
            value={app.settings["vault.unlock"]}
            onchange={async (e) => {
              const v = (e.target as HTMLSelectElement).value;
              await app.setSetting("vault.unlock", v);
              if (v === "password") await api.vaultForget();
              await vault.refresh();
            }}
          >
            <option value="hello" disabled={!vault.status.hello}>Unlock with Windows Hello (PIN, fingerprint or face){vault.status.hello ? "" : " - not set up on this PC"}</option>
            <option value="remember">Unlock without asking</option>
            <option value="password">Ask for the master password every time</option>
          </select>
        </label>
        <p class="muted small">
          {#if app.settings["vault.unlock"] === "password"}
            Safest: nothing about the vault stays on this PC, so every start asks for the master password.
          {:else}
            After one unlock with the master password, this PC keeps the vault key and the vault login, encrypted so that
            only your Windows account can read them (the same protection as Cablebag's saved passwords).
            {app.settings["vault.unlock"] === "hello"
              ? "Windows Hello is asked once per start before they are used."
              : "They are used without a question, so anyone who can use your Windows account can open the vault."}
            Signing out on the vault server, or choosing "every time", removes them.
          {/if}
          {#if vault.status.remembered}<strong> This PC keeps the key now.</strong>{/if}
        </p>
        {#if vault.status.configured}
          <div class="vstate">
            {#if vault.status.unlocked}
              <span class="ok">Unlocked</span>
              <span class="muted">{vault.status.items} logins, synced {new Date(vault.status.synced * 1000).toLocaleTimeString()}</span>
              <span class="grow"></span>
              <button class="btn" disabled={vault.busy} onclick={() => vault.sync()}><RefreshCw size={14} /> Sync now</button>
              <button class="btn" onclick={() => vault.lock()}><Lock size={14} /> Lock</button>
            {:else}
              <span class="muted">Locked</span>
              <span class="grow"></span>
              <button class="btn primary" disabled={vault.busy} onclick={() => vault.unlock()}><LockOpen size={14} /> {vault.busy ? "Unlocking..." : "Unlock"}</button>
            {/if}
          </div>
          <p class="muted small">Two-step login with an authenticator app or email code is supported; Cablebag can remember this PC so the code is asked only once.</p>
        {/if}
      {:else if page === "sync"}
        <SyncSettings />
      {:else if page === "proxmox"}
        <ProxmoxSettings />
      {:else if page === "passwords"}
        <p class="muted small">Passwords are encrypted and can only be read by your Windows account on this PC.</p>
        {#if !secrets.length}<p class="muted">No saved passwords yet.</p>{/if}
        <table>
          <tbody>
            {#each secrets as s (s.id)}
              <tr>
                <td>{s.kind === "passphrase" ? "Key passphrase" : s.protocol.toUpperCase()}</td>
                <td class="mono">{s.kind === "passphrase" ? s.label : `${s.username}@${s.host}${s.port ? ":" + s.port : ""}`}</td>
                <td class="mono">{revealed[s.id] ?? "••••••••"}</td>
                <td class="act">
                  <button class="icon-btn" title="Show" onclick={async () => (revealed[s.id] = await api.revealSecret(s.id))}><Eye size={14} /></button>
                  <button class="icon-btn" title="Forget" onclick={async () => { await api.deleteSecret(s.id); secrets = await api.secrets(); }}><Trash2 size={14} /></button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else if page === "hosts"}
        <p class="muted small">Servers whose identity you confirmed. Forget one to be asked again on the next connection.</p>
        {#if !hosts.length}<p class="muted">No trusted servers yet.</p>{/if}
        <table>
          <tbody>
            {#each hosts as h (h.host + h.port + h.algo)}
              <tr>
                <td class="mono">{h.host}:{h.port}</td>
                <td>{h.algo}</td>
                <td class="mono fp" title={h.fingerprint}>{h.fingerprint}</td>
                <td>{date(h.added)}</td>
                <td class="act">
                  <button class="icon-btn" title="Forget" onclick={async () => { await api.deleteKnownHost(h.host, h.port, h.algo); hosts = await api.knownHosts(); }}><Trash2 size={14} /></button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </div>
</div>

<style>
  .settings {
    position: absolute;
    inset: 0;
    overflow: auto;
    padding: 20px 28px 40px;
    background: var(--bg);
  }
  .layout {
    display: flex;
    gap: 28px;
    max-width: 980px;
  }
  nav {
    position: sticky;
    top: 0;
    align-self: flex-start;
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 180px;
    flex-shrink: 0;
  }
  nav button {
    text-align: left;
    padding: 7px 10px;
    border: none;
    background: none;
    border-radius: 3px;
    color: var(--muted);
  }
  nav button.on {
    background: var(--selected);
    color: var(--text);
  }
  .page {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .upd {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .bad {
    color: var(--danger);
  }
  .row2 {
    display: flex;
    gap: 10px;
  }
  .grow {
    flex: 1;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
  }
  .with-btn {
    display: flex;
    gap: 6px;
  }
  .vstate {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
  .vstate .ok {
    color: var(--ok);
    font-weight: 600;
  }
  .vstate .grow {
    flex: 1;
  }
  .with-btn input {
    flex: 1;
  }
  .page {
    max-height: 64vh;
    overflow: auto;
    padding-right: 4px;
  }
  table {
    border-collapse: collapse;
    width: 100%;
  }
  td {
    padding: 5px 6px;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }
  .fp {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .act {
    text-align: right;
    white-space: nowrap;
  }
</style>

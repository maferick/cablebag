<script lang="ts">
  // Settings opens as a tab, so the terminal stays in view while fonts and colours change.
  import { app, SETTINGS_PAGES, SCALES } from "../state.svelte";
  import { proxmox } from "../proxmox.svelte";
  import ProxmoxSettings from "./ProxmoxSettings.svelte";
  import SyncSettings from "./SyncSettings.svelte";
  import AssistantSettings from "./AssistantSettings.svelte";
  import { api, type KnownHost, type SecretInfo } from "../api";
  import { SCHEMES } from "../schemes";
  import { BUILTIN_SYNTAX, type SyntaxSet } from "../highlight";
  import { vault } from "../vault.svelte";
  import { update } from "../update.svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { Trash2, Eye, FolderOpen, Lock, LockOpen, RefreshCw, Search } from "@lucide/svelte";
  import SettingsSection from "./SettingsSection.svelte";
  import SettingsRow from "./SettingsRow.svelte";

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

  /** One line under each page title. */
  const INTRO: Record<string, string> = {
    general: "How Cablebag looks and behaves, and its updates.",
    terminal: "Font, colours, cursor and copying in every terminal.",
    highlighting: "Words such as error and warning in colour, so they stand out in output.",
    logging: "Keep what terminals show in files.",
    ssh: "Keys and the remote files of SSH sessions.",
    vault: "Logins from your Vaultwarden or Bitwarden-compatible vault.",
    proxmox: "Proxmox servers whose machines show in the session tree.",
    sync: "The same sessions on every PC, through a git repository.",
    assistant: "The AI assistant: its hosts, its memory and what it may do.",
    passwords: "Passwords and key passphrases Cablebag keeps on this PC.",
    hosts: "The SSH servers whose identity you confirmed.",
  };

  /** Words a page answers to in "Find a setting", besides its name and intro. */
  const WORDS: Record<string, string> = {
    general: "theme dark light size zoom scale shell close import update version reach dots ping port",
    terminal: "font size scheme colours cursor blink scrollback copy paste right click",
    highlighting: "words colour syntax error warning regex",
    logging: "log file folder timestamps",
    ssh: "keys agent pageant openssh remote files sftp",
    vault: "vaultwarden bitwarden master password windows hello unlock lock",
    proxmox: "pve token server",
    sync: "git gitea github gitlab repository encrypt passphrase",
    assistant: "ai llm model openai litellm ollama memory protected folders rules always log",
    passwords: "saved secrets passphrase",
    hosts: "known hosts fingerprint trusted",
  };
  let find = $state("");
  const shownPages = $derived(
    pages.filter((p) => {
      const q = find.trim().toLowerCase();
      if (!q) return true;
      const all = `${p.label} ${INTRO[p.id] ?? ""} ${WORDS[p.id] ?? ""}`.toLowerCase();
      return q.split(/\s+/).every((w) => all.includes(w));
    }),
  );
  // The first matching page opens while you type.
  $effect(() => {
    if (find.trim() && shownPages.length && !shownPages.some((p) => p.id === app.settingsPage)) app.settingsPage = shownPages[0].id;
  });
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
      <div class="find">
        <Search size={13} />
        <input bind:value={find} placeholder="Find a setting" aria-label="Find a setting" />
      </div>
      {#each shownPages as p}
        <button class:on={page === p.id} onclick={() => (app.settingsPage = p.id)}>{p.label}</button>
      {:else}
        <p class="none">No setting matches "{find}".</p>
      {/each}
    </nav>
    <div class="page">
      <header class="phead">
        <h2>{pages.find((p) => p.id === page)?.label}</h2>
        {#if INTRO[page]}<p>{INTRO[page]}</p>{/if}
      </header>

      {#if page === "general"}
        <SettingsSection title="Look">
          <SettingsRow label="Theme">
            <select value={app.settings["ui.theme"]} onchange={set("ui.theme")}>
              <option value="system">Same as Windows</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </SettingsRow>
          <SettingsRow label="Interface size" help="Everything in the window; the terminal text has its own size on top (Terminal page). Also Ctrl+Shift+plus and minus.">
            <select value={String(app.settings["ui.scale"] ?? 100)} onchange={(e) => app.setSetting("ui.scale", Number((e.target as HTMLSelectElement).value))}>
              {#each SCALES as v}<option value={String(v)}>{v}%{v === 100 ? " (normal)" : ""}</option>{/each}
            </select>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Working">
          <SettingsRow label="Default local shell" help="What the + button opens.">
            <select value={app.settings["shell.default"]} onchange={set("shell.default")}>
              <option value="">First available</option>
              {#each app.shells as s}<option value={s.id}>{s.name}</option>{/each}
            </select>
          </SettingsRow>
          <SettingsRow label="Ask before closing" help="When sessions are still connected.">
            <input type="checkbox" class="switch" checked={app.settings["ui.confirmClose"]} onchange={set("ui.confirmClose")} />
          </SettingsRow>
          <SettingsRow label="Import sessions" help="From PuTTY, KiTTY, WinSCP, the OpenSSH config, Remote Desktop files or session files.">
            <button class="btn" onclick={() => (app.importOpen = true)}>Import sessions...</button>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Which hosts answer" desc="A dot on each session in the tree. Only sessions in open folders are checked, and not those behind a jump host or gateway.">
          <SettingsRow label="Show the dots">
            <input type="checkbox" class="switch" checked={app.settings["reach.enabled"]} onchange={set("reach.enabled")} />
          </SettingsRow>
          <SettingsRow label="How to check" help="Trying the port leaves a line in the server's log each time, which tools like fail2ban may count against this PC; ping does not.">
            <select value={app.settings["reach.mode"]} onchange={set("reach.mode")} disabled={!app.settings["reach.enabled"]}>
              <option value="ping">Ping first, the port only without an answer</option>
              <option value="port">Always the port (also shows a stopped service)</option>
            </select>
          </SettingsRow>
          <SettingsRow label="Check every" help="Seconds.">
            <input type="number" min="30" max="3600" value={app.settings["reach.interval"]} onchange={set("reach.interval")} disabled={!app.settings["reach.enabled"]} />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Updates" desc="Only from the official page, github.com/maferick/cablebag, and only with the Cablebag signature.">
          <SettingsRow label="Cablebag {update.current}" help={update.found ? `Version ${update.found.version} is available.` : update.lastChecked && !update.error ? "The newest version." : update.error || ""}>
            <div class="btns">
              {#if update.found}<button class="btn primary" disabled={update.installing} onclick={() => update.install()}>Install and restart</button>{/if}
              <button class="btn" disabled={update.checking} onclick={() => update.check()}>{update.checking ? "Checking..." : "Check now"}</button>
            </div>
          </SettingsRow>
          <SettingsRow label="Check by itself" help="At the start and every six hours.">
            <input type="checkbox" class="switch" checked={app.settings["update.auto"] !== false} onchange={set("update.auto")} />
          </SettingsRow>
        </SettingsSection>

        <p class="foot">Cablebag keeps its database in <span class="mono">{dataFolder}</span></p>
      {:else if page === "terminal"}
        <SettingsSection title="Text" desc="Font and colour changes apply to open terminals straight away.">
          <SettingsRow label="Font">
            <input value={app.settings["term.font"]} onchange={set("term.font")} />
          </SettingsRow>
          <SettingsRow label="Size" help="Also Ctrl+plus and minus in a terminal.">
            <input type="number" min="7" max="40" value={app.settings["term.fontSize"]} onchange={set("term.fontSize")} />
          </SettingsRow>
          <SettingsRow label="Colour scheme">
            <select value={app.settings["term.scheme"]} onchange={set("term.scheme")}>
              {#each SCHEMES as s}<option value={s.id}>{s.name}</option>{/each}
            </select>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Cursor and history" desc="These apply to terminals opened afterwards.">
          <SettingsRow label="Cursor">
            <select value={app.settings["term.cursor"]} onchange={set("term.cursor")}>
              <option value="block">Block</option>
              <option value="underline">Underline</option>
              <option value="bar">Line</option>
            </select>
          </SettingsRow>
          <SettingsRow label="Blinking cursor">
            <input type="checkbox" class="switch" checked={app.settings["term.cursorBlink"]} onchange={set("term.cursorBlink")} />
          </SettingsRow>
          <SettingsRow label="Lines kept in scrollback">
            <input type="number" value={app.settings["term.scrollback"]} onchange={set("term.scrollback")} />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Copy and paste">
          <SettingsRow label="Copy as soon as text is selected">
            <input type="checkbox" class="switch" checked={app.settings["term.copyOnSelect"]} onchange={set("term.copyOnSelect")} />
          </SettingsRow>
          <SettingsRow label="Paste with the right mouse button">
            <input type="checkbox" class="switch" checked={app.settings["term.rightClickPaste"]} onchange={set("term.rightClickPaste")} />
          </SettingsRow>
          <SettingsRow label="Ask before pasting several lines" help="So a pasted block does not run line by line by accident.">
            <input type="checkbox" class="switch" checked={app.settings["term.pasteWarning"]} onchange={set("term.pasteWarning")} />
          </SettingsRow>
        </SettingsSection>
      {:else if page === "highlighting"}
        <SettingsSection title="Words in colour" desc="Programs that colour their own output, and full-screen programs such as vim, are left alone. Sessions can choose their own set in the session editor.">
          <SettingsRow label="Colour these words">
            <select value={app.settings["term.syntax"]} onchange={set("term.syntax")}>
              {#each BUILTIN_SYNTAX as s}<option value={s.id}>{s.name}</option>{/each}
              <option value="custom">My own words (below)</option>
            </select>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="My own words" desc="Words separated by commas. Changes apply to terminals opened afterwards.">
          {#each COLOURS as c}
            <SettingsRow label={COLOUR_NAMES[c]}>
              <input value={patternsFor(c)} onchange={(e) => setPatterns(c, (e.target as HTMLInputElement).value)} placeholder="Words separated by commas" />
            </SettingsRow>
          {/each}
          <SettingsRow label="Regular expressions" help="Treat the words as patterns.">
            <input type="checkbox" class="switch" checked={!!custom().regex} onchange={(e) => app.setSetting("term.customSyntax", { ...custom(), regex: (e.target as HTMLInputElement).checked })} />
          </SettingsRow>
          <SettingsRow label="Match case exactly">
            <input type="checkbox" class="switch" checked={!!custom().caseSensitive} onchange={(e) => app.setSetting("term.customSyntax", { ...custom(), caseSensitive: (e.target as HTMLInputElement).checked })} />
          </SettingsRow>
        </SettingsSection>
      {:else if page === "logging"}
        <SettingsSection title="Terminal logs" desc="Logging starts with terminals opened after you switch it on.">
          <SettingsRow label="Write every terminal to a file">
            <input type="checkbox" class="switch" checked={app.settings["log.enabled"]} onchange={(e) => enableLogging((e.target as HTMLInputElement).checked)} />
          </SettingsRow>
          <SettingsRow label="Folder">
            <div class="with-btn">
              <input value={app.settings["log.dir"]} onchange={set("log.dir")} placeholder="Documents\Cablebag logs" />
              <button class="btn" onclick={chooseLogDir}><FolderOpen size={14} /> Browse</button>
            </div>
          </SettingsRow>
          <SettingsRow label="What to write">
            <select value={app.settings["log.kind"]} onchange={set("log.kind")}>
              <option value="text">The text on screen</option>
              <option value="timestamps">The text, each line with date and time</option>
              <option value="raw">Everything, with control codes</option>
            </select>
          </SettingsRow>
          <SettingsRow label="File name" help="&S session type, &H host, &U user, &P port, &Y &M &D the date, &T the time.">
            <input value={app.settings["log.name"]} onchange={set("log.name")} />
          </SettingsRow>
        </SettingsSection>
      {:else if page === "ssh"}
        <SettingsSection title="Keys from an agent" desc="Agent keys are tried before the key file and password of a session.">
          <SettingsRow label="PuTTY Pageant">
            <input type="checkbox" class="switch" checked={app.settings["ssh.agent.pageant"]} onchange={set("ssh.agent.pageant")} />
          </SettingsRow>
          <SettingsRow label="Windows OpenSSH agent">
            <input type="checkbox" class="switch" checked={app.settings["ssh.agent.openssh"]} onchange={set("ssh.agent.openssh")} />
          </SettingsRow>
        </SettingsSection>
        <SettingsSection title="Remote files">
          <SettingsRow label="When an SSH session connects" help="A session can choose differently in its editor. Ctrl+Shift+E shows or hides them for the tab you are in.">
            <select value={app.settings["ssh.autoFiles"] === false ? "hide" : "show"} onchange={(e) => app.setSetting("ssh.autoFiles", (e.target as HTMLSelectElement).value === "show")}>
              <option value="show">Show</option>
              <option value="hide">Don't show</option>
            </select>
          </SettingsRow>
        </SettingsSection>
      {:else if page === "vault"}
        <SettingsSection title="Vault server" desc="Sessions, Proxmox servers and AI hosts can take their login from it. The opened vault stays in memory only.">
          <SettingsRow label="Server">
            <input bind:value={vwServer} placeholder="https://vault.example.com" />
          </SettingsRow>
          <SettingsRow label="Email address of the account">
            <input bind:value={vwEmail} />
          </SettingsRow>
          <SettingsRow>
            <div class="btns end"><button class="btn" onclick={saveVault} disabled={!vwServer.trim() || !vwEmail.trim()}>Save</button></div>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Opening the vault">
          <SettingsRow
            label="When Cablebag starts"
            help={app.settings["vault.unlock"] === "password"
              ? "Safest: nothing about the vault stays on this PC."
              : app.settings["vault.unlock"] === "hello"
                ? "This PC keeps the key, readable only by your Windows account; Windows Hello confirms it is you once per start."
                : "This PC keeps the key, readable only by your Windows account, and uses it without asking: anyone using your Windows account can open the vault."}
          >
            <select
              value={app.settings["vault.unlock"]}
              onchange={async (e) => {
                const v = (e.target as HTMLSelectElement).value;
                await app.setSetting("vault.unlock", v);
                if (v === "password") await api.vaultForget();
                await vault.refresh();
              }}
            >
              <option value="hello" disabled={!vault.status.hello}>Windows Hello (PIN, fingerprint or face){vault.status.hello ? "" : ", not set up here"}</option>
              <option value="remember">Without asking</option>
              <option value="password">The master password every time</option>
            </select>
          </SettingsRow>
          {#if vault.status.configured}
            <SettingsRow label={vault.status.unlocked ? "Open" : "Locked"} help={vault.status.unlocked ? `${vault.status.items} logins, synced ${new Date(vault.status.synced * 1000).toLocaleTimeString()}${vault.status.remembered ? "; this PC keeps the key" : ""}` : "Two-step login with an authenticator or email code is supported."}>
              <div class="btns end">
                {#if vault.status.unlocked}
                  <button class="btn" disabled={vault.busy} onclick={() => vault.sync()}><RefreshCw size={14} /> Sync now</button>
                  <button class="btn" onclick={() => vault.lock()}><Lock size={14} /> Lock</button>
                {:else}
                  <button class="btn primary" disabled={vault.busy} onclick={() => vault.unlock()}><LockOpen size={14} /> {vault.busy ? "Unlocking..." : "Unlock"}</button>
                {/if}
              </div>
            </SettingsRow>
          {/if}
        </SettingsSection>
      {:else if page === "assistant"}
        <AssistantSettings />
      {:else if page === "sync"}
        <SettingsSection title="Sync through git" desc="Sessions, folders, macros, tunnels, Proxmox servers and the assistant's memory, to a private repository; passwords never go there.">
          <div class="inner"><SyncSettings /></div>
        </SettingsSection>
      {:else if page === "proxmox"}
        <SettingsSection title="Proxmox servers" desc="Each server shows as a live folder of its machines in the session tree.">
          <div class="inner"><ProxmoxSettings /></div>
        </SettingsSection>
      {:else if page === "passwords"}
        <SettingsSection title="Saved passwords" desc="Encrypted; only your Windows account on this PC can read them.">
          {#if !secrets.length}
            <SettingsRow><span class="muted">No saved passwords yet.</span></SettingsRow>
          {/if}
          {#each secrets as s (s.id)}
            <div class="item">
              <span class="what">{s.kind === "passphrase" ? "Key passphrase" : s.protocol.toUpperCase()}</span>
              <span class="mono grow">{s.kind === "passphrase" ? s.label : `${s.username}@${s.host}${s.port ? ":" + s.port : ""}`}</span>
              <span class="mono">{revealed[s.id] ?? "••••••••"}</span>
              <button class="icon-btn" title="Show" onclick={async () => (revealed[s.id] = await api.revealSecret(s.id))}><Eye size={14} /></button>
              <button class="icon-btn" title="Forget" onclick={async () => { await api.deleteSecret(s.id); secrets = await api.secrets(); }}><Trash2 size={14} /></button>
            </div>
          {/each}
        </SettingsSection>
      {:else if page === "hosts"}
        <SettingsSection title="Trusted servers" desc="Servers whose identity you confirmed. Forget one to be asked again on the next connection.">
          {#if !hosts.length}
            <SettingsRow><span class="muted">No trusted servers yet.</span></SettingsRow>
          {/if}
          {#each hosts as h (h.host + h.port + h.algo)}
            <div class="item">
              <span class="mono what">{h.host}:{h.port}</span>
              <span class="muted">{h.algo}</span>
              <span class="mono grow fp" title={h.fingerprint}>{h.fingerprint}</span>
              <span class="muted">{date(h.added)}</span>
              <button class="icon-btn" title="Forget" onclick={async () => { await api.deleteKnownHost(h.host, h.port, h.algo); hosts = await api.knownHosts(); }}><Trash2 size={14} /></button>
            </div>
          {/each}
        </SettingsSection>
      {/if}
    </div>
  </div>
</div>

<style>
  .settings {
    position: absolute;
    inset: 0;
    overflow: auto;
    background: var(--bg);
  }
  .layout {
    display: flex;
    gap: 32px;
    max-width: 1080px;
    padding: 24px 32px 48px;
  }
  nav {
    position: sticky;
    top: 0;
    align-self: flex-start;
    display: flex;
    flex-direction: column;
    gap: 1px;
    width: 190px;
    flex-shrink: 0;
  }
  .find {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    margin-bottom: 10px;
    border: 1px solid var(--border-strong);
    border-radius: 3px;
    background: var(--field);
    color: var(--faint);
  }
  .find input {
    flex: 1;
    min-width: 0;
    border: none;
    padding: 5px 0;
    background: none;
    box-shadow: none;
  }
  nav button {
    text-align: left;
    padding: 6px 10px;
    border: none;
    background: none;
    border-radius: 3px;
    color: var(--muted);
  }
  nav button:hover {
    background: var(--hover);
    color: var(--text);
  }
  nav button.on {
    background: var(--selected);
    color: var(--text);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .none {
    margin: 4px 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .page {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .phead h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }
  .phead p {
    margin: 4px 0 0;
    color: var(--muted);
    line-height: 1.5;
  }
  .btns {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .btns.end {
    justify-content: flex-end;
  }
  .with-btn {
    display: flex;
    gap: 6px;
  }
  .with-btn input {
    flex: 1;
  }
  .foot {
    margin: 0;
    font-size: 12px;
    color: var(--muted);
  }
  .inner {
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px 6px 14px;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }
  .item:last-child {
    border-bottom: none;
  }
  .what {
    min-width: 110px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .fp {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>

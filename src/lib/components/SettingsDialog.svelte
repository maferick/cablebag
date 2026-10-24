<script lang="ts">
  // Settings opens as a tab, so the terminal stays in view while fonts and colours change.
  import { tick } from "svelte";
  import { app, SETTINGS_PAGES, SCALES } from "../state.svelte";
  import { proxmox } from "../proxmox.svelte";
  import ProxmoxSettings from "./ProxmoxSettings.svelte";
  import SyncSettings from "./SyncSettings.svelte";
  import AssistantSettings from "./AssistantSettings.svelte";
  import AssistantLog from "./AssistantLog.svelte";
  import MemoryList from "./MemoryList.svelte";
  import CredentialSources from "./CredentialSources.svelte";
  import BrowserProfilesSettings from "./BrowserProfilesSettings.svelte";
  import { api, type KnownHost, type SecretInfo } from "../api";
  import { SCHEMES } from "../schemes";
  import { BUILTIN_SYNTAX, type SyntaxSet } from "../highlight";
  import { vault } from "../vault.svelte";
  import { memory } from "../memory.svelte";
  import { update } from "../update.svelte";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { writeText } from "../clipboard";
  import { ask } from "../ask.svelte";
  import { macros } from "../macros.svelte";
  import { SHORTCUTS } from "../shortcutList";
  import { Trash2, Eye, FolderOpen, Search, ExternalLink, Copy, KeyRound, ShieldCheck } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Logo from "./Logo.svelte";
  import SettingsSection from "./SettingsSection.svelte";
  import SettingsRow from "./SettingsRow.svelte";
  import SettingsMore from "./SettingsMore.svelte";
  import SettingsItem from "./SettingsItem.svelte";

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

  /** Log file names by what they hold; the &-codes only show for a pattern of your own. */
  const LOG_NAMES = [
    { value: "&S_&H_&Y-&M-&D_&T.log", name: "Kind of session, host, date and time" },
    { value: "&H_&Y-&M-&D_&T.log", name: "Host, date and time" },
    { value: "&H_&U_&Y-&M-&D_&T.log", name: "Host, user, date and time" },
    { value: "&H_&Y-&M-&D.log", name: "Host and date (one file per day)" },
  ];
  const logName = $derived(String(app.settings["log.name"] ?? LOG_NAMES[0].value));
  const ownLogName = $derived(!LOG_NAMES.some((n) => n.value === logName));
  let typingLogName = $state(false);

  /** How often the dots in the tree are checked, in seconds, by name. */
  const REACH_EVERY = [
    { value: 60, name: "Every minute" },
    { value: 120, name: "Every 2 minutes" },
    { value: 300, name: "Every 5 minutes" },
    { value: 900, name: "Every 15 minutes" },
    { value: 1800, name: "Every 30 minutes" },
  ];

  const pages = SETTINGS_PAGES;
  const groups = $derived([...new Set(pages.map((p) => p.group))]);

  /** One line under each page title. */
  const INTRO: Record<string, string> = {
    general: "How Cablebag looks, and how it starts and closes.",
    terminal: "Font, colours, cursor and copying in every terminal, words in colour and logs.",
    connections: "How sessions connect: SSH, keys from an agent, and which hosts answer.",
    browsers: "How web tabs behave, and the browser profiles that keep logins apart.",
    vault: "Where logins come from: Vaultwarden or Bitwarden, CyberArk, Delinea Secret Server and Proton Pass.",
    proxmox: "Proxmox servers whose machines show as a folder in the session tree, with SSH, Remote Desktop, the console, and start and stop.",
    passwords: "Passwords and keys Cablebag keeps on this PC, encrypted for your Windows account.",
    hosts: "SSH servers whose identity you confirmed. Forget one to be asked again on the next connection.",
    assistant: "The AI hosts, and what the assistant may do by itself.",
    memory: "What the assistant has learnt: per connection, per folder and in general.",
    ailog: "Every question with the screen text that went with it, every answer and every command that ran.",
    sync: "The same sessions on every PC, through a private git repository; passwords never go there.",
    keyboard: "Every shortcut, and the macros that have one.",
    about: "Your version, updates, the licence, and where your data is.",
  };

  /** Words a page answers to in "Find a setting", besides its name and intro (sections add their own). */
  const WORDS: Record<string, string> = {
    general: "theme dark light size zoom scale shell close tabs last time reopen start",
    terminal: "font size scheme colours cursor blink scrollback copy paste right click screen reader highlighting words colour syntax error warning regex log logging file folder timestamps",
    connections: "ssh keys agent pageant openssh reconnect connect again drop remote files sftp reach dots ping port answer fail2ban",
    browsers: "web browser profile tabs cookies tenant notifications do not disturb sleep memory teams private",
    vault: "vault vaultwarden bitwarden cyberark privilege cloud pvwa psm psmp delinea secret server thycotic proton pass pass-cli credential source master password windows hello unlock lock sign in check out",
    proxmox: "pve token server guests",
    passwords: "saved passwords secrets passphrase vault key remembered",
    hosts: "known hosts fingerprint trusted ssh key",
    assistant: "ai llm model openai litellm ollama hosts protected folders rules always snapshot limits commands minutes",
    memory: "assistant memory remembers facts",
    ailog: "assistant log sent answers commands history",
    sync: "git gitea github gitlab repository encrypt passphrase token",
    about: "version update licence license changelog what's new data folder issues backup restore problem report dotnet",
    keyboard: "shortcuts keys hotkeys ctrl alt tiling",
  };
  let find = $state("");
  const findWords = $derived(find.trim().toLowerCase().split(/\s+/).filter(Boolean));
  const shownPages = $derived(
    pages.filter((p) => {
      if (!findWords.length) return true;
      const all = `${p.label} ${INTRO[p.id] ?? ""} ${WORDS[p.id] ?? ""}`.toLowerCase();
      return findWords.every((w) => all.includes(w));
    }),
  );
  // The first matching page opens while you type, at the section the words are about.
  $effect(() => {
    if (findWords.length && shownPages.length && !shownPages.some((p) => p.id === app.settingsPage)) app.settingsPage = shownPages[0].id;
  });
  $effect(() => {
    const words = findWords;
    void app.settingsPage;
    if (!words.length) return;
    tick().then(() => {
      const sec = [...document.querySelectorAll<HTMLElement>(".settings section[data-find]")].find((s) => words.every((w) => (s.dataset.find ?? "").includes(w)));
      if (!sec) return;
      sec.scrollIntoView({ block: "start", behavior: "smooth" });
      sec.classList.add("found");
      setTimeout(() => sec.classList.remove("found"), 1500);
    });
  });
  let page = $derived(app.settingsPage);
  let hosts = $state<KnownHost[]>([]);
  let secrets = $state<SecretInfo[]>([]);
  let revealed = $state<Record<string, string>>({});
  let listFilter = $state("");
  let memoryFilter = $state("");
  let dataFolder = $state("");
  let sys = $state<{ version: string; windows: string; webview: string; dotnet: string } | null>(null);
  const details = $derived(
    sys ? [`Cablebag ${sys.version}`, sys.windows, `WebView2 ${sys.webview}`, `.NET 8 Desktop Runtime: ${sys.dotnet || "not installed"}`].join("\n") : "",
  );
  // A clock for "checked ... ago".
  let nowMs = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (nowMs = Date.now()), 30_000);
    return () => clearInterval(t);
  });
  const ago = (t: number) => {
    const m = Math.round((nowMs - t) / 60_000);
    return m < 1 ? "just now" : m < 60 ? `${m} min ago` : `${Math.round(m / 60)} h ago`;
  };

  async function backup() {
    const day = new Date().toISOString().slice(0, 10);
    const path = await saveDialog({ title: "Back up Cablebag's data", defaultPath: `cablebag-backup-${day}.db`, filters: [{ name: "Cablebag backup", extensions: ["db"] }] });
    if (!path) return;
    try {
      await api.backupData(path);
      app.notify(`Backed up to ${path}`);
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  async function restore() {
    const path = await openDialog({ title: "Choose a Cablebag backup", filters: [{ name: "Cablebag backup", extensions: ["db"] }] });
    if (!path || Array.isArray(path)) return;
    try {
      const n = await api.restoreData(path);
      const now = await ask.confirm(
        "Restore the backup",
        `The backup holds ${n} sessions. It takes the place of the current data when Cablebag starts again; the current data is kept in the data folder. Start again now?`,
        { okLabel: "Start again now" },
      );
      if (now) await api.restartApp();
      else app.notify("The backup is used the next time Cablebag starts.");
    } catch (e) {
      app.notify(String(e), "error");
    }
  }

  /** A page of the official project on GitHub. */
  const site = (path: string) => openUrl(`https://github.com/maferick/cablebag${path}`).catch((e) => app.notify(String(e), "error"));

  $effect(() => {
    listFilter = "";
    if (page === "hosts") api.knownHosts().then((h) => (hosts = h));
    if (page === "passwords") {
      api.secrets().then((s) => (secrets = s));
      vault.refresh();
    }
    if (page === "about") {
      api.dataFolder().then((d) => (dataFolder = d));
      api.systemInfo().then((i) => (sys = i));
    }
    if (page === "proxmox") proxmox.load();
    if (page === "vault") {
      vault.refresh();
      vault.loadHello();
    }
  });

  const set = (k: string) => (e: Event) => {
    const t = e.target as HTMLInputElement;
    app.setSetting(k, t.type === "checkbox" ? t.checked : t.type === "number" ? Number(t.value) : t.value);
  };

  function date(t: number) {
    return new Date(t * 1000).toLocaleDateString();
  }

  // ---- Passwords on this PC and Trusted servers: names, not codes ----

  const PROTOCOLS: Record<string, string> = {
    rdp: "Remote Desktop", ssh: "SSH", sftp: "SFTP", ftp: "FTP", telnet: "Telnet", vnc: "VNC", winrm: "PowerShell remoting", wireguard: "WireGuard",
  };
  const TOKENS: Record<string, string> = {
    refresh: "Sign-in kept",
    user_key: "Vault key kept",
    remember: "Two-step sign-in remembered",
  };

  /** What a kept secret is, in words; `show`: a password a person may want to read. */
  function secretInfo(s: SecretInfo): { name: string; detail: string; show: boolean } {
    if (s.kind === "passphrase") return { name: "Key passphrase", detail: s.label, show: true };
    if (s.kind === "token") {
      const src = vault.sources.find((x) => x.id === s.host) ?? (s.host ? undefined : vault.sources.find((x) => x.kind === "vaultwarden"));
      const name = src?.name ?? (s.protocol ? `${s.protocol.charAt(0).toUpperCase()}${s.protocol.slice(1)}` : "a credential source");
      return { name: `${TOKENS[s.label] ?? "Kept"} for ${name}`, detail: "Opens it after Cablebag starts (Settings, Credential sources, At start)", show: false };
    }
    const where = `${s.username ? `${s.username}@` : ""}${s.host}${s.port ? `:${s.port}` : ""}`;
    return { name: where || s.label || "A password", detail: PROTOCOLS[s.protocol] ?? (s.protocol ? s.protocol.toUpperCase() : "Password"), show: true };
  }

  function keyType(algo: string): string {
    if (algo.includes("ed25519")) return "Ed25519 key";
    if (algo.includes("ecdsa")) return "ECDSA key";
    if (algo.includes("rsa")) return "RSA key";
    if (algo.includes("dss")) return "DSA key";
    return algo;
  }

  const fits = (text: string) => !listFilter.trim() || listFilter.trim().toLowerCase().split(/\s+/).every((w) => text.toLowerCase().includes(w));
  const shownSecrets = $derived(secrets.filter((s) => fits(`${secretInfo(s).name} ${secretInfo(s).detail}`)));
  const shownHosts = $derived(hosts.filter((h) => fits(`${h.host}:${h.port} ${keyType(h.algo)}`)));
</script>

<div class="settings">
  <div class="layout">
    <nav>
      <div class="find">
        <Search size={13} />
        <input bind:value={find} placeholder="Find a setting" aria-label="Find a setting" />
      </div>
      {#each groups as g}
        {@const inGroup = shownPages.filter((p) => p.group === g)}
        {#if inGroup.length}
          <span class="group">{g}</span>
          {#each inGroup as p}
            <button class:on={page === p.id} onclick={() => (app.settingsPage = p.id)}>{p.label}</button>
          {/each}
        {/if}
      {/each}
      {#if !shownPages.length}
        <p class="none">No setting matches "{find}".</p>
      {/if}
    </nav>
    <div class="page">
      <header class="phead">
        <h2>{pages.find((p) => p.id === page)?.label}</h2>
        {#if INTRO[page]}<p>{INTRO[page]}</p>{/if}
      </header>

      {#if page === "general"}
        <SettingsSection title="Look" find="theme dark light size zoom scale">
          <SettingsRow label="Theme">
            <select value={app.settings["ui.theme"]} onchange={set("ui.theme")}>
              <option value="system">Same as Windows</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </SettingsRow>
          <SettingsRow label="Interface size" help="Also Ctrl+Shift+plus and minus." info="Everything in the window; the terminal text has its own size on top (Terminal page).">
            <select value={String(app.settings["ui.scale"] ?? 100)} onchange={(e) => app.setSetting("ui.scale", Number((e.target as HTMLSelectElement).value))}>
              {#each SCALES as v}<option value={String(v)}>{v}%{v === 100 ? " (normal)" : ""}</option>{/each}
            </select>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Starting and closing" find="shell close tabs last time reopen">
          <SettingsRow label="Default local shell" help="What the + button opens.">
            <select value={app.settings["shell.default"]} onchange={set("shell.default")}>
              <option value="">First available</option>
              {#each app.shells as s}<option value={s.id}>{s.name}</option>{/each}
            </select>
          </SettingsRow>
          <SettingsRow label="Ask before closing" help="When sessions are still connected.">
            <input type="checkbox" class="switch" checked={app.settings["ui.confirmClose"]} onchange={set("ui.confirmClose")} />
          </SettingsRow>
          <SettingsRow label="Tabs from last time" info="The tabs that were open when Cablebag closed. Home offers them, or they open by themselves.">
            <select value={app.settings["tabs.reopen"] ?? "ask"} onchange={set("tabs.reopen")}>
              <option value="ask">Offer them on Home</option>
              <option value="always">Open them at the start</option>
              <option value="never">Don't offer them</option>
            </select>
          </SettingsRow>
        </SettingsSection>
      {:else if page === "terminal"}
        <SettingsSection title="Text" desc="Changes apply to open terminals straight away." find="font size scheme colour">
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

        <SettingsSection title="Cursor and history" desc="These apply to terminals opened afterwards." find="cursor blink screen reader scrollback">
          <SettingsRow label="Cursor">
            <select value={app.settings["term.cursor"]} onchange={set("term.cursor")}>
              <option value="block">Block</option>
              <option value="underline">Underline</option>
              <option value="bar">Line</option>
            </select>
          </SettingsRow>
          <SettingsRow label="Blinking cursor" help="Never blinks when Windows is set to show fewer animations.">
            <input type="checkbox" class="switch" checked={app.settings["term.cursorBlink"]} onchange={set("term.cursorBlink")} />
          </SettingsRow>
          <SettingsRow label="Screen reader support" info="Lets screen readers such as Narrator and NVDA read the terminal's text and new output.">
            <input type="checkbox" class="switch" checked={app.settings["term.screenReader"]} onchange={set("term.screenReader")} />
          </SettingsRow>
          <SettingsRow label="Lines kept in scrollback">
            <input type="number" value={app.settings["term.scrollback"]} onchange={set("term.scrollback")} />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Copy and paste" find="copy paste right click">
          <SettingsRow label="Copy as soon as text is selected">
            <input type="checkbox" class="switch" checked={app.settings["term.copyOnSelect"]} onchange={set("term.copyOnSelect")} />
          </SettingsRow>
          <SettingsRow label="Paste with the right mouse button" help="Shift+right-click opens the menu with Copy, Paste and the macros. When this is off, a plain right-click opens it.">
            <input type="checkbox" class="switch" checked={app.settings["term.rightClickPaste"]} onchange={set("term.rightClickPaste")} />
          </SettingsRow>
          <SettingsRow label="Ask before pasting several lines" help="So a pasted block does not run line by line by accident.">
            <input type="checkbox" class="switch" checked={app.settings["term.pasteWarning"]} onchange={set("term.pasteWarning")} />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Words in colour" find="highlighting syntax error warning regex words colour">
          <SettingsRow label="Colour these words" info="Words such as error and warning stand out in output. Programs that colour their own output, and full-screen programs such as vim, are left alone. A session can choose its own set in the session editor.">
            <select value={app.settings["term.syntax"]} onchange={set("term.syntax")}>
              {#each BUILTIN_SYNTAX as s}<option value={s.id}>{s.name}</option>{/each}
              <option value="custom">My own words</option>
            </select>
          </SettingsRow>
          <SettingsMore label="My own words" changed={app.settings["term.syntax"] === "custom"}>
            <SettingsRow help="Words separated by commas, per colour. Changes apply to terminals opened afterwards."><span></span></SettingsRow>
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
          </SettingsMore>
        </SettingsSection>

        <SettingsSection title="Write terminals to files" find="log logging file folder timestamps">
          <SettingsRow label="Write every terminal to a file" help="Starts with terminals opened after you switch it on.">
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
          <SettingsRow label="File names">
            <select
              value={ownLogName || typingLogName ? "own" : logName}
              onchange={(e) => {
                const v = (e.target as HTMLSelectElement).value;
                typingLogName = v === "own";
                if (v !== "own") app.setSetting("log.name", v);
              }}
            >
              {#each LOG_NAMES as n}<option value={n.value}>{n.name}</option>{/each}
              <option value="own">A pattern of my own</option>
            </select>
          </SettingsRow>
          {#if ownLogName || typingLogName}
            <SettingsRow label="Pattern" info="&S kind of session, &H host, &U user, &P port, &Y &M &D the date, &T the time.">
              <input value={logName} onchange={set("log.name")} />
            </SettingsRow>
          {/if}
        </SettingsSection>
      {:else if page === "connections"}
        <SettingsSection title="SSH" find="ssh remote files sftp reconnect connect again drop">
          <SettingsRow label="Remote files when an SSH session connects" info="A session can choose differently in its editor. Ctrl+Shift+E shows or hides them for the tab you are in.">
            <select value={app.settings["ssh.autoFiles"] === false ? "hide" : "show"} onchange={(e) => app.setSetting("ssh.autoFiles", (e.target as HTMLSelectElement).value === "show")}>
              <option value="show">Show</option>
              <option value="hide">Don't show</option>
            </select>
          </SettingsRow>
          <SettingsRow label="Connect again by itself" help="After sleep, a network change or a VPN that drops." info="In the same terminal, keeping what it showed, for up to ten minutes. Esc stops it.">
            <input type="checkbox" class="switch" checked={app.settings["ssh.autoReconnect"] !== false} onchange={set("ssh.autoReconnect")} />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Keys from an agent" desc="Agent keys are tried before the key file and password of a session." find="keys agent pageant openssh">
          <SettingsRow label="PuTTY Pageant">
            <input type="checkbox" class="switch" checked={app.settings["ssh.agent.pageant"]} onchange={set("ssh.agent.pageant")} />
          </SettingsRow>
          <SettingsRow label="Windows OpenSSH agent">
            <input type="checkbox" class="switch" checked={app.settings["ssh.agent.openssh"]} onchange={set("ssh.agent.openssh")} />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Dots in the session tree" desc="A dot on each session shows whether its host answers." info="Only sessions in open folders are checked, and not those behind a jump host or gateway." find="reach dots ping port answer fail2ban">
          <SettingsRow label="Show the dots">
            <input type="checkbox" class="switch" checked={app.settings["reach.enabled"]} onchange={set("reach.enabled")} />
          </SettingsRow>
          <SettingsRow label="How to check" info="Trying the port leaves a line in the server's log each time, which tools like fail2ban may count against this PC; ping does not.">
            <select value={app.settings["reach.mode"]} onchange={set("reach.mode")} disabled={!app.settings["reach.enabled"]}>
              <option value="ping">Ping first, the port only without an answer</option>
              <option value="port">Always the port (also shows a stopped service)</option>
            </select>
          </SettingsRow>
          <SettingsRow label="How often">
            <select value={String(app.settings["reach.interval"] ?? 120)} onchange={(e) => app.setSetting("reach.interval", Number((e.target as HTMLSelectElement).value))} disabled={!app.settings["reach.enabled"]}>
              {#each REACH_EVERY as r}<option value={String(r.value)}>{r.name}</option>{/each}
              {#if !REACH_EVERY.some((r) => r.value === Number(app.settings["reach.interval"]))}
                <option value={String(app.settings["reach.interval"])}>Every {app.settings["reach.interval"]} seconds</option>
              {/if}
            </select>
          </SettingsRow>
        </SettingsSection>
      {:else if page === "vault"}
        <CredentialSources />
      {:else if page === "assistant"}
        <AssistantSettings />
      {:else if page === "memory"}
        <SettingsSection title="General, for every conversation" desc="Facts that go along with every question." info="It travels with Sync, encrypted; secrets are hidden before anything is stored." find="general facts add">
          <div class="inner"><MemoryList scope="general" /></div>
        </SettingsSection>
        <SettingsSection title={`Everything it remembers (${memory.all.length})`} desc="Grouped by connection and folder; a session's own list is also in its editor." find="everything all remembers">
          <div class="inner">
            {#if memory.all.length > 8}<input bind:value={memoryFilter} placeholder="Show only what holds these words" />{/if}
            <MemoryList all filter={memoryFilter} />
          </div>
        </SettingsSection>
      {:else if page === "ailog"}
        <AssistantLog />
      {:else if page === "sync"}
        <SettingsSection title="Repository" find="git gitea github gitlab repository encrypt passphrase token">
          <div class="inner"><SyncSettings /></div>
        </SettingsSection>
      {:else if page === "browsers"}
        <BrowserProfilesSettings />
      {:else if page === "proxmox"}
        <SettingsSection title={`Servers (${proxmox.sources.length})`} find="pve token server guests">
          <div class="inner"><ProxmoxSettings /></div>
        </SettingsSection>
      {:else if page === "passwords"}
        <SettingsSection title={`Kept on this PC (${secrets.length})`} info="Encrypted; only your Windows account on this PC can read them. A password lands here when you tick Remember on a login question.">
          {#if secrets.length > 10}<div class="filter"><input bind:value={listFilter} placeholder="Show only what holds these words" /></div>{/if}
          {#if !secrets.length}
            <SettingsRow><span class="muted">Nothing kept yet. A password lands here when you tick Remember on a login question.</span></SettingsRow>
          {/if}
          {#each shownSecrets as s (s.id)}
            {@const info = secretInfo(s)}
            <SettingsItem name={info.name} detail={info.detail}>
              {#snippet mark()}<KeyRound size={15} />{/snippet}
              {#snippet actions()}
                {#if info.show}
                  <span class="mono small">{revealed[s.id] ?? "••••••••"}</span>
                  <button class="icon-btn" title="Show" aria-label="Show" onclick={async () => (revealed[s.id] = await api.revealSecret(s.id))}><Eye size={14} /></button>
                {/if}
                <button class="icon-btn" title="Forget" aria-label="Forget" onclick={async () => { await api.deleteSecret(s.id); secrets = await api.secrets(); }}><Trash2 size={14} /></button>
              {/snippet}
            </SettingsItem>
          {/each}
        </SettingsSection>
      {:else if page === "hosts"}
        <SettingsSection title={`Servers you trust (${hosts.length})`}>
          {#if hosts.length > 10}<div class="filter"><input bind:value={listFilter} placeholder="Show only a host" /></div>{/if}
          {#if !hosts.length}
            <SettingsRow><span class="muted">None yet. When you connect to an SSH server for the first time, Cablebag asks you to confirm its identity; it is listed here after that.</span></SettingsRow>
          {/if}
          {#each shownHosts as h (h.host + h.port + h.algo)}
            <SettingsItem name={`${h.host}:${h.port}`} mono detail={`${keyType(h.algo)} · trusted since ${date(h.added)} · ${h.fingerprint.slice(0, 24)}...`}>
              {#snippet mark()}<ShieldCheck size={15} />{/snippet}
              {#snippet actions()}
                <button class="icon-btn" title="Copy the fingerprint" aria-label="Copy the fingerprint" onclick={() => writeText(h.fingerprint).then(() => app.notify("Fingerprint copied"))}><Copy size={14} /></button>
                <button class="icon-btn" title="Forget" aria-label="Forget" onclick={async () => { await api.deleteKnownHost(h.host, h.port, h.algo); hosts = await api.knownHosts(); }}><Trash2 size={14} /></button>
              {/snippet}
            </SettingsItem>
          {/each}
        </SettingsSection>
      {:else if page === "keyboard"}
        {#each SHORTCUTS as g}
          <SettingsSection title={g.group}>
            {#each g.items as k}
              <SettingsRow label={k.what}><span class="keys">{k.keys}</span></SettingsRow>
            {/each}
          </SettingsSection>
        {/each}
        <SettingsSection title="Macros" desc="A macro can have its own shortcut; set it in the macro (Macros in the sidebar).">
          {#each macros.list.filter((m) => m.hotkey) as m}
            <SettingsRow label={m.name}><span class="keys">{m.hotkey}</span></SettingsRow>
          {:else}
            <SettingsRow><span class="muted">No macro has a shortcut yet.</span></SettingsRow>
          {/each}
        </SettingsSection>
      {:else if page === "about"}
        <div class="hero">
          <Logo height={44} />
          <div class="hero-txt">
            <span class="ver">Version {update.current || "..."}</span>
            <span class="muted">Remote access for system administrators: SSH, RDP, VNC, files, Proxmox and an assistant, as tabs in one window.</span>
          </div>
          <div class="btns">
            <button class="btn" onclick={() => site("/blob/main/CHANGELOG.md")}><ExternalLink size={14} /> What's new</button>
            <button class="btn" onclick={() => site("")}><ExternalLink size={14} /> Project page</button>
          </div>
        </div>

        <SettingsSection title="Updates" info="Only from the official page, github.com/maferick/cablebag, and only with the Cablebag signature." find="update version">
          <SettingsRow label="Check for a newer version" help={update.found ? `Version ${update.found.version} is available.` : update.error ? update.error : update.lastChecked ? `Checked ${ago(update.lastChecked)}: this is the newest version.` : ""}>
            <div class="btns">
              {#if update.found}<button class="btn primary" disabled={update.installing} onclick={() => update.install()}>Install and restart</button>{/if}
              <button class="btn" disabled={update.checking} onclick={() => update.check()}>{update.checking ? "Checking..." : "Check now"}</button>
            </div>
          </SettingsRow>
          <SettingsRow label="Check by itself" help="At the start and every six hours.">
            <input type="checkbox" class="switch" checked={app.settings["update.auto"] !== false} onchange={set("update.auto")} />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Licence" desc="Free to use at home and at work; not open source." find="licence license">
          <SettingsMore label="What you may and may not do">
            <div class="inner lic">
              <p><strong>You may</strong> download it from the official page, install it and use it, at home or at work, on as many computers as you like.</p>
              <p><strong>Not without permission:</strong> pass it on, change it or build on it, sell it or offer it as a service. Others can download it from the official page themselves.</p>
            </div>
          </SettingsMore>
          <SettingsRow label="The full text" help="Also the licences of the open-source parts inside Cablebag.">
            <div class="btns">
              <button class="btn" onclick={() => site("/blob/main/LICENSE")}><ExternalLink size={14} /> Licence</button>
              <button class="btn" onclick={() => site("/blob/main/THIRD-PARTY-NOTICES.md")}><ExternalLink size={14} /> Open-source parts</button>
            </div>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Your data" desc="Everything stays on this PC unless you set up Sync or an AI host." find="data folder backup restore">
          <SettingsRow label="Data folder" help={dataFolder}>
            <button class="btn" disabled={!dataFolder} onclick={() => api.fileOpen(dataFolder).catch((e) => app.notify(String(e), "error"))}><FolderOpen size={14} /> Open</button>
          </SettingsRow>
          <SettingsRow label="Back up" help="A copy of everything in one file." info="Saved passwords in it only open on this PC, with your Windows account.">
            <button class="btn" onclick={backup}>Back up...</button>
          </SettingsRow>
          <SettingsRow label="Restore a backup" info="It takes the place of the current data when Cablebag starts again; the current data is kept.">
            <button class="btn" onclick={restore}>Restore...</button>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Problems" find="problem report issues dotnet">
          <SettingsRow label="Report a problem" help="You read it over and submit it on GitHub, in your web browser." info="A few questions, then GitHub's form opens with your answers and the details of this PC filled in.">
            <button class="btn primary" onclick={() => app.openReport()}>Report a problem...</button>
          </SettingsRow>
          <SettingsRow label="This PC" help={details.split("\n").join(", ")}>
            <div class="btns">
              <button class="btn" disabled={!details} onclick={() => writeText(details).then(() => app.notify("Details copied"))}>Copy details</button>
              <button class="btn" onclick={() => site("/issues")}><ExternalLink size={14} /> Issues</button>
            </div>
          </SettingsRow>
          {#if sys && !sys.dotnet}
            <SettingsRow label="Remote Desktop needs .NET 8" help="The .NET 8 Desktop Runtime is not installed, so Remote Desktop tabs cannot open. Install it from Microsoft (x64).">
              <button class="btn" onclick={() => openUrl("https://dotnet.microsoft.com/download/dotnet/8.0").catch(() => {})}><ExternalLink size={14} /> Download</button>
            </SettingsRow>
          {/if}
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
  .group {
    margin: 12px 10px 3px;
    font-size: 11px;
    font-weight: 600;
    color: var(--faint);
  }
  .find + .group {
    margin-top: 0;
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
  .hero {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 14px 20px;
    padding: 18px 16px;
    margin-bottom: 14px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--panel);
  }
  .hero-txt {
    flex: 1;
    min-width: 220px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    line-height: 1.45;
  }
  .ver {
    font-size: 13px;
    font-weight: 600;
  }
  .keys {
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .lic p {
    margin: 0 0 6px;
    font-size: 12px;
    line-height: 1.5;
  }
  .lic p:last-child {
    margin: 0;
  }
  .btns {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .with-btn {
    display: flex;
    gap: 6px;
  }
  .with-btn input {
    flex: 1;
  }
  .inner {
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .filter {
    padding: 8px 14px;
    border-bottom: 1px solid var(--border);
  }
  .filter input {
    width: 100%;
  }
  .small {
    font-size: 12px;
  }
</style>

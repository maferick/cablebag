<script lang="ts">
  import { TAB_COLORS } from "../tabColors";
  import { untrack } from "svelte";
  import Modal from "./Modal.svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { app } from "../state.svelte";
  import { KINDS, kindInfo } from "../kinds";
  import { BUILTIN_SYNTAX } from "../highlight";
  import VaultPicker from "./VaultPicker.svelte";
  import { vault } from "../vault.svelte";
  import { api, type Session } from "../api";
  import { Plus, Trash2, ArrowUp, ArrowDown, FolderOpen, Info, ServerCog, ChevronRight } from "@lucide/svelte";
  import ProxmoxSettings from "./ProxmoxSettings.svelte";
  import MemoryList from "./MemoryList.svelte";

  let { session }: { session: Session } = $props();
  const initial = untrack(() => session);
  let s = $state<Session>(JSON.parse(JSON.stringify(initial)));
  type Part = "basic" | "advanced" | "rdp" | "network" | "bookmark" | "memory";
  /** The folded parts under the main form, open or closed. */
  let open = $state<Record<string, boolean>>({});
  let error = $state("");

  if (!s.data) s.data = {};
  s.data.vault_item ??= "";
  const k = $derived(kindInfo(s.kind));
  const isNew = !initial.id;
  let proxmoxMode = $state(false);

  // Serial: the COM ports on this PC.
  let ports = $state<{ name: string; description: string }[]>([]);
  async function loadPorts() {
    ports = await api.serialPorts().catch(() => []);
  }
  $effect(() => {
    if (s.kind === "serial") untrack(() => loadPorts());
  });

  async function browsePath(folder: boolean) {
    const p = await openDialog({ title: folder ? "Choose a folder" : "Choose a file", directory: folder, multiple: false });
    if (typeof p === "string") {
      s.host = p;
      if (!s.name) s.name = p.split(/[\\/]/).filter(Boolean).pop() ?? p;
    }
  }

  const folders = $derived(
    app.folders
      .map((f) => ({ id: f.id, path: app.folderPath(f.id) }))
      .sort((a, b) => a.path.localeCompare(b.path)),
  );
  const distros = $derived(app.shells.filter((x) => x.id.startsWith("wsl:")).map((x) => x.id.slice(4)));
  const localShells = $derived(app.shells.filter((x) => !x.id.startsWith("wsl:")));

  function pickKind(id: string) {
    const old = kindInfo(s.kind);
    s.kind = id;
    if (!s.port || s.port === old.defaultPort) s.port = kindInfo(id).defaultPort;
  }

  async function browseKey() {
    const f = await openDialog({ title: "Choose a private key file", filters: [{ name: "Keys", extensions: ["*"] }] });
    if (f && !Array.isArray(f)) s.data.key_path = f;
  }

  function addJump() {
    s.data.jump_hosts = [...(s.data.jump_hosts ?? []), { host: "", port: 22, user: "", key_path: "" }];
  }

  function moveJump(i: number, d: number) {
    const j = [...s.data.jump_hosts];
    const [x] = j.splice(i, 1);
    j.splice(i + d, 0, x);
    s.data.jump_hosts = j;
  }

  async function save() {
    error = "";
    if (k.needsHost && !s.host.trim()) {
      error = "Fill in the remote host.";
      return;
    }
    if (!s.name.trim()) s.name = s.host.trim() || (s.kind === "wsl" ? `WSL ${s.data.distro ?? ""}`.trim() : k.label);
    s.host = s.host.trim();
    s.port = Number(s.port) || 0;
    try {
      await app.saveSession($state.snapshot(s) as Session);
      app.editing = null;
      app.notify(`Saved "${s.name}"`);
    } catch (e) {
      error = String(e);
    }
  }

  /** The session types, in groups, for the list on the left. */
  const GROUPS: { name: string; ids: string[] }[] = [
    { name: "Terminal", ids: ["ssh", "telnet", "serial", "shell", "wsl", "mosh", "rsh"] },
    { name: "Remote desktop", ids: ["rdp", "vnc", "xdmcp"] },
    { name: "Files", ids: ["sftp", "ftp", "s3"] },
    { name: "Other", ids: ["browser", "file"] },
  ];
  const groups = $derived(
    GROUPS.map((g) => ({ name: g.name, kinds: g.ids.map((id) => KINDS.find((x) => x.id === id)).filter((x) => x && (!x.hidden || x.id === s.kind)) as typeof KINDS })).filter(
      (g) => g.kinds.length,
    ),
  );

  /** The folded parts that apply to this type of session. */
  const more = $derived(
    [
      s.kind === "ssh" ? (["advanced", "SSH options and jump hosts"] as const) : null,
      s.kind === "rdp" ? (["rdp", "Screen, sharing and Remote Desktop options"] as const) : null,
      s.kind === "rdp" || s.kind === "vnc" ? (["network", "Gateways"] as const) : null,
      ["bookmark", "Notes, tab colour and word colours"] as const,
      s.id ? (["memory", "What the assistant remembers about it"] as const) : null,
    ].filter(Boolean) as (readonly [Part, string])[],
  );

  async function saveAndOpen() {
    await save();
    if (!error) {
      const saved = app.sessions.find((x) => x.name === s.name && x.folder_id === s.folder_id);
      app.openSession(saved ?? ($state.snapshot(s) as Session));
    }
  }
</script>

<Modal
  title={proxmoxMode ? "Add a Proxmox server" : isNew ? "New session" : `Edit ${initial.name}`}
  width={900}
  onclose={() => (app.editing = null)}
  footer={proxmoxMode ? undefined : sessionFooter}
>
  <div class="editor">
  <nav class="types" aria-label="Session type">
    {#each groups as g}
      <div class="ghead">{g.name}</div>
      {#each g.kinds as kk}
        <button
          class="type"
          class:on={!proxmoxMode && s.kind === kk.id}
          role="radio"
          aria-checked={!proxmoxMode && s.kind === kk.id}
          onclick={() => {
            proxmoxMode = false;
            pickKind(kk.id);
          }}
        >
          <kk.icon size={15} />
          <span class="tl">{kk.label}<em>{kk.description}</em></span>
        </button>
      {/each}
    {/each}
    {#if isNew}
      <div class="ghead">Source</div>
      <button class="type" class:on={proxmoxMode} role="radio" aria-checked={proxmoxMode} onclick={() => (proxmoxMode = true)}>
        <ServerCog size={15} />
        <span class="tl">Proxmox server<em>Its machines as a live folder</em></span>
      </button>
    {/if}
  </nav>

  <div class="pane">
  {#if proxmoxMode}
    <ProxmoxSettings only="new" onDone={() => (app.editing = null)} />
  {:else}

  {#if !k.ready}
    <div class="notice">
      <Info size={15} />
      <span>Cablebag cannot open {k.label} sessions yet; you can save one now and it opens once Cablebag can.</span>
    </div>
  {/if}

  <div class="form">
    {@render part("basic")}
    {#each more as [id, title] (id)}
      <section class="fold" class:open={open[id]}>
        <button class="fh" aria-expanded={!!open[id]} onclick={() => (open[id] = !open[id])}>
          <span class="chev"><ChevronRight size={14} /></span>{title}
        </button>
        {#if open[id]}<div class="fbody">{@render part(id)}</div>{/if}
      </section>
    {/each}
    {#if error}<p class="error">{error}</p>{/if}
  </div>
  {/if}
  </div>
  </div>
</Modal>

{#snippet part(section: Part)}
    {#if section === "memory"}
      <MemoryList scope="session" target={s.id} />
    {:else if section === "basic"}
      <div class="row2n">
        <label class="field grow"><span>Session name</span><input bind:value={s.name} placeholder={s.host || "Name shown in the sessions list"} /></label>
        <label class="field grow"><span>Folder</span>
          <select bind:value={s.folder_id}>
            <option value={null}>No folder</option>
            {#each folders as f}<option value={f.id}>{f.path}</option>{/each}
          </select>
        </label>
      </div>
      {#if k.needsHost}
        <div class="row3">
          <label class="field grow"><span>Remote host</span><input bind:value={s.host} placeholder="Name or IP address" /></label>
          <label class="field"><span>User name</span><input bind:value={s.username} placeholder="Ask when connecting" /></label>
          <label class="field port"><span>Port</span><input type="number" bind:value={s.port} /></label>
        </div>
      {/if}
      {#if s.kind === "vnc"}
        <div class="grid2">
          <label class="check"><input type="checkbox" bind:checked={s.data.view_only} /> View only (no keyboard or mouse)</label>
          <label class="check"><input type="checkbox" checked={s.data.shared_clipboard !== false} onchange={(e) => (s.data.shared_clipboard = (e.target as HTMLInputElement).checked)} /> Copy the remote clipboard to this PC</label>
        </div>
      {/if}
      {#if s.kind === "telnet"}
        <p class="muted small">With a user name, Cablebag answers the login and password prompts by itself (password from the vault or saved); without one you log in by hand.</p>
      {/if}
      {#if s.kind === "ftp"}
        <div class="grid2">
          <label class="field"><span>Encryption</span>
            <select value={s.data.tls ?? ""} onchange={(e) => (s.data.tls = (e.target as HTMLSelectElement).value)}>
              <option value="">None (plain FTP)</option>
              <option value="explicit">TLS after connecting (explicit FTPS, port 21)</option>
              <option value="implicit">TLS from the start (implicit FTPS, port 990)</option>
            </select>
          </label>
          <label class="check"><input type="checkbox" bind:checked={s.data.accept_any_cert} disabled={!s.data.tls} /> Accept a certificate no authority vouches for</label>
        </div>
        <p class="muted small">Without a user name Cablebag logs in as anonymous.</p>
      {/if}
      {#if s.kind === "serial"}
        <div class="row3">
          <label class="field grow"><span>COM port</span>
            <select value={s.data.serial_port ?? s.host ?? ""} onchange={(e) => (s.data.serial_port = (e.target as HTMLSelectElement).value)}>
              {#if !ports.some((p) => p.name === (s.data.serial_port ?? s.host))}<option value={s.data.serial_port ?? s.host ?? ""}>{s.data.serial_port || s.host || "Choose a port"}</option>{/if}
              {#each ports as p}<option value={p.name}>{p.name}{p.description ? ` (${p.description})` : ""}</option>{/each}
            </select>
          </label>
          <label class="field"><span>Speed (baud)</span>
            <select value={String(s.data.baud ?? 9600)} onchange={(e) => (s.data.baud = Number((e.target as HTMLSelectElement).value))}>
              {#each [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600] as b}<option value={String(b)}>{b}</option>{/each}
            </select>
          </label>
          <button class="btn refresh" title="Look for ports again" onclick={loadPorts}>Refresh</button>
        </div>
        <div class="row3">
          <label class="field"><span>Data bits</span>
            <select value={String(s.data.data_bits ?? 8)} onchange={(e) => (s.data.data_bits = Number((e.target as HTMLSelectElement).value))}>
              {#each [8, 7, 6, 5] as b}<option value={String(b)}>{b}</option>{/each}
            </select>
          </label>
          <label class="field"><span>Parity</span>
            <select value={s.data.parity ?? "none"} onchange={(e) => (s.data.parity = (e.target as HTMLSelectElement).value)}>
              <option value="none">None</option><option value="even">Even</option><option value="odd">Odd</option>
            </select>
          </label>
          <label class="field"><span>Stop bits</span>
            <select value={String(s.data.stop_bits ?? 1)} onchange={(e) => (s.data.stop_bits = Number((e.target as HTMLSelectElement).value))}>
              <option value="1">1</option><option value="2">2</option>
            </select>
          </label>
          <label class="field"><span>Flow control</span>
            <select value={s.data.flow ?? "none"} onchange={(e) => (s.data.flow = (e.target as HTMLSelectElement).value)}>
              <option value="none">None</option><option value="hardware">Hardware (RTS/CTS)</option><option value="software">Software (XON/XOFF)</option>
            </select>
          </label>
        </div>
        <p class="muted small">Most switch and router consoles use 9600 baud, 8 data bits, no parity, 1 stop bit, no flow control.</p>
      {/if}
      {#if s.kind === "browser"}
        <label class="field"><span>Web address</span><input bind:value={s.host} placeholder="https://192.168.1.1 or proxmox.example.com:8006" /></label>
      {/if}
      {#if s.kind === "file"}
        <label class="field"><span>File, folder or link</span>
          <div class="with-btn">
            <input bind:value={s.host} placeholder="C:\\Users\\you\\notes.txt, \\\\server\\share or https://..." />
            <button class="btn" onclick={() => browsePath(false)}><FolderOpen size={14} /> File</button>
            <button class="btn" onclick={() => browsePath(true)}><FolderOpen size={14} /> Folder</button>
          </div>
        </label>
        <p class="muted small">Opens with the program Windows uses for it; a folder opens in Explorer, a link in your web browser.</p>
      {/if}
      {#if s.kind === "ssh" || s.kind === "rdp" || s.kind === "sftp" || s.kind === "vnc" || s.kind === "telnet" || s.kind === "ftp"}
        <div class="field"><span>Login from the vault</span>
          <VaultPicker bind:itemId={s.data.vault_item} hint={s.host} />
        </div>
      {/if}
      {#if s.kind === "ssh" || s.kind === "sftp"}
        <label class="field"><span>Private key file</span>
          <div class="with-btn">
            <input bind:value={s.data.key_path} placeholder="None: log in with a password or an agent" />
            <button class="btn" onclick={browseKey}><FolderOpen size={14} /> Browse</button>
          </div>
        </label>
      {/if}
      {#if s.kind === "shell"}
        <label class="field"><span>Shell</span>
          <select bind:value={s.data.shell}>
            <option value="">Default shell</option>
            {#each localShells as sh}<option value={sh.id}>{sh.name}</option>{/each}
          </select>
        </label>
        <label class="field"><span>Start folder</span><input bind:value={s.data.startup_dir} placeholder="Your user folder" /></label>
        <label class="field"><span>Run these commands at start</span><textarea rows="3" bind:value={s.data.startup_commands}></textarea></label>
      {/if}
      {#if s.kind === "wsl"}
        <label class="field"><span>Distribution</span>
          <select bind:value={s.data.distro}>
            <option value="">Default distribution</option>
            {#each distros as d}<option value={d}>{d}</option>{/each}
          </select>
        </label>
        <label class="field"><span>User name</span><input bind:value={s.username} placeholder="Default user of the distribution" /></label>
        <label class="field"><span>Start folder</span><input bind:value={s.data.startup_dir} placeholder="~" /></label>
      {/if}
      {#if s.kind === "ssh"}
        <label class="field"><span>Run this command after login</span><input bind:value={s.data.command} placeholder="Nothing" /></label>
        <label class="field"><span>Remote files when this session connects</span>
          <select value={s.data.files ?? ""} onchange={(e) => (s.data.files = (e.target as HTMLSelectElement).value || undefined)}>
            <option value="">Same as Settings ({app.settings["ssh.autoFiles"] === false ? "don't show" : "show"})</option>
            <option value="show">Show</option>
            <option value="hide">Don't show</option>
          </select>
        </label>
      {/if}
    {:else if section === "advanced"}
      <div class="grid2">
        <label class="check"><input type="checkbox" bind:checked={s.data.compression} /> Compress traffic (helps on slow links)</label>
        <label class="check"><input type="checkbox" checked={s.data.keepalive !== false} onchange={(e) => (s.data.keepalive = (e.target as HTMLInputElement).checked)} /> Keep the connection alive when idle</label>
        <label class="check"><input type="checkbox" checked={s.data.stay_after_command !== false} onchange={(e) => (s.data.stay_after_command = (e.target as HTMLInputElement).checked)} /> Stay logged in after the command ends</label>
        <label class="check" title="Cablebag has no X server yet, so programs with windows cannot be shown"><input type="checkbox" disabled checked={!!s.data.x11} /> X11 forwarding <em class="soon">not available yet</em></label>
      </div>
      <label class="field"><span>Terminal type reported to the server</span>
        <select bind:value={s.data.term_type}>
          <option value="">xterm-256color</option>
          <option value="xterm">xterm</option>
          <option value="vt100">vt100</option>
          <option value="vt220">vt220</option>
          <option value="linux">linux</option>
        </select>
      </label>
      <div class="jumps">
        <div class="jhead">
          <strong>Jump hosts</strong>
          <span class="muted">Reach the server through one or more SSH gateways, in this order.</span>
          <button class="btn" onclick={addJump}><Plus size={14} /> Add jump host</button>
        </div>
        {#each s.data.jump_hosts ?? [] as j, i}
          <div class="jrow">
            <input bind:value={j.host} placeholder="Gateway host" />
            <input bind:value={j.user} placeholder="User" class="small" />
            <input type="number" bind:value={j.port} class="tiny" />
            <input bind:value={j.key_path} placeholder="Key file (optional)" />
            <button class="icon-btn" title="Move up" disabled={i === 0} onclick={() => moveJump(i, -1)}><ArrowUp size={14} /></button>
            <button class="icon-btn" title="Move down" disabled={i === s.data.jump_hosts.length - 1} onclick={() => moveJump(i, 1)}><ArrowDown size={14} /></button>
            <button class="icon-btn" title="Remove" onclick={() => (s.data.jump_hosts = s.data.jump_hosts.filter((_: unknown, x: number) => x !== i))}><Trash2 size={14} /></button>
          </div>
        {/each}
      </div>
    {:else if section === "rdp"}
      <div class="row3">
        <label class="field grow"><span>Screen size</span>
          <select value={s.data.resolution ?? "fit"} onchange={(e) => (s.data.resolution = (e.target as HTMLSelectElement).value)}>
            <option value="fit">Fit to the tab (follows resizing)</option>
            {#each ["1280x720", "1280x1024", "1366x768", "1600x900", "1920x1080", "1920x1200", "2560x1440", "3840x2160"] as r}
              <option value={r}>{r.replace("x", " x ")}</option>
            {/each}
          </select>
        </label>
        <label class="field"><span>Colours</span>
          <select value={String(s.data.color_depth ?? 32)} onchange={(e) => (s.data.color_depth = Number((e.target as HTMLSelectElement).value))}>
            <option value="32">32 bit</option><option value="24">24 bit</option><option value="16">16 bit</option><option value="15">15 bit</option>
          </select>
        </label>
        <label class="field"><span>Sound</span>
          <select value={s.data.audio ?? "local"} onchange={(e) => (s.data.audio = (e.target as HTMLSelectElement).value)}>
            <option value="local">Play on this PC</option><option value="remote">Play on the remote computer</option><option value="none">No sound</option>
          </select>
        </label>
      </div>
      <strong class="sub">Share with the remote computer</strong>
      <div class="grid2">
        {#each [["redirect_clipboard", "Clipboard", true], ["redirect_drives", "Drives", false], ["redirect_printers", "Printers", false], ["redirect_smartcards", "Smart cards", false], ["redirect_ports", "Serial ports", false], ["redirect_microphone", "Microphone", false]] as [key, label, def]}
          <label class="check"><input type="checkbox" checked={s.data[key as string] ?? def} onchange={(e) => (s.data[key as string] = (e.target as HTMLInputElement).checked)} /> {label}</label>
        {/each}
      </div>
      <strong class="sub">Connection</strong>
      <div class="grid2">
        {#each [["forward_shortcuts", "Send Windows key combinations to the remote computer", true], ["admin_console", "Connect to the admin console", false], ["credssp", "Network level authentication (CredSSP)", true], ["auto_reconnect", "Reconnect automatically after a network drop", true], ["restricted_admin", "Restricted admin mode", false], ["remote_guard", "Remote Credential Guard", false], ["smart_sizing", "Scale the remote screen instead of resizing it", false], ["wallpaper", "Show the remote wallpaper", false]] as [key, label, def]}
          <label class="check"><input type="checkbox" checked={s.data[key as string] ?? def} onchange={(e) => (s.data[key as string] = (e.target as HTMLInputElement).checked)} /> {label}</label>
        {/each}
      </div>
      <label class="field"><span>Start this program instead of the desktop</span><input bind:value={s.data.remote_program} placeholder="Full desktop" /></label>
    {:else if section === "network"}
      {#if s.kind === "rdp"}
      <strong class="sub">Remote Desktop Gateway</strong>
      <div class="row3">
        <label class="field grow"><span>Gateway server</span><input value={s.data.gateway?.host ?? ""} oninput={(e) => (s.data.gateway = { ...(s.data.gateway ?? {}), host: (e.target as HTMLInputElement).value })} placeholder="None" /></label>
        <label class="field"><span>Use it</span>
          <select value={s.data.gateway?.usage ?? "always"} onchange={(e) => (s.data.gateway = { ...(s.data.gateway ?? {}), usage: (e.target as HTMLSelectElement).value })}>
            <option value="always">Always</option><option value="detect">Only when needed</option>
          </select>
        </label>
        <label class="field"><span>Log in to the gateway</span>
          <select value={s.data.gateway?.auth ?? "same"} onchange={(e) => (s.data.gateway = { ...(s.data.gateway ?? {}), auth: (e.target as HTMLSelectElement).value })}>
            <option value="same">Same as the server</option><option value="ask">Ask when connecting</option><option value="smartcard">Smart card</option>
          </select>
        </label>
      </div>
      {/if}
      <strong class="sub">SSH gateway</strong>
      <p class="muted small">Reach the server through an SSH server: Cablebag forwards the connection over SSH.</p>
      <div class="row3">
        <label class="field grow"><span>SSH server</span><input value={s.data.ssh_gateway?.host ?? ""} oninput={(e) => (s.data.ssh_gateway = { ...(s.data.ssh_gateway ?? { port: 22 }), host: (e.target as HTMLInputElement).value })} placeholder="None" /></label>
        <label class="field"><span>User</span><input value={s.data.ssh_gateway?.user ?? ""} oninput={(e) => (s.data.ssh_gateway = { ...(s.data.ssh_gateway ?? { port: 22 }), user: (e.target as HTMLInputElement).value })} /></label>
        <label class="field port"><span>Port</span><input type="number" value={s.data.ssh_gateway?.port ?? 22} oninput={(e) => (s.data.ssh_gateway = { ...(s.data.ssh_gateway ?? {}), port: Number((e.target as HTMLInputElement).value) })} /></label>
      </div>
      <label class="field"><span>SSH key file for the gateway</span><input value={s.data.ssh_gateway?.key_path ?? ""} oninput={(e) => (s.data.ssh_gateway = { ...(s.data.ssh_gateway ?? { port: 22 }), key_path: (e.target as HTMLInputElement).value })} placeholder="None" /></label>
    {:else if section === "bookmark"}
      <label class="field"><span>Notes</span><textarea rows="3" bind:value={s.comment}></textarea></label>
      {#if ["ssh", "shell", "wsl", "telnet", "serial", "mosh"].includes(s.kind)}
        <label class="field"><span>Colour words in the output</span>
          <select value={s.data.syntax ?? ""} onchange={(e) => (s.data.syntax = (e.target as HTMLSelectElement).value || undefined)}>
            <option value="">Same as the settings</option>
            {#each BUILTIN_SYNTAX as x}<option value={x.id}>{x.name}</option>{/each}
            <option value="custom">My own words</option>
          </select>
        </label>
      {/if}
      <label class="field"><span>Tab colour</span>
        <select bind:value={s.data.tab_color}>
          {#each TAB_COLORS as c}<option value={c.value}>{c.name}</option>{/each}
        </select>
      </label>
    {/if}
{/snippet}

{#snippet sessionFooter()}
  <button class="btn" onclick={() => (app.editing = null)}>Cancel</button>
  <button class="btn" onclick={save}>Save</button>
  <button class="btn primary" onclick={saveAndOpen} disabled={!k.ready}>Save and open</button>
{/snippet}

<style>
  .row2n {
    display: flex;
    gap: 10px;
  }
  .editor {
    display: flex;
    gap: 0;
    margin: -4px 0;
    min-height: 470px;
  }
  .types {
    width: 210px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding-right: 12px;
    margin-right: 18px;
    border-right: 1px solid var(--border);
  }
  .ghead {
    padding: 10px 8px 3px;
    font-size: 11px;
    color: var(--muted);
  }
  .ghead:first-child {
    padding-top: 2px;
  }
  .type {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 5px 8px;
    border: none;
    border-radius: 3px;
    background: none;
    color: var(--muted);
    text-align: left;
  }
  .type :global(svg) {
    margin-top: 2px;
    flex-shrink: 0;
  }
  .type:hover {
    background: var(--hover);
    color: var(--text);
  }
  .type.on {
    background: var(--selected);
    color: var(--text);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .tl {
    display: flex;
    flex-direction: column;
    line-height: 1.25;
  }
  .tl em {
    font-style: normal;
    font-size: 11px;
    color: var(--muted);
  }
  .pane {
    flex: 1;
    min-width: 0;
  }
  .fold {
    border-top: 1px solid var(--border);
  }
  .fh {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 9px 0 7px;
    border: none;
    background: none;
    color: var(--muted);
    font-weight: 600;
    text-align: left;
  }
  .fh:hover {
    color: var(--text);
  }
  .chev {
    display: inline-flex;
    transition: transform 0.12s;
  }
  .fold.open .chev {
    transform: rotate(90deg);
  }
  .fbody {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 4px 0 14px;
  }
  .notice {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px 10px;
    margin-bottom: 10px;
    border-radius: 3px;
    background: var(--panel);
    color: var(--muted);
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 260px;
  }
  .row3 {
    display: flex;
    gap: 10px;
  }
  .grow {
    flex: 1;
  }
  .port input {
    width: 90px;
  }
  .with-btn {
    display: flex;
    gap: 6px;
  }
  .with-btn input {
    flex: 1;
  }
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px 16px;
  }
  .soon {
    font-style: normal;
    font-size: 11px;
    color: var(--faint);
  }
  .jumps {
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 10px;
  }
  .jhead {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
  }
  .jhead .muted {
    flex: 1;
    font-size: 12px;
  }
  .jrow {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  .jrow input {
    flex: 1;
  }
  .jrow .small {
    flex: 0 0 110px;
  }
  .jrow .tiny {
    flex: 0 0 70px;
  }
  textarea {
    resize: vertical;
    font-family: var(--font-mono);
  }
  .error {
    color: var(--danger);
    margin: 0;
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
    font-weight: 600;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
</style>

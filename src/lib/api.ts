import { invoke, Channel } from "@tauri-apps/api/core";

export interface Folder {
  id: string;
  parent_id: string | null;
  name: string;
  sort: number;
  /** What sessions inside use when they have no value of their own (see FOLDER_DEFAULTS). */
  data?: Record<string, any>;
}

export interface Session {
  id: string;
  folder_id: string | null;
  name: string;
  kind: string;
  host: string;
  port: number;
  username: string;
  data: Record<string, any>;
  comment: string;
  sort: number;
  last_used: number;
}

export interface LocalShell {
  id: string;
  name: string;
  program: string;
  args: string[];
}

export interface Entry {
  name: string;
  path: string;
  is_dir: boolean;
  is_link: boolean;
  size: number;
  modified: number;
  mode: number;
  permissions: string;
  owner: string;
  group: string;
}

export interface Listing {
  path: string;
  entries: Entry[];
}

export interface PromptField {
  name: string;
  label: string;
  kind: "text" | "password" | "checkbox" | "choice";
  value: any;
}

export interface PromptRequest {
  id: string;
  title: string;
  message: string;
  details: [string, string][];
  fields: PromptField[];
  buttons: { id: string; label: string; style: string }[];
  warning: boolean;
}

export interface KnownHost {
  host: string;
  port: number;
  algo: string;
  key: string;
  fingerprint: string;
  added: number;
}

export interface SecretInfo {
  id: string;
  kind: string;
  label: string;
  protocol: string;
  host: string;
  port: number;
  username: string;
  updated: number;
}

export interface Hop {
  host: string;
  port: number;
  user: string;
  key_path: string;
}

export interface TunnelDef {
  id: string;
  name: string;
  kind: "local" | "remote" | "dynamic";
  bind_address: string;
  bind_port: number;
  dest_host: string;
  dest_port: number;
  ssh: Hop;
  jump_hosts: Hop[];
  /** Kept by the interface only: start when Cablebag starts. */
  autostart?: boolean;
  /** Kept by the interface only: the saved session the SSH details came from. */
  session_id?: string;
}

export interface TunnelStatus {
  id: string;
  state: "starting" | "running" | "stopped" | "failed";
  message: string;
  connections: number;
}

export interface HostResult {
  scan: string;
  ip: string;
  name: string;
  mac: string;
  rtt_ms: number | null;
  open_ports: number[];
}

export interface SocketInfo {
  protocol: string;
  local_address: string;
  local_port: number;
  remote_address: string;
  remote_port: number;
  state: string;
  pid: number;
  process: string;
}

export interface KeyInfo {
  id: string;
  algorithm: string;
  comment: string;
  public_key: string;
  fingerprint: string;
}

export interface SyncResult {
  at: number;
  received: number;
  sent: number;
  conflicts: number;
  /** The items changed here and elsewhere since the last sync, and whose version was kept. */
  conflict_list?: { what: string; name: string; kept: string }[];
  sessions: number;
  folders: number;
  error: string;
}

/** The kinds of credential source. */
export type SourceKind = "vaultwarden" | "cyberark" | "delinea";

/** A credential source as set up in Settings. No secrets: this syncs. */
export interface SourceConfig {
  id: string;
  name: string;
  kind: SourceKind;
  /** The vault, PVWA or Privilege Cloud, or Secret Server address. */
  url: string;
  /** The vault's email address, or the user name to sign in with (empty for single sign-on). */
  account: string;
  /** What else the kind needs, such as the sign-in method; each kind has its own keys. */
  options: Record<string, any>;
}

/** A source with its state. */
export interface SourceStatus extends SourceConfig {
  configured: boolean;
  /** Open (Vaultwarden) or signed in (CyberArk, Secret Server). */
  unlocked: boolean;
  /** This PC keeps something that opens it again after a restart. */
  remembered: boolean;
  items: number;
  synced: number;
  /** More logins exist than were loaded: searching asks the server. */
  partial: boolean;
  /** Logins checked out to you, to check in again. */
  checked_out: { id: string; name: string }[];
  /** One line about the sign-in. */
  detail: string;
}

export interface ProxmoxSource {
  id: string;
  name: string;
  url: string;
  token_id: string;
  vault_item: string;
  fingerprint: string;
  ssh_user: string;
  key_path: string;
  rdp_user: string;
  /** WireGuard network the server and its guests are reached through; empty = directly. */
  wireguard?: string;
  /** Only in listings: whether Cablebag keeps the token secret itself. */
  has_secret?: boolean;
}

export interface WireGuardProfile {
  id: string;
  name: string;
  /** This side's addresses in the tunnel, "10.8.0.2/32". */
  address: string;
  dns: string;
  mtu: number;
  /** The server's public key. */
  public_key: string;
  endpoint: string;
  allowed_ips: string;
  keepalive: number;
  vault_item: string;
}

export interface WireGuardStatus {
  /** Seconds since the last handshake; null = none yet. */
  handshake: number | null;
  sent: number;
  received: number;
  connections: number;
  warning: string;
}

/** A profile in a listing: with whether its keys are there and, while it runs, its state. */
export interface WireGuardInfo extends WireGuardProfile {
  has_key: boolean;
  has_preshared: boolean;
  /** This side's public key, from the private key Cablebag keeps. */
  own_public_key: string;
  status: WireGuardStatus | null;
}

export interface ProxmoxGuest {
  vmid: number;
  name: string;
  node: string;
  kind: "qemu" | "lxc";
  status: string;
  os: string;
  tags: string[];
  ips: string[];
  uptime: number;
  mem: number;
  maxmem: number;
  cpu: number;
}

export interface ProxmoxSnapshot {
  name: string;
  description: string;
  time: number;
  parent: string;
  ram: boolean;
}

export interface ProxmoxTask {
  upid: string;
  node: string;
  kind: string;
  id: string;
  user: string;
  status: string;
  start: number;
  end: number;
}

export interface VaultItem {
  /** The reference sessions keep: "source:item", or a bare id for the first vault. */
  id: string;
  /** The id of the source it comes from. */
  source: string;
  name: string;
  username: string;
  uris: string[];
  /** The vault folder, the CyberArk safe or the Secret Server folder. */
  folder: string;
  /** The Bitwarden organization, the CyberArk platform or the Secret Server template. */
  organization: string;
  has_password: boolean;
}

export interface LogSpec {
  path: string;
  kind: string;
}

export interface ImportSource {
  id: string;
  name: string;
  description: string;
  /** Sessions found on this PC; null when the source is a file you choose. */
  found: number | null;
  extensions: string[];
}

export interface ImportReport {
  file: string;
  folders: number;
  sessions: number;
  updated: number;
  by_kind: Record<string, number>;
}

export interface UpdateInfo {
  current: string;
  /** The newer version, or empty when Cablebag is up to date. */
  version: string;
  notes: string;
  date: string;
}

export const api = {
  appVersion: () => invoke<string>("app_version"),
  updateCheck: () => invoke<UpdateInfo>("update_check"),
  updateInstall: () => invoke<void>("update_install"),
  library: () => invoke<{ folders: Folder[]; sessions: Session[] }>("library"),
  saveSession: (session: Session) => invoke<Session>("save_session", { session }),
  deleteSession: (id: string) => invoke<void>("delete_session", { id }),
  saveFolder: (folder: Folder) => invoke<Folder>("save_folder", { folder }),
  deleteFolder: (id: string) => invoke<void>("delete_folder", { id }),
  settings: () => invoke<Record<string, any>>("get_settings"),
  setSetting: (key: string, value: any) => invoke<void>("set_setting", { key, value }),
  localShells: () => invoke<LocalShell[]>("local_shells"),
  promptAnswer: (id: string, answer: { button: string; values: Record<string, any> }) =>
    invoke<void>("prompt_answer", { id, answer }),
  openLocal: (
    id: string,
    spec: { program: string; args: string[]; cwd?: string; startup?: string },
    cols: number,
    rows: number,
    log: LogSpec | undefined,
    onData: Channel<ArrayBuffer>,
  ) => invoke<void>("term_open_local", { id, ...spec, cols, rows, log, onData }),
  openSsh: (id: string, session: Session, cols: number, rows: number, log: LogSpec | undefined, onData: Channel<ArrayBuffer>) =>
    invoke<void>("term_open_ssh", { id, session, cols, rows, log, onData }),
  writeTextFile: (path: string, text: string) => invoke<void>("write_text_file", { path, text }),
  defaultLogDir: () => invoke<string>("default_log_dir"),
  input: (id: string, data: string) => invoke<void>("term_input", { id, data }),
  resize: (id: string, cols: number, rows: number) => invoke<void>("term_resize", { id, cols, rows }),
  close: (id: string) => invoke<void>("term_close", { id }),
  sftpHome: (tab: string) => invoke<string>("sftp_home", { tab }),
  sftpList: (tab: string, path: string) => invoke<Listing>("sftp_list", { tab, path }),
  sftpMkdir: (tab: string, path: string) => invoke<void>("sftp_mkdir", { tab, path }),
  sftpNewFile: (tab: string, path: string) => invoke<void>("sftp_new_file", { tab, path }),
  sftpRename: (tab: string, from: string, to: string) => invoke<void>("sftp_rename", { tab, from, to }),
  sftpRemove: (tab: string, paths: string[]) => invoke<void>("sftp_remove", { tab, paths }),
  sftpChmod: (tab: string, path: string, mode: number, recursive: boolean) =>
    invoke<void>("sftp_chmod", { tab, path, mode, recursive }),
  sftpDownload: (tab: string, remotes: string[], localDir: string) =>
    invoke<void>("sftp_download", { tab, remotes, localDir }),
  sftpUpload: (tab: string, locals: string[], remoteDir: string) =>
    invoke<void>("sftp_upload", { tab, locals, remoteDir }),
  sftpFetchTemp: (tab: string, remote: string) => invoke<string>("sftp_fetch_temp", { tab, remote }),
  sftpPushBack: (tab: string, local: string, remote: string) =>
    invoke<void>("sftp_push_back", { tab, local, remote }),
  fileModified: (path: string) => invoke<number>("file_modified", { path }),
  importSources: () => invoke<ImportSource[]>("import_sources"),
  importSessions: (source: string, paths: string[]) => invoke<ImportReport>("import_sessions", { source, paths }),
  knownHosts: () => invoke<KnownHost[]>("known_hosts"),
  deleteKnownHost: (host: string, port: number, algo: string) =>
    invoke<void>("delete_known_host", { host, port, algo }),
  secrets: () => invoke<SecretInfo[]>("secrets"),
  deleteSecret: (id: string) => invoke<void>("delete_secret", { id }),
  revealSecret: (id: string) => invoke<string>("reveal_secret", { id }),
  dataFolder: () => invoke<string>("data_folder"),
  systemInfo: () => invoke<{ version: string; windows: string; webview: string; dotnet: string }>("system_info"),
  /** Hides passwords Cablebag knows and anything that looks like a key or token (the assistant's filter). */
  redact: (text: string) => invoke<string>("ai_redact", { text }),
  backupData: (path: string) => invoke<void>("backup_data", { path }),
  restoreData: (path: string) => invoke<number>("restore_data", { path }),
  restartApp: () => invoke<void>("restart_app"),
  filesOpen: (id: string, session: Session, onData: Channel<ArrayBuffer>) => invoke<void>("files_open", { id, session, onData }),
  openStream: (id: string, session: Session, cols: number, rows: number, log: LogSpec | undefined, onData: Channel<ArrayBuffer>) =>
    invoke<void>("term_open_stream", { id, session, cols, rows, log: log ?? null, onData }),
  fileOpen: (target: string) => invoke<void>("file_open", { target }),
  serialPorts: () => invoke<{ name: string; description: string }[]>("serial_ports"),
  browserOpen: (id: string, url: string, wireguard: string | undefined, profile: string | undefined, r: { x: number; y: number; w: number; h: number }) =>
    invoke<void>("browser_open", { id, url, wireguard: wireguard || null, profile: profile || null, ...r }),
  browserProfileForget: (id: string) => invoke<void>("browser_profile_forget", { id }),
  browserBounds: (id: string, r: { x: number; y: number; w: number; h: number }) => invoke<void>("browser_bounds", { id, ...r }),
  browserVisible: (id: string, show: boolean) => invoke<void>("browser_visible", { id, show }),
  browserCommand: (id: string, what: string, url?: string) => invoke<void>("browser_command", { id, what, url: url ?? null }),
  browserClose: (id: string) => invoke<void>("browser_close", { id }),
  rdpOpen: (id: string, session: Session, r: { x: number; y: number; w: number; h: number }, scale: number) =>
    invoke<void>("rdp_open", { id, session, ...r, scale }),
  rdpBounds: (id: string, r: { x: number; y: number; w: number; h: number }) => invoke<void>("rdp_bounds", { id, ...r }),
  rdpVisible: (id: string, on: boolean) => invoke<void>("rdp_visible", { id, on }),
  rdpCommand: (id: string, cmd: "cad" | "focus" | "disconnect") => invoke<void>("rdp_command", { id, cmd }),
  rdpClose: (id: string) => invoke<void>("rdp_close", { id }),
  /** A picture of the remote screen as it shows now (a JPEG data URL). */
  rdpSnapshot: (id: string) => invoke<string>("rdp_snapshot", { id }),
  sources: () => invoke<SourceStatus[]>("sources_list"),
  sourcesHello: () => invoke<boolean>("sources_hello"),
  sourceSave: (source: SourceConfig) => invoke<SourceConfig>("source_save", { source }),
  sourceDelete: (id: string) => invoke<void>("source_delete", { id }),
  sourcesReorder: (ids: string[]) => invoke<void>("sources_reorder", { ids }),
  sourceUnlock: (id: string) => invoke<void>("source_unlock", { id }),
  sourceSync: (id: string) => invoke<void>("source_sync", { id }),
  sourceLock: (id: string) => invoke<void>("source_lock", { id }),
  sourceCheckIn: (item: string) => invoke<void>("source_check_in", { item }),
  sourceForget: (id: string) => invoke<void>("source_forget", { id }),
  vaultItems: () => invoke<VaultItem[]>("vault_items"),
  vaultSearch: (query: string) => invoke<VaultItem[]>("vault_search", { query }),
  syncNow: () => invoke<SyncResult>("sync_now"),
  syncConfigure: (c: { kind: string; url: string; repo: string; vaultItem: string; token?: string; encrypt: string; passphrase?: string }) =>
    invoke<void>("sync_configure", { ...c, token: c.token || null, passphrase: c.passphrase || null }),
  syncHasToken: () => invoke<boolean>("sync_has_token"),
  syncHasPassphrase: () => invoke<boolean>("sync_has_passphrase"),
  wireguardProfiles: () => invoke<WireGuardInfo[]>("wireguard_profiles"),
  wireguardSave: (profile: WireGuardProfile, privateKey?: string, presharedKey?: string) =>
    invoke<WireGuardProfile>("wireguard_save", { profile, privateKey: privateKey ?? null, presharedKey: presharedKey ?? null }),
  wireguardDelete: (id: string) => invoke<void>("wireguard_delete", { id }),
  wireguardImport: (path: string) => invoke<WireGuardProfile>("wireguard_import", { path }),
  wireguardPublicKey: (id: string) => invoke<string>("wireguard_public_key", { id }),
  wireguardKeypair: () => invoke<[string, string]>("wireguard_keypair"),
  wireguardTest: (id: string) => invoke<WireGuardStatus>("wireguard_test", { id }),
  proxmoxSources: () => invoke<ProxmoxSource[]>("proxmox_sources"),
  proxmoxSaveSource: (source: ProxmoxSource, secret?: string) => invoke<ProxmoxSource>("proxmox_save_source", { source, secret: secret || null }),
  proxmoxDeleteSource: (id: string) => invoke<void>("proxmox_delete_source", { id }),
  proxmoxList: (id: string) => invoke<ProxmoxGuest[]>("proxmox_list", { id }),
  proxmoxPower: (id: string, g: ProxmoxGuest, action: string) =>
    invoke<void>("proxmox_power", { id, node: g.node, kind: g.kind, vmid: g.vmid, action }),
  tunnelStart: (def: TunnelDef) => invoke<void>("tunnel_start", { def }),
  tunnelStop: (id: string) => invoke<void>("tunnel_stop", { id }),
  tunnelStatuses: () => invoke<TunnelStatus[]>("tunnel_statuses"),
  localRanges: () => invoke<string[]>("net_local_ranges"),
  checkRange: (range: string) => invoke<number>("net_check_range", { range }),
  netScan: (scan: string, range: string, ports: number[]) => invoke<void>("net_scan", { scan, range, ports }),
  portScan: (scan: string, host: string, ports: number[]) => invoke<void>("net_port_scan", { scan, host, ports }),
  stopScan: (scan: string) => invoke<void>("net_stop_scan", { scan }),
  openPorts: () => invoke<SocketInfo[]>("net_open_ports"),
  wake: (mac: string, broadcast: string, port: number) => invoke<void>("net_wake", { mac, broadcast, port }),
  keyGenerate: (kind: string, comment: string) => invoke<KeyInfo>("key_generate", { kind, comment }),
  keyLoad: (path: string, passphrase?: string) => invoke<KeyInfo>("key_load", { path, passphrase }),
  keySave: (id: string, path: string, passphrase: string, comment: string) =>
    invoke<[string, string]>("key_save", { id, path, passphrase, comment }),
  keyForget: (id: string) => invoke<void>("key_forget", { id }),
  keyDefaultDir: () => invoke<string>("key_default_dir"),
  keyInstall: (id: string, session: Session) => invoke<void>("key_install", { id, session }),
};

export { Channel };

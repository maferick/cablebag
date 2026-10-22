mod agent;
mod ai;
mod bitwarden;
mod browser;
mod files;
mod ftp;
mod gitremote;
mod gitsync;
mod importers;
mod keygen;
mod nettools;
mod prompt;
mod proxmox;
mod pty;
mod rdp;
mod serial;
mod ssh;
mod store;
mod telnet;
mod term;
mod termlog;
mod tunnel;
mod tunnels;
mod updater;
mod vault;
mod vnc;

use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use store::{Folder, KnownHost, SecretInfo, Session, Store};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Manager, State};
use term::{TermCmd, TermOut, Terms};

pub struct AppState {
    pub store: Mutex<Store>,
    pub vault: vault::Vault,
    pub prompts: prompt::Prompts,
    pub terms: Terms,
    pub ssh: Mutex<HashMap<String, Arc<ssh::SshConn>>>,
    pub rdp: rdp::Rdps,
    pub tunnels: Arc<tunnels::Tunnels>,
    pub scans: Arc<nettools::Scans>,
    pub keys: keygen::Keys,
    pub vw: bitwarden::Vaultwarden,
    pub vncs: Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<vnc::VncCmd>>>,
    /// One Gitea sync at a time.
    pub sync_lock: tokio::sync::Mutex<()>,
    pub ftps: ftp::Ftps,
}

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize)]
struct Library {
    folders: Vec<Folder>,
    sessions: Vec<Session>,
}

#[tauri::command]
fn library(state: State<AppState>) -> CmdResult<Library> {
    let s = state.store.lock();
    Ok(Library { folders: s.folders().map_err(err)?, sessions: s.sessions().map_err(err)? })
}

#[tauri::command]
fn save_session(state: State<AppState>, mut session: Session) -> CmdResult<Session> {
    if session.id.is_empty() {
        session.id = uuid::Uuid::new_v4().to_string();
    }
    state.store.lock().save_session(&session).map_err(err)?;
    Ok(session)
}

#[tauri::command]
fn delete_session(state: State<AppState>, id: String) -> CmdResult<()> {
    state.store.lock().delete_session(&id).map_err(err)
}

#[tauri::command]
fn save_folder(state: State<AppState>, mut folder: Folder) -> CmdResult<Folder> {
    if folder.id.is_empty() {
        folder.id = uuid::Uuid::new_v4().to_string();
    }
    state.store.lock().save_folder(&folder).map_err(err)?;
    Ok(folder)
}

#[tauri::command]
fn delete_folder(state: State<AppState>, id: String) -> CmdResult<()> {
    state.store.lock().delete_folder(&id).map_err(err)
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> CmdResult<serde_json::Map<String, Value>> {
    state.store.lock().settings().map_err(err)
}

#[tauri::command]
fn set_setting(state: State<AppState>, key: String, value: Value) -> CmdResult<()> {
    state.store.lock().set_setting(&key, &value).map_err(err)
}

#[tauri::command]
fn local_shells() -> Vec<pty::LocalShell> {
    pty::list_shells()
}

#[tauri::command]
fn prompt_answer(state: State<AppState>, id: String, answer: prompt::PromptAnswer) {
    state.prompts.answer(&id, answer);
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn term_open_local(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
    startup: Option<String>,
    cols: u16,
    rows: u16,
    log: Option<termlog::LogSpec>,
    on_data: Channel<InvokeResponseBody>,
) -> CmdResult<()> {
    let log = log.map(|l| termlog::open(&l)).transpose().map_err(err)?;
    let out = TermOut { id: id.clone(), channel: on_data, app, log };
    let tx = pty::spawn(pty::LocalSpec { program, args, cwd, startup }, cols, rows, out).map_err(err)?;
    state.terms.insert(&id, tx);
    Ok(())
}

#[tauri::command]
fn term_open_ssh(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    session: Session,
    cols: u16,
    rows: u16,
    log: Option<termlog::LogSpec>,
    on_data: Channel<InvokeResponseBody>,
) -> CmdResult<()> {
    if !session.id.is_empty() {
        let _ = state.store.lock().touch_session(&session.id);
    }
    let log = log.map(|l| termlog::open(&l)).transpose().map_err(err)?;
    let out = TermOut { id: id.clone(), channel: on_data, app: app.clone(), log };
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    state.terms.insert(&id, tx);
    tauri::async_runtime::spawn(ssh::run_terminal(app, id, session, cols, rows, out, rx));
    Ok(())
}

/// Telnet and serial sessions: terminals over a TCP connection or a COM port.
#[tauri::command]
fn term_open_stream(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    session: Session,
    cols: u16,
    rows: u16,
    log: Option<termlog::LogSpec>,
    on_data: Channel<InvokeResponseBody>,
) -> CmdResult<()> {
    if !session.id.is_empty() {
        let _ = state.store.lock().touch_session(&session.id);
    }
    let log = log.map(|l| termlog::open(&l)).transpose().map_err(err)?;
    let out = TermOut { id: id.clone(), channel: on_data, app: app.clone(), log };
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    state.terms.insert(&id, tx);
    match session.kind.as_str() {
        "telnet" => {
            tauri::async_runtime::spawn(telnet::run(app, session, cols, rows, out, rx));
        }
        "serial" => {
            tauri::async_runtime::spawn(serial::run(session, out, rx));
        }
        other => return Err(format!("{other} is not a terminal session.")),
    }
    Ok(())
}

/// Async on purpose: making a web view inside a synchronous command deadlocks on Windows.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn browser_open(app: AppHandle, id: String, url: String, x: f64, y: f64, w: f64, h: f64) -> CmdResult<()> {
    browser::open(&app, &id, &url, x, y, w, h).map_err(err)
}

#[tauri::command]
fn browser_bounds(app: AppHandle, id: String, x: f64, y: f64, w: f64, h: f64) -> CmdResult<()> {
    browser::bounds(&app, &id, x, y, w, h).map_err(err)
}

#[tauri::command]
fn browser_visible(app: AppHandle, id: String, show: bool) -> CmdResult<()> {
    browser::visible(&app, &id, show).map_err(err)
}

#[tauri::command]
fn browser_command(app: AppHandle, id: String, what: String, url: Option<String>) -> CmdResult<()> {
    browser::command(&app, &id, &what, url).map_err(err)
}

#[tauri::command]
fn browser_close(app: AppHandle, id: String) {
    browser::close(&app, &id);
}

/// File or folder sessions: a path opens in the program Windows uses for it, a link in the web browser.
#[tauri::command]
fn file_open(app: AppHandle, target: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let t = target.trim();
    let is_link = t.split_once("://").is_some_and(|(scheme, _)| scheme.len() > 1 && scheme.chars().all(|c| c.is_ascii_alphabetic()));
    if is_link {
        let url = browser::parse(t).map_err(err)?;
        app.opener().open_url(url.as_str(), None::<&str>).map_err(err)
    } else {
        let p = std::path::Path::new(t);
        if !p.exists() {
            return Err(format!("{t} does not exist (or this PC cannot reach it)."));
        }
        app.opener().open_path(t, None::<&str>).map_err(err)
    }
}

#[tauri::command]
fn serial_ports() -> Vec<serial::Port> {
    serial::ports()
}

#[tauri::command]
fn term_input(state: State<AppState>, id: String, data: String) {
    state.terms.send(&id, TermCmd::Input(data.into_bytes()));
}

#[tauri::command]
fn term_resize(state: State<AppState>, id: String, cols: u16, rows: u16) {
    state.terms.send(&id, TermCmd::Resize(cols, rows));
}

#[tauri::command]
fn term_close(state: State<AppState>, id: String) {
    state.terms.remove(&id);
}

/// FTP tabs use the same file commands as SFTP.
fn is_ftp(app: &AppHandle, tab: &str) -> bool {
    app.state::<AppState>().ftps.has(tab)
}

#[tauri::command]
async fn sftp_home(app: AppHandle, tab: String) -> CmdResult<String> {
    (if is_ftp(&app, &tab) { ftp::home(&app, &tab).await } else { files::home(&app, &tab).await }).map_err(err)
}

#[tauri::command]
async fn sftp_list(app: AppHandle, tab: String, path: String) -> CmdResult<files::Listing> {
    (if is_ftp(&app, &tab) { ftp::list(&app, &tab, &path).await } else { files::list(&app, &tab, &path).await }).map_err(err)
}

#[tauri::command]
async fn sftp_mkdir(app: AppHandle, tab: String, path: String) -> CmdResult<()> {
    (if is_ftp(&app, &tab) { ftp::mkdir(&app, &tab, &path).await } else { files::mkdir(&app, &tab, &path).await }).map_err(err)
}

#[tauri::command]
async fn sftp_new_file(app: AppHandle, tab: String, path: String) -> CmdResult<()> {
    (if is_ftp(&app, &tab) { ftp::new_file(&app, &tab, &path).await } else { files::new_file(&app, &tab, &path).await }).map_err(err)
}

#[tauri::command]
async fn sftp_rename(app: AppHandle, tab: String, from: String, to: String) -> CmdResult<()> {
    (if is_ftp(&app, &tab) { ftp::rename(&app, &tab, &from, &to).await } else { files::rename(&app, &tab, &from, &to).await }).map_err(err)
}

#[tauri::command]
async fn sftp_remove(app: AppHandle, tab: String, paths: Vec<String>) -> CmdResult<()> {
    for p in paths {
        (if is_ftp(&app, &tab) { ftp::remove(&app, &tab, &p).await } else { files::remove(&app, &tab, &p).await }).map_err(err)?;
    }
    Ok(())
}

#[tauri::command]
async fn sftp_chmod(app: AppHandle, tab: String, path: String, mode: u32, recursive: bool) -> CmdResult<()> {
    (if is_ftp(&app, &tab) { ftp::chmod(&app, &tab, &path, mode).await } else { files::chmod(&app, &tab, &path, mode, recursive).await }).map_err(err)
}

#[tauri::command]
async fn sftp_download(app: AppHandle, tab: String, remotes: Vec<String>, local_dir: String) -> CmdResult<()> {
    (if is_ftp(&app, &tab) { ftp::download(&app, &tab, &remotes, &local_dir).await } else { files::download(&app, &tab, &remotes, &local_dir).await }).map_err(err)
}

#[tauri::command]
async fn sftp_upload(app: AppHandle, tab: String, locals: Vec<String>, remote_dir: String) -> CmdResult<()> {
    (if is_ftp(&app, &tab) { ftp::upload(&app, &tab, &locals, &remote_dir).await } else { files::upload(&app, &tab, &locals, &remote_dir).await }).map_err(err)
}

#[tauri::command]
async fn sftp_fetch_temp(app: AppHandle, tab: String, remote: String) -> CmdResult<String> {
    (if is_ftp(&app, &tab) { ftp::fetch_temp(&app, &tab, &remote).await } else { files::fetch_temp(&app, &tab, &remote).await }).map(|p| p.to_string_lossy().to_string()).map_err(err)
}

#[tauri::command]
async fn sftp_push_back(app: AppHandle, tab: String, local: String, remote: String) -> CmdResult<()> {
    (if is_ftp(&app, &tab) { ftp::push_back(&app, &tab, &local, &remote).await } else { files::push_back(&app, &tab, &local, &remote).await }).map_err(err)
}

#[tauri::command]
fn file_modified(path: String) -> u64 {
    std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[tauri::command]
fn import_sources() -> Vec<importers::Source> {
    importers::sources()
}

#[tauri::command]
fn import_sessions(state: State<AppState>, source: String, paths: Vec<String>) -> CmdResult<importers::Report> {
    importers::import(&state.store.lock(), &source, &paths).map_err(err)
}

#[tauri::command]
fn known_hosts(state: State<AppState>) -> CmdResult<Vec<KnownHost>> {
    state.store.lock().all_known_hosts().map_err(err)
}

#[tauri::command]
fn delete_known_host(state: State<AppState>, host: String, port: i64, algo: String) -> CmdResult<()> {
    state.store.lock().delete_known_host(&host, port, &algo).map_err(err)
}

#[tauri::command]
fn secrets(state: State<AppState>) -> CmdResult<Vec<SecretInfo>> {
    state.store.lock().list_secrets().map_err(err)
}

#[tauri::command]
fn delete_secret(state: State<AppState>, id: String) -> CmdResult<()> {
    state.store.lock().delete_secret(&id).map_err(err)
}

#[tauri::command]
fn reveal_secret(state: State<AppState>, id: String) -> CmdResult<String> {
    let store = state.store.lock();
    let (n, c) = store.secret_by_id(&id).map_err(err)?.ok_or("not found")?;
    state.vault.decrypt(&store, &n, &c).map_err(err)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn rdp_open(app: AppHandle, id: String, session: Session, x: i32, y: i32, w: i32, h: i32, scale: u32) -> CmdResult<()> {
    if !session.id.is_empty() {
        let _ = app.state::<AppState>().store.lock().touch_session(&session.id);
    }
    let res = rdp::open(app.clone(), id.clone(), session, rdp::Geometry { x, y, w, h, scale }).await;
    if let Err(e) = &res {
        let _ = tauri::Emitter::emit(&app, "term-status", serde_json::json!({"id": id, "state": "failed", "message": e.to_string()}));
    }
    res.map_err(err)
}

#[tauri::command]
fn rdp_bounds(state: State<AppState>, id: String, x: i32, y: i32, w: i32, h: i32) {
    state.rdp.send(&id, serde_json::json!({"cmd": "bounds", "x": x, "y": y, "w": w, "h": h}));
}

#[tauri::command]
fn rdp_visible(state: State<AppState>, id: String, on: bool) {
    state.rdp.send(&id, serde_json::json!({"cmd": "visible", "on": on}));
}

#[tauri::command]
fn rdp_command(state: State<AppState>, id: String, cmd: String) {
    state.rdp.send(&id, serde_json::json!({"cmd": cmd}));
}

#[tauri::command]
fn rdp_close(state: State<AppState>, id: String) {
    state.rdp.remove(&id);
}

#[tauri::command]
fn write_text_file(path: String, text: String) -> CmdResult<()> {
    std::fs::write(&path, text.replace('\n', "\r\n")).map_err(err)
}

#[tauri::command]
fn default_log_dir() -> String {
    dirs::document_dir().unwrap_or_else(store::data_dir).join("Cablebag logs").display().to_string()
}

#[tauri::command]
async fn tunnel_start(app: AppHandle, state: State<'_, AppState>, def: tunnels::TunnelDef) -> CmdResult<()> {
    let t = state.tunnels.clone();
    t.start(app, def).await.map_err(err)
}

#[tauri::command]
async fn tunnel_stop(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let t = state.tunnels.clone();
    t.stop(&app, &id).await;
    Ok(())
}

#[tauri::command]
fn tunnel_statuses(state: State<AppState>) -> Vec<tunnels::TunnelStatus> {
    state.tunnels.statuses()
}

#[tauri::command]
fn net_local_ranges() -> Vec<String> {
    nettools::local_ranges()
}

#[tauri::command]
fn net_check_range(range: String) -> CmdResult<usize> {
    nettools::parse_range(&range).map(|v| v.len()).map_err(err)
}

#[tauri::command]
async fn net_scan(app: AppHandle, state: State<'_, AppState>, scan: String, range: String, ports: Vec<u16>) -> CmdResult<()> {
    let s = state.scans.clone();
    nettools::scan_network(app, s, scan, range, ports).await.map_err(err)
}

#[tauri::command]
async fn net_port_scan(app: AppHandle, state: State<'_, AppState>, scan: String, host: String, ports: Vec<u16>) -> CmdResult<()> {
    let s = state.scans.clone();
    nettools::scan_ports(app, s, scan, host, ports).await.map_err(err)
}

#[tauri::command]
fn net_stop_scan(state: State<AppState>, scan: String) {
    state.scans.stop(&scan);
}

#[tauri::command]
async fn net_open_ports() -> CmdResult<Vec<nettools::Socket>> {
    tauri::async_runtime::spawn_blocking(nettools::open_ports).await.map_err(err)?.map_err(err)
}

#[derive(serde::Serialize)]
struct ProxmoxSourceInfo {
    #[serde(flatten)]
    source: proxmox::Source,
    has_secret: bool,
}

#[tauri::command]
fn proxmox_sources(app: AppHandle) -> Vec<ProxmoxSourceInfo> {
    proxmox::sources(&app).into_iter().map(|s| ProxmoxSourceInfo { has_secret: proxmox::has_secret(&app, &s), source: s }).collect()
}

#[tauri::command]
fn proxmox_save_source(app: AppHandle, source: proxmox::Source, secret: Option<String>) -> CmdResult<proxmox::Source> {
    proxmox::save_source(&app, source, secret).map_err(err)
}

#[tauri::command]
fn proxmox_delete_source(app: AppHandle, id: String) -> CmdResult<()> {
    proxmox::delete_source(&app, &id).map_err(err)
}

#[tauri::command]
async fn proxmox_list(app: AppHandle, id: String) -> CmdResult<Vec<proxmox::Guest>> {
    proxmox::list(&app, &id).await.map_err(err)
}

#[tauri::command]
async fn proxmox_power(app: AppHandle, id: String, node: String, kind: String, vmid: u64, action: String) -> CmdResult<()> {
    proxmox::power(&app, &id, &node, &kind, vmid, &action).await.map_err(err)
}

#[tauri::command]
async fn net_reach(targets: Vec<nettools::ReachTarget>, port_first: bool) -> Vec<nettools::Reach> {
    nettools::reach(targets, port_first).await
}

#[tauri::command]
fn net_wake(mac: String, broadcast: String, port: u16) -> CmdResult<()> {
    nettools::wake(&mac, &broadcast, port).map_err(err)
}

#[tauri::command]
async fn key_generate(state: State<'_, AppState>, kind: String, comment: String) -> CmdResult<keygen::KeyInfo> {
    state.keys.generate(&kind, &comment).map_err(err)
}

#[tauri::command]
fn key_load(state: State<AppState>, path: String, passphrase: Option<String>) -> CmdResult<keygen::KeyInfo> {
    state.keys.load(&path, passphrase.as_deref()).map_err(err)
}

#[tauri::command]
fn key_save(state: State<AppState>, id: String, path: String, passphrase: String, comment: String) -> CmdResult<(String, String)> {
    state.keys.save(&id, &path, &passphrase, &comment).map_err(err)
}

#[tauri::command]
fn key_forget(state: State<AppState>, id: String) {
    state.keys.forget(&id);
}

#[tauri::command]
fn key_default_dir() -> String {
    keygen::default_key_dir()
}

#[tauri::command]
async fn key_install(app: AppHandle, state: State<'_, AppState>, id: String, session: Session) -> CmdResult<()> {
    let public = state.keys.public_key(&id).ok_or("That key is no longer open.")?;
    keygen::install(&app, &public, &ssh::hops_for(&session)).await.map_err(err)
}

#[tauri::command]
fn vault_status(app: AppHandle, state: State<AppState>) -> bitwarden::VaultStatus {
    state.vw.status(&app)
}

#[tauri::command]
fn vault_configure(app: AppHandle, state: State<AppState>, server: String, email: String) -> CmdResult<()> {
    let server = server.trim().trim_end_matches('/').to_string();
    let server = if server.starts_with("http://") || server.starts_with("https://") { server } else { format!("https://{server}") };
    state.vw.lock();
    // Another server or account: the key this PC kept belongs to the old one.
    bitwarden::forget(&app);
    let store = state.store.lock();
    store.set_setting("vault.server", &serde_json::Value::String(server)).map_err(err)?;
    store.set_setting("vault.email", &serde_json::Value::String(email.trim().to_string())).map_err(err)?;
    Ok(())
}

#[tauri::command]
async fn vault_unlock(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.vw.unlock_interactive(&app).await.map_err(err)
}

#[tauri::command]
async fn vault_sync(state: State<'_, AppState>) -> CmdResult<()> {
    state.vw.sync().await.map_err(err)
}

#[tauri::command]
fn vault_lock(state: State<AppState>) {
    state.vw.lock();
}

#[tauri::command]
async fn sync_now(app: AppHandle) -> CmdResult<gitsync::SyncResult> {
    gitsync::sync(&app).await.map_err(err)
}

/// Saves where to sync, and the token when one is typed (kept encrypted on this PC).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn sync_configure(
    app: AppHandle,
    state: State<AppState>,
    kind: String,
    url: String,
    repo: String,
    vault_item: String,
    token: Option<String>,
    encrypt: String,
    passphrase: Option<String>,
) -> CmdResult<()> {
    let url = url.trim().trim_end_matches('/').to_string();
    let url = if url.starts_with("http://") || url.starts_with("https://") || url.is_empty() { url } else { format!("https://{url}") };
    {
        let store = state.store.lock();
        let old: String = store.setting("sync.url").ok().flatten().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        let old_repo: String = store.setting("sync.repo").ok().flatten().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        store.set_setting("sync.url", &serde_json::Value::String(url.clone())).map_err(err)?;
        store.set_setting("sync.repo", &serde_json::Value::String(repo.trim().trim_matches('/').to_string())).map_err(err)?;
        store.set_setting("sync.vault_item", &serde_json::Value::String(vault_item)).map_err(err)?;
        store.set_setting("sync.kind", &serde_json::Value::String(kind)).map_err(err)?;
        store.set_setting("sync.encrypt", &serde_json::Value::String(encrypt)).map_err(err)?;
        // Another place: start over as a first sync there (both sides combined, nothing deleted).
        if old != url || old_repo != repo.trim().trim_matches('/') {
            store.set_setting("sync.base", &serde_json::Value::Null).map_err(err)?;
        }
    }
    if let Some(t) = token.filter(|t| !t.trim().is_empty()) {
        gitsync::save_token(&app, &t).map_err(err)?;
    }
    if let Some(p) = passphrase.filter(|p| !p.is_empty()) {
        gitsync::save_passphrase(&app, &p).map_err(err)?;
    }
    Ok(())
}

#[tauri::command]
fn sync_has_passphrase(app: AppHandle) -> bool {
    gitsync::has_passphrase(&app)
}

#[tauri::command]
fn sync_has_token(app: AppHandle) -> bool {
    gitsync::has_token(&app)
}

/// Which vault logins fit a host, best first (names and user names only).
#[tauri::command]
fn vault_find(state: State<AppState>, kind: String, host: String, user: String, name: String) -> Vec<bitwarden::VaultItem> {
    state.vw.candidates(&kind, &host, &user, &name)
}

/// Stops keeping the vault key on this PC; the next unlock needs the master password.
#[tauri::command]
fn vault_forget(app: AppHandle) {
    bitwarden::forget(&app);
}

#[tauri::command]
fn vault_items(state: State<AppState>) -> Vec<bitwarden::VaultItem> {
    state.vw.items()
}

#[tauri::command]
fn files_open(app: AppHandle, state: State<AppState>, id: String, session: Session, on_data: Channel<InvokeResponseBody>) -> CmdResult<()> {
    if !session.id.is_empty() {
        let _ = state.store.lock().touch_session(&session.id);
    }
    let out = TermOut { id: id.clone(), channel: on_data, app: app.clone(), log: None };
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    state.terms.insert(&id, tx);
    if session.kind == "ftp" {
        tauri::async_runtime::spawn(ftp::run(app, id, session, out, rx));
    } else {
        tauri::async_runtime::spawn(ssh::run_files_only(app, id, session, out, rx));
    }
    Ok(())
}

#[tauri::command]
fn vnc_open(app: AppHandle, state: State<AppState>, id: String, session: Session, on_data: Channel<InvokeResponseBody>) {
    if !session.id.is_empty() {
        let _ = state.store.lock().touch_session(&session.id);
    }
    let out = TermOut { id: id.clone(), channel: on_data, app: app.clone(), log: None };
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    if let Some(old) = state.vncs.lock().insert(id, tx) {
        let _ = old.send(vnc::VncCmd::Close);
    }
    tauri::async_runtime::spawn(vnc::run(app, session, out, rx));
}

#[tauri::command]
fn vnc_input(state: State<AppState>, id: String, kind: String, a: u32, b: u32, c: u32, text: Option<String>) {
    let cmd = match kind.as_str() {
        "key" => vnc::VncCmd::Key(a, b != 0),
        "pointer" => vnc::VncCmd::Pointer(a as u16, b as u16, c as u8),
        "clipboard" => vnc::VncCmd::Clipboard(text.unwrap_or_default()),
        "cad" => vnc::VncCmd::CtrlAltDel,
        _ => return,
    };
    if let Some(tx) = state.vncs.lock().get(&id) {
        let _ = tx.send(cmd);
    }
}

#[tauri::command]
fn vnc_close(state: State<AppState>, id: String) {
    if let Some(tx) = state.vncs.lock().remove(&id) {
        let _ = tx.send(vnc::VncCmd::Close);
    }
}

#[derive(serde::Serialize)]
struct AiHostInfo {
    #[serde(flatten)]
    host: ai::Host,
    has_key: bool,
}

#[tauri::command]
fn ai_hosts(app: AppHandle) -> Vec<AiHostInfo> {
    ai::hosts(&app).into_iter().map(|h| AiHostInfo { has_key: ai::has_key(&app, &h), host: h }).collect()
}

#[tauri::command]
fn ai_save_host(app: AppHandle, host: ai::Host, key: Option<String>) -> CmdResult<ai::Host> {
    ai::save_host(&app, host, key).map_err(err)
}

#[tauri::command]
fn ai_delete_host(app: AppHandle, id: String) -> CmdResult<()> {
    ai::delete_host(&app, &id).map_err(err)
}

#[tauri::command]
async fn ai_models(app: AppHandle, id: String) -> CmdResult<Vec<ai::ModelInfo>> {
    ai::models(&app, &id).await.map_err(err)
}

#[tauri::command]
async fn ai_chat(app: AppHandle, req: ai::ChatRequest, on_event: tauri::ipc::Channel<ai::ChatEvent>) -> CmdResult<()> {
    ai::chat(app, req, on_event).await.map_err(err)
}

#[tauri::command]
fn ai_stop(app: AppHandle, request_id: String) {
    ai::stop(&app, &request_id)
}

/// Hides secrets in a text before the assistant stores it in its memory.
#[tauri::command]
fn ai_redact(app: AppHandle, text: String) -> String {
    ai::redact_known(&app, &text)
}

#[tauri::command]
fn agent_classify(command: String) -> agent::Verdict {
    agent::classify(&command)
}

/// Runs a command the assistant may run (the window decided with the classifier and the user's approval).
#[tauri::command]
async fn agent_exec(app: AppHandle, tab_id: String, session: store::Session, command: String, root: bool, timeout_secs: Option<u64>) -> CmdResult<agent::ExecResult> {
    let secs = timeout_secs.unwrap_or(60).clamp(5, 600);
    let res = agent::exec(&app, &tab_id, &session, &command, root, std::time::Duration::from_secs(secs)).await.map_err(err)?;
    let state = app.state::<AppState>();
    let note = format!(
        "{}{}
exit {}{}
{}",
        if root { "sudo " } else { "" },
        command,
        res.exit.map(|e| e.to_string()).unwrap_or_else(|| "unknown".into()),
        if res.timed_out { ", stopped after the time limit" } else { "" },
        res.output.chars().take(2000).collect::<String>()
    );
    // Hide secrets first: that reads the database too, which must not be locked twice.
    let note = ai::redact_known(&app, &note);
    let _ = state.store.lock().ai_log_add("this PC", "", &format!("{} on {}", session.name, session.host), "ran", &note);
    Ok(res)
}

#[tauri::command]
fn ai_log(state: State<AppState>, limit: Option<usize>) -> CmdResult<Vec<store::AiLogEntry>> {
    state.store.lock().ai_log(limit.unwrap_or(100)).map_err(err)
}

#[tauri::command]
fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
async fn update_check(app: AppHandle) -> Result<updater::UpdateInfo, String> {
    updater::check(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_install(app: AppHandle) -> Result<(), String> {
    updater::install(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn data_folder() -> String {
    store::data_dir().display().to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let store = Store::open().expect("could not open the Cablebag database");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::Pending::default())
        .manage(std::sync::Arc::new(ai::Chats::default()))
        .manage(AppState {
            store: Mutex::new(store),
            vault: vault::Vault::new(),
            prompts: prompt::Prompts::default(),
            terms: Terms::default(),
            ssh: Mutex::new(HashMap::new()),
            rdp: rdp::Rdps::default(),
            tunnels: Arc::new(tunnels::Tunnels::default()),
            scans: Arc::new(nettools::Scans::default()),
            keys: keygen::Keys::default(),
            vw: bitwarden::Vaultwarden::default(),
            vncs: Mutex::new(HashMap::new()),
            sync_lock: tokio::sync::Mutex::new(()),
            ftps: ftp::Ftps::default(),
        })
        .setup(|app| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_title("Cablebag");
                // WebView2 acts on browser shortcuts itself (Ctrl+Shift+W, Ctrl+F, Ctrl+P, Ctrl+R...) before
                // the page sees them, so Cablebag's own Ctrl+Shift+W never arrived. Off: the page gets every key.
                #[cfg(windows)]
                let _ = w.with_webview(|p| unsafe {
                    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
                    use windows::core::Interface;
                    if let Ok(settings) = p.controller().CoreWebView2().and_then(|c| c.Settings()) {
                        if let Ok(s3) = settings.cast::<ICoreWebView2Settings3>() {
                            let _ = s3.SetAreBrowserAcceleratorKeysEnabled(false);
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            ai_hosts,
            ai_save_host,
            ai_delete_host,
            ai_models,
            ai_chat,
            ai_stop,
            ai_log,
            agent_classify,
            ai_redact,
            agent_exec,
            update_check,
            update_install,
            library,
            save_session,
            delete_session,
            save_folder,
            delete_folder,
            get_settings,
            set_setting,
            local_shells,
            prompt_answer,
            term_open_local,
            term_open_ssh,
            term_input,
            term_resize,
            term_close,
            term_open_stream,
            serial_ports,
            file_open,
            browser_open,
            browser_bounds,
            browser_visible,
            browser_command,
            browser_close,
            sftp_home,
            sftp_list,
            sftp_mkdir,
            sftp_new_file,
            sftp_rename,
            sftp_remove,
            sftp_chmod,
            sftp_download,
            sftp_upload,
            sftp_fetch_temp,
            sftp_push_back,
            file_modified,
            import_sources,
            import_sessions,
            known_hosts,
            delete_known_host,
            secrets,
            delete_secret,
            reveal_secret,
            data_folder,
            vnc_open,
            vnc_input,
            vnc_close,
            files_open,
            vault_status,
            vault_configure,
            vault_unlock,
            vault_sync,
            vault_lock,
            vault_forget,
            vault_find,
            sync_now,
            sync_configure,
            sync_has_token,
            sync_has_passphrase,
            vault_items,
            tunnel_start,
            tunnel_stop,
            tunnel_statuses,
            net_local_ranges,
            net_check_range,
            net_scan,
            net_reach,
            proxmox_sources,
            proxmox_save_source,
            proxmox_delete_source,
            proxmox_list,
            proxmox_power,
            net_port_scan,
            net_stop_scan,
            net_open_ports,
            net_wake,
            key_generate,
            key_load,
            key_save,
            key_forget,
            key_default_dir,
            key_install,
            write_text_file,
            default_log_dir,
            rdp_open,
            rdp_bounds,
            rdp_visible,
            rdp_command,
            rdp_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cablebag");
}

//! SSH sessions: connection (optionally through jump hosts), host key checks, login,
//! the interactive terminal, X11 forwarding (see x11.rs), and SFTP on the same connection for the file browser.

use crate::prompt::{button, field, PromptRequest};
use crate::store::{now, KnownHost, Session};
use crate::term::{Sink, TermCmd, TermOut};
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use russh::client::{self, KeyboardInteractiveAuthResponse};
use russh::keys::{self, HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use russh::{ChannelMsg, Disconnect, MethodKind};
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

pub type Handle = client::Handle<Client>;

pub struct SshConn {
    pub handle: Arc<Handle>,
    /// Jump host connections that carry `handle`; kept alive for as long as the session runs.
    _jumps: Vec<Arc<Handle>>,
    sftp: tokio::sync::Mutex<Option<Arc<SftpSession>>>,
    /// Remote port forwards: server port -> local destination.
    pub forwards: Forwards,
    /// The X11 forwarding of the session, while it shows Linux windows on this PC.
    x11: X11Slot,
}

pub type Forwards = Arc<parking_lot::Mutex<std::collections::HashMap<u32, (String, u16)>>>;
type X11Slot = Arc<parking_lot::Mutex<Option<Arc<crate::x11::Forward>>>>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Hop {
    /// WireGuard network to reach this host through (only the first hop of a session).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub via_wg: String,
    /// Lets the server use this PC's SSH agent while connected (only the last hop of a session).
    #[serde(default)]
    pub agent_forward: bool,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub key_path: String,
    /// Vaultwarden item that holds the login for this host.
    #[serde(default)]
    pub vault_item: String,
    /// The saved session this hop belongs to, to link a login found in the vault to it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub session_id: String,
    /// The session's name, which can also find its login in the vault.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

fn default_port() -> u16 {
    22
}

pub struct Client {
    app: AppHandle,
    host: String,
    port: u16,
    forwards: Forwards,
    agent_forward: bool,
    x11: X11Slot,
}

/// One forwarded agent channel: the bytes go to the Windows OpenSSH agent or to Pageant, as switched on in
/// Settings, SSH (the OpenSSH agent first when both or neither are on).
async fn serve_agent(app: AppHandle, channel: russh::Channel<client::Msg>) {
    let mut stream = channel.into_stream();
    let pageant = settings_flag(&app, "ssh.agent.pageant", false);
    let openssh = settings_flag(&app, "ssh.agent.openssh", false);
    // shortcut: only Windows connects an agent so far; the SSH_AUTH_SOCK path will take this over.
    #[cfg(not(windows))]
    let _ = (&mut stream, pageant, openssh);
    #[cfg(windows)]
    {
        if openssh || !pageant {
            if let Ok(mut pipe) = tokio::net::windows::named_pipe::ClientOptions::new().open(r"\\.\pipe\openssh-ssh-agent") {
                let _ = tokio::io::copy_bidirectional(&mut stream, &mut pipe).await;
                return;
            }
        }
        if let Ok(mut p) = pageant::PageantStream::new().await {
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut p).await;
        }
    }
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let key = match key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(c) => PublicKey::from(c.public_key().clone()),
        };
        Ok(verify_host_key(&self.app, &self.host, self.port, &key).await)
    }

    /// The server wants to use this PC's SSH agent: only when forwarding is on for this session.
    async fn server_channel_open_agent_forward(
        &mut self,
        channel: russh::Channel<client::Msg>,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        if !self.agent_forward {
            reply.reject(russh::ChannelOpenFailure::AdministrativelyProhibited).await;
            return Ok(());
        }
        reply.accept().await;
        tokio::spawn(serve_agent(self.app.clone(), channel));
        Ok(())
    }

    /// A program on the server opens a window: only when this session asked for X11 forwarding.
    async fn server_channel_open_x11(
        &mut self,
        channel: russh::Channel<client::Msg>,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let fwd = self.x11.lock().clone();
        let Some(fwd) = fwd else {
            reply.reject(russh::ChannelOpenFailure::AdministrativelyProhibited).await;
            return Ok(());
        };
        reply.accept().await;
        tokio::spawn(fwd.serve(channel));
        Ok(())
    }

    /// A connection arrived on a port the server listens on for us (remote port forwarding).
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<client::Msg>,
        _connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let target = self.forwards.lock().get(&connected_port).cloned();
        let Some((host, port)) = target else {
            reply.reject(russh::ChannelOpenFailure::AdministrativelyProhibited).await;
            return Ok(());
        };
        reply.accept().await;
        tokio::spawn(async move {
            if let Ok(mut sock) = tokio::net::TcpStream::connect((host.as_str(), port)).await {
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut sock, &mut stream).await;
            }
        });
        Ok(())
    }
}

pub fn md5_fingerprint(key: &PublicKey) -> String {
    use md5::{Digest, Md5};
    let bytes = key.to_bytes().unwrap_or_default();
    let digest = Md5::digest(&bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":")
}

async fn verify_host_key(app: &AppHandle, host: &str, port: u16, key: &PublicKey) -> bool {
    let state = app.state::<AppState>();
    let algo = key.algorithm().as_str().to_string();
    let openssh = key.to_openssh().unwrap_or_default();
    let sha256 = key.fingerprint(HashAlg::Sha256).to_string();
    let known = state.store.lock().known_hosts_for(host, port as i64).unwrap_or_default();

    if known.iter().any(|k| k.algo == algo && k.key == openssh) {
        return true;
    }
    let changed = known.iter().find(|k| k.algo == algo);
    let (title, message, warning) = match changed {
        Some(old) => (
            "The server's identity has changed".to_string(),
            format!(
                "The host key of {host}:{port} is different from the one saved on {}. \
                 This happens after a server reinstall, but it can also mean someone is intercepting the connection. \
                 Only continue if you know why the key changed.\n\nPreviously saved: {}",
                fmt_date(old.added),
                old.fingerprint
            ),
            true,
        ),
        None => (
            "Confirm the server's identity".to_string(),
            format!("This is the first connection to {host}:{port}. Check the fingerprint below before trusting this server."),
            false,
        ),
    };
    let ans = state
        .prompts
        .ask(
            app,
            PromptRequest {
                id: String::new(),
                title,
                message,
                details: vec![
                    ("Key type".into(), algo.clone()),
                    ("SHA-256 fingerprint".into(), sha256.clone()),
                    ("MD5 fingerprint".into(), md5_fingerprint(key)),
                ],
                fields: vec![],
                buttons: vec![
                    button("trust", if warning { "Replace the saved key and connect" } else { "Trust and remember" }, if warning { "danger" } else { "primary" }),
                    button("once", "Connect this time only", "normal"),
                    button("cancel", "Cancel", "normal"),
                ],
                warning,
            },
        )
        .await;
    match ans.button.as_str() {
        "trust" => {
            let _ = state.store.lock().save_known_host(&KnownHost {
                host: host.into(),
                port: port as i64,
                algo,
                key: openssh,
                fingerprint: sha256,
                added: now(),
            });
            true
        }
        "once" => true,
        _ => false,
    }
}

fn fmt_date(ts: i64) -> String {
    let days = ts / 86400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y}-{m:02}-{d:02}")
}

/// Expands the _ProfileDir_ and _MyDocuments_ placeholders some session files use, plus %VARIABLES% and a
/// leading ~.
pub fn expand_path(p: &str) -> String {
    let home = dirs::home_dir().map(|h| h.to_string_lossy().to_string()).unwrap_or_default();
    let docs = dirs::document_dir().map(|h| h.to_string_lossy().to_string()).unwrap_or_default();
    let mut s = p.replace("_ProfileDir_", &home).replace("_MyDocuments_", &docs).replace("_CurrentDrive_", "C:");
    if let Some(rest) = s.strip_prefix('~') {
        s = format!("{home}{rest}");
    }
    while let Some(start) = s.find('%') {
        let Some(len) = s[start + 1..].find('%') else { break };
        let name = &s[start + 1..start + 1 + len];
        match std::env::var(name) {
            Ok(v) => s = format!("{}{}{}", &s[..start], v, &s[start + 2 + len..]),
            Err(_) => break,
        }
    }
    s
}

fn settings_flag(app: &AppHandle, key: &str, default: bool) -> bool {
    app.state::<AppState>()
        .store
        .lock()
        .setting(key)
        .ok()
        .flatten()
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

async fn try_agent<S>(h: &mut Handle, user: &str, agent: &mut keys::agent::client::AgentClient<S>) -> bool
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let Ok(ids) = agent.request_identities().await else { return false };
    let hash = h.best_supported_rsa_hash().await.ok().flatten().flatten();
    for id in ids {
        let key = id.public_key().into_owned();
        if let Ok(r) = h.authenticate_publickey_with(user, key, hash, agent).await {
            if r.success() {
                return true;
            }
        }
    }
    false
}

fn cancelled() -> anyhow::Error {
    anyhow!("Login cancelled.")
}

async fn authenticate(h: &mut Handle, hop: &Hop, app: &AppHandle, out: &impl Sink) -> Result<()> {
    let state = app.state::<AppState>();
    let host = hop.host.clone();
    let port = hop.port as i64;

    // A linked vault item provides the password, and the user name when the session has none.
    let mut vault_password: Option<String> = None;
    let mut user = hop.user.trim().to_string();
    if !hop.vault_item.trim().is_empty() {
        match state.vw.login(app, hop.vault_item.trim()).await {
            Ok((vault_user, pw)) => {
                if user.is_empty() {
                    user = vault_user;
                }
                if !pw.is_empty() {
                    vault_password = Some(pw);
                }
                out.info("Using the login from the vault.");
            }
            Err(e) => out.error(&format!("Vault: {e}")),
        }
    }
    if user == "<default>" {
        user = std::env::var("USERNAME").unwrap_or_default();
    }
    // No linked login: the vault may still have one for this host.
    let mut found: Option<(String, String)> = None;
    let mut searched = !hop.vault_item.trim().is_empty();
    if user.is_empty() && !searched {
        searched = true;
        if let Some((item, name, u, pw)) = state.vw.pick_login(app, "ssh", &host, "", &hop.name).await? {
            user = u;
            if !pw.is_empty() {
                vault_password = Some(pw);
            }
            out.info(&format!("Using the vault login \"{name}\"."));
            found = Some((item, name));
        }
    }
    if user.is_empty() {
        let ans = state
            .prompts
            .ask(
                app,
                PromptRequest {
                    id: String::new(),
                    title: format!("Log in to {host}"),
                    message: "Which user name should be used?".into(),
                    details: vec![],
                    fields: vec![field("user", "User name", "text", Value::String(String::new()))],
                    buttons: vec![button("ok", "Continue", "primary"), button("cancel", "Cancel", "normal")],
                    warning: false,
                },
            )
            .await;
        if ans.button != "ok" || ans.text("user").is_empty() {
            return Err(cancelled());
        }
        user = ans.text("user");
    }

    let first = h.authenticate_none(&user).await?;
    let methods: Vec<MethodKind> = match first {
        russh::client::AuthResult::Success => return Ok(()),
        russh::client::AuthResult::Failure { remaining_methods, .. } => remaining_methods.iter().cloned().collect(),
    };
    let offers = |m: MethodKind| methods.is_empty() || methods.contains(&m);

    // 1. SSH agents (Pageant and the Windows OpenSSH agent), when switched on in the settings.
    if offers(MethodKind::PublicKey) {
        #[cfg(windows)]
        {
            if settings_flag(app, "ssh.agent.pageant", false) {
                if let Ok(mut agent) = keys::agent::client::AgentClient::connect_pageant().await {
                    if try_agent(h, &user, &mut agent).await {
                        out.info("Logged in with a key from Pageant.");
                        return Ok(());
                    }
                }
            }
            if settings_flag(app, "ssh.agent.openssh", false) {
                if let Ok(mut agent) =
                    keys::agent::client::AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await
                {
                    if try_agent(h, &user, &mut agent).await {
                        out.info("Logged in with a key from the Windows OpenSSH agent.");
                        return Ok(());
                    }
                }
            }
        }
    }

    // 2. Private key file.
    if !hop.key_path.trim().is_empty() && offers(MethodKind::PublicKey) {
        let path = expand_path(hop.key_path.trim());
        let key = match keys::load_secret_key(&path, None) {
            Ok(k) => Some(k),
            Err(keys::Error::KeyIsEncrypted) => {
                let saved = { state.vault.passphrase(&state.store.lock(), &path).ok().flatten() };
                let mut loaded = saved.and_then(|p| keys::load_secret_key(&path, Some(&p)).ok());
                let mut tries = 0;
                while loaded.is_none() && tries < 3 {
                    tries += 1;
                    let Some((pass, save)) = state
                        .prompts
                        .ask_secret(app, "Key passphrase", &format!("The key {path} is protected with a passphrase."), "Passphrase", true)
                        .await
                    else {
                        break;
                    };
                    match keys::load_secret_key(&path, Some(&pass)) {
                        Ok(k) => {
                            if save {
                                let _ = state.vault.save_passphrase(&state.store.lock(), &path, &pass);
                            }
                            loaded = Some(k);
                        }
                        Err(_) => out.error("Wrong passphrase."),
                    }
                }
                loaded
            }
            Err(e) => {
                out.error(&format!("Could not read the key file {path}: {e}"));
                None
            }
        };
        if let Some(k) = key {
            let hash = h.best_supported_rsa_hash().await?.flatten();
            let r = h.authenticate_publickey(&user, PrivateKeyWithHashAlg::new(Arc::new(k), hash)).await?;
            if r.success() {
                return Ok(());
            }
            out.error("The server did not accept the key.");
        }
    }

    // 3. Saved password (linked vault login, Cablebag's own, or one found in the vault), then
    //    keyboard-interactive, then typed password.
    let mut saved = match vault_password {
        Some(pw) => Some(pw),
        None => state.vault.password(&state.store.lock(), "ssh", &host, port, &user).ok().flatten(),
    };
    if saved.is_none() && !searched {
        if let Some((item, name, _, pw)) = state.vw.pick_login(app, "ssh", &host, &user, &hop.name).await? {
            if !pw.is_empty() {
                out.info(&format!("Using the vault login \"{name}\"."));
                saved = Some(pw);
                found = Some((item, name));
            }
        }
    }
    // A login found in the vault that works gets linked to the session.
    let link = |found: &Option<(String, String)>| {
        if let Some((item, _)) = found {
            crate::bitwarden::link_session(app, &hop.session_id, item);
        }
    };
    if let Some(pw) = &saved {
        if offers(MethodKind::Password) {
            if h.authenticate_password(&user, pw.clone()).await?.success() {
                link(&found);
                return Ok(());
            }
            // Say why a password is asked after all.
            match &found {
                Some((_, name)) => out.error(&format!("The server did not accept the password from the vault login \"{name}\".")),
                None => out.error("The server did not accept the saved password."),
            }
        }
    }

    if offers(MethodKind::KeyboardInteractive) {
        let mut used_saved = false;
        let mut resp = h.authenticate_keyboard_interactive_start(&user, None::<String>).await?;
        loop {
            match resp {
                KeyboardInteractiveAuthResponse::Success => {
                    if used_saved {
                        link(&found);
                    }
                    return Ok(());
                }
                KeyboardInteractiveAuthResponse::Failure { .. } => break,
                KeyboardInteractiveAuthResponse::InfoRequest { name, instructions, prompts } => {
                    if prompts.is_empty() {
                        resp = h.authenticate_keyboard_interactive_respond(vec![]).await?;
                        continue;
                    }
                    let single_pw = prompts.len() == 1
                        && !prompts[0].echo
                        && prompts[0].prompt.to_lowercase().contains("password");
                    let answers = if single_pw && saved.is_some() && !used_saved {
                        used_saved = true;
                        vec![saved.clone().unwrap_or_default()]
                    } else {
                        let mut fields: Vec<_> = prompts
                            .iter()
                            .enumerate()
                            .map(|(i, p)| {
                                field(
                                    &format!("p{i}"),
                                    p.prompt.trim().trim_end_matches(':'),
                                    if p.echo { "text" } else { "password" },
                                    Value::String(String::new()),
                                )
                            })
                            .collect();
                        if single_pw {
                            fields.push(field("save", "Remember in the password vault", "checkbox", Value::Bool(false)));
                        }
                        let msg = [name.as_str(), instructions.as_str()]
                            .iter()
                            .filter(|s| !s.trim().is_empty())
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("\n");
                        let ans = state
                            .prompts
                            .ask(
                                app,
                                PromptRequest {
                                    id: String::new(),
                                    title: format!("Log in to {user}@{host}"),
                                    message: msg,
                                    details: vec![],
                                    fields,
                                    buttons: vec![button("ok", "Continue", "primary"), button("cancel", "Cancel", "normal")],
                                    warning: false,
                                },
                            )
                            .await;
                        if ans.button != "ok" {
                            return Err(cancelled());
                        }
                        let a: Vec<String> = (0..prompts.len()).map(|i| ans.text(&format!("p{i}"))).collect();
                        if single_pw && ans.flag("save") {
                            let _ = state.vault.save_password(&state.store.lock(), "ssh", &host, port, &user, &a[0]);
                        }
                        a
                    };
                    resp = h.authenticate_keyboard_interactive_respond(answers).await?;
                }
            }
        }
    }

    if offers(MethodKind::Password) {
        for _ in 0..3 {
            let Some((pw, save)) = state
                .prompts
                .ask_secret(app, &format!("Log in to {host}"), &format!("Password for {user}@{host}"), "Password", true)
                .await
            else {
                return Err(cancelled());
            };
            if h.authenticate_password(&user, pw.clone()).await?.success() {
                if save {
                    let _ = state.vault.save_password(&state.store.lock(), "ssh", &host, port, &user, &pw);
                }
                return Ok(());
            }
            out.error("Access denied.");
        }
    }
    bail!("Login failed for {user}@{host}.")
}

pub async fn connect_chain(app: &AppHandle, hops: &[Hop], keepalive: bool, out: &impl Sink) -> Result<SshConn> {
    let config = Arc::new(client::Config {
        keepalive_interval: if keepalive { Some(Duration::from_secs(60)) } else { None },
        keepalive_max: 5,
        nodelay: true,
        ..Default::default()
    });
    let forwards: Forwards = Default::default();
    let mut prev: Option<Arc<Handle>> = None;
    let mut jumps = Vec::new();
    // Only the server itself (the last hop) gets to open X11 channels.
    let mut x11 = X11Slot::default();
    for (i, hop) in hops.iter().enumerate() {
        let via = if i + 1 < hops.len() { " (jump host)" } else { "" };
        if i == 0 && !hop.via_wg.is_empty() {
            let t = crate::wg::tunnel(app, &hop.via_wg).await?;
            out.info(&format!("Through the WireGuard network {}.", t.name()));
            if !t.warning().is_empty() {
                out.error(&t.warning());
            }
        }
        out.info(&format!("Connecting to {}:{}{}...", hop.host, hop.port, via));
        x11 = X11Slot::default();
        let handler = Client {
            app: app.clone(),
            host: hop.host.clone(),
            port: hop.port,
            forwards: forwards.clone(),
            agent_forward: hop.agent_forward,
            x11: x11.clone(),
        };
        let fut = async {
            match &prev {
                None if !hop.via_wg.is_empty() => {
                    let stream = crate::wg::connect(app, &hop.via_wg, &hop.host, hop.port).await.map_err(|e| russh::Error::IO(std::io::Error::other(e.to_string())))?;
                    client::connect_stream(config.clone(), stream, handler).await
                }
                None => client::connect(config.clone(), (hop.host.as_str(), hop.port), handler).await,
                Some(p) => {
                    let ch = p.channel_open_direct_tcpip(hop.host.clone(), hop.port as u32, "127.0.0.1", 0).await?;
                    client::connect_stream(config.clone(), ch.into_stream(), handler).await
                }
            }
        };
        let mut h = match tokio::time::timeout(Duration::from_secs(20), fut).await {
            Err(_) => bail!("No answer from {}:{} within 20 seconds.", hop.host, hop.port),
            Ok(Err(russh::Error::UnknownKey)) => bail!("The server's host key was not accepted."),
            Ok(Err(e)) => bail!("Could not connect to {}:{}: {e}", hop.host, hop.port),
            Ok(Ok(h)) => h,
        };
        authenticate(&mut h, hop, app, out).await?;
        if let Some(p) = prev.take() {
            jumps.push(p);
        }
        prev = Some(Arc::new(h));
    }
    let handle = prev.ok_or_else(|| anyhow!("no host to connect to"))?;
    Ok(SshConn { handle, _jumps: jumps, sftp: tokio::sync::Mutex::new(None), forwards, x11 })
}

fn str_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn bool_of(v: &Value, k: &str, default: bool) -> bool {
    v.get(k).and_then(|x| x.as_bool()).unwrap_or(default)
}

/// The full list of hops for a session: jump hosts first, then the server itself.
pub fn hops_for(s: &Session) -> Vec<Hop> {
    let mut hops: Vec<Hop> = s
        .data
        .get("jump_hosts")
        .and_then(|j| serde_json::from_value::<Vec<Hop>>(j.clone()).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|h| !h.host.trim().is_empty())
        .collect();
    hops.push(Hop {
        via_wg: String::new(),
        agent_forward: bool_of(&s.data, "agent_forward", false),
        host: s.host.trim().to_string(),
        port: if s.port > 0 { s.port as u16 } else { 22 },
        user: s.username.clone(),
        key_path: str_of(&s.data, "key_path"),
        vault_item: str_of(&s.data, "vault_item"),
        session_id: s.id.clone(),
        name: s.name.clone(),
    });
    if let (Some(first), Some(wg)) = (hops.first_mut(), crate::wg::of(s)) {
        first.via_wg = wg;
    }
    hops
}

pub async fn run_terminal(
    app: AppHandle,
    id: String,
    session: Session,
    cols: u16,
    rows: u16,
    out: TermOut,
    mut rx: mpsc::UnboundedReceiver<TermCmd>,
) {
    out.status("connecting", "");
    let hops = hops_for(&session);
    let keepalive = bool_of(&session.data, "keepalive", true);
    let conn = match connect_chain(&app, &hops, keepalive, &out).await {
        Ok(c) => Arc::new(c),
        Err(e) => {
            out.error(&e.to_string());
            out.status("failed", &e.to_string());
            return;
        }
    };
    let state = app.state::<AppState>();
    state.ssh.lock().insert(id.clone(), conn.clone());
    let x11 = bool_of(&session.data, "x11", false).then(|| crate::x11::Forward::new(app.clone(), out.clone()));

    let result: Result<()> = async {
        let mut channel = conn.handle.channel_open_session().await?;
        let term_type = {
            let t = str_of(&session.data, "term_type");
            if t.is_empty() { "xterm-256color".to_string() } else { t }
        };
        if bool_of(&session.data, "agent_forward", false) {
            channel.agent_forward(false).await?;
        }
        if let Some(fwd) = &x11 {
            request_x11(&mut channel, &conn, fwd, &out).await?;
        }
        channel.request_pty(false, &term_type, cols as u32, rows as u32, 0, 0, &[]).await?;
        channel.request_shell(true).await?;
        out.status("connected", "");
        let command = str_of(&session.data, "command");
        if !command.trim().is_empty() {
            let stay = bool_of(&session.data, "stay_after_command", true);
            let line = if stay { format!("{command}\n") } else { format!("{command}; exit\n") };
            channel.data(line.as_bytes()).await?;
        }
        loop {
            tokio::select! {
                cmd = rx.recv() => match cmd {
                    Some(TermCmd::Input(b)) => channel.data(&b[..]).await?,
                    Some(TermCmd::Resize(c, r)) => channel.window_change(c as u32, r as u32, 0, 0).await?,
                    Some(TermCmd::Close) | None => {
                        let _ = channel.close().await;
                        break;
                    }
                },
                msg = channel.wait() => match msg {
                    Some(ChannelMsg::Data { data }) => out.data(&data),
                    Some(ChannelMsg::ExtendedData { data, .. }) => out.data(&data),
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        if exit_status != 0 { out.info(&format!("\r\nRemote shell ended with exit code {exit_status}.")); }
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) => break,
                    // The connection itself went away (sleep, network, VPN): not an ordinary end.
                    None => anyhow::bail!("The connection to the server was lost."),
                    _ => {}
                }
            }
        }
        Ok(())
    }
    .await;

    state.ssh.lock().remove(&id);
    conn.x11.lock().take();
    if let Some(fwd) = &x11 {
        fwd.release().await;
    }
    let _ = conn.handle.disconnect(Disconnect::ByApplication, "", "en").await;
    match result {
        Ok(()) => {
            out.info("\r\nSession closed.");
            out.status("closed", "");
        }
        Err(e) => {
            out.error(&format!("Connection lost: {e}"));
            out.status("closed", &e.to_string());
        }
    }
}

/// Asks the server to forward X11 for this session, and waits for its answer: a server that does not allow it
/// leaves the session as it is, with a line saying why.
async fn request_x11(channel: &mut russh::Channel<client::Msg>, conn: &SshConn, fwd: &Arc<crate::x11::Forward>, out: &TermOut) -> Result<()> {
    *conn.x11.lock() = Some(fwd.clone());
    channel.request_x11(true, false, crate::x11::PROTOCOL, fwd.fake_cookie(), 0).await?;
    let answer = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Success) => return Some(true),
                Some(ChannelMsg::Failure) => return Some(false),
                None => return None,
                _ => {}
            }
        }
    })
    .await;
    match answer {
        Ok(Some(true)) => {}
        Ok(Some(false)) | Err(_) => {
            conn.x11.lock().take();
            out.info("This server does not allow showing Linux windows on this PC (X11Forwarding is off in its SSH server settings).");
        }
        Ok(None) => bail!("The connection to the server was lost."),
    }
    Ok(())
}

struct StatusSink {
    out: TermOut,
}

impl Sink for StatusSink {
    fn info(&self, text: &str) {
        self.out.status("connecting", text);
    }
    fn error(&self, text: &str) {
        self.out.status("connecting", text);
    }
}

/// An SFTP session: an SSH connection used only for the file browser, without a shell.
pub async fn run_files_only(app: AppHandle, id: String, session: Session, out: TermOut, mut rx: mpsc::UnboundedReceiver<TermCmd>) {
    out.status("connecting", "");
    let sink = StatusSink { out: out.clone() };
    let conn = match connect_chain(&app, &hops_for(&session), true, &sink).await {
        Ok(c) => Arc::new(c),
        Err(e) => {
            out.status("failed", &e.to_string());
            return;
        }
    };
    let state = app.state::<AppState>();
    state.ssh.lock().insert(id.clone(), conn.clone());
    out.status("connected", "");
    loop {
        tokio::select! {
            cmd = rx.recv() => if matches!(cmd, Some(TermCmd::Close) | None) { break },
            _ = tokio::time::sleep(Duration::from_secs(2)) => if conn.handle.is_closed() {
                state.ssh.lock().remove(&id);
                out.status("closed", "The connection was lost.");
                return;
            }
        }
    }
    state.ssh.lock().remove(&id);
    let _ = conn.handle.disconnect(Disconnect::ByApplication, "", "en").await;
    out.status("closed", "");
}

/// The SFTP session of an open SSH tab, opened on first use.
pub async fn sftp(app: &AppHandle, id: &str) -> Result<Arc<SftpSession>> {
    let conn = app
        .state::<AppState>()
        .ssh
        .lock()
        .get(id)
        .cloned()
        .ok_or_else(|| anyhow!("This tab has no open SSH connection."))?;
    let mut guard = conn.sftp.lock().await;
    if let Some(s) = guard.as_ref() {
        return Ok(s.clone());
    }
    let channel = conn.handle.channel_open_session().await?;
    channel.request_subsystem(true, "sftp").await?;
    let s = Arc::new(SftpSession::new(channel.into_stream()).await?);
    *guard = Some(s.clone());
    Ok(s)
}

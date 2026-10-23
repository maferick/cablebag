//! FTP sessions: the same file browser as SFTP, over FTP or FTPS (TLS from the start, or after AUTH TLS).
//! One connection per tab; its commands run one after the other.

use crate::files::{join, Entry, Listing};
use crate::store::Session;
use crate::term::{TermCmd, TermOut};
use crate::AppState;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use suppaftp::list::{File, PosixPexQuery};
use suppaftp::tokio::{AsyncFtpStream, AsyncRustlsConnector, AsyncRustlsFtpStream};
use suppaftp::types::FileType;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

/// A connection, plain or with TLS.
enum Conn {
    Plain(AsyncFtpStream),
    Tls(AsyncRustlsFtpStream),
}

/// Calls the same method on either kind of connection.
macro_rules! on {
    ($c:expr, $s:ident => $body:expr) => {
        match $c {
            Conn::Plain($s) => $body,
            Conn::Tls($s) => $body,
        }
    };
}

#[derive(Default)]
pub struct Ftps {
    map: Mutex<HashMap<String, Arc<tokio::sync::Mutex<Conn>>>>,
}

impl Ftps {
    pub fn has(&self, tab: &str) -> bool {
        self.map.lock().contains_key(tab)
    }
    fn get(&self, tab: &str) -> Result<Arc<tokio::sync::Mutex<Conn>>> {
        self.map.lock().get(tab).cloned().ok_or_else(|| anyhow!("This FTP tab is not connected."))
    }
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

#[derive(Clone, Serialize)]
struct Progress<'a> {
    tab: &'a str,
    name: &'a str,
    done: u64,
    total: u64,
    finished: bool,
}

/// The password: from the linked vault login, Cablebag's saved one, a vault login found for the host, or asked.
async fn password(app: &AppHandle, session: &Session, user: &mut String) -> Result<String> {
    let state = app.state::<AppState>();
    let item = s(&session.data, "vault_item");
    if !item.is_empty() {
        let (u, pw) = state.creds.login(app, &item).await?;
        if user.is_empty() {
            *user = u;
        }
        return Ok(pw);
    }
    if user.is_empty() || user == "anonymous" {
        *user = if user.is_empty() { "anonymous".into() } else { user.clone() };
        return Ok("anonymous@".into());
    }
    let saved = { state.vault.password(&state.store.lock(), "ftp", &session.host, session.port, user).ok().flatten() };
    if let Some(pw) = saved {
        return Ok(pw);
    }
    if let Some((found, _, _, pw)) = state.creds.pick_login(app, "ftp", &session.host, user, &session.name).await? {
        crate::sources::link_session(app, &session.id, &found);
        return Ok(pw);
    }
    let (pw, save) = state
        .prompts
        .ask_secret(app, &format!("Log in to {}", session.host), &format!("Password for {user} on {}", session.host), "Password", true)
        .await
        .ok_or_else(|| anyhow!("Login cancelled."))?;
    if save {
        let _ = state.vault.save_password(&state.store.lock(), "ftp", &session.host, session.port, user, &pw);
    }
    Ok(pw)
}

async fn connect(app: &AppHandle, session: &Session) -> Result<Conn> {
    let host = session.host.trim().to_string();
    let tls = s(&session.data, "tls");
    let port = if session.port > 0 { session.port as u16 } else if tls == "implicit" { 990 } else { 21 };
    let addr = format!("{host}:{port}");
    let mut user = session.username.trim().to_string();
    let pw = password(app, session, &mut user).await?;
    let any_cert = session.data.get("accept_any_cert").and_then(|v| v.as_bool()).unwrap_or(false);
    // One TLS setup for the login connection and every file transfer: servers such as vsftpd only accept a
    // transfer that reuses the login's TLS session, and rustls keeps that session to reuse.
    let tls_config = std::sync::Arc::new(tls_config(any_cert)?);
    let connector = || -> AsyncRustlsConnector { AsyncRustlsConnector::from(tokio_rustls::TlsConnector::from(tls_config.clone())) };
    let timeout = Duration::from_secs(15);
    let mut conn = match tls.as_str() {
        "explicit" => {
            let plain = tokio::time::timeout(timeout, AsyncRustlsFtpStream::connect(&addr)).await.map_err(|_| anyhow!("No answer from {addr}."))??;
            Conn::Tls(plain.into_secure(connector(), &host).await.map_err(|e| anyhow!("TLS with {host} failed: {e}"))?)
        }
        "implicit" => {
            #[allow(deprecated)]
            let c = tokio::time::timeout(timeout, AsyncRustlsFtpStream::connect_secure_implicit(&addr, connector(), &host))
                .await
                .map_err(|_| anyhow!("No answer from {addr}."))??;
            Conn::Tls(c)
        }
        _ => Conn::Plain(tokio::time::timeout(timeout, AsyncFtpStream::connect(&addr)).await.map_err(|_| anyhow!("No answer from {addr}."))??),
    };
    on!(&mut conn, c => {
        c.login(&user, &pw).await.map_err(|e| anyhow!("The FTP server refused the login for {user}: {e}"))?;
        c.transfer_type(FileType::Binary).await?;
    });
    Ok(conn)
}

/// Certificates a public authority vouches for, or (when the session allows it) any certificate.
fn tls_config(any_cert: bool) -> Result<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone()).with_safe_default_protocol_versions()?;
    Ok(if any_cert {
        builder.dangerous().with_custom_certificate_verifier(Arc::new(AnyCert(provider))).with_no_client_auth()
    } else {
        let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        builder.with_root_certificates(roots).with_no_client_auth()
    })
}

/// For servers with a self-signed certificate, when the session says so: the connection is still
/// encrypted, but who is on the other side is not checked.
#[derive(Debug)]
struct AnyCert(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for AnyCert {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, m: &[u8], c: &rustls::pki_types::CertificateDer<'_>, d: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(m, c, d, &self.0.signature_verification_algorithms)
    }
    fn verify_tls13_signature(&self, m: &[u8], c: &rustls::pki_types::CertificateDer<'_>, d: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(m, c, d, &self.0.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Runs an FTP tab: connects, then keeps the connection until the tab closes.
pub async fn run(app: AppHandle, id: String, session: Session, out: TermOut, mut rx: mpsc::UnboundedReceiver<TermCmd>) {
    out.status("connecting", "");
    let conn = match connect(&app, &session).await {
        Ok(c) => Arc::new(tokio::sync::Mutex::new(c)),
        Err(e) => {
            out.status("failed", &e.to_string());
            return;
        }
    };
    let state = app.state::<AppState>();
    state.ftps.map.lock().insert(id.clone(), conn.clone());
    out.status("connected", "");
    loop {
        tokio::select! {
            cmd = rx.recv() => if matches!(cmd, Some(TermCmd::Close) | None) { break },
            // Servers drop idle connections; a NOOP now and then keeps it, and notices a lost one.
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                let mut c = conn.lock().await;
                if on!(&mut *c, f => f.noop().await).is_err() {
                    state.ftps.map.lock().remove(&id);
                    out.status("closed", "The connection was lost.");
                    return;
                }
            }
        }
    }
    state.ftps.map.lock().remove(&id);
    let mut c = conn.lock().await;
    let _ = on!(&mut *c, f => f.quit().await);
    out.status("closed", "");
}

fn entry(dir: &str, f: &File) -> Entry {
    let bits = |q: PosixPexQuery, shift: u32| -> u32 { ((f.can_read(q) as u32) << 2 | (f.can_write(q) as u32) << 1 | f.can_execute(q) as u32) << shift };
    let mode = bits(PosixPexQuery::Owner, 6) | bits(PosixPexQuery::Group, 3) | bits(PosixPexQuery::Others, 0);
    let is_dir = f.is_directory();
    let is_link = f.is_symlink();
    let r = |b: u32, c: char| if mode & b != 0 { c } else { '-' };
    let permissions = format!(
        "{}{}{}{}{}{}{}{}{}{}",
        if is_dir { 'd' } else if is_link { 'l' } else { '-' },
        r(0o400, 'r'), r(0o200, 'w'), r(0o100, 'x'), r(0o040, 'r'), r(0o020, 'w'), r(0o010, 'x'), r(0o004, 'r'), r(0o002, 'w'), r(0o001, 'x')
    );
    Entry {
        name: f.name().to_string(),
        path: join(dir, f.name()),
        is_dir,
        is_link,
        size: f.size() as u64,
        modified: f.modified().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        mode,
        permissions,
        owner: String::new(),
        group: String::new(),
    }
}

pub async fn home(app: &AppHandle, tab: &str) -> Result<String> {
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    Ok(on!(&mut *c, f => f.pwd().await)?)
}

pub async fn list(app: &AppHandle, tab: &str, path: &str) -> Result<Listing> {
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    let path = if path.is_empty() { on!(&mut *c, f => f.pwd().await)? } else { path.to_string() };
    // MLSD gives exact names, sizes and dates; LIST is the fallback for older servers.
    let mut entries: Vec<Entry> = match on!(&mut *c, f => f.mlsd(Some(&path)).await) {
        Ok(lines) => lines.iter().filter_map(|l| suppaftp::list::ListParser::parse_mlsd(l).ok()).map(|f| entry(&path, &f)).collect(),
        Err(_) => on!(&mut *c, f => f.list(Some(&path)).await)?
            .iter()
            .filter_map(|l| l.parse::<File>().ok())
            .map(|f| entry(&path, &f))
            .collect(),
    };
    entries.retain(|e| e.name != "." && e.name != "..");
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(Listing { path, entries })
}

pub async fn mkdir(app: &AppHandle, tab: &str, path: &str) -> Result<()> {
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    Ok(on!(&mut *c, f => f.mkdir(path).await)?)
}

pub async fn new_file(app: &AppHandle, tab: &str, path: &str) -> Result<()> {
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    let mut empty: &[u8] = &[];
    on!(&mut *c, f => f.put_file(path, &mut empty).await)?;
    Ok(())
}

pub async fn rename(app: &AppHandle, tab: &str, from: &str, to: &str) -> Result<()> {
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    Ok(on!(&mut *c, f => f.rename(from, to).await)?)
}

pub async fn chmod(app: &AppHandle, tab: &str, path: &str, mode: u32) -> Result<()> {
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    on!(&mut *c, f => f.site(format!("CHMOD {:o} {path}", mode & 0o7777)).await).map_err(|e| anyhow!("The server does not allow changing permissions: {e}"))?;
    Ok(())
}

pub async fn remove(app: &AppHandle, tab: &str, path: &str) -> Result<()> {
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    remove_inner(&mut c, path).await
}

fn remove_inner<'a>(c: &'a mut Conn, path: &'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        if on!(&mut *c, f => f.rm(path).await).is_ok() {
            return Ok(());
        }
        // A folder: its contents first.
        let lines = match on!(&mut *c, f => f.mlsd(Some(path)).await) {
            Ok(l) => l.iter().filter_map(|l| suppaftp::list::ListParser::parse_mlsd(l).ok()).collect::<Vec<File>>(),
            Err(_) => on!(&mut *c, f => f.list(Some(path)).await)?.iter().filter_map(|l| l.parse::<File>().ok()).collect(),
        };
        for f in lines.iter().filter(|f| f.name() != "." && f.name() != "..") {
            remove_inner(c, &join(path, f.name())).await?;
        }
        on!(&mut *c, f => f.rmdir(path).await)?;
        Ok(())
    })
}

async fn download_file(app: &AppHandle, tab: &str, c: &mut Conn, remote: &str, local: &Path) -> Result<()> {
    let name = remote.rsplit('/').next().unwrap_or(remote).to_string();
    let total = on!(&mut *c, f => f.size(remote).await).unwrap_or(0) as u64;
    let mut dst = tokio::fs::File::create(local).await?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last = std::time::Instant::now();
    on!(&mut *c, f => {
        let mut src = f.retr_as_stream(remote).await?;
        loop {
            let n = src.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n]).await?;
            done += n as u64;
            if last.elapsed().as_millis() > 200 {
                last = std::time::Instant::now();
                let _ = app.emit("transfer", Progress { tab, name: &name, done, total, finished: false });
            }
        }
        src.finish().await?;
    });
    dst.flush().await?;
    let _ = app.emit("transfer", Progress { tab, name: &name, done, total, finished: true });
    Ok(())
}

fn download_inner<'a>(app: &'a AppHandle, tab: &'a str, c: &'a mut Conn, remote: &'a str, local_dir: &'a Path) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let name = remote.trim_end_matches('/').rsplit('/').next().unwrap_or(remote).to_string();
        let as_dir = match on!(&mut *c, f => f.mlsd(Some(remote)).await) {
            Ok(l) => Some(l.iter().filter_map(|l| suppaftp::list::ListParser::parse_mlsd(l).ok()).collect::<Vec<File>>()),
            Err(_) => None,
        };
        // SIZE answers for files only; a listing that works on a non-file means a folder.
        let is_file = on!(&mut *c, f => f.size(remote).await).is_ok();
        match as_dir.filter(|_| !is_file) {
            Some(children) => {
                let dir = local_dir.join(&name);
                tokio::fs::create_dir_all(&dir).await?;
                for f in children.iter().filter(|f| f.name() != "." && f.name() != "..") {
                    download_inner(app, tab, c, &join(remote, f.name()), &dir).await?;
                }
                Ok(())
            }
            None => download_file(app, tab, c, remote, &local_dir.join(&name)).await,
        }
    })
}

pub async fn download(app: &AppHandle, tab: &str, remotes: &[String], local_dir: &str) -> Result<()> {
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    for r in remotes {
        download_inner(app, tab, &mut c, r, Path::new(local_dir)).await?;
    }
    Ok(())
}

async fn upload_file(app: &AppHandle, tab: &str, c: &mut Conn, local: &Path, remote: &str) -> Result<()> {
    let name = local.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let total = tokio::fs::metadata(local).await.map(|m| m.len()).unwrap_or(0);
    let mut src = tokio::fs::File::open(local).await?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last = std::time::Instant::now();
    on!(&mut *c, f => {
        let mut dst = f.put_with_stream(remote).await?;
        loop {
            let n = src.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n]).await?;
            done += n as u64;
            if last.elapsed().as_millis() > 200 {
                last = std::time::Instant::now();
                let _ = app.emit("transfer", Progress { tab, name: &name, done, total, finished: false });
            }
        }
        dst.finish().await?;
    });
    let _ = app.emit("transfer", Progress { tab, name: &name, done, total, finished: true });
    Ok(())
}

fn upload_inner<'a>(app: &'a AppHandle, tab: &'a str, c: &'a mut Conn, local: &'a Path, remote_dir: &'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let name = local.file_name().map(|n| n.to_string_lossy().to_string()).ok_or_else(|| anyhow!("bad path"))?;
        let target = join(remote_dir, &name);
        if local.is_dir() {
            let _ = on!(&mut *c, f => f.mkdir(&target).await);
            let mut rd = tokio::fs::read_dir(local).await?;
            while let Some(e) = rd.next_entry().await? {
                upload_inner(app, tab, c, &e.path(), &target).await?;
            }
            Ok(())
        } else {
            upload_file(app, tab, c, local, &target).await
        }
    })
}

pub async fn upload(app: &AppHandle, tab: &str, locals: &[String], remote_dir: &str) -> Result<()> {
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    for l in locals {
        upload_inner(app, tab, &mut c, Path::new(l), remote_dir).await?;
    }
    Ok(())
}

/// Downloads one file to a temporary folder to open it on this PC.
pub async fn fetch_temp(app: &AppHandle, tab: &str, remote: &str) -> Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("Cablebag").join(uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&dir).await?;
    let name = remote.rsplit('/').next().unwrap_or("file");
    let local = dir.join(name);
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    download_file(app, tab, &mut c, remote, &local).await?;
    Ok(local)
}

pub async fn push_back(app: &AppHandle, tab: &str, local: &str, remote: &str) -> Result<()> {
    let c = app.state::<AppState>().ftps.get(tab)?;
    let mut c = c.lock().await;
    upload_file(app, tab, &mut c, Path::new(local), remote).await
}

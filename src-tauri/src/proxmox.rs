//! Proxmox VE as a session source: the virtual machines and containers of a node or cluster, their
//! state and addresses, and power actions, over the Proxmox API with an API token.
//!
//! Proxmox normally runs with a self-signed certificate. A certificate a public authority vouches
//! for is accepted as is; any other one only after the person trusted its SHA-256 fingerprint,
//! which is checked during the TLS handshake, so the token never goes to a server that was not
//! trusted.

use crate::prompt::{button, PromptRequest};
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;

const SOURCES_KEY: &str = "proxmox.sources";

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Source {
    pub id: String,
    pub name: String,
    /// https://host:8006
    pub url: String,
    /// user@realm!tokenname
    pub token_id: String,
    /// Vaultwarden item whose password is the token secret; empty = secret kept in Cablebag's vault.
    pub vault_item: String,
    /// SHA-256 of the server certificate the person trusted.
    pub fingerprint: String,
    pub ssh_user: String,
    pub key_path: String,
    pub rdp_user: String,
}

#[derive(Serialize)]
pub struct Guest {
    pub vmid: u64,
    pub name: String,
    pub node: String,
    /// qemu or lxc
    pub kind: String,
    /// running, stopped, paused...
    pub status: String,
    /// Proxmox ostype: l26, win11, debian...
    pub os: String,
    pub tags: Vec<String>,
    /// Addresses, the most useful first.
    pub ips: Vec<String>,
    pub uptime: u64,
    pub mem: u64,
    pub maxmem: u64,
    pub cpu: f64,
}

pub fn sources(app: &AppHandle) -> Vec<Source> {
    let state = app.state::<AppState>();
    let v = state.store.lock().setting(SOURCES_KEY).ok().flatten().unwrap_or(Value::Array(vec![]));
    serde_json::from_value(v).unwrap_or_default()
}

fn save_sources(app: &AppHandle, list: &[Source]) -> Result<()> {
    let state = app.state::<AppState>();
    state.store.lock().set_setting(SOURCES_KEY, &serde_json::to_value(list)?)?;
    Ok(())
}

fn source(app: &AppHandle, id: &str) -> Result<Source> {
    sources(app).into_iter().find(|s| s.id == id).ok_or_else(|| anyhow!("This Proxmox server is no longer in the settings."))
}

/// Saves a source (new when its id is empty) and, when given, its token secret in Cablebag's vault.
pub fn save_source(app: &AppHandle, mut src: Source, secret: Option<String>) -> Result<Source> {
    src.url = normalize_url(&src.url)?;
    let mut list = sources(app);
    if src.id.is_empty() {
        src.id = uuid::Uuid::new_v4().to_string();
    }
    match list.iter_mut().find(|s| s.id == src.id) {
        Some(old) => {
            // Another address means another certificate.
            if old.url != src.url {
                src.fingerprint.clear();
            } else if src.fingerprint.is_empty() {
                src.fingerprint = old.fingerprint.clone();
            }
            *old = src.clone();
        }
        None => list.push(src.clone()),
    }
    save_sources(app, &list)?;
    if let Some(secret) = secret.filter(|s| !s.trim().is_empty()) {
        let state = app.state::<AppState>();
        state.vault.save_password(&state.store.lock(), "proxmox", &src.url, 0, &src.token_id, secret.trim())?;
    }
    Ok(src)
}

/// Removes a source and the token secret Cablebag kept for it.
pub fn delete_source(app: &AppHandle, id: &str) -> Result<()> {
    let (gone, list): (Vec<Source>, Vec<Source>) = sources(app).into_iter().partition(|s| s.id == id);
    save_sources(app, &list)?;
    let state = app.state::<AppState>();
    let store = state.store.lock();
    for src in gone {
        for sec in store.list_secrets()? {
            if sec.protocol == "proxmox" && sec.host == src.url && sec.username == src.token_id {
                store.delete_secret(&sec.id)?;
            }
        }
    }
    Ok(())
}

fn normalize_url(url: &str) -> Result<String> {
    let mut u = url.trim().trim_end_matches('/').to_string();
    if u.is_empty() {
        bail!("Fill in the address of the Proxmox server.");
    }
    if let Some(i) = u.find("/api2") {
        u.truncate(i);
    }
    // A bare name gets Proxmox's own port; an address with https:// is taken as written (a proxy on 443).
    if !u.contains("://") {
        let bare_port = u.rsplit_once(':').is_some_and(|(_, p)| p.chars().all(|c| c.is_ascii_digit())) && !u.ends_with(']');
        u = format!("https://{u}{}", if bare_port { "" } else { ":8006" });
    }
    Ok(u)
}

fn hex_sha256(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

/// Accepts a certificate a public authority vouches for, or the one with the trusted fingerprint.
#[derive(Debug)]
struct Pinned {
    pin: String,
    seen: Arc<Mutex<Option<String>>>,
    public: Arc<rustls::client::WebPkiServerVerifier>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fp = hex_sha256(end_entity.as_ref());
        *self.seen.lock() = Some(fp.clone());
        if !self.pin.is_empty() && fp.eq_ignore_ascii_case(&self.pin) {
            return Ok(ServerCertVerified::assertion());
        }
        self.public.verify_server_cert(end_entity, intermediates, server_name, ocsp, now)
    }

    fn verify_tls12_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

fn tls_config(pin: &str, seen: Arc<Mutex<Option<String>>>) -> Result<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    let public = rustls::client::WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone()).build()?;
    let verifier = Pinned { pin: pin.to_string(), seen, public, provider: provider.clone() };
    Ok(rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth())
}

fn http_client(pin: &str, seen: Arc<Mutex<Option<String>>>) -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .use_preconfigured_tls(tls_config(pin, seen)?)
        .user_agent("Cablebag")
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(20))
        .build()?)
}

/// A connection to one Proxmox server with its token.
pub struct Api {
    http: reqwest::Client,
    base: String,
    auth: String,
    /// The trusted certificate fingerprint, for connections outside the HTTP client.
    pin: String,
}

impl Api {
    async fn call(&self, method: reqwest::Method, path: &str) -> Result<Value> {
        self.call_form(method, path, &[]).await
    }

    async fn call_form(&self, method: reqwest::Method, path: &str, form: &[(&str, &str)]) -> Result<Value> {
        let mut req = self.http.request(method, format!("{}/api2/json{path}", self.base)).header("Authorization", &self.auth);
        if !form.is_empty() {
            req = req.form(form);
        }
        let res = req
            .send()
            .await
            .map_err(|e| anyhow!("Could not reach the Proxmox server: {}", root_cause(&e)))?;
        let status = res.status();
        if status.is_success() {
            let v: Value = res.json().await?;
            return Ok(v.get("data").cloned().unwrap_or(Value::Null));
        }
        let text = res.text().await.unwrap_or_default();
        match status.as_u16() {
            401 => bail!("Proxmox refused the API token. Check the token ID and secret."),
            403 => bail!("The API token is not allowed to do this{}.", permission_hint(&text)),
            _ => bail!("Proxmox answered {status}: {}", text.chars().take(200).collect::<String>()),
        }
    }

    async fn get(&self, path: &str) -> Result<Value> {
        self.call(reqwest::Method::GET, path).await
    }
}

fn permission_hint(body: &str) -> String {
    // Proxmox names the missing privilege, e.g. "Permission check failed (/vms/100, VM.PowerMgmt)".
    match body.find("VM.") {
        Some(i) => {
            let p: String = body[i..].chars().take_while(|c| c.is_alphanumeric() || *c == '.').collect();
            format!(" (it needs the {p} privilege)")
        }
        None => String::new(),
    }
}

fn root_cause(e: &dyn std::error::Error) -> String {
    let mut cur: &dyn std::error::Error = e;
    while let Some(next) = cur.source() {
        cur = next;
    }
    cur.to_string()
}

/// Connects, asking for the token secret or for trust in the certificate when needed.
pub async fn connect(app: &AppHandle, id: &str) -> Result<Api> {
    let src = source(app, id)?;
    let state = app.state::<AppState>();

    let (mut token_id, secret) = if !src.vault_item.is_empty() {
        let (user, pw) = state.vw.login(app, &src.vault_item).await?;
        (user, pw)
    } else {
        let saved = { state.vault.password(&state.store.lock(), "proxmox", &src.url, 0, &src.token_id).ok().flatten() };
        match saved {
            Some(s) => (String::new(), s),
            None => {
                let (s, save) = state
                    .prompts
                    .ask_secret(app, &format!("Connect to {}", src.name), &format!("API token secret for {}", src.token_id), "Secret", true)
                    .await
                    .ok_or_else(|| anyhow!("Cancelled."))?;
                if save {
                    let _ = state.vault.save_password(&state.store.lock(), "proxmox", &src.url, 0, &src.token_id, &s);
                }
                (String::new(), s)
            }
        }
    };
    if !src.token_id.is_empty() {
        token_id = src.token_id.clone();
    }
    if !token_id.contains('!') {
        bail!("The API token ID looks like user@realm!name, for example root@pam!cablebag.");
    }
    let auth = format!("PVEAPIToken={}={}", token_id.trim(), secret.trim());

    let mut pin = src.fingerprint.clone();
    for _ in 0..2 {
        let seen = Arc::new(Mutex::new(None));
        let api = Api { http: http_client(&pin, seen.clone())?, base: src.url.clone(), auth: auth.clone(), pin: pin.clone() };
        match api.get("/version").await {
            Ok(_) => return Ok(api),
            Err(e) => {
                let fp = seen.lock().clone();
                // The handshake saw a certificate it did not accept: ask whether to trust it.
                match fp {
                    Some(fp) if !fp.eq_ignore_ascii_case(&pin) && e.to_string().to_lowercase().contains("certificate") => {
                        if !trust(app, &src, &fp).await {
                            bail!("Not connected: the certificate was not trusted.");
                        }
                        pin = fp.clone();
                        let mut list = sources(app);
                        if let Some(s) = list.iter_mut().find(|s| s.id == src.id) {
                            s.fingerprint = fp;
                        }
                        save_sources(app, &list)?;
                    }
                    _ => return Err(e),
                }
            }
        }
    }
    bail!("Could not connect to {}.", src.url)
}

async fn trust(app: &AppHandle, src: &Source, fp: &str) -> bool {
    let state = app.state::<AppState>();
    let changed = !src.fingerprint.is_empty();
    let req = PromptRequest {
        id: String::new(),
        title: if changed { "The certificate changed".into() } else { "Trust this Proxmox server?".into() },
        message: if changed {
            format!("{} now shows a different certificate than the one you trusted before. That happens after renewing it, but can also mean someone is in between. Only continue when you expect this.", src.url)
        } else {
            format!("{} uses a certificate that no public authority vouches for, which is normal for Proxmox. Compare the fingerprint with the one under Datacenter, the node, System, Certificates before trusting it.", src.url)
        },
        details: vec![("SHA-256".into(), fp.into())],
        fields: vec![],
        buttons: vec![button("cancel", "Cancel", "normal"), button("trust", "Trust and connect", if changed { "danger" } else { "primary" })],
        warning: changed,
    };
    state.prompts.ask(app, req).await.button == "trust"
}

fn s(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(x)) => x.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn u(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(|x| x.as_u64().or_else(|| x.as_f64().map(|f| f as u64))).unwrap_or(0)
}

/// All guests with their addresses. Templates are left out.
pub async fn list(app: &AppHandle, id: &str) -> Result<Vec<Guest>> {
    guests(Arc::new(connect(app, id).await?)).await
}

async fn guests(api: Arc<Api>) -> Result<Vec<Guest>> {
    let res = api.get("/cluster/resources?type=vm").await?;
    let items: Vec<Value> = res.as_array().cloned().unwrap_or_default().into_iter().filter(|v| u(v, "template") == 0).collect();

    let sem = Arc::new(Semaphore::new(8));
    let tasks: Vec<_> = items
        .into_iter()
        .map(|v| {
            let api = api.clone();
            let sem = sem.clone();
            tokio::spawn(async move {
                let _p = sem.acquire().await;
                let node = s(&v, "node");
                let kind = s(&v, "type");
                let vmid = u(&v, "vmid");
                let status = s(&v, "status");
                let base = format!("/nodes/{node}/{kind}/{vmid}");
                let config = tokio::time::timeout(Duration::from_secs(5), api.get(&format!("{base}/config")))
                    .await
                    .ok()
                    .and_then(|r| r.ok())
                    .unwrap_or(Value::Null);
                let mut ips = if status == "running" { live_ips(&api, &base, &kind, &config).await } else { vec![] };
                for ip in config_ips(&config) {
                    if !ips.contains(&ip) {
                        ips.push(ip);
                    }
                }
                Guest {
                    vmid,
                    name: s(&v, "name"),
                    node,
                    os: s(&config, "ostype"),
                    kind,
                    status,
                    tags: s(&v, "tags").split([';', ',', ' ']).filter(|t| !t.is_empty()).map(String::from).collect(),
                    ips,
                    uptime: u(&v, "uptime"),
                    mem: u(&v, "mem"),
                    maxmem: u(&v, "maxmem"),
                    cpu: v.get("cpu").and_then(|c| c.as_f64()).unwrap_or(0.0),
                }
            })
        })
        .collect();
    let mut out = Vec::new();
    for t in tasks {
        if let Ok(g) = t.await {
            out.push(g);
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.vmid.cmp(&b.vmid)));
    Ok(out)
}

/// Interfaces that are not where you reach a machine from outside.
fn skip_interface(name: &str) -> bool {
    let n = name.to_lowercase();
    n == "lo" || n.starts_with("docker") || n.starts_with("br-") || n.starts_with("veth") || n.starts_with("cni") || n.starts_with("flannel")
        || n.starts_with("virbr") || n.starts_with("podman") || n.starts_with("tailscale") || n.contains("loopback") || n.starts_with("vethernet")
}

fn useful_ip(ip: &str) -> bool {
    !(ip.starts_with("127.") || ip.starts_with("169.254.") || ip.starts_with("fe80") || ip == "::1" || ip.starts_with("172.17."))
}

/// Addresses the running guest reports: the container's interfaces, or the QEMU guest agent.
async fn live_ips(api: &Api, base: &str, kind: &str, config: &Value) -> Vec<String> {
    let mut v4 = Vec::new();
    let mut v6 = Vec::new();
    if kind == "lxc" {
        let Ok(Ok(list)) = tokio::time::timeout(Duration::from_secs(5), api.get(&format!("{base}/interfaces"))).await else { return vec![] };
        for i in list.as_array().cloned().unwrap_or_default() {
            if skip_interface(&s(&i, "name")) {
                continue;
            }
            for (key, out) in [("inet", &mut v4), ("inet6", &mut v6)] {
                let ip = s(&i, key);
                let ip = ip.split('/').next().unwrap_or("").to_string();
                if !ip.is_empty() && useful_ip(&ip) {
                    out.push(ip);
                }
            }
        }
    } else {
        // Without the agent switched on in the VM's options the call only times out.
        let agent = s(config, "agent");
        if !(agent.starts_with('1') || agent.contains("enabled=1")) {
            return vec![];
        }
        let Ok(Ok(r)) = tokio::time::timeout(Duration::from_secs(5), api.get(&format!("{base}/agent/network-get-interfaces"))).await else { return vec![] };
        for i in r.get("result").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
            if skip_interface(&s(&i, "name")) {
                continue;
            }
            for a in i.get("ip-addresses").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
                let ip = s(&a, "ip-address");
                if ip.is_empty() || !useful_ip(&ip) {
                    continue;
                }
                if s(&a, "ip-address-type") == "ipv4" {
                    v4.push(ip);
                } else {
                    v6.push(ip);
                }
            }
        }
    }
    // The local network first, then other addresses, carrier-grade NAT and VPN ranges (100.64/10) last.
    v4.sort_by_key(|ip| ip_rank(ip));
    v6.sort_by_key(|ip| u8::from(ip.starts_with("fd") || ip.starts_with("fc")));
    v4.extend(v6);
    v4
}

fn ip_rank(ip: &str) -> u8 {
    let o: Vec<u8> = ip.split('.').filter_map(|x| x.parse().ok()).collect();
    match o.as_slice() {
        [192, 168, ..] | [10, ..] => 0,
        [172, b, ..] if (16..32).contains(b) => 0,
        [100, b, ..] if (64..128).contains(b) => 2,
        _ => 1,
    }
}

/// Fixed addresses written in the configuration (containers: net0 ip=..., VMs: cloud-init ipconfig0).
fn config_ips(config: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(map) = config.as_object() {
        let mut keys: Vec<&String> = map.keys().filter(|k| k.starts_with("net") || k.starts_with("ipconfig")).collect();
        keys.sort();
        for k in keys {
            let line = map[k].as_str().unwrap_or("");
            for part in line.split(',') {
                if let Some(ip) = part.strip_prefix("ip=") {
                    let ip = ip.split('/').next().unwrap_or("");
                    if !ip.is_empty() && ip != "dhcp" && ip != "manual" && useful_ip(ip) {
                        out.push(ip.to_string());
                    }
                }
            }
        }
    }
    out
}

/// start, shutdown, reboot or stop.
pub async fn power(app: &AppHandle, id: &str, node: &str, kind: &str, vmid: u64, action: &str) -> Result<()> {
    if !["start", "shutdown", "reboot", "stop"].contains(&action) || !["qemu", "lxc"].contains(&kind) {
        bail!("Unknown action.");
    }
    let api = connect(app, id).await?;
    api.call(reqwest::Method::POST, &format!("/nodes/{node}/{kind}/{vmid}/status/{action}")).await?;
    Ok(())
}

/// The guest's console as a VNC byte stream, plus the one-time password for it.
///
/// Proxmox hands out a ticket (vncproxy), then carries the console's VNC over a WebSocket on its own
/// port (vncwebsocket). Both go over the same trusted TLS, with the API token.
pub async fn console_stream(app: &AppHandle, id: &str, node: &str, kind: &str, vmid: u64) -> Result<(tokio::io::DuplexStream, String)> {
    let api = connect(app, id).await?;
    open_console(&api, node, kind, vmid).await
}

async fn open_console(api: &Api, node: &str, kind: &str, vmid: u64) -> Result<(tokio::io::DuplexStream, String)> {
    use futures::{SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

    if !["qemu", "lxc"].contains(&kind) {
        bail!("Unknown guest type.");
    }
    let proxy = api
        .call_form(reqwest::Method::POST, &format!("/nodes/{node}/{kind}/{vmid}/vncproxy"), &[("websocket", "1")])
        .await?;
    let ticket = s(&proxy, "ticket");
    let port = s(&proxy, "port");
    if ticket.is_empty() || port.is_empty() {
        bail!("Proxmox did not hand out a console for this guest.");
    }

    let base = api.base.trim_start_matches("https://").trim_start_matches("http://").to_string();
    let (host, web_port) = match base.rsplit_once(':') {
        Some((h, p)) if !h.ends_with(':') && p.chars().all(|c| c.is_ascii_digit()) => (h.trim_matches(['[', ']']).to_string(), p.parse::<u16>()?),
        _ => (base.trim_matches(['[', ']']).to_string(), 443),
    };
    let tcp = tokio::time::timeout(Duration::from_secs(8), tokio::net::TcpStream::connect((host.as_str(), web_port)))
        .await
        .map_err(|_| anyhow!("No answer from {host}:{web_port}."))??;
    let tls = tokio_rustls::TlsConnector::from(Arc::new(tls_config(&api.pin, Arc::new(Mutex::new(None)))?));
    let name = rustls::pki_types::ServerName::try_from(host.clone())?;
    let tls = tls.connect(name, tcp).await.map_err(|e| anyhow!("Could not open a secure connection for the console: {e}"))?;

    let url = format!("wss://{base}/api2/json/nodes/{node}/{kind}/{vmid}/vncwebsocket?port={port}&vncticket={}", url_encode(&ticket));
    let mut req = url.into_client_request()?;
    req.headers_mut().insert("Authorization", api.auth.parse()?);
    req.headers_mut().insert("Sec-WebSocket-Protocol", "binary".parse()?);
    let (ws, _) = tokio_tungstenite::client_async(req, tls).await.map_err(|e| anyhow!("Proxmox refused the console connection: {e}"))?;

    // The VNC engine reads and writes plain bytes; the WebSocket carries them in binary messages.
    let (ours, theirs) = tokio::io::duplex(1 << 16);
    let (mut rd, mut wr) = tokio::io::split(theirs);
    let (mut sink, mut stream) = ws.split();
    tokio::spawn(async move {
        while let Some(Ok(msg)) = stream.next().await {
            let data = match msg {
                Message::Binary(d) => d.to_vec(),
                Message::Text(t) => t.as_bytes().to_vec(),
                Message::Close(_) => break,
                _ => continue,
            };
            if wr.write_all(&data).await.is_err() {
                break;
            }
        }
        let _ = wr.shutdown().await;
    });
    tokio::spawn(async move {
        let mut buf = vec![0u8; 1 << 15];
        loop {
            match rd.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if sink.send(Message::Binary(buf[..n].to_vec().into())).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = sink.close().await;
    });
    Ok((ours, ticket))
}

fn url_encode(v: &str) -> String {
    v.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Whether Cablebag already keeps a secret for this source.
pub fn has_secret(app: &AppHandle, src: &Source) -> bool {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    matches!(store.find_secret("password", "", "proxmox", &src.url, 0, &src.token_id), Ok(Some(_)))
}

// ---- Snapshots, tasks and the node's shell ----

#[derive(Serialize)]
pub struct Snapshot {
    pub name: String,
    pub description: String,
    /// Unix time; 0 when Proxmox does not say.
    pub time: u64,
    pub parent: String,
    /// The VM's memory is in it, so a rollback resumes the running VM.
    pub ram: bool,
}

/// The snapshots of a guest, oldest first ("current", the live state, left out).
pub async fn snapshots(app: &AppHandle, id: &str, node: &str, kind: &str, vmid: u64) -> Result<Vec<Snapshot>> {
    if !["qemu", "lxc"].contains(&kind) {
        bail!("Unknown guest type.");
    }
    let api = connect(app, id).await?;
    let list = api.get(&format!("/nodes/{node}/{kind}/{vmid}/snapshot")).await?;
    let mut out: Vec<Snapshot> = list
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| s(v, "name") != "current")
        .map(|v| Snapshot { name: s(v, "name"), description: s(v, "description").trim().to_string(), time: u(v, "snaptime"), parent: s(v, "parent"), ram: u(v, "vmstate") == 1 })
        .collect();
    out.sort_by_key(|x| x.time);
    Ok(out)
}

/// Takes, rolls back or deletes a snapshot. Returns the Proxmox task (UPID) to follow.
pub async fn snapshot_action(app: &AppHandle, id: &str, node: &str, kind: &str, vmid: u64, action: &str, name: &str, description: &str, ram: bool) -> Result<String> {
    if !["qemu", "lxc"].contains(&kind) {
        bail!("Unknown guest type.");
    }
    // Proxmox's own rule for snapshot names.
    let valid = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && name.len() <= 40
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !valid {
        bail!("A snapshot name starts with a letter and has only letters, digits, - and _ (at most 40).");
    }
    let api = connect(app, id).await?;
    let base = format!("/nodes/{node}/{kind}/{vmid}/snapshot");
    let upid = match action {
        "create" => {
            let ram = if ram && kind == "qemu" { "1" } else { "0" };
            let mut form = vec![("snapname", name), ("description", description)];
            if kind == "qemu" {
                form.push(("vmstate", ram));
            }
            api.call_form(reqwest::Method::POST, &base, &form).await?
        }
        "rollback" => api.call(reqwest::Method::POST, &format!("{base}/{name}/rollback")).await?,
        "delete" => api.call(reqwest::Method::DELETE, &format!("{base}/{name}")).await?,
        _ => bail!("Unknown snapshot action."),
    };
    Ok(upid.as_str().unwrap_or_default().to_string())
}

#[derive(Serialize)]
pub struct Task {
    pub upid: String,
    pub node: String,
    /// What it did: qmsnapshot, vzdump, qmstart...
    pub kind: String,
    /// The guest it was about, if any.
    pub id: String,
    pub user: String,
    /// Empty while running; "OK" or an error text when done.
    pub status: String,
    pub start: u64,
    pub end: u64,
}

/// The recent tasks of the whole cluster, newest first; with a guest ID, only those about that guest.
pub async fn tasks(app: &AppHandle, id: &str, vmid: Option<u64>) -> Result<Vec<Task>> {
    let api = connect(app, id).await?;
    let list = api.get("/cluster/tasks").await?;
    let mut out: Vec<Task> = list
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| Task {
            upid: s(v, "upid"),
            node: s(v, "node"),
            kind: s(v, "type"),
            id: s(v, "id"),
            user: s(v, "user"),
            status: s(v, "status"),
            start: u(v, "starttime"),
            end: u(v, "endtime"),
        })
        .filter(|t| vmid.is_none_or(|n| t.id == n.to_string()))
        .collect();
    out.sort_by(|a, b| b.start.cmp(&a.start));
    Ok(out)
}

/// Whether a task still runs, and how it ended ("OK" or the error).
pub async fn task_status(app: &AppHandle, id: &str, node: &str, upid: &str) -> Result<(bool, String)> {
    let api = connect(app, id).await?;
    let st = api.get(&format!("/nodes/{node}/tasks/{}/status", url_encode(upid))).await?;
    Ok((s(&st, "status") == "running", s(&st, "exitstatus")))
}

/// What a task wrote, line by line.
pub async fn task_log(app: &AppHandle, id: &str, node: &str, upid: &str) -> Result<String> {
    let api = connect(app, id).await?;
    let lines = api.get(&format!("/nodes/{node}/tasks/{}/log?limit=5000", url_encode(upid))).await?;
    Ok(lines.as_array().into_iter().flatten().map(|l| s(l, "t")).collect::<Vec<_>>().join("\n"))
}

/// A session for a shell on the node: your saved SSH session for its address or name when there is one,
/// otherwise root on its address (the login then comes from the vault or is asked).
pub async fn node_session(app: &AppHandle, id: &str, node: &str) -> Result<crate::store::Session> {
    let api = connect(app, id).await?;
    let mut addr = node.to_string();
    if let Ok(list) = api.get("/cluster/status").await {
        for e in list.as_array().into_iter().flatten() {
            if s(e, "type") == "node" && s(e, "name") == node && !s(e, "ip").is_empty() {
                addr = s(e, "ip");
            }
        }
    }
    let state = app.state::<AppState>();
    let saved = state.store.lock().sessions().unwrap_or_default().into_iter().find(|x| x.kind == "ssh" && (x.host.eq_ignore_ascii_case(&addr) || x.host.eq_ignore_ascii_case(node)));
    Ok(saved.unwrap_or(crate::store::Session {
        id: String::new(),
        folder_id: None,
        name: format!("{node} (Proxmox node)"),
        kind: "ssh".into(),
        host: addr,
        port: 22,
        username: "root".into(),
        data: serde_json::json!({}),
        comment: String::new(),
        sort: 0,
        last_used: 0,
    }))
}

// ---- Commands inside guests, for the assistant ----

/// Runs one command inside a guest as root: in a VM through the QEMU guest agent (PowerShell on Windows,
/// sh elsewhere), in a container with `pct exec` on its node, over SSH.
pub async fn guest_exec(app: &AppHandle, id: &str, node: &str, kind: &str, vmid: u64, command: &str, timeout: Duration) -> Result<crate::agent::ExecResult> {
    let api = connect(app, id).await?;
    match kind {
        "qemu" => vm_exec(&api, node, vmid, command, timeout).await,
        "lxc" => container_exec(app, id, &api, node, vmid, command, timeout).await,
        _ => bail!("Unknown guest type."),
    }
}

async fn vm_exec(api: &Api, node: &str, vmid: u64, command: &str, timeout: Duration) -> Result<crate::agent::ExecResult> {
    let base = format!("/nodes/{node}/qemu/{vmid}");
    let config = api.get(&format!("{base}/config")).await?;
    let agent = s(&config, "agent");
    if !(agent.starts_with('1') || agent.contains("enabled=1")) {
        bail!("The QEMU guest agent is off for this VM (its Options in Proxmox), so commands cannot run in it.");
    }
    let argv: Vec<&str> = if s(&config, "ostype").starts_with("win") {
        vec!["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", command]
    } else {
        vec!["sh", "-c", command]
    };
    // The API takes the program and its arguments as a list: one form field per item.
    let form: Vec<(&str, &str)> = argv.iter().map(|a| ("command", *a)).collect();
    let started = api
        .call_form(reqwest::Method::POST, &format!("{base}/agent/exec"), &form)
        .await
        .map_err(|e| {
            // A refused token says so itself; anything else is most likely the agent missing in the VM.
            if e.to_string().contains("not allowed") { e } else { anyhow!("{e} Is qemu-guest-agent installed and running in the VM?") }
        })?;
    let pid = started["pid"].as_u64().ok_or_else(|| anyhow!("The guest agent did not start the command."))?;
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        tokio::time::sleep(Duration::from_millis(400)).await;
        let st = api.get(&format!("{base}/agent/exec-status?pid={pid}")).await?;
        if st["exited"].as_u64() == Some(1) || st["exited"].as_bool() == Some(true) {
            let mut text = s(&st, "out-data");
            let err = s(&st, "err-data");
            if !err.is_empty() {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&err);
            }
            let (output, cut) = crate::agent::limit_output(text);
            return Ok(crate::agent::ExecResult {
                output,
                exit: st["exitcode"].as_u64().map(|c| c as u32),
                timed_out: false,
                truncated: cut || st["out-truncated"].as_u64() == Some(1) || st["out-truncated"].as_bool() == Some(true),
            });
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(crate::agent::ExecResult { output: String::new(), exit: None, timed_out: true, truncated: false });
        }
    }
}

/// SSH connections to Proxmox nodes for `pct exec`, kept open between commands, by source and node.
static NODE_SSH: std::sync::OnceLock<Mutex<std::collections::HashMap<String, Arc<crate::ssh::SshConn>>>> = std::sync::OnceLock::new();

struct Quiet;

impl crate::term::Sink for Quiet {
    fn info(&self, _: &str) {}
    fn error(&self, _: &str) {}
}

async fn container_exec(app: &AppHandle, id: &str, api: &Api, node: &str, vmid: u64, command: &str, timeout: Duration) -> Result<crate::agent::ExecResult> {
    let quoted = format!("'{}'", command.replace('\'', r"'\''"));
    let (conn, root) = node_ssh(app, id, api, node).await?;
    let line = format!("{}pct exec {vmid} -- sh -c {quoted}", if root { "" } else { "sudo -n " });
    match crate::agent::exec_line(&conn, &line, timeout).await {
        Ok(r) => Ok(r),
        Err(_) => {
            // The kept connection may have dropped: once more with a new one.
            NODE_SSH.get_or_init(Default::default).lock().remove(&format!("{id}/{node}"));
            let (conn, _) = node_ssh(app, id, api, node).await?;
            crate::agent::exec_line(&conn, &line, timeout).await
        }
    }
}

/// An SSH connection to the node, and whether it logs in as root. The login comes from a saved SSH session
/// for the node's address or name when there is one, otherwise root with the vault and saved passwords.
async fn node_ssh(app: &AppHandle, id: &str, api: &Api, node: &str) -> Result<(Arc<crate::ssh::SshConn>, bool)> {
    let key = format!("{id}/{node}");
    let mut addr = node.to_string();
    if let Ok(list) = api.get("/cluster/status").await {
        for e in list.as_array().into_iter().flatten() {
            if s(e, "type") == "node" && s(e, "name") == node && !s(e, "ip").is_empty() {
                addr = s(e, "ip");
            }
        }
    }
    let saved = {
        let state = app.state::<AppState>();
        let store = state.store.lock();
        store.sessions().unwrap_or_default().into_iter().find(|x| x.kind == "ssh" && (x.host.eq_ignore_ascii_case(&addr) || x.host.eq_ignore_ascii_case(node)))
    };
    let hops = match &saved {
        Some(sess) => crate::ssh::hops_for(sess),
        None => vec![crate::ssh::Hop { via_wg: String::new(), agent_forward: false, host: addr.clone(), port: 22, user: "root".into(), key_path: String::new(), vault_item: String::new(), session_id: String::new(), name: node.to_string() }],
    };
    let root = hops.last().map(|h| h.user.is_empty() || h.user == "root").unwrap_or(true);
    if let Some(c) = NODE_SSH.get_or_init(Default::default).lock().get(&key).cloned() {
        return Ok((c, root));
    }
    let conn = Arc::new(
        crate::ssh::connect_chain(app, &hops, true, &Quiet)
            .await
            .map_err(|e| anyhow!("Could not log in to the Proxmox node {node} ({addr}) over SSH: {e}"))?,
    );
    NODE_SSH.get_or_init(Default::default).lock().insert(key, conn.clone());
    Ok((conn, root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(normalize_url("pve").unwrap(), "https://pve:8006");
        assert_eq!(normalize_url("https://10.20.30.53:8006/").unwrap(), "https://10.20.30.53:8006");
        assert_eq!(normalize_url("https://proxmox2.example.net/api2/json").unwrap(), "https://proxmox2.example.net");
        assert_eq!(normalize_url("10.0.0.5:9000").unwrap(), "https://10.0.0.5:9000");
    }

    /// Against a real server: PVE_URL=https://host:8006 PVE_TOKEN=user@realm!name=secret cargo test -- --ignored live
    #[tokio::test]
    #[ignore]
    async fn live() {
        let url = std::env::var("PVE_URL").unwrap();
        let auth = format!("PVEAPIToken={}", std::env::var("PVE_TOKEN").unwrap());
        // Unknown certificate: refused during the handshake, fingerprint seen.
        let seen = Arc::new(Mutex::new(None));
        let api = Api { http: http_client("", seen.clone()).unwrap(), base: url.clone(), auth: auth.clone(), pin: String::new() };
        let e = api.get("/version").await.unwrap_err().to_string();
        let fp = seen.lock().clone().expect("no certificate seen");
        println!("refused: {e}
fingerprint {fp}");
        assert!(e.to_lowercase().contains("certificate"));
        // Trusted fingerprint: works.
        let api = Arc::new(Api { http: http_client(&fp, Arc::new(Mutex::new(None))).unwrap(), base: url, auth, pin: fp.clone() });
        println!("version {}", api.get("/version").await.unwrap());
        for g in guests(api.clone()).await.unwrap() {
            println!("{:>4} {:<5} {:<8} {:<14} {:<7} {:?}", g.vmid, g.kind, g.status, g.name, g.os, g.ips);
        }
        // The console: Proxmox's WebSocket should carry a VNC server greeting ("RFB 003.008").
        if let Ok(vm) = std::env::var("PVE_CONSOLE") {
            use tokio::io::AsyncReadExt;
            let (kind, vmid) = vm.split_once('/').unwrap();
            let (mut stream, ticket) = open_console(&api, "pve", kind, vmid.parse().unwrap()).await.unwrap();
            let mut hello = [0u8; 12];
            tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut hello)).await.unwrap().unwrap();
            println!("console greeting {:?}, ticket length {}", String::from_utf8_lossy(&hello), ticket.len());
        }
        // A VM number that does not exist: the rights check runs first, then the task fails harmlessly.
        // A read-only token gets "not allowed"; a token with VM.PowerMgmt gets a task id.
        match api.call(reqwest::Method::POST, "/nodes/pve/qemu/999999/status/start").await {
            Ok(task) => println!("power allowed, task {task}"),
            Err(e) => println!("power: {e}"),
        }
    }

    #[test]
    fn fixed_addresses() {
        let c = serde_json::json!({ "net0": "name=eth0,bridge=vmbr0,ip=10.20.30.70/24,gw=10.20.30.1", "net1": "name=eth1,ip=dhcp", "ipconfig0": "ip=10.0.0.5/24" });
        assert_eq!(config_ips(&c), vec!["10.0.0.5", "10.20.30.70"]);
    }
}

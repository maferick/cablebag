//! WireGuard inside Cablebag. Every profile is its own small network: boringtun speaks WireGuard over a UDP
//! socket and smoltcp is the TCP/IP stack behind it. Nothing is installed in Windows (no adapter, no routes, no
//! admin rights), so two profiles can reach the same addresses on two different networks at the same time.
//! A profile starts when the first connection needs it and stops when the last one closes.

use crate::store::Session;
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use boringtun::noise::{Tunn, TunnResult};
use boringtun::x25519::{PublicKey, StaticSecret};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::socket::{dns, tcp};
use smoltcp::wire::{DnsQueryType, HardwareAddress, IpAddress, IpCidr};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::task::JoinHandle;

const PROFILES_KEY: &str = "wireguard.profiles";

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// This side's addresses in the tunnel, as in the conf: "10.8.0.2/32, fd00::2/128".
    pub address: String,
    /// DNS servers inside the tunnel (search domains in the conf are left out).
    pub dns: String,
    /// 0 = 1420, the WireGuard default.
    pub mtu: u16,
    /// The server's public key.
    pub public_key: String,
    /// host:port of the server.
    pub endpoint: String,
    /// What the server routes for this peer; shown only, Cablebag sends whatever a session asks for.
    pub allowed_ips: String,
    /// Seconds between keepalives, 0 = none.
    pub keepalive: u16,
    /// Vaultwarden item whose password is the private key; empty = kept in Cablebag's vault.
    pub vault_item: String,
}

/// A profile as the settings page sees it: without its keys, with whether they are there and how it runs now.
#[derive(Serialize)]
pub struct ProfileInfo {
    #[serde(flatten)]
    pub profile: Profile,
    pub has_key: bool,
    pub has_preshared: bool,
    /// This side's public key (from the private key kept in Cablebag), for the server's peer list.
    pub own_public_key: String,
    pub status: Option<Status>,
}

#[derive(Clone, Serialize, Default)]
pub struct Status {
    /// Seconds since the last handshake; None = no handshake yet.
    pub handshake: Option<u64>,
    pub sent: u64,
    pub received: u64,
    pub connections: usize,
    /// Set when this PC already has the profile's address on an adapter: the WireGuard app has the same tunnel on.
    pub warning: String,
}

pub fn profiles(app: &AppHandle) -> Vec<Profile> {
    let state = app.state::<AppState>();
    let v = state.store.lock().setting(PROFILES_KEY).ok().flatten().unwrap_or(Value::Array(vec![]));
    serde_json::from_value(v).unwrap_or_default()
}

fn save_profiles(app: &AppHandle, list: &[Profile]) -> Result<()> {
    let state = app.state::<AppState>();
    state.store.lock().set_setting(PROFILES_KEY, &serde_json::to_value(list)?)?;
    Ok(())
}

fn profile(app: &AppHandle, id: &str) -> Result<Profile> {
    profiles(app).into_iter().find(|p| p.id == id).ok_or_else(|| anyhow!("This WireGuard network is no longer in Cablebag."))
}

fn saved_key(app: &AppHandle, id: &str, which: &str) -> Option<String> {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    state.vault.password(&store, "wireguard", id, 0, which).ok().flatten()
}

pub fn infos(app: &AppHandle) -> Vec<ProfileInfo> {
    let live = app.state::<AppState>().wg.status();
    profiles(app)
        .into_iter()
        .map(|p| ProfileInfo {
            has_key: !p.vault_item.is_empty() || saved_key(app, &p.id, "private").is_some(),
            has_preshared: saved_key(app, &p.id, "preshared").is_some(),
            own_public_key: saved_key(app, &p.id, "private").and_then(|k| parse_key(&k).ok()).map(|k| encode_key(PublicKey::from(&StaticSecret::from(k)).as_bytes())).unwrap_or_default(),
            status: live.get(&p.id).cloned(),
            profile: p,
        })
        .collect()
}

/// Saves a profile (new when its id is empty) and, when given, its keys in Cablebag's vault.
pub fn save(app: &AppHandle, mut p: Profile, private_key: Option<String>, preshared_key: Option<String>) -> Result<Profile> {
    p.name = p.name.trim().to_string();
    if p.name.is_empty() {
        bail!("Give the WireGuard network a name.");
    }
    parse_key(&p.public_key).map_err(|_| anyhow!("The server's public key is not a WireGuard key (44 characters, ending in =)."))?;
    if p.endpoint.trim().rsplit_once(':').and_then(|(_, port)| port.parse::<u16>().ok()).is_none() {
        bail!("The endpoint is the server's address and port, for example vpn.example.com:51820.");
    }
    if addresses(&p.address).is_empty() {
        bail!("Fill in this side's address in the tunnel, for example 10.8.0.2/32.");
    }
    let private_key = private_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    let preshared_key = preshared_key.map(|k| k.trim().to_string());
    if let Some(k) = &private_key {
        parse_key(k).map_err(|_| anyhow!("The private key is not a WireGuard key (44 characters, ending in =)."))?;
    }
    if let Some(k) = preshared_key.as_ref().filter(|k| !k.is_empty()) {
        parse_key(k).map_err(|_| anyhow!("The preshared key is not a WireGuard key (44 characters, ending in =)."))?;
    }
    if p.id.is_empty() {
        p.id = uuid::Uuid::new_v4().to_string();
    }
    let mut list = profiles(app);
    match list.iter_mut().find(|x| x.id == p.id) {
        Some(old) => *old = p.clone(),
        None => list.push(p.clone()),
    }
    save_profiles(app, &list)?;
    let state = app.state::<AppState>();
    let store = state.store.lock();
    if let Some(k) = private_key {
        state.vault.save_password(&store, "wireguard", &p.id, 0, "private", &k)?;
    }
    match preshared_key {
        Some(k) if !k.is_empty() => state.vault.save_password(&store, "wireguard", &p.id, 0, "preshared", &k)?,
        // An emptied field removes the preshared key.
        Some(_) => forget_keys(&store, &p.id, Some("preshared"))?,
        None => {}
    }
    Ok(p)
}

fn forget_keys(store: &crate::store::Store, id: &str, which: Option<&str>) -> Result<()> {
    for sec in store.list_secrets()? {
        if sec.protocol == "wireguard" && sec.host == id && which.is_none_or(|w| sec.username == w) {
            store.delete_secret(&sec.id)?;
        }
    }
    Ok(())
}

pub fn delete(app: &AppHandle, id: &str) -> Result<()> {
    let list: Vec<Profile> = profiles(app).into_iter().filter(|p| p.id != id).collect();
    save_profiles(app, &list)?;
    let state = app.state::<AppState>();
    let store = state.store.lock();
    forget_keys(&store, id, None)
}

/// Reads a WireGuard .conf file into a new profile with its keys. A conf with more than one peer gets the first.
pub fn import(app: &AppHandle, path: &str) -> Result<Profile> {
    let text = std::fs::read_to_string(path).map_err(|e| anyhow!("Could not read {path}: {e}"))?;
    let name = std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("WireGuard").to_string();
    let (mut p, private, preshared) = parse_conf(&text)?;
    p.name = name;
    save(app, p, Some(private), preshared)
}

fn parse_conf(text: &str) -> Result<(Profile, String, Option<String>)> {
    let mut p = Profile::default();
    let (mut private, mut preshared) = (String::new(), None);
    let mut section = String::new();
    let mut peers = 0;
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase();
            if section == "peer" {
                peers += 1;
            }
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_string());
        let join = |old: &str| if old.is_empty() { v.clone() } else { format!("{old}, {v}") };
        match (section.as_str(), k.as_str()) {
            ("interface", "privatekey") => private = v,
            ("interface", "address") => p.address = join(&p.address),
            ("interface", "dns") => p.dns = join(&p.dns),
            ("interface", "mtu") => p.mtu = v.parse().unwrap_or(0),
            ("peer", _) if peers > 1 => {}
            ("peer", "publickey") => p.public_key = v,
            ("peer", "presharedkey") => preshared = Some(v),
            ("peer", "endpoint") => p.endpoint = v,
            ("peer", "allowedips") => p.allowed_ips = join(&p.allowed_ips),
            ("peer", "persistentkeepalive") => p.keepalive = v.parse().unwrap_or(0),
            _ => {}
        }
    }
    if private.is_empty() || p.public_key.is_empty() {
        bail!("This does not look like a WireGuard configuration: it needs an [Interface] with a PrivateKey and a [Peer] with a PublicKey.");
    }
    if p.endpoint.is_empty() {
        bail!("The [Peer] in this file has no Endpoint, so Cablebag would not know where the server is.");
    }
    Ok((p, private, preshared))
}

fn encode_key(k: &[u8; 32]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(k)
}

/// A new key pair for a peer of Cablebag's own: (private, public).
pub fn keypair() -> (String, String) {
    let private = StaticSecret::from(rand::random::<[u8; 32]>());
    let public = PublicKey::from(&private);
    (encode_key(&private.to_bytes()), encode_key(public.as_bytes()))
}

fn parse_key(s: &str) -> Result<[u8; 32]> {
    use base64::Engine;
    let b = base64::engine::general_purpose::STANDARD.decode(s.trim())?;
    b.try_into().map_err(|_| anyhow!("not 32 bytes"))
}

/// "10.8.0.2/32, fd00::2/128" as addresses (the prefix is not needed: everything goes into the tunnel).
fn addresses(s: &str) -> Vec<IpAddr> {
    s.split(',').filter_map(|a| a.trim().split('/').next()?.parse().ok()).collect()
}

/// Whether this PC already has one of these addresses on an adapter, which means the WireGuard app runs the same tunnel.
fn local_clash(addrs: &[IpAddr]) -> bool {
    let nets = sysinfo::Networks::new_with_refreshed_list();
    nets.iter().any(|(_, data)| data.ip_networks().iter().any(|n| addrs.contains(&n.addr)))
}

// ---------- running tunnels ----------

/// The running tunnels, by profile id. A tunnel lives as long as a connection holds it.
#[derive(Default)]
pub struct Tunnels {
    live: Mutex<HashMap<String, Weak<Tunnel>>>,
    /// One start at a time per profile, so two sessions opening together share one tunnel (two would fight over
    /// the key), while a key prompt for one network does not hold up another.
    starting: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl Tunnels {
    fn get(&self, id: &str) -> Option<Arc<Tunnel>> {
        self.live.lock().get(id).and_then(|w| w.upgrade())
    }

    pub fn status(&self) -> HashMap<String, Status> {
        let mut live = self.live.lock();
        live.retain(|_, w| w.strong_count() > 0);
        live.iter().filter_map(|(id, w)| Some((id.clone(), w.upgrade()?.stats.lock().clone()))).collect()
    }
}

pub struct Tunnel {
    name: String,
    endpoint: String,
    has_dns: bool,
    cmds: mpsc::UnboundedSender<Cmd>,
    stats: Arc<Mutex<Status>>,
    task: JoinHandle<()>,
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Tunnel {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn warning(&self) -> String {
        self.stats.lock().warning.clone()
    }
}

enum Cmd {
    Connect { host: String, ip: Option<IpAddr>, port: u16, reply: oneshot::Sender<Result<DuplexStream>> },
}

/// The tunnel of a profile: the running one, or a new one.
/// Starts a tunnel: looks up the server, opens the UDP socket and runs the network stack.
async fn start(name: &str, endpoint: &str, tunn: Tunn, addrs: Vec<IpAddr>, dns: Vec<IpAddr>, mtu: usize, warning: String) -> Result<Arc<Tunnel>> {
    let to = tokio::net::lookup_host(endpoint.trim())
        .await
        .map_err(|e| anyhow!("Could not look up the WireGuard server {endpoint}: {e}"))?
        .next()
        .ok_or_else(|| anyhow!("Could not look up the WireGuard server {endpoint}."))?;
    let udp = UdpSocket::bind(if to.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).await?;
    // Room for bursts: Windows gives a UDP socket 64 KB, less than one TCP window.
    let sock = socket2::SockRef::from(&udp);
    let _ = sock.set_recv_buffer_size(4 << 20);
    let _ = sock.set_send_buffer_size(4 << 20);
    udp.connect(to).await?;
    let dns: Vec<IpAddress> = dns.into_iter().map(IpAddress::from).collect();
    let stats = Arc::new(Mutex::new(Status { warning, ..Default::default() }));
    let (tx, rx) = mpsc::unbounded_channel();
    let task = tokio::spawn(run(tunn, udp, to, addrs, dns.clone(), mtu, rx, stats.clone()));
    Ok(Arc::new(Tunnel { name: name.to_string(), endpoint: endpoint.to_string(), has_dns: !dns.is_empty(), cmds: tx, stats, task }))
}

pub async fn tunnel(app: &AppHandle, id: &str) -> Result<Arc<Tunnel>> {
    let state = app.state::<AppState>();
    if let Some(t) = state.wg.get(id) {
        return Ok(t);
    }
    let lock = state.wg.starting.lock().entry(id.to_string()).or_default().clone();
    let _one = lock.lock().await;
    if let Some(t) = state.wg.get(id) {
        return Ok(t);
    }
    let p = profile(app, id)?;
    let private = if !p.vault_item.is_empty() {
        state.vw.login(app, &p.vault_item).await?.1
    } else {
        match saved_key(app, id, "private") {
            Some(k) => k,
            None => {
                let (k, save) = state
                    .prompts
                    .ask_secret(app, &format!("WireGuard: {}", p.name), "The private key of this side", "Private key", true)
                    .await
                    .ok_or_else(|| anyhow!("Cancelled."))?;
                parse_key(&k).map_err(|_| anyhow!("That is not a WireGuard key (44 characters, ending in =)."))?;
                if save {
                    let _ = state.vault.save_password(&state.store.lock(), "wireguard", id, 0, "private", k.trim());
                }
                k
            }
        }
    };
    let private = StaticSecret::from(parse_key(&private).map_err(|_| anyhow!("The private key of {} is not a WireGuard key.", p.name))?);
    let public = PublicKey::from(parse_key(&p.public_key)?);
    let preshared = saved_key(app, id, "preshared").and_then(|k| parse_key(&k).ok());
    let keepalive = if p.keepalive > 0 { Some(p.keepalive) } else { None };
    let tunn = Tunn::new(private, public, preshared, keepalive, rand::random::<u32>() >> 8, None).map_err(|e| anyhow!("{e}"))?;

    let addrs = addresses(&p.address);
    let warning = if local_clash(&addrs) {
        format!(
            "The WireGuard app on this PC has {} switched on as well. With the same key the server answers only one of the two: switch it off there, or give Cablebag a peer of its own.",
            p.name
        )
    } else {
        String::new()
    };
    let dns: Vec<IpAddr> = addresses(&p.dns);
    let mtu = if p.mtu >= 1280 { p.mtu as usize } else { 1420 };
    let t = start(&p.name, &p.endpoint, tunn, addrs, dns, mtu, warning).await?;
    state.wg.live.lock().insert(id.to_string(), Arc::downgrade(&t));
    Ok(t)
}

/// A TCP connection through a WireGuard tunnel; it keeps the tunnel running while it is open.
pub struct WgStream {
    io: DuplexStream,
    _tunnel: Arc<Tunnel>,
}

impl AsyncRead for WgStream {
    fn poll_read(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &mut ReadBuf<'_>) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl AsyncWrite for WgStream {
    fn poll_write(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &[u8]) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.io).poll_write(cx, buf)
    }
    fn poll_flush(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.io).poll_flush(cx)
    }
    fn poll_shutdown(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.io).poll_shutdown(cx)
    }
}

/// Opens a TCP connection to host:port inside the network of a profile.
pub async fn connect(app: &AppHandle, id: &str, host: &str, port: u16) -> Result<WgStream> {
    tunnel(app, id).await?.open(host, port).await
}

impl Tunnel {
    /// Opens a TCP connection to host:port inside this tunnel's network.
    pub async fn open(self: &Arc<Self>, host: &str, port: u16) -> Result<WgStream> {
        let mut ip = host.trim_matches(|c| c == '[' || c == ']').parse::<IpAddr>().ok();
        // Without DNS servers in the profile, names are looked up the ordinary way.
        if ip.is_none() && !self.has_dns {
            ip = tokio::net::lookup_host((host, port)).await.ok().and_then(|mut a| a.next()).map(|a| a.ip());
            if ip.is_none() {
                bail!("Could not look up {host}. Give the WireGuard network {} a DNS server, or use an IP address.", self.name);
            }
        }
        let (reply, rx) = oneshot::channel();
        self.cmds.send(Cmd::Connect { host: host.to_string(), ip, port, reply }).map_err(|_| anyhow!("The WireGuard tunnel {} stopped.", self.name))?;
        let io = rx.await.map_err(|_| anyhow!("The WireGuard tunnel {} stopped.", self.name))??;
        Ok(WgStream { io, _tunnel: self.clone() })
    }
}

/// A port on 127.0.0.1 that leads to host:port through a WireGuard tunnel, for programs that need an address
/// (the RDP control, VNC, telnet).
pub struct WgForward {
    pub port: u16,
    task: JoinHandle<()>,
    _tunnel: Arc<Tunnel>,
}

impl Drop for WgForward {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn local_forward(app: &AppHandle, id: &str, host: &str, port: u16) -> Result<WgForward> {
    // The handshake first, so a wrong key or a server that does not answer shows straight away.
    let tun = handshake(app, id).await?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let local = listener.local_addr()?.port();
    let (app, id, host) = (app.clone(), id.to_string(), host.to_string());
    let task = tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let (app, id, host) = (app.clone(), id.clone(), host.clone());
            tokio::spawn(async move {
                if let Ok(mut s) = connect(&app, &id, &host, port).await {
                    let _ = tokio::io::copy_bidirectional(&mut sock, &mut s).await;
                }
            });
        }
    });
    Ok(WgForward { port: local, task, _tunnel: tun })
}

/// Starts the tunnel of a profile (or uses the running one) and waits up to 10 seconds for the handshake.
pub async fn test(app: &AppHandle, id: &str) -> Result<Status> {
    let t = handshake(app, id).await?;
    let s = t.stats.lock().clone();
    Ok(s)
}

async fn handshake(app: &AppHandle, id: &str) -> Result<Arc<Tunnel>> {
    let t = tunnel(app, id).await?;
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if t.stats.lock().handshake.is_some() {
            return Ok(t);
        }
        if Instant::now() > until {
            bail!("The WireGuard server at {} did not answer. Check the endpoint and the keys, and that the server knows this peer.", t.endpoint);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// The profile a session goes through, if any (its own choice or its folder's, already merged in by the window).
pub fn of(session: &Session) -> Option<String> {
    // "none" is a session that goes directly although its folder has a network.
    session.data.get("wireguard").and_then(|v| v.as_str()).filter(|s| !s.is_empty() && *s != "none").map(String::from)
}

// ---------- the network stack ----------

/// smoltcp's view of the tunnel: IP packets in from WireGuard, IP packets out to WireGuard.
struct Pipe {
    rx: VecDeque<Vec<u8>>,
    tx: VecDeque<Vec<u8>>,
    mtu: usize,
}

struct Rx(Vec<u8>);
struct Tx<'a>(&'a mut VecDeque<Vec<u8>>);

impl RxToken for Rx {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
        f(&self.0)
    }
}

impl TxToken for Tx<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let mut b = vec![0; len];
        let r = f(&mut b);
        self.0.push_back(b);
        r
    }
}

impl Device for Pipe {
    type RxToken<'a> = Rx;
    type TxToken<'a> = Tx<'a>;

    fn receive(&mut self, _: smoltcp::time::Instant) -> Option<(Rx, Tx<'_>)> {
        let p = self.rx.pop_front()?;
        Some((Rx(p), Tx(&mut self.tx)))
    }

    fn transmit(&mut self, _: smoltcp::time::Instant) -> Option<Tx<'_>> {
        Some(Tx(&mut self.tx))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut c = DeviceCapabilities::default();
        c.medium = Medium::Ip;
        c.max_transmission_unit = self.mtu;
        c
    }
}

enum Phase {
    /// Waiting for the name to be looked up inside the tunnel.
    Lookup(dns::QueryHandle),
    Connecting,
    Open,
}

struct Conn {
    host: String,
    port: u16,
    handle: Option<SocketHandle>,
    phase: Phase,
    deadline: Instant,
    reply: Option<oneshot::Sender<Result<DuplexStream>>>,
    /// Data for the program, and data from it (None until the connection is open).
    to_app: Option<mpsc::Sender<Vec<u8>>>,
    from_app: Option<mpsc::Receiver<Vec<u8>>>,
    pending: Option<(Vec<u8>, usize)>,
    closed: bool,
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

#[allow(clippy::too_many_arguments)]
async fn run(
    mut tunn: Tunn,
    udp: UdpSocket,
    endpoint: SocketAddr,
    addrs: Vec<IpAddr>,
    dns_servers: Vec<IpAddress>,
    mtu: usize,
    mut cmds: mpsc::UnboundedReceiver<Cmd>,
    stats: Arc<Mutex<Status>>,
) {
    let now = || smoltcp::time::Instant::now();
    let mut dev = Pipe { rx: VecDeque::new(), tx: VecDeque::new(), mtu };
    let mut config = Config::new(HardwareAddress::Ip);
    config.random_seed = rand::random();
    let mut iface = Interface::new(config, &mut dev, now());
    // A prefix of 0: every address counts as directly reachable, so smoltcp needs no routes and sends it all into the tunnel.
    iface.update_ip_addrs(|ips| {
        for a in &addrs {
            let _ = ips.push(IpCidr::new(IpAddress::from(*a), 0));
        }
    });
    let mut sockets = SocketSet::new(vec![]);
    let dns_handle = (!dns_servers.is_empty()).then(|| sockets.add(dns::Socket::new(&dns_servers, vec![])));
    let mut conns: Vec<Conn> = Vec::new();
    let wake = Arc::new(Notify::new());
    let mut next_port: u16 = 40000 + rand::random::<u16>() % 20000;
    let mut buf = vec![0u8; 65536];
    let mut out = vec![0u8; 65536];
    let mut last_timers = Instant::now();
    let mut woke_by: Option<Cmd> = None;
    // Shake hands straight away, so the first connection does not wait for it and a test sees the answer.
    if let TunnResult::WriteToNetwork(d) = tunn.format_handshake_initiation(&mut out, false) {
        let _ = udp.send(d).await;
    }

    loop {
        // Commands: new connections (the one select! woke up for first).
        while let Some(Cmd::Connect { host, ip, port, reply }) = woke_by.take().or_else(|| cmds.try_recv().ok()) {
            let mut c = Conn {
                host: host.clone(),
                port,
                handle: None,
                phase: Phase::Connecting,
                deadline: Instant::now() + CONNECT_TIMEOUT,
                reply: Some(reply),
                to_app: None,
                from_app: None,
                pending: None,
                closed: false,
            };
            match ip {
                Some(ip) => open_socket(&mut c, &mut iface, &mut sockets, ip, &mut next_port),
                None => match dns_handle {
                    Some(h) => match sockets.get_mut::<dns::Socket>(h).start_query(iface.context(), &host, if addrs.iter().any(|a| a.is_ipv4()) { DnsQueryType::A } else { DnsQueryType::Aaaa }) {
                        Ok(q) => c.phase = Phase::Lookup(q),
                        Err(e) => fail(&mut c, format!("Could not look up {host} inside the tunnel: {e}")),
                    },
                    None => fail(&mut c, format!("Could not look up {host}.")),
                },
            }
            conns.push(c);
        }

        iface.poll(now(), &mut dev, &mut sockets);
        pump(&mut conns, &mut iface, &mut sockets, dns_handle, &mut next_port, &wake, &tunn);
        iface.poll(now(), &mut dev, &mut sockets);

        // Packets out: through WireGuard to the server.
        while let Some(p) = dev.tx.pop_front() {
            if let TunnResult::WriteToNetwork(d) = tunn.encapsulate(&p, &mut out) {
                let _ = udp.send(d).await;
            }
        }
        if last_timers.elapsed() >= Duration::from_millis(200) {
            last_timers = Instant::now();
            if let TunnResult::WriteToNetwork(d) = tunn.update_timers(&mut out) {
                let _ = udp.send(d).await;
            }
            let (hs, sent, received, _, _) = tunn.stats();
            let mut s = stats.lock();
            s.handshake = hs.map(|d| d.as_secs());
            s.sent = sent as u64;
            s.received = received as u64;
            s.connections = conns.iter().filter(|c| c.handle.is_some()).count();
        }
        let delay = iface.poll_delay(now(), &sockets).map(|d| Duration::from_micros(d.total_micros())).unwrap_or(Duration::from_millis(250));
        let delay = delay.min(Duration::from_millis(250));
        tokio::select! {
            r = udp.recv(&mut buf) => {
                let Ok(n) = r else { continue };
                arrive(&mut tunn, &udp, endpoint, &buf[..n], &mut dev, &mut out).await;
                // Everything else that is waiting too, before the stack runs: a burst must not overflow the socket.
                while let Ok(n) = udp.try_recv(&mut buf) {
                    arrive(&mut tunn, &udp, endpoint, &buf[..n], &mut dev, &mut out).await;
                }
            }
            c = cmds.recv() => match c {
                Some(c) => woke_by = Some(c),
                // Every sender is gone: the tunnel was dropped.
                None => return,
            },
            _ = wake.notified() => {}
            _ = tokio::time::sleep(delay) => {}
        }
    }
}

/// One datagram from the server: WireGuard's own answers go back, IP packets go to the stack.
async fn arrive(tunn: &mut Tunn, udp: &UdpSocket, from: SocketAddr, data: &[u8], dev: &mut Pipe, out: &mut [u8]) {
    let mut res = tunn.decapsulate(Some(from.ip()), data, out);
    loop {
        match res {
            // After a handshake answer the packets that waited for it follow: repeat until Done.
            TunnResult::WriteToNetwork(d) => {
                let _ = udp.send(d).await;
            }
            TunnResult::WriteToTunnelV4(p, _) | TunnResult::WriteToTunnelV6(p, _) => {
                dev.rx.push_back(p.to_vec());
                return;
            }
            TunnResult::Done | TunnResult::Err(_) => return,
        }
        res = tunn.decapsulate(None, &[], out);
    }
}

fn fail(c: &mut Conn, msg: String) {
    if let Some(r) = c.reply.take() {
        let _ = r.send(Err(anyhow!(msg)));
    }
    c.closed = true;
}

fn open_socket(c: &mut Conn, iface: &mut Interface, sockets: &mut SocketSet, ip: IpAddr, next_port: &mut u16) {
    let mut s = tcp::Socket::new(tcp::SocketBuffer::new(vec![0; 256 * 1024]), tcp::SocketBuffer::new(vec![0; 256 * 1024]));
    s.set_nagle_enabled(false);
    // Without it smoltcp sends the whole window at once, the UDP buffers overflow and it waits in long backoffs.
    s.set_congestion_control(tcp::CongestionControl::Cubic);
    s.set_keep_alive(Some(smoltcp::time::Duration::from_secs(30)));
    *next_port = if *next_port >= 60000 { 40000 } else { *next_port + 1 };
    match s.connect(iface.context(), (IpAddress::from(ip), c.port), *next_port) {
        Ok(()) => {
            c.handle = Some(sockets.add(s));
            c.phase = Phase::Connecting;
        }
        Err(e) => fail(c, format!("Could not connect to {}:{}: {e}", c.host, c.port)),
    }
}

/// Moves data between the sockets and the programs, and finishes or cleans up connections.
fn pump(conns: &mut Vec<Conn>, iface: &mut Interface, sockets: &mut SocketSet, dns_handle: Option<SocketHandle>, next_port: &mut u16, wake: &Arc<Notify>, tunn: &Tunn) {
    for c in conns.iter_mut() {
        if let Phase::Lookup(q) = c.phase {
            let h = dns_handle.expect("lookup without DNS");
            match sockets.get_mut::<dns::Socket>(h).get_query_result(q) {
                Ok(list) => match list.first() {
                    Some(a) => {
                        let ip: IpAddr = (*a).into();
                        open_socket(c, iface, sockets, ip, next_port);
                    }
                    None => fail(c, format!("{} has no address in the tunnel's DNS.", c.host)),
                },
                Err(dns::GetQueryResultError::Pending) => {
                    if Instant::now() > c.deadline {
                        fail(c, format!("The DNS server in the tunnel did not answer for {}.", c.host));
                    }
                }
                Err(dns::GetQueryResultError::Failed) => fail(c, format!("The DNS server in the tunnel does not know {}.", c.host)),
            }
            continue;
        }
        let Some(h) = c.handle else { continue };
        let s = sockets.get_mut::<tcp::Socket>(h);
        if let Phase::Connecting = c.phase {
            match s.state() {
                tcp::State::Established => {
                    let (app_io, net_io) = tokio::io::duplex(256 * 1024);
                    let (mut r, mut w) = tokio::io::split(net_io);
                    let (to_app, mut to_app_rx) = mpsc::channel::<Vec<u8>>(16);
                    let (from_app_tx, from_app) = mpsc::channel::<Vec<u8>>(16);
                    let w1 = wake.clone();
                    tokio::spawn(async move {
                        let mut b = vec![0u8; 16 * 1024];
                        while let Ok(n) = r.read(&mut b).await {
                            if n == 0 || from_app_tx.send(b[..n].to_vec()).await.is_err() {
                                break;
                            }
                            w1.notify_one();
                        }
                        drop(from_app_tx);
                        w1.notify_one();
                    });
                    let w2 = wake.clone();
                    tokio::spawn(async move {
                        while let Some(v) = to_app_rx.recv().await {
                            if w.write_all(&v).await.is_err() {
                                break;
                            }
                            w2.notify_one();
                        }
                        let _ = w.shutdown().await;
                    });
                    c.to_app = Some(to_app);
                    c.from_app = Some(from_app);
                    c.phase = Phase::Open;
                    if let Some(r) = c.reply.take() {
                        let _ = r.send(Ok(app_io));
                    }
                }
                tcp::State::Closed => {
                    let msg = format!("{}:{} refused the connection (through WireGuard).", c.host, c.port);
                    fail(c, msg);
                }
                _ if Instant::now() > c.deadline => {
                    s.abort();
                    let msg = if tunn.time_since_last_handshake().is_none() {
                        "The WireGuard server did not answer. Check the endpoint and the keys, and that the server knows this peer.".to_string()
                    } else {
                        format!("No answer from {}:{} through WireGuard within 20 seconds.", c.host, c.port)
                    };
                    fail(c, msg);
                }
                _ => {}
            }
            continue;
        }

        // The connection can take no more (reset, keepalive gave up, or closed): what the program still had
        // for it is dropped, so the connection can be cleaned up below.
        if !s.may_send() {
            c.pending = None;
            c.from_app = None;
        }

        // From the program into the tunnel.
        loop {
            if c.pending.is_none() {
                match c.from_app.as_mut().map(|r| r.try_recv()) {
                    Some(Ok(v)) => c.pending = Some((v, 0)),
                    Some(Err(mpsc::error::TryRecvError::Disconnected)) => {
                        c.from_app = None;
                        s.close();
                    }
                    _ => {}
                }
            }
            let Some((v, off)) = c.pending.as_mut() else { break };
            if !s.can_send() {
                break;
            }
            match s.send_slice(&v[*off..]) {
                Ok(n) => *off += n,
                Err(_) => {
                    c.pending = None;
                    break;
                }
            }
            if *off >= v.len() {
                c.pending = None;
            } else {
                break;
            }
        }

        // From the tunnel to the program, as far as it keeps up.
        if let Some(tx) = &c.to_app {
            while s.can_recv() {
                let Ok(permit) = tx.try_reserve() else { break };
                let mut b = vec![0u8; 16 * 1024];
                match s.recv_slice(&mut b) {
                    Ok(n) if n > 0 => {
                        b.truncate(n);
                        permit.send(b);
                    }
                    _ => break,
                }
            }
            if !s.may_recv() && !s.can_recv() {
                c.to_app = None;
            }
        }
        // The program closed its end entirely: a server that never closes its side must not keep the connection.
        if c.from_app.is_none() && c.to_app.as_ref().is_some_and(|t| t.is_closed()) {
            s.abort();
            c.to_app = None;
        }
        if c.to_app.is_none() && c.from_app.is_none() && c.pending.is_none() && matches!(s.state(), tcp::State::Closed | tcp::State::TimeWait) {
            c.closed = true;
        }
        if !s.is_open() && c.to_app.is_some() {
            c.to_app = None;
        }
    }
    conns.retain(|c| {
        if c.closed {
            if let Some(h) = c.handle {
                sockets.remove(h);
            }
        }
        !c.closed
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WireGuard server in this process with one service on 10.77.0.1:8000: it says `banner`, then echoes
    /// what it gets, and closes after the other side has.
    async fn fake_server(banner: &'static str, key: StaticSecret, client: PublicKey) -> SocketAddr {
        let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let at = udp.local_addr().unwrap();
        tokio::spawn(async move {
            let mut tunn = Tunn::new(key, client, None, None, 7, None).unwrap();
            let mut dev = Pipe { rx: VecDeque::new(), tx: VecDeque::new(), mtu: 1420 };
            let mut iface = Interface::new(Config::new(HardwareAddress::Ip), &mut dev, smoltcp::time::Instant::now());
            iface.update_ip_addrs(|ips| {
                let _ = ips.push(IpCidr::new(IpAddress::v4(10, 77, 0, 1), 24));
            });
            let mut sockets = SocketSet::new(vec![]);
            let listen = || {
                let mut s = tcp::Socket::new(tcp::SocketBuffer::new(vec![0; 128 * 1024]), tcp::SocketBuffer::new(vec![0; 128 * 1024]));
                s.set_congestion_control(tcp::CongestionControl::Cubic);
                s.listen(8000).unwrap();
                s
            };
            let mut h = sockets.add(listen());
            let mut greeted = false;
            let mut peer: Option<SocketAddr> = None;
            let (mut buf, mut out) = (vec![0u8; 65536], vec![0u8; 65536]);
            loop {
                iface.poll(smoltcp::time::Instant::now(), &mut dev, &mut sockets);
                let s = sockets.get_mut::<tcp::Socket>(h);
                if s.is_active() && s.may_send() && !greeted {
                    s.send_slice(banner.as_bytes()).unwrap();
                    greeted = true;
                }
                while s.can_recv() && s.send_capacity() - s.send_queue() > 0 {
                    let room = (s.send_capacity() - s.send_queue()).min(buf.len());
                    let n = s.recv_slice(&mut buf[..room]).unwrap();
                    s.send_slice(&buf[..n]).unwrap();
                }
                if greeted && !s.may_recv() && s.send_queue() == 0 && s.state() == tcp::State::CloseWait {
                    s.close();
                }
                if !s.is_open() && greeted {
                    sockets.remove(h);
                    h = sockets.add(listen());
                    greeted = false;
                }
                iface.poll(smoltcp::time::Instant::now(), &mut dev, &mut sockets);
                while let Some(p) = dev.tx.pop_front() {
                    if let (TunnResult::WriteToNetwork(d), Some(to)) = (tunn.encapsulate(&p, &mut out), peer) {
                        let _ = udp.send_to(d, to).await;
                    }
                }
                tokio::select! {
                    r = udp.recv_from(&mut buf) => {
                        let Ok((n, from)) = r else { continue };
                        peer = Some(from);
                        let mut more = vec![buf[..n].to_vec()];
                        while let Ok((n, _)) = udp.try_recv_from(&mut buf) {
                            more.push(buf[..n].to_vec());
                        }
                        for d in more {
                        let mut res = tunn.decapsulate(Some(from.ip()), &d, &mut out);
                        loop {
                            match res {
                                TunnResult::WriteToNetwork(d) => { let _ = udp.send_to(d, from).await; }
                                TunnResult::WriteToTunnelV4(p, _) | TunnResult::WriteToTunnelV6(p, _) => { dev.rx.push_back(p.to_vec()); break; }
                                _ => break,
                            }
                            res = tunn.decapsulate(None, &[], &mut out);
                        }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(5)) => {}
                }
            }
        });
        at
    }

    /// A tunnel to a fake server, as this side 10.77.0.2.
    async fn tunnel_to(banner: &'static str) -> Arc<Tunnel> {
        let server = StaticSecret::from(rand::random::<[u8; 32]>());
        let client = StaticSecret::from(rand::random::<[u8; 32]>());
        let at = fake_server(banner, server.clone(), PublicKey::from(&client)).await;
        let tunn = Tunn::new(client, PublicKey::from(&server), None, None, rand::random::<u32>() >> 8, None).unwrap();
        start(banner, &at.to_string(), tunn, vec!["10.77.0.2".parse().unwrap()], vec![], 1420, String::new()).await.unwrap()
    }

    async fn banner(s: &mut WgStream, len: usize) -> String {
        let mut b = vec![0u8; len];
        tokio::time::timeout(Duration::from_secs(10), s.read_exact(&mut b)).await.expect("no banner in time").unwrap();
        String::from_utf8(b).unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn two_networks_with_the_same_addresses() {
        let a = tunnel_to("network A").await;
        let b = tunnel_to("network B").await;
        let (sa, sb) = tokio::join!(a.open("10.77.0.1", 8000), b.open("10.77.0.1", 8000));
        let (mut sa, mut sb) = (sa.unwrap(), sb.unwrap());
        assert_eq!(banner(&mut sa, 9).await, "network A");
        assert_eq!(banner(&mut sb, 9).await, "network B");
        assert!(a.stats.lock().clone().connections <= 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn megabytes_arrive_whole_and_in_order() {
        let t = tunnel_to("echo").await;
        let mut s = t.open("10.77.0.1", 8000).await.unwrap();
        assert_eq!(banner(&mut s, 4).await, "echo");
        let data: Vec<u8> = (0..4 * 1024 * 1024u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        let (mut r, mut w) = tokio::io::split(s);
        let sent = data.clone();
        let writer = tokio::spawn(async move {
            w.write_all(&sent).await.unwrap();
            w.shutdown().await.unwrap();
        });
        let mut got = Vec::new();
        let mut b = vec![0u8; 65536];
        let began = Instant::now();
        loop {
            match tokio::time::timeout(Duration::from_secs(5), r.read(&mut b)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => got.extend_from_slice(&b[..n]),
                Ok(Err(e)) => panic!("{e}"),
                Err(_) => panic!("stalled after {} of {} bytes, writer done: {}", got.len(), data.len(), writer.is_finished()),
            }
        }
        eprintln!("4 MB echoed in {:?}", began.elapsed());
        writer.await.unwrap();
        assert_eq!(got.len(), data.len());
        assert!(got == data, "the echo differs from what was sent");
        // The connection is cleaned up after both sides closed, and so is one the program just drops.
        let dropped = t.open("10.77.0.1", 8000).await.unwrap();
        drop(dropped);
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert_eq!(t.stats.lock().connections, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_closed_port_is_refused() {
        let t = tunnel_to("x").await;
        let e = t.open("10.77.0.1", 9).await.err().expect("port 9 should be refused");
        assert!(e.to_string().contains("refused"), "{e}");
    }

    #[test]
    fn reads_a_conf() {
        let (p, private, preshared) = parse_conf(
            "[Interface]\nPrivateKey = aaaa\nAddress = 10.8.0.2/32, fd00::2/128\nDNS = 10.8.0.1, corp.example\n\n[Peer]\nPublicKey = bbbb\nPresharedKey = cccc\nEndpoint = vpn.example.com:51820\nAllowedIPs = 10.0.0.0/24\nAllowedIPs = 10.1.0.0/24\nPersistentKeepalive = 25 # comment\n\n[Peer]\nPublicKey = dddd\n",
        )
        .unwrap();
        assert_eq!(private, "aaaa");
        assert_eq!(preshared.as_deref(), Some("cccc"));
        assert_eq!(p.public_key, "bbbb");
        assert_eq!(p.endpoint, "vpn.example.com:51820");
        assert_eq!(p.allowed_ips, "10.0.0.0/24, 10.1.0.0/24");
        assert_eq!(p.keepalive, 25);
        assert_eq!(addresses(&p.address).len(), 2);
        assert_eq!(addresses(&p.dns), vec!["10.8.0.1".parse::<IpAddr>().unwrap()]);
        assert!(parse_conf("[Interface]\nPrivateKey = a\n").is_err());
    }

    /// For trying Cablebag itself: a WireGuard server on 127.0.0.1:51999 whose 10.77.0.1:22 leads to
    /// WG_TARGET (host:port). Writes the client .conf to WG_CONF and runs until stopped.
    /// WG_TARGET=192.0.2.10:22 WG_CONF=test.conf cargo test --lib wg::tests::gateway -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn gateway() {
        let target = std::env::var("WG_TARGET").expect("WG_TARGET");
        let server = StaticSecret::from(rand::random::<[u8; 32]>());
        let client = StaticSecret::from(rand::random::<[u8; 32]>());
        let conf = format!(
            "[Interface]
PrivateKey = {}
Address = 10.77.0.2/32

[Peer]
PublicKey = {}
Endpoint = 127.0.0.1:51999
AllowedIPs = 10.77.0.0/24
",
            encode_key(&client.to_bytes()),
            encode_key(PublicKey::from(&server).as_bytes())
        );
        std::fs::write(std::env::var("WG_CONF").expect("WG_CONF"), conf).unwrap();
        let udp = UdpSocket::bind("127.0.0.1:51999").await.unwrap();
        let mut tunn = Tunn::new(server, PublicKey::from(&client), None, None, 9, None).unwrap();
        let mut dev = Pipe { rx: VecDeque::new(), tx: VecDeque::new(), mtu: 1420 };
        let mut iface = Interface::new(Config::new(HardwareAddress::Ip), &mut dev, smoltcp::time::Instant::now());
        iface.update_ip_addrs(|ips| {
            let _ = ips.push(IpCidr::new(IpAddress::v4(10, 77, 0, 1), 24));
        });
        let mut sockets = SocketSet::new(vec![]);
        let listen = |sockets: &mut SocketSet<'static>| {
            let mut s = tcp::Socket::new(tcp::SocketBuffer::new(vec![0; 256 * 1024]), tcp::SocketBuffer::new(vec![0; 256 * 1024]));
            s.set_congestion_control(tcp::CongestionControl::Cubic);
            s.listen(22).unwrap();
            sockets.add(s)
        };
        let mut waiting = listen(&mut sockets);
        // Accepted connections: socket, data to it, data from it.
        let mut open: Vec<(SocketHandle, mpsc::Receiver<Vec<u8>>, mpsc::Sender<Vec<u8>>, Option<Vec<u8>>)> = Vec::new();
        let wake = Arc::new(Notify::new());
        let mut peer: Option<SocketAddr> = None;
        let (mut buf, mut out) = (vec![0u8; 65536], vec![0u8; 65536]);
        let mut timers = Instant::now();
        loop {
            iface.poll(smoltcp::time::Instant::now(), &mut dev, &mut sockets);
            if sockets.get::<tcp::Socket>(waiting).is_active() {
                let (to_sock_tx, to_sock) = mpsc::channel::<Vec<u8>>(16);
                let (from_sock, mut from_sock_rx) = mpsc::channel::<Vec<u8>>(16);
                let (target, w) = (target.clone(), wake.clone());
                tokio::spawn(async move {
                    let Ok(tcp) = tokio::net::TcpStream::connect(&target).await else { return };
                    eprintln!("gateway: connection to {target}");
                    let (mut r, mut wr) = tcp.into_split();
                    let w2 = w.clone();
                    tokio::spawn(async move {
                        while let Some(v) = from_sock_rx.recv().await {
                            if wr.write_all(&v).await.is_err() {
                                break;
                            }
                            w2.notify_one();
                        }
                    });
                    let mut b = vec![0u8; 16384];
                    while let Ok(n) = r.read(&mut b).await {
                        if n == 0 || to_sock_tx.send(b[..n].to_vec()).await.is_err() {
                            break;
                        }
                        w.notify_one();
                    }
                });
                open.push((waiting, to_sock, from_sock, None));
                waiting = listen(&mut sockets);
            }
            open.retain_mut(|(h, to_sock, from_sock, pending)| {
                let s = sockets.get_mut::<tcp::Socket>(*h);
                if pending.is_none() {
                    *pending = to_sock.try_recv().ok();
                }
                if let Some(v) = pending.take() {
                    match s.send_slice(&v) {
                        Ok(n) if n < v.len() => *pending = Some(v[n..].to_vec()),
                        _ => {}
                    }
                }
                while s.can_recv() {
                    let Ok(permit) = from_sock.try_reserve() else { break };
                    let mut b = vec![0u8; 16384];
                    let n = s.recv_slice(&mut b).unwrap_or(0);
                    b.truncate(n);
                    permit.send(b);
                }
                s.is_open()
            });
            iface.poll(smoltcp::time::Instant::now(), &mut dev, &mut sockets);
            while let Some(p) = dev.tx.pop_front() {
                if let (TunnResult::WriteToNetwork(d), Some(to)) = (tunn.encapsulate(&p, &mut out), peer) {
                    let _ = udp.send_to(d, to).await;
                }
            }
            if timers.elapsed() > Duration::from_millis(250) {
                timers = Instant::now();
                if let (TunnResult::WriteToNetwork(d), Some(to)) = (tunn.update_timers(&mut out), peer) {
                    let _ = udp.send_to(d, to).await;
                }
            }
            tokio::select! {
                r = udp.recv_from(&mut buf) => {
                    let Ok((n, from)) = r else { continue };
                    peer = Some(from);
                    let mut all = vec![buf[..n].to_vec()];
                    while let Ok((n, _)) = udp.try_recv_from(&mut buf) {
                        all.push(buf[..n].to_vec());
                    }
                    for d in all {
                        let mut res = tunn.decapsulate(Some(from.ip()), &d, &mut out);
                        loop {
                            match res {
                                TunnResult::WriteToNetwork(x) => { let _ = udp.send_to(x, from).await; }
                                TunnResult::WriteToTunnelV4(p, _) | TunnResult::WriteToTunnelV6(p, _) => { dev.rx.push_back(p.to_vec()); break; }
                                _ => break,
                            }
                            res = tunn.decapsulate(None, &[], &mut out);
                        }
                    }
                }
                _ = wake.notified() => {}
                _ = tokio::time::sleep(Duration::from_millis(10)) => {}
            }
        }
    }
}

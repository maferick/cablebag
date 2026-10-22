//! Network tools: network scanner, port scanner, open ports on this PC, Wake on LAN.

use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;

/// Running scans by id, so they can be stopped.
#[derive(Default)]
pub struct Scans {
    cancel: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Scans {
    fn begin(&self, id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel.lock().insert(id.to_string(), flag.clone());
        flag
    }
    fn end(&self, id: &str) {
        self.cancel.lock().remove(id);
    }
    pub fn stop(&self, id: &str) {
        if let Some(f) = self.cancel.lock().get(id) {
            f.store(true, Ordering::Relaxed);
        }
    }
}

#[derive(Clone, Serialize)]
struct Progress<'a> {
    scan: &'a str,
    done: usize,
    total: usize,
    finished: bool,
}

#[derive(Clone, Serialize)]
pub struct HostResult {
    pub scan: String,
    pub ip: String,
    pub name: String,
    pub mac: String,
    pub rtt_ms: Option<u32>,
    pub open_ports: Vec<u16>,
}

#[derive(Clone, Serialize)]
struct PortResult<'a> {
    scan: &'a str,
    port: u16,
}

/// Parses "192.168.1.0/24", "192.168.1.10-192.168.1.50", "192.168.1.10-50" or a single address.
pub fn parse_range(spec: &str) -> Result<Vec<Ipv4Addr>> {
    let s = spec.trim();
    let (start, end): (u32, u32) = if let Some((net, bits)) = s.split_once('/') {
        let ip: Ipv4Addr = net.trim().parse()?;
        let bits: u32 = bits.trim().parse()?;
        if !(16..=32).contains(&bits) {
            bail!("Use a network of /16 or smaller.");
        }
        let mask = if bits == 32 { u32::MAX } else { !(u32::MAX >> bits) };
        let base = u32::from(ip) & mask;
        let size = 1u32 << (32 - bits);
        if size <= 2 { (base, base + size - 1) } else { (base + 1, base + size - 2) }
    } else if let Some((a, b)) = s.split_once('-') {
        let a: Ipv4Addr = a.trim().parse()?;
        let b = b.trim();
        let b: Ipv4Addr = if b.contains('.') {
            b.parse()?
        } else {
            let o = a.octets();
            Ipv4Addr::new(o[0], o[1], o[2], b.parse()?)
        };
        (u32::from(a), u32::from(b))
    } else {
        let ip: Ipv4Addr = s.parse()?;
        (u32::from(ip), u32::from(ip))
    };
    if end < start {
        bail!("The range ends before it starts.");
    }
    if end - start > 65535 {
        bail!("That range is too large; scan at most 65536 addresses at once.");
    }
    Ok((start..=end).map(Ipv4Addr::from).collect())
}

/// The /24 networks of this PC's own addresses, for a sensible default range.
pub fn local_ranges() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(ifaces) = local_ipv4s() {
        for ip in ifaces {
            let o = ip.octets();
            if o[0] == 127 || (o[0] == 169 && o[1] == 254) {
                continue;
            }
            out.push(format!("{}.{}.{}.0/24", o[0], o[1], o[2]));
        }
    }
    out.sort();
    out.dedup();
    out
}

fn local_ipv4s() -> Result<Vec<Ipv4Addr>> {
    // Connecting a UDP socket picks the outgoing interface without sending anything.
    let mut v = Vec::new();
    for target in ["192.168.0.1:9", "10.0.0.1:9", "172.16.0.1:9", "8.8.8.8:9"] {
        if let Ok(s) = std::net::UdpSocket::bind("0.0.0.0:0") {
            if s.connect(target).is_ok() {
                if let Ok(SocketAddr::V4(a)) = s.local_addr() {
                    v.push(*a.ip());
                }
            }
        }
    }
    Ok(v)
}

async fn port_open(ip: IpAddr, port: u16, timeout: Duration) -> bool {
    matches!(tokio::time::timeout(timeout, TcpStream::connect((ip, port))).await, Ok(Ok(_)))
}

#[cfg(windows)]
fn ping(ip: Ipv4Addr, timeout_ms: u32) -> Option<u32> {
    use windows::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY};
    unsafe {
        let h = IcmpCreateFile().ok()?;
        let data = *b"Cablebag";
        let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + data.len() + 8];
        let n = IcmpSendEcho(
            h,
            u32::from_ne_bytes(ip.octets()),
            data.as_ptr() as *const _,
            data.len() as u16,
            None,
            reply.as_mut_ptr() as *mut _,
            reply.len() as u32,
            timeout_ms,
        );
        let _ = IcmpCloseHandle(h);
        if n == 0 {
            return None;
        }
        let r = &*(reply.as_ptr() as *const ICMP_ECHO_REPLY);
        if r.Status == 0 { Some(r.RoundTripTime) } else { None }
    }
}

#[cfg(not(windows))]
fn ping(_ip: Ipv4Addr, _timeout_ms: u32) -> Option<u32> {
    None
}

#[cfg(windows)]
fn mac_of(ip: Ipv4Addr) -> String {
    use windows::Win32::NetworkManagement::IpHelper::SendARP;
    let mut mac = [0u8; 8];
    let mut len: u32 = 6;
    let r = unsafe { SendARP(u32::from_ne_bytes(ip.octets()), 0, mac.as_mut_ptr() as *mut _, &mut len) };
    if r != 0 || len < 6 {
        return String::new();
    }
    mac[..6].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

#[cfg(not(windows))]
fn mac_of(_ip: Ipv4Addr) -> String {
    String::new()
}

/// Finds live hosts: answers to ping or any of the probe ports. Results stream as "scan-host" events.
pub async fn scan_network(app: AppHandle, scans: Arc<Scans>, scan: String, range: String, ports: Vec<u16>) -> Result<()> {
    let ips = parse_range(&range)?;
    let total = ips.len();
    let cancel = scans.begin(&scan);
    let done = Arc::new(AtomicUsize::new(0));
    // A whole /24 at once: unused addresses spend seconds on address resolution, so batches stay small in number.
    let sem = Arc::new(Semaphore::new(256));
    let ports = Arc::new(ports);
    let mut handles = Vec::with_capacity(total);
    for ip in ips {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let permit = sem.clone().acquire_owned().await?;
        let (app2, scan2, ports2, done2, cancel2) = (app.clone(), scan.clone(), ports.clone(), done.clone(), cancel.clone());
        handles.push(tokio::spawn(async move {
            let _p = permit;
            if cancel2.load(Ordering::Relaxed) {
                return;
            }
            let rtt = tokio::time::timeout(Duration::from_millis(1000), tokio::task::spawn_blocking(move || ping(ip, 700)))
                .await
                .ok()
                .and_then(|r| r.ok())
                .flatten();
            let mut open = Vec::new();
            let checks = ports2.iter().map(|&p| async move { (p, port_open(IpAddr::V4(ip), p, Duration::from_millis(600)).await) });
            for (p, ok) in futures::future::join_all(checks).await {
                if ok {
                    open.push(p);
                }
            }
            let n = done2.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = app2.emit("scan-progress", Progress { scan: &scan2, done: n, total, finished: false });
            if rtt.is_some() || !open.is_empty() {
                // Show the host straight away; the name and hardware address can take seconds to find.
                let mut host = HostResult { scan: scan2.clone(), ip: ip.to_string(), name: String::new(), mac: String::new(), rtt_ms: rtt, open_ports: open };
                let _ = app2.emit("scan-host", host.clone());
                let (name, mac) = tokio::task::spawn_blocking(move || {
                    let name = dns_lookup::lookup_addr(&IpAddr::V4(ip)).unwrap_or_default();
                    let name = if name == ip.to_string() { String::new() } else { name };
                    (name, mac_of(ip))
                })
                .await
                .unwrap_or_default();
                if !name.is_empty() || !mac.is_empty() {
                    host.name = name;
                    host.mac = mac;
                    let _ = app2.emit("scan-host", host);
                }
            }
        }));
    }
    for h in handles {
        let _ = h.await;
    }
    scans.end(&scan);
    let _ = app.emit("scan-progress", Progress { scan: &scan, done: done.load(Ordering::Relaxed), total, finished: true });
    Ok(())
}

/// Checks a list of ports on one host. Open ports stream as "scan-port" events.
pub async fn scan_ports(app: AppHandle, scans: Arc<Scans>, scan: String, host: String, ports: Vec<u16>) -> Result<()> {
    let ip = tokio::net::lookup_host((host.as_str(), 0))
        .await?
        .next()
        .map(|a| a.ip())
        .ok_or_else(|| anyhow!("{host} could not be resolved."))?;
    let total = ports.len();
    let cancel = scans.begin(&scan);
    let done = Arc::new(AtomicUsize::new(0));
    let sem = Arc::new(Semaphore::new(1000));
    let mut handles = Vec::with_capacity(total);
    let mut last = Instant::now();
    for port in ports {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let permit = sem.clone().acquire_owned().await?;
        let (app2, scan2, done2) = (app.clone(), scan.clone(), done.clone());
        handles.push(tokio::spawn(async move {
            let _p = permit;
            if port_open(ip, port, Duration::from_millis(500)).await {
                let _ = app2.emit("scan-port", PortResult { scan: &scan2, port });
            }
            done2.fetch_add(1, Ordering::Relaxed);
        }));
        if last.elapsed().as_millis() > 250 {
            last = Instant::now();
            let _ = app.emit("scan-progress", Progress { scan: &scan, done: done.load(Ordering::Relaxed), total, finished: false });
        }
    }
    for h in handles {
        let _ = h.await;
    }
    scans.end(&scan);
    let _ = app.emit("scan-progress", Progress { scan: &scan, done: done.load(Ordering::Relaxed), total, finished: true });
    Ok(())
}

#[derive(Serialize)]
pub struct Socket {
    pub protocol: String,
    pub local_address: String,
    pub local_port: u16,
    pub remote_address: String,
    pub remote_port: u16,
    pub state: String,
    pub pid: u32,
    pub process: String,
}

/// Listening and connected sockets on this PC, with the program that owns them.
pub fn open_ports() -> Result<Vec<Socket>> {
    use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};
    let sockets = get_sockets_info(AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6, ProtocolFlags::TCP | ProtocolFlags::UDP)
        .map_err(|e| anyhow!("could not list sockets: {e}"))?;
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let name_of = |pid: u32| {
        sys.process(sysinfo::Pid::from_u32(pid)).map(|p| p.name().to_string_lossy().to_string()).unwrap_or_default()
    };
    let mut out = Vec::new();
    for s in sockets {
        let pid = s.associated_pids.first().copied().unwrap_or(0);
        match s.protocol_socket_info {
            ProtocolSocketInfo::Tcp(t) => out.push(Socket {
                protocol: "TCP".into(),
                local_address: t.local_addr.to_string(),
                local_port: t.local_port,
                remote_address: t.remote_addr.to_string(),
                remote_port: t.remote_port,
                state: format!("{:?}", t.state),
                pid,
                process: name_of(pid),
            }),
            ProtocolSocketInfo::Udp(u) => out.push(Socket {
                protocol: "UDP".into(),
                local_address: u.local_addr.to_string(),
                local_port: u.local_port,
                remote_address: String::new(),
                remote_port: 0,
                state: String::new(),
                pid,
                process: name_of(pid),
            }),
        }
    }
    out.sort_by(|a, b| a.local_port.cmp(&b.local_port).then(a.protocol.cmp(&b.protocol)));
    Ok(out)
}

/// Sends a Wake on LAN magic packet.
pub fn wake(mac: &str, broadcast: &str, port: u16) -> Result<()> {
    let hex: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 12 {
        bail!("A MAC address has 12 hexadecimal digits, for example 00:11:22:AA:BB:CC.");
    }
    let bytes: Vec<u8> = (0..6).map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0)).collect();
    let mut packet = vec![0xFFu8; 6];
    for _ in 0..16 {
        packet.extend_from_slice(&bytes);
    }
    let sock = std::net::UdpSocket::bind("0.0.0.0:0")?;
    sock.set_broadcast(true)?;
    let target = if broadcast.trim().is_empty() { "255.255.255.255" } else { broadcast.trim() };
    sock.send_to(&packet, (target, port))?;
    Ok(())
}

/// One host to check for the dots in the sessions tree.
#[derive(serde::Deserialize)]
pub struct ReachTarget {
    pub key: String,
    pub host: String,
    pub port: u16,
}

#[derive(Serialize)]
pub struct Reach {
    pub key: String,
    /// "open": the port answers; "up": the host answers ping (port not tried, or closed);
    /// "down": nothing answers; "unknown-name": the name does not resolve.
    pub state: &'static str,
    pub ms: Option<u32>,
}

/// Checks whether hosts answer. With `port_first` the session's port is tried first, which tells a
/// stopped service apart from a host that is off, but leaves a line in the server's log. Without it
/// ping goes first and the port is only tried when ping gets no answer, which servers do not log.
pub async fn reach(targets: Vec<ReachTarget>, port_first: bool) -> Vec<Reach> {
    let sem = Arc::new(Semaphore::new(64));
    let tasks: Vec<_> = targets
        .into_iter()
        .map(|t| {
            let sem = sem.clone();
            tokio::spawn(async move {
                let _p = sem.acquire().await;
                reach_one(t, port_first).await
            })
        })
        .collect();
    let mut out = Vec::new();
    for t in tasks {
        if let Ok(r) = t.await {
            out.push(r);
        }
    }
    out
}

async fn reach_one(t: ReachTarget, port_first: bool) -> Reach {
    let r = |state, ms| Reach { key: t.key.clone(), state, ms };
    let addr = match tokio::time::timeout(Duration::from_secs(3), tokio::net::lookup_host((t.host.as_str(), t.port))).await {
        Ok(Ok(mut a)) => match a.next() {
            Some(a) => a,
            None => return r("unknown-name", None),
        },
        _ => return r("unknown-name", None),
    };
    let tcp = || async {
        let start = Instant::now();
        port_open(addr.ip(), addr.port(), Duration::from_millis(1500))
            .await
            .then(|| start.elapsed().as_millis() as u32)
    };
    let icmp = || async {
        match addr.ip() {
            IpAddr::V4(ip) => tokio::task::spawn_blocking(move || ping(ip, 1000)).await.ok().flatten(),
            IpAddr::V6(_) => None,
        }
    };
    if port_first {
        if let Some(ms) = tcp().await {
            return r("open", Some(ms));
        }
        if let Some(ms) = icmp().await {
            return r("up", Some(ms));
        }
    } else {
        if let Some(ms) = icmp().await {
            return r("up", Some(ms));
        }
        if let Some(ms) = tcp().await {
            return r("open", Some(ms));
        }
    }
    r("down", None)
}

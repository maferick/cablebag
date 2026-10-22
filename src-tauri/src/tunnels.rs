//! Tunnel manager: named port forwards over SSH that run on their own, independent of terminal tabs.
//! Local: a port on this PC reaches a host behind the SSH server.
//! Remote: a port on the SSH server reaches a host near this PC.
//! Dynamic: a SOCKS 4/5 proxy on this PC; every connection leaves through the SSH server.

use crate::ssh::{self, Hop, SshConn};
use crate::term::Sink;
use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelDef {
    pub id: String,
    pub name: String,
    /// local, remote or dynamic
    pub kind: String,
    #[serde(default = "default_bind")]
    pub bind_address: String,
    pub bind_port: u16,
    #[serde(default)]
    pub dest_host: String,
    #[serde(default)]
    pub dest_port: u16,
    pub ssh: Hop,
    #[serde(default)]
    pub jump_hosts: Vec<Hop>,
}

fn default_bind() -> String {
    "127.0.0.1".into()
}

struct Running {
    tasks: Vec<JoinHandle<()>>,
    conn: Arc<SshConn>,
    remote: Option<(String, u32)>,
    active: Arc<AtomicUsize>,
}

impl Drop for Running {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

#[derive(Default)]
pub struct Tunnels {
    running: Mutex<HashMap<String, Running>>,
}

#[derive(Clone, Serialize)]
pub struct TunnelStatus {
    pub id: String,
    /// starting, running, stopped, failed
    pub state: String,
    pub message: String,
    pub connections: usize,
}

fn emit(app: &AppHandle, id: &str, state: &str, message: &str, connections: usize) {
    let _ = app.emit(
        "tunnel-status",
        TunnelStatus { id: id.into(), state: state.into(), message: message.into(), connections },
    );
}

struct Log<'a> {
    app: &'a AppHandle,
    id: &'a str,
}

impl Sink for Log<'_> {
    fn info(&self, text: &str) {
        emit(self.app, self.id, "starting", text, 0);
    }
    fn error(&self, text: &str) {
        emit(self.app, self.id, "starting", text, 0);
    }
}

impl Tunnels {
    pub fn is_running(&self, id: &str) -> bool {
        self.running.lock().contains_key(id)
    }

    pub fn statuses(&self) -> Vec<TunnelStatus> {
        self.running
            .lock()
            .iter()
            .map(|(id, r)| TunnelStatus {
                id: id.clone(),
                state: "running".into(),
                message: String::new(),
                connections: r.active.load(Ordering::Relaxed),
            })
            .collect()
    }

    pub async fn stop(&self, app: &AppHandle, id: &str) {
        let r = self.running.lock().remove(id);
        if let Some(r) = r {
            if let Some((addr, port)) = &r.remote {
                let _ = r.conn.handle.cancel_tcpip_forward(addr.clone(), *port).await;
            }
            let _ = r.conn.handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
        }
        emit(app, id, "stopped", "", 0);
    }

    pub async fn start(self: &Arc<Self>, app: AppHandle, def: TunnelDef) -> Result<()> {
        if self.is_running(&def.id) {
            return Ok(());
        }
        emit(&app, &def.id, "starting", "Connecting...", 0);
        let result = self.start_inner(&app, &def).await;
        if let Err(e) = &result {
            emit(&app, &def.id, "failed", &e.to_string(), 0);
        }
        result
    }

    async fn start_inner(self: &Arc<Self>, app: &AppHandle, def: &TunnelDef) -> Result<()> {
        if def.ssh.host.trim().is_empty() {
            bail!("No SSH server is set for this tunnel.");
        }
        let mut hops: Vec<Hop> = def.jump_hosts.iter().filter(|h| !h.host.trim().is_empty()).cloned().collect();
        hops.push(def.ssh.clone());
        let log = Log { app, id: &def.id };
        let conn = Arc::new(ssh::connect_chain(app, &hops, true, &log).await?);
        let active = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        let mut remote = None;

        match def.kind.as_str() {
            "local" | "dynamic" => {
                let listener = TcpListener::bind((def.bind_address.as_str(), def.bind_port))
                    .await
                    .map_err(|e| anyhow!("Port {} on {} cannot be used: {e}", def.bind_port, def.bind_address))?;
                let c = conn.clone();
                let a = active.clone();
                let dynamic = def.kind == "dynamic";
                let (dh, dp) = (def.dest_host.clone(), def.dest_port);
                let (app2, id2) = (app.clone(), def.id.clone());
                tasks.push(tokio::spawn(async move {
                    while let Ok((sock, peer)) = listener.accept().await {
                        let (c, a, dh, app3, id3) = (c.clone(), a.clone(), dh.clone(), app2.clone(), id2.clone());
                        tokio::spawn(async move {
                            a.fetch_add(1, Ordering::Relaxed);
                            emit(&app3, &id3, "running", "", a.load(Ordering::Relaxed));
                            let res = if dynamic {
                                socks(sock, &c).await
                            } else {
                                forward(sock, &c, &dh, dp, &peer.ip().to_string(), peer.port()).await
                            };
                            a.fetch_sub(1, Ordering::Relaxed);
                            let msg = res.err().map(|e| e.to_string()).unwrap_or_default();
                            emit(&app3, &id3, "running", &msg, a.load(Ordering::Relaxed));
                        });
                    }
                }));
            }
            "remote" => {
                let bind = if def.bind_address.trim().is_empty() { "localhost".to_string() } else { def.bind_address.clone() };
                conn.forwards.lock().insert(def.bind_port as u32, (def.dest_host.clone(), def.dest_port));
                let port = conn
                    .handle
                    .tcpip_forward(bind.clone(), def.bind_port as u32)
                    .await
                    .map_err(|e| anyhow!("The SSH server refused to listen on port {}: {e}", def.bind_port))?;
                let port = if port == 0 { def.bind_port as u32 } else { port };
                if port != def.bind_port as u32 {
                    let target = conn.forwards.lock().remove(&(def.bind_port as u32));
                    if let Some(t) = target {
                        conn.forwards.lock().insert(port, t);
                    }
                }
                remote = Some((bind, port));
            }
            other => bail!("Unknown tunnel type {other}."),
        }

        // Notice when the SSH connection drops.
        {
            let me = self.clone();
            let c = conn.clone();
            let (app2, id2) = (app.clone(), def.id.clone());
            tasks.push(tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    if c.handle.is_closed() {
                        me.running.lock().remove(&id2);
                        emit(&app2, &id2, "failed", "The SSH connection was lost.", 0);
                        break;
                    }
                }
            }));
        }

        self.running.lock().insert(def.id.clone(), Running { tasks, conn, remote, active });
        emit(app, &def.id, "running", "", 0);
        Ok(())
    }
}

async fn forward(mut sock: TcpStream, conn: &SshConn, host: &str, port: u16, peer_ip: &str, peer_port: u16) -> Result<()> {
    let ch = conn
        .handle
        .channel_open_direct_tcpip(host.to_string(), port as u32, peer_ip.to_string(), peer_port as u32)
        .await
        .map_err(|e| anyhow!("The SSH server could not reach {host}:{port}: {e}"))?;
    let mut stream = ch.into_stream();
    let _ = tokio::io::copy_bidirectional(&mut sock, &mut stream).await;
    Ok(())
}

/// A minimal SOCKS 4, 4a and 5 server (CONNECT only, no authentication).
async fn socks(mut sock: TcpStream, conn: &SshConn) -> Result<()> {
    let ver = sock.read_u8().await?;
    let (host, port) = match ver {
        5 => {
            let n = sock.read_u8().await? as usize;
            let mut methods = vec![0u8; n];
            sock.read_exact(&mut methods).await?;
            sock.write_all(&[5, 0]).await?;
            let mut head = [0u8; 4];
            sock.read_exact(&mut head).await?;
            if head[1] != 1 {
                sock.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
                bail!("only CONNECT is supported");
            }
            let host = match head[3] {
                1 => {
                    let mut a = [0u8; 4];
                    sock.read_exact(&mut a).await?;
                    std::net::Ipv4Addr::from(a).to_string()
                }
                3 => {
                    let len = sock.read_u8().await? as usize;
                    let mut name = vec![0u8; len];
                    sock.read_exact(&mut name).await?;
                    String::from_utf8_lossy(&name).to_string()
                }
                4 => {
                    let mut a = [0u8; 16];
                    sock.read_exact(&mut a).await?;
                    std::net::Ipv6Addr::from(a).to_string()
                }
                _ => bail!("unknown address type"),
            };
            let port = sock.read_u16().await?;
            (host, port)
        }
        4 => {
            let cmd = sock.read_u8().await?;
            let port = sock.read_u16().await?;
            let mut ip = [0u8; 4];
            sock.read_exact(&mut ip).await?;
            read_cstr(&mut sock).await?; // user id
            let host = if ip[0] == 0 && ip[1] == 0 && ip[2] == 0 && ip[3] != 0 {
                read_cstr(&mut sock).await? // SOCKS 4a: name follows
            } else {
                std::net::Ipv4Addr::from(ip).to_string()
            };
            if cmd != 1 {
                sock.write_all(&[0, 0x5b, 0, 0, 0, 0, 0, 0]).await?;
                bail!("only CONNECT is supported");
            }
            (host, port)
        }
        _ => bail!("not a SOCKS request"),
    };
    let ch = match conn.handle.channel_open_direct_tcpip(host.clone(), port as u32, "127.0.0.1", 0).await {
        Ok(ch) => ch,
        Err(e) => {
            let fail: &[u8] = if ver == 5 { &[5, 5, 0, 1, 0, 0, 0, 0, 0, 0] } else { &[0, 0x5b, 0, 0, 0, 0, 0, 0] };
            sock.write_all(fail).await?;
            bail!("{host}:{port} could not be reached: {e}");
        }
    };
    let ok: &[u8] = if ver == 5 { &[5, 0, 0, 1, 0, 0, 0, 0, 0, 0] } else { &[0, 0x5a, 0, 0, 0, 0, 0, 0] };
    sock.write_all(ok).await?;
    let mut stream = ch.into_stream();
    let _ = tokio::io::copy_bidirectional(&mut sock, &mut stream).await;
    Ok(())
}

async fn read_cstr(sock: &mut TcpStream) -> Result<String> {
    let mut out = Vec::new();
    loop {
        let b = sock.read_u8().await?;
        if b == 0 || out.len() > 255 {
            break;
        }
        out.push(b);
    }
    Ok(String::from_utf8_lossy(&out).to_string())
}

//! VNC sessions: the vnc-rs engine decodes the remote screen; frames go to the tab as small binary
//! messages and are drawn on a canvas there. Keyboard, mouse and clipboard go the other way.
//!
//! Message layout to the window (little endian):
//!   1 size     w u16, h u16
//!   2 pixels   x, y, w, h u16, then RGBA bytes
//!   3 copy     dx, dy, w, h, sx, sy u16
//!   4 jpeg     x, y, w, h u16, then JPEG bytes
//!   5 bell
//!   6 text     UTF-8 clipboard text from the server

use crate::ssh::{self, Hop};
use crate::store::Session;
use crate::term::{Sink, TermOut};
use crate::tunnel;
use crate::AppState;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use std::future::Future;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};
use vnc::{ClientKeyEvent, ClientMouseEvent, PixelFormat, VncClient, VncConnector, VncEncoding, VncEvent, X11Event};

pub enum VncCmd {
    Key(u32, bool),
    Pointer(u16, u16, u8),
    Clipboard(String),
    CtrlAltDel,
    Close,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn put16(buf: &mut Vec<u8>, vals: &[u16]) {
    for v in vals {
        buf.extend_from_slice(&v.to_le_bytes());
    }
}

struct StatusSink<'a>(&'a TermOut);

impl Sink for StatusSink<'_> {
    fn info(&self, text: &str) {
        self.0.status("connecting", text);
    }
    fn error(&self, text: &str) {
        self.0.status("connecting", text);
    }
}

pub async fn run(app: AppHandle, session: Session, out: TermOut, mut rx: mpsc::UnboundedReceiver<VncCmd>) {
    if let Err(e) = run_inner(&app, &session, &out, &mut rx).await {
        out.status("failed", &e.to_string());
        return;
    }
    out.status("closed", "");
}

async fn run_inner(app: &AppHandle, session: &Session, out: &TermOut, rx: &mut mpsc::UnboundedReceiver<VncCmd>) -> Result<()> {
    out.status("connecting", "");

    // A Proxmox guest's console: VNC over Proxmox's WebSocket, with a one-time ticket as password.
    if let Some(px) = session.data.get("proxmox").filter(|p| !s(p, "source").is_empty()) {
        let vmid = px.get("vmid").and_then(|v| v.as_u64()).unwrap_or(0);
        let (stream, ticket) = crate::proxmox::console_stream(app, &s(px, "source"), &s(px, "node"), &s(px, "kind"), vmid).await?;
        let client = handshake(stream, Box::pin(async move { Ok(ticket) })).await?;
        return serve(session, out, rx, client).await;
    }

    let mut host = session.host.trim().to_string();
    let mut port = if session.port > 0 { session.port as u16 } else { 5900 };

    // Optional SSH gateway, as for RDP.
    let mut _fwd = None;
    if let Some(gw) = session.data.get("ssh_gateway").filter(|g| !s(g, "host").is_empty()) {
        let hop = Hop {
            via_wg: crate::wg::of(&session).unwrap_or_default(),
            agent_forward: false,
            host: s(gw, "host"),
            port: gw.get("port").and_then(|p| p.as_u64()).unwrap_or(22) as u16,
            user: s(gw, "user"),
            key_path: s(gw, "key_path"),
            vault_item: s(gw, "vault_item"),
            session_id: String::new(),
            name: String::new(),
            ..Default::default()
        };
        let conn = Arc::new(ssh::connect_chain(app, &[hop], true, &StatusSink(out)).await?);
        let f = tunnel::local_forward(conn, 0, host.clone(), port).await?;
        host = "127.0.0.1".into();
        port = f.port;
        _fwd = Some(Box::new(f) as Box<dyn std::any::Any + Send + Sync>);
    } else if let Some(wg) = crate::wg::of(&session) {
        let f = crate::wg::local_forward(app, &wg, &host, port).await?;
        host = "127.0.0.1".into();
        port = f.port;
        _fwd = Some(Box::new(f));
    }

    let tcp = tokio::time::timeout(Duration::from_secs(15), TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|_| anyhow!("No answer from {}:{} within 15 seconds.", session.host, port))?
        .map_err(|e| anyhow!("Could not connect to {}:{port}: {e}", session.host))?;

    // The password is only asked for when the server wants one.
    let (app2, sess2) = (app.clone(), session.clone());
    let auth = async move {
        let state = app2.state::<AppState>();
        let item = s(&sess2.data, "vault_item");
        if !item.is_empty() {
            if let Ok((_, pw)) = state.creds.login(&app2, &item).await {
                if !pw.is_empty() {
                    return Ok(pw);
                }
            }
        }
        let saved = { state.vault.password(&state.store.lock(), "vnc", &sess2.host, sess2.port, "").ok().flatten() };
        if let Some(pw) = saved {
            return Ok(pw);
        }
        if item.is_empty() {
            match state.creds.pick_login(&app2, "vnc", &sess2.host, "", &sess2.name).await {
                Ok(Some((found, _, _, pw))) if !pw.is_empty() => {
                    crate::sources::link_session(&app2, &sess2.id, &found);
                    return Ok(pw);
                }
                Ok(_) => {}
                Err(_) => return Err(vnc::VncError::General("Login cancelled.".into())),
            }
        }
        match state
            .prompts
            .ask_secret(&app2, &format!("Connect to {}", sess2.host), &format!("VNC password for {}", sess2.host), "Password", true)
            .await
        {
            Some((pw, save)) => {
                if save {
                    let _ = state.vault.save_password(&state.store.lock(), "vnc", &sess2.host, sess2.port, "", &pw);
                }
                Ok(pw)
            }
            None => Err(vnc::VncError::General("Login cancelled.".into())),
        }
    };

    let client = handshake(tcp, Box::pin(auth)).await?;
    serve(session, out, rx, client).await
}

type AuthFuture = Pin<Box<dyn Future<Output = Result<String, vnc::VncError>> + Send + Sync>>;

/// The VNC handshake over any byte stream: a TCP socket or a Proxmox console WebSocket.
async fn handshake<S>(stream: S, auth: AuthFuture) -> Result<VncClient>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    VncConnector::new(stream)
        .set_auth_method(auth)
        .add_encoding(VncEncoding::Tight)
        .add_encoding(VncEncoding::Zrle)
        .add_encoding(VncEncoding::CopyRect)
        .add_encoding(VncEncoding::Raw)
        .add_encoding(VncEncoding::DesktopSizePseudo)
        .allow_shared(true)
        .set_pixel_format(PixelFormat::rgba())
        .build()
        .map_err(|e| anyhow!("{e}"))?
        .try_start()
        .await
        .map_err(|e| anyhow!("VNC: {e}"))?
        .finish()
        .map_err(|e| anyhow!("{e}"))
}

async fn serve(session: &Session, out: &TermOut, rx: &mut mpsc::UnboundedReceiver<VncCmd>, client: VncClient) -> Result<()> {
    out.status("connected", "");
    // Ask for the whole screen once; after that only changes are requested.
    let _ = client.input(X11Event::FullRefresh).await;
    let view_only = session.data.get("view_only").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut tick = tokio::time::interval(Duration::from_millis(33));
    loop {
        // Everything the server has sent so far.
        loop {
            match client.poll_event().await {
                Ok(Some(ev)) => send_event(out, ev),
                Ok(None) => break,
                Err(e) => return Err(anyhow!("Connection lost: {e}")),
            }
        }
        tokio::select! {
            _ = tick.tick() => {
                if client.input(X11Event::Refresh).await.is_err() { break; }
            }
            cmd = rx.recv() => {
                let ev = match cmd {
                    None | Some(VncCmd::Close) => break,
                    Some(_) if view_only => continue,
                    Some(VncCmd::Key(k, down)) => X11Event::KeyEvent(ClientKeyEvent { keycode: k, down }),
                    Some(VncCmd::Pointer(x, y, b)) => X11Event::PointerEvent(ClientMouseEvent { position_x: x, position_y: y, bottons: b }),
                    Some(VncCmd::Clipboard(t)) => X11Event::CopyText(t),
                    Some(VncCmd::CtrlAltDel) => {
                        for (k, d) in [(0xffe3, true), (0xffe9, true), (0xffff, true), (0xffff, false), (0xffe9, false), (0xffe3, false)] {
                            let _ = client.input(X11Event::KeyEvent(ClientKeyEvent { keycode: k, down: d })).await;
                        }
                        continue;
                    }
                };
                if client.input(ev).await.is_err() { break; }
            }
        }
    }
    let _ = client.close().await;
    Ok(())
}

fn send_event(out: &TermOut, ev: VncEvent) {
    let mut buf = Vec::new();
    match ev {
        VncEvent::SetResolution(scr) => {
            buf.push(1);
            put16(&mut buf, &[scr.width, scr.height]);
        }
        VncEvent::RawImage(r, data) => {
            buf.reserve(9 + data.len());
            buf.push(2);
            put16(&mut buf, &[r.x, r.y, r.width, r.height]);
            buf.extend_from_slice(&data);
        }
        VncEvent::Copy(dst, src) => {
            buf.push(3);
            put16(&mut buf, &[dst.x, dst.y, dst.width, dst.height, src.x, src.y]);
        }
        VncEvent::JpegImage(r, data) => {
            buf.push(4);
            put16(&mut buf, &[r.x, r.y, r.width, r.height]);
            buf.extend_from_slice(&data);
        }
        VncEvent::Bell => buf.push(5),
        VncEvent::Text(t) => {
            buf.push(6);
            buf.extend_from_slice(t.as_bytes());
        }
        VncEvent::Error(e) => {
            out.status("connecting", &e);
            return;
        }
        _ => return,
    }
    // Raw frames bypass the terminal logger: TermOut::data would try to log them.
    let _ = out.channel.send(tauri::ipc::InvokeResponseBody::Raw(buf));
}

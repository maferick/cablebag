//! Telnet sessions: a TCP terminal with the Telnet option handshake (window size, terminal type, echo,
//! go-ahead) and, when the session has a user name, an automatic login with a password from the vault or
//! Cablebag's own store.

use crate::store::Session;
use crate::term::{TermCmd, TermOut};
use crate::AppState;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;
const ECHO: u8 = 1;
const SGA: u8 = 3;
const TTYPE: u8 = 24;
const NAWS: u8 = 31;

/// The handshake state, kept across reads because a command can be split over two of them.
#[derive(Default)]
struct Telnet {
    /// Bytes of an unfinished command.
    pending: Vec<u8>,
    /// Options already answered, so a server that repeats itself gets no answer loop.
    answered: Vec<(u8, u8)>,
    naws: bool,
}

fn naws(cols: u16, rows: u16) -> Vec<u8> {
    let mut v = vec![IAC, SB, NAWS];
    for b in [cols.to_be_bytes(), rows.to_be_bytes()].concat() {
        v.push(b);
        if b == IAC {
            v.push(IAC);
        }
    }
    v.extend([IAC, SE]);
    v
}

impl Telnet {
    /// Splits what arrived into text for the screen and answers for the server.
    fn feed(&mut self, data: &[u8], cols: u16, rows: u16) -> (Vec<u8>, Vec<u8>) {
        let mut buf = std::mem::take(&mut self.pending);
        buf.extend_from_slice(data);
        let (mut text, mut reply) = (Vec::new(), Vec::new());
        let mut i = 0;
        while i < buf.len() {
            if buf[i] != IAC {
                text.push(buf[i]);
                i += 1;
                continue;
            }
            if i + 1 >= buf.len() {
                break;
            }
            match buf[i + 1] {
                IAC => {
                    text.push(IAC);
                    i += 2;
                }
                cmd @ (DO | DONT | WILL | WONT) => {
                    if i + 2 >= buf.len() {
                        break;
                    }
                    let opt = buf[i + 2];
                    i += 3;
                    if self.answered.contains(&(cmd, opt)) {
                        continue;
                    }
                    self.answered.push((cmd, opt));
                    match (cmd, opt) {
                        (DO, NAWS) => {
                            reply.extend([IAC, WILL, NAWS]);
                            reply.extend(naws(cols, rows));
                            self.naws = true;
                        }
                        (DO, TTYPE) | (DO, SGA) => reply.extend([IAC, WILL, opt]),
                        (WILL, ECHO) | (WILL, SGA) => reply.extend([IAC, DO, opt]),
                        (DO, _) => reply.extend([IAC, WONT, opt]),
                        (WILL, _) => reply.extend([IAC, DONT, opt]),
                        _ => {}
                    }
                }
                SB => {
                    // Up to IAC SE.
                    let Some(end) = buf[i + 2..].windows(2).position(|w| w == [IAC, SE]) else { break };
                    let body = &buf[i + 2..i + 2 + end];
                    if body.first() == Some(&TTYPE) && body.get(1) == Some(&1) {
                        reply.extend([IAC, SB, TTYPE, 0]);
                        reply.extend(b"XTERM-256COLOR");
                        reply.extend([IAC, SE]);
                    }
                    i += 2 + end + 2;
                }
                _ => i += 2,
            }
        }
        self.pending = buf[i..].to_vec();
        (text, reply)
    }
}

/// What is typed: IAC doubled, Enter as CR LF (a Telnet new line).
fn encode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    for &b in input {
        match b {
            IAC => out.extend([IAC, IAC]),
            b'\r' => out.extend(b"\r\n"),
            _ => out.push(b),
        }
    }
    out
}

/// The password for the automatic login: the linked vault login, Cablebag's saved one, or a vault login found for the host.
async fn password(app: &AppHandle, session: &Session) -> Option<String> {
    let state = app.state::<AppState>();
    let item = session.data.get("vault_item").and_then(|v| v.as_str()).unwrap_or("");
    if !item.is_empty() {
        return state.creds.login(app, item).await.ok().map(|(_, p)| p);
    }
    let saved = { state.vault.password(&state.store.lock(), "telnet", &session.host, session.port, &session.username).ok().flatten() };
    if saved.is_some() {
        return saved;
    }
    match state.creds.pick_login(app, "telnet", &session.host, &session.username, &session.name).await {
        Ok(Some((found, _, _, pw))) => {
            crate::sources::link_session(app, &session.id, &found);
            Some(pw)
        }
        _ => None,
    }
}

pub async fn run(app: AppHandle, session: Session, mut cols: u16, mut rows: u16, out: TermOut, mut rx: mpsc::UnboundedReceiver<TermCmd>) {
    let host = session.host.trim().to_string();
    let port = if session.port > 0 { session.port as u16 } else { 23 };
    out.status("connecting", "");
    out.info(&format!("Connecting to {host}:{port}..."));
    // Through WireGuard: a local port that leads into the tunnel, kept open while the session runs.
    let mut _wg = None;
    let (to_host, to_port) = match crate::wg::of(&session) {
        Some(wg) => match crate::wg::local_forward(&app, &wg, &host, port).await {
            Ok(f) => {
                let p = f.port;
                _wg = Some(f);
                ("127.0.0.1".to_string(), p)
            }
            Err(e) => {
                out.error(&e.to_string());
                out.status("failed", &e.to_string());
                return;
            }
        },
        None => (host.clone(), port),
    };
    let tcp = match tokio::time::timeout(Duration::from_secs(15), tokio::net::TcpStream::connect((to_host.as_str(), to_port))).await {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            out.error(&format!("Could not connect to {host}:{port}: {e}"));
            out.status("failed", &e.to_string());
            return;
        }
        Err(_) => {
            out.error(&format!("No answer from {host}:{port} within 15 seconds."));
            out.status("failed", "No answer.");
            return;
        }
    };
    let _ = tcp.set_nodelay(true);
    out.status("connected", "");
    let (mut rd, mut wr) = tcp.into_split();
    let mut telnet = Telnet::default();
    let mut buf = vec![0u8; 16 * 1024];

    // Automatic login: the user name at the login prompt, then the password at its prompt.
    let user = session.username.trim().to_string();
    let mut login_step = if user.is_empty() { 2 } else { 0 };
    let started = Instant::now();
    let mut seen = String::new();

    loop {
        tokio::select! {
            n = rd.read(&mut buf) => {
                let n = match n { Ok(0) | Err(_) => break, Ok(n) => n };
                let (text, reply) = telnet.feed(&buf[..n], cols, rows);
                if !reply.is_empty() && wr.write_all(&reply).await.is_err() {
                    break;
                }
                if !text.is_empty() {
                    out.data(&text);
                    if login_step < 2 && started.elapsed() < Duration::from_secs(60) {
                        seen.push_str(&String::from_utf8_lossy(&text).to_lowercase());
                        let tail: String = seen.trim_end().chars().rev().take(20).collect::<Vec<_>>().into_iter().rev().collect();
                        if login_step == 0 && (tail.ends_with("login:") || tail.ends_with("username:") || tail.ends_with("user name:") || tail.ends_with("user:")) {
                            let _ = wr.write_all(&encode(format!("{user}\r").as_bytes())).await;
                            login_step = 1;
                            seen.clear();
                        } else if login_step == 1 && tail.ends_with("password:") {
                            login_step = 2;
                            match password(&app, &session).await {
                                Some(pw) if !pw.is_empty() => { let _ = wr.write_all(&encode(format!("{pw}\r").as_bytes())).await; }
                                _ => {}
                            }
                        }
                        if seen.len() > 4000 {
                            seen.drain(..2000);
                        }
                    }
                }
            }
            cmd = rx.recv() => match cmd {
                Some(TermCmd::Input(data)) => {
                    if wr.write_all(&encode(&data)).await.is_err() {
                        break;
                    }
                }
                Some(TermCmd::Resize(c, r)) => {
                    cols = c;
                    rows = r;
                    if telnet.naws && wr.write_all(&naws(cols, rows)).await.is_err() {
                        break;
                    }
                }
                Some(TermCmd::Close) | None => {
                    let _ = wr.shutdown().await;
                    out.status("closed", "");
                    return;
                }
            }
        }
    }
    out.info("\r\nThe connection was closed.");
    out.status("closed", "");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake() {
        let mut t = Telnet::default();
        // Server: DO NAWS, DO TTYPE, WILL ECHO, DO 99 (unknown), then "login: " split over two reads.
        let (text, reply) = t.feed(&[IAC, DO, NAWS, IAC, DO, TTYPE, IAC, WILL, ECHO, IAC, DO, 99, b'l', b'o', IAC], 80, 24);
        assert_eq!(text, b"lo");
        assert!(reply.windows(3).any(|w| w == [IAC, WILL, NAWS]));
        assert!(reply.windows(7).any(|w| w == [IAC, SB, NAWS, 0, 80, 0, 24]));
        assert!(reply.windows(3).any(|w| w == [IAC, WILL, TTYPE]));
        assert!(reply.windows(3).any(|w| w == [IAC, DO, ECHO]));
        assert!(reply.windows(3).any(|w| w == [IAC, WONT, 99]));
        assert!(t.naws);
        // The split IAC IAC is one 255 byte of text.
        let (text, _) = t.feed(&[IAC, b'g', b'i', b'n'], 80, 24);
        assert_eq!(text, vec![IAC, b'g', b'i', b'n']);
        // Terminal type asked: answered; the same DO again gets no second answer.
        let (_, reply) = t.feed(&[IAC, SB, TTYPE, 1, IAC, SE, IAC, DO, NAWS], 80, 24);
        assert!(reply.starts_with(&[IAC, SB, TTYPE, 0]));
        assert!(!reply.windows(3).any(|w| w == [IAC, WILL, NAWS]));
    }

    #[test]
    fn typing() {
        assert_eq!(encode(b"ls\r"), b"ls\r\n");
        assert_eq!(encode(&[IAC]), vec![IAC, IAC]);
    }
}

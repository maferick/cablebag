//! RDP tabs. Each tab runs one `cablebag-rdp` helper process that hosts the Windows Remote Desktop
//! control as a child window of the Cablebag window; this module starts it and passes commands.

use crate::prompt::{button, field, PromptRequest};
use crate::ssh::{self, Hop};
use crate::store::Session;
use crate::term::Sink;
use crate::tunnel::{self, LocalForward};
use crate::AppState;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

pub struct RdpProc {
    stdin: Mutex<ChildStdin>,
    child: Mutex<Child>,
    _tunnel: Option<LocalForward>,
}

impl RdpProc {
    pub fn send(&self, v: &Value) {
        let mut w = self.stdin.lock();
        let _ = writeln!(w, "{v}");
        let _ = w.flush();
    }
}

impl Drop for RdpProc {
    fn drop(&mut self) {
        self.send(&json!({"cmd": "quit"}));
        let mut c = self.child.lock();
        std::thread::sleep(std::time::Duration::from_millis(150));
        let _ = c.kill();
    }
}

#[derive(Default)]
pub struct Rdps {
    map: Mutex<HashMap<String, Arc<RdpProc>>>,
}

impl Rdps {
    pub fn send(&self, id: &str, v: Value) {
        if let Some(p) = self.map.lock().get(id) {
            p.send(&v);
        }
    }
    pub fn remove(&self, id: &str) {
        self.map.lock().remove(id);
    }
    /// Ends every helper (each one quits and is stopped as it is dropped), before an update replaces its file.
    pub fn close_all(&self) {
        self.map.lock().clear();
    }
}

#[derive(Clone, Serialize)]
struct RdpEvent {
    id: String,
    event: String,
    text: String,
    data: Value,
}

struct Log<'a> {
    app: &'a AppHandle,
    id: &'a str,
}

impl Sink for Log<'_> {
    fn info(&self, text: &str) {
        emit(self.app, self.id, "log", text, Value::Null);
    }
    fn error(&self, text: &str) {
        emit(self.app, self.id, "log", text, Value::Null);
    }
}

fn emit(app: &AppHandle, id: &str, event: &str, text: &str, data: Value) {
    let _ = app.emit("rdp-event", RdpEvent { id: id.into(), event: event.into(), text: text.into(), data });
}

fn status(app: &AppHandle, id: &str, state: &str, message: &str) {
    let _ = app.emit("term-status", json!({"id": id, "state": state, "message": message}));
}

fn helper_path() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("CABLEBAG_RDP_HELPER") {
        return Ok(PathBuf::from(p));
    }
    let exe_dir = std::env::current_exe()?.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rdp-helper/publish/cablebag-rdp.exe");
    // Development builds use the freshly published helper, not the copy the bundler left next to the exe.
    let order = if cfg!(debug_assertions) {
        vec![dev, exe_dir.join("cablebag-rdp.exe")]
    } else {
        vec![exe_dir.join("cablebag-rdp.exe"), exe_dir.join("rdp").join("cablebag-rdp.exe"), dev]
    };
    for p in order {
        if p.exists() {
            return Ok(p);
        }
    }
    Err(anyhow!("The RDP helper (cablebag-rdp.exe) was not found next to Cablebag."))
}

/// The web view does not clip its siblings, so it would paint over the RDP window placed on top of it.
/// Turn on clipping for the main window and every direct child.
#[cfg(windows)]
fn clip_children(hwnd: i64) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindow, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, GW_CHILD, GW_HWNDNEXT,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    };
    unsafe {
        let main = HWND(hwnd as isize as *mut core::ffi::c_void);
        let style = GetWindowLongPtrW(main, GWL_STYLE);
        SetWindowLongPtrW(main, GWL_STYLE, style | WS_CLIPCHILDREN.0 as isize);
        let mut child = GetWindow(main, GW_CHILD).ok();
        while let Some(c) = child {
            let s = GetWindowLongPtrW(c, GWL_STYLE);
            if s & WS_CLIPSIBLINGS.0 as isize == 0 {
                SetWindowLongPtrW(c, GWL_STYLE, s | WS_CLIPSIBLINGS.0 as isize);
                let _ = SetWindowPos(
                    c,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
                );
            }
            child = GetWindow(c, GW_HWNDNEXT).ok();
        }
    }
}

#[cfg(not(windows))]
fn clip_children(_hwnd: i64) {}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub scale: u32,
}

pub async fn open(app: AppHandle, id: String, session: Session, geo: Geometry) -> Result<()> {
    let state = app.state::<AppState>();
    let mut host = session.host.trim().to_string();
    let mut port = if session.port > 0 { session.port as u16 } else { 3389 };
    let mut user = session.username.clone();
    status(&app, &id, "connecting", "");

    // Optional SSH gateway: forward a local port to the RDP server through SSH first.
    let mut fwd = None;
    if let Some(gw) = session.data.get("ssh_gateway").filter(|g| !s(g, "host").is_empty()) {
        let hop = Hop {
            host: s(gw, "host"),
            port: gw.get("port").and_then(|p| p.as_u64()).unwrap_or(22) as u16,
            user: s(gw, "user"),
            key_path: s(gw, "key_path"),
            vault_item: s(gw, "vault_item"),
            session_id: String::new(),
            name: String::new(),
        };
        let log = Log { app: &app, id: &id };
        log.info(&format!("Opening SSH gateway {}...", hop.host));
        let conn = Arc::new(ssh::connect_chain(&app, &[hop], true, &log).await?);
        let f = tunnel::local_forward(conn, 0, host.clone(), port).await?;
        host = "127.0.0.1".into();
        port = f.port;
        fwd = Some(f);
    }

    // Password: from a linked Vaultwarden item, saved in Cablebag, or asked once.
    let vault_host = session.host.trim().to_string();
    let mut from_vw = None;
    let item = s(&session.data, "vault_item");
    if !item.is_empty() {
        match state.vw.login(&app, &item).await {
            Ok((u, pw)) => {
                if user.is_empty() {
                    user = u;
                }
                if !pw.is_empty() {
                    from_vw = Some(pw);
                }
            }
            Err(e) => emit(&app, &id, "log", &format!("Vault: {e}"), Value::Null),
        }
    }
    let mut saved = match from_vw {
        Some(pw) => Some(pw),
        None => state.vault.password(&state.store.lock(), "rdp", &vault_host, session.port, &user).ok().flatten(),
    };
    // No linked login: the vault may still have one for this host.
    if saved.is_none() && item.is_empty() {
        match state.vw.pick_login(&app, "rdp", &vault_host, &user, &session.name).await {
            Ok(Some((found, name, u, pw))) if !pw.is_empty() => {
                if user.is_empty() {
                    user = u;
                }
                emit(&app, &id, "log", &format!("Using the vault login \"{name}\"."), Value::Null);
                crate::bitwarden::link_session(&app, &session.id, &found);
                saved = Some(pw);
            }
            Ok(_) => {}
            Err(_) => {
                status(&app, &id, "closed", "Login cancelled.");
                return Ok(());
            }
        }
    }
    let password = match saved {
        Some(p) => p,
        None if !user.is_empty() => {
            let ans = state
                .prompts
                .ask(
                    &app,
                    PromptRequest {
                        id: String::new(),
                        title: format!("Log in to {vault_host}"),
                        message: format!("Password for {user} on {vault_host}"),
                        details: vec![],
                        fields: vec![
                            field("secret", "Password", "password", Value::String(String::new())),
                            field("save", "Remember in the password vault", "checkbox", Value::Bool(false)),
                        ],
                        buttons: vec![
                            button("ok", "Connect", "primary"),
                            button("remote", "Log in on the remote screen", "normal"),
                            button("cancel", "Cancel", "normal"),
                        ],
                        warning: false,
                    },
                )
                .await;
            match ans.button.as_str() {
                "ok" => {
                    let pw = ans.text("secret");
                    if ans.flag("save") {
                        let _ = state.vault.save_password(&state.store.lock(), "rdp", &vault_host, session.port, &user, &pw);
                    }
                    pw
                }
                "remote" => String::new(),
                _ => {
                    status(&app, &id, "closed", "Login cancelled.");
                    return Ok(());
                }
            }
        }
        None => String::new(),
    };

    let window = app.get_webview_window("main").ok_or_else(|| anyhow!("main window missing"))?;
    let hwnd = window.hwnd()?.0 as isize as i64;
    clip_children(hwnd);
    let mut cmd = Command::new(helper_path()?);
    cmd.args(["--parent", &hwnd.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| anyhow!("Could not start the RDP helper: {e}"))?;
    let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
    let proc_ = Arc::new(RdpProc { stdin: Mutex::new(stdin), child: Mutex::new(child), _tunnel: fwd });

    let app2 = app.clone();
    let id2 = id.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            let ev = s(&v, "event");
            match ev.as_str() {
                "connecting" => status(&app2, &id2, "connecting", ""),
                "connected" | "login" => status(&app2, &id2, "connected", ""),
                "disconnected" => status(&app2, &id2, "closed", &s(&v, "text")),
                "fatal" => status(&app2, &id2, "failed", "The remote desktop control reported a fatal error."),
                "error" => status(&app2, &id2, "failed", &s(&v, "text")),
                _ => {}
            }
            emit(&app2, &id2, &ev, &s(&v, "text"), v);
        }
        status(&app2, &id2, "closed", "");
    });

    proc_.send(&json!({"cmd": "bounds", "x": geo.x, "y": geo.y, "w": geo.w, "h": geo.h}));
    proc_.send(&json!({
        "cmd": "connect",
        "host": host,
        "port": port,
        "user": user,
        "domain": s(&session.data, "domain"),
        "password": password,
        "width": geo.w,
        "height": geo.h,
        "scale": geo.scale,
        "options": session.data,
    }));
    state.rdp.map.lock().insert(id, proc_);
    Ok(())
}

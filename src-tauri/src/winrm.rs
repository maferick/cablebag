//! PowerShell remoting (WinRM) sessions. Windows PowerShell on this PC runs in Cablebag's pseudo console and
//! opens the remote session itself (New-PSSession, then Enter-PSSession), so the remote prompt, tab
//! completion and every authentication Windows knows work as in any PowerShell window.
//!
//! Its command line only names a pipe. The settings and the password come through that pipe, which Cablebag
//! creates for this one process and checks the process id of: the password is never on a command line, on
//! the screen, in PowerShell's history or in a log, and inside PowerShell it only lives as a SecureString.
//! Through the same pipe PowerShell says whether the login worked.

#![cfg_attr(not(windows), allow(dead_code))]

use crate::store::Session;
use crate::term::{TermCmd, TermOut};
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

/// What PowerShell gets through the pipe, apart from the password.
#[derive(Debug, Clone, Serialize, PartialEq)]
struct Options {
    host: String,
    port: u16,
    ssl: bool,
    /// As New-PSSession names it: Negotiate, Kerberos, Credssp, Basic.
    auth: String,
    endpoint: String,
    user: String,
    skip_ca: bool,
    skip_cn: bool,
    /// For the assistant: one command to run instead of an interactive session.
    command: Option<String>,
    /// The server as the user knows it (host is 127.0.0.1 through WireGuard), for messages.
    #[serde(skip)]
    shown: String,
    #[serde(skip)]
    through_wg: bool,
}

/// The login a tab connected with, kept while the tab is open so the assistant can run commands there too.
/// An empty user name is the Windows login of the person at this PC.
#[derive(Clone)]
struct Login {
    user: String,
    password: String,
}

#[derive(Default)]
pub struct Logins {
    map: Mutex<HashMap<String, (String, Login)>>,
}

fn s(session: &Session, key: &str) -> String {
    session.data.get(key).and_then(|v| v.as_str()).unwrap_or("").trim().to_string()
}

fn flag(session: &Session, key: &str) -> bool {
    session.data.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn https(session: &Session) -> bool {
    flag(session, "https")
}

fn port(session: &Session) -> u16 {
    match session.port {
        p if p > 0 && p <= 65535 => p as u16,
        _ if https(session) => 5986,
        _ => 5985,
    }
}

fn auth(session: &Session) -> &'static str {
    match s(session, "auth").as_str() {
        "kerberos" => "Kerberos",
        "credssp" => "Credssp",
        "basic" => "Basic",
        _ => "Negotiate",
    }
}

/// Kinds of login that need a user name and password (Negotiate and Kerberos can use the Windows login).
fn needs_password(auth: &str) -> bool {
    matches!(auth, "Credssp" | "Basic")
}

/// Settings that cannot work, said before anything starts.
fn check(session: &Session, through_wg: bool) -> Result<()> {
    let a = auth(session);
    if a == "Basic" && !https(session) {
        bail!("Basic login only works over HTTPS: Windows will not send a password unencrypted. Turn on HTTPS for this session, or choose Negotiate.");
    }
    if a == "Kerberos" && through_wg {
        bail!("Kerberos needs the server's own name, and through WireGuard the server is reached on a local address. Choose Negotiate for this session.");
    }
    if a == "Kerberos" && session.host.trim().parse::<std::net::IpAddr>().is_ok() {
        bail!("Kerberos needs the server's name, not its IP address. Fill in the name, or choose Negotiate.");
    }
    Ok(())
}

/// A plain explanation for the errors people run into most, under PowerShell's own message.
fn hint(message: &str, o: &Options) -> Option<String> {
    let m = message.to_lowercase();
    let (host, port) = o.shown.rsplit_once(':').unwrap_or((&o.shown, ""));
    if m.contains("trustedhosts") {
        return Some(if o.through_wg {
            "Through WireGuard, Windows only sends this login over HTTPS. Turn on HTTPS for this session (the server needs a WinRM HTTPS listener).".into()
        } else {
            format!(
                "Windows only sends this login to a server outside your domain over HTTPS, or to a server on its list of trusted hosts. Turn on HTTPS for this session, or ask an administrator of this PC to add the server to that list (Set-Item WSMan:\\localhost\\Client\\TrustedHosts -Value {host} -Concatenate)."
            )
        });
    }
    if m.contains("certificate") || m.contains("skipcacheck") || m.contains("skipcncheck") {
        return Some("The server's certificate was not accepted: Windows does not trust who issued it, or it was made for another name. Use the name the certificate was made for, or, for a self-signed certificate, turn on \"Accept any certificate\" for this session.".into());
    }
    if m.contains("credssp") {
        return Some("CredSSP must be allowed on this PC first (by an administrator: Enable-WSManCredSSP -Role Client -DelegateComputer <server>) and on the server (Enable-WSManCredSSP -Role Server).".into());
    }
    if m.contains("access is denied") || m.contains("access denied") || m.contains("logon failure") || m.contains("user name or password is incorrect") {
        return Some("The server refused this login. Check the user name (DOMAIN\\user or user@domain for a domain account) and the password; the account also has to be an administrator or a member of Remote Management Users on the server.".into());
    }
    if m.contains("cannot connect to the destination") || m.contains("cannot complete the operation") || m.contains("network path was not found") || m.contains("unable to connect") {
        return Some(format!(
            "No WinRM service answered at {}. Check that PowerShell remoting is switched on there (Enable-PSRemoting, as an administrator), that the {} listener is set up, and that the firewall lets port {port} through.",
            o.shown,
            if o.ssl { "HTTPS" } else { "HTTP" },
        ));
    }
    None
}

/// The PowerShell that reads everything from the pipe and opens the session. No double quotes in here: Windows
/// PowerShell's command line would take them apart.
fn script(pipe: &str, interactive: bool) -> String {
    // Back at the local prompt means the remote session ended (or never started): so does this terminal.
    let prompt = if interactive { "function global:prompt { if (-not $Host.IsRunspacePushed) { [Environment]::Exit(0) }; 'PS> ' }\n" } else { "" };
    let head = format!(
        r#"{prompt}$ErrorActionPreference = 'Stop'
$w = $null
function Send-Cablebag($t) {{ $b = [Text.Encoding]::UTF8.GetBytes([string]$t); $w.Write([int]$b.Length); $w.Write($b); $w.Flush() }}
try {{
  $p = New-Object IO.Pipes.NamedPipeClientStream('.', '{pipe}', [IO.Pipes.PipeDirection]::InOut)
  $p.Connect(20000)
  $r = New-Object IO.BinaryReader($p)
  $w = New-Object IO.BinaryWriter($p)
  $o = [Text.Encoding]::UTF8.GetString($r.ReadBytes($r.ReadInt32())) | ConvertFrom-Json
  $b = $r.ReadBytes($r.ReadInt32()); $c = [Text.Encoding]::UTF8.GetChars($b); [Array]::Clear($b, 0, $b.Length)
  $pw = New-Object Security.SecureString; foreach ($x in $c) {{ $pw.AppendChar($x) }}; [Array]::Clear($c, 0, $c.Length); $pw.MakeReadOnly()
  $so = @{{ OpenTimeout = 60000 }}
  if ($o.skip_ca) {{ $so.SkipCACheck = $true; $so.SkipRevocationCheck = $true }}
  if ($o.skip_cn) {{ $so.SkipCNCheck = $true }}
  $a = @{{ ComputerName = $o.host; Port = $o.port; Authentication = $o.auth; SessionOption = (New-PSSessionOption @so) }}
  if ($o.ssl) {{ $a.UseSSL = $true }}
  if ($o.endpoint) {{ $a.ConfigurationName = $o.endpoint }}
  if ($o.user) {{ $a.Credential = New-Object Management.Automation.PSCredential($o.user, $pw) }}
  $s = New-PSSession @a
  Send-Cablebag 'ok'
}} catch {{ if ($w) {{ Send-Cablebag ('error:' + $_.Exception.Message) }}; [Environment]::Exit(1) }}
"#
    );
    let tail = if interactive {
        r#"$ErrorActionPreference = 'Continue'
Enter-PSSession -Session $s
"#
    } else {
        r#"try {
  $res = Invoke-Command -Session $s -ArgumentList $o.command -ScriptBlock {
    param($c)
    $global:LASTEXITCODE = 0
    $Error.Clear()
    $out = try { & ([scriptblock]::Create($c)) 2>&1 | Out-String -Width 200 } catch { $_ | Out-String -Width 200 }
    $code = if ($LASTEXITCODE) { $LASTEXITCODE } elseif ($Error.Count) { 1 } else { 0 }
    [pscustomobject]@{ exit = $code; output = [string]$out }
  }
  Send-Cablebag ([pscustomobject]@{ exit = $res.exit; output = $res.output } | ConvertTo-Json -Compress)
} catch { Send-Cablebag ([pscustomobject]@{ exit = 1; output = ($_ | Out-String -Width 200) } | ConvertTo-Json -Compress) }
Remove-PSSession $s
[Environment]::Exit(0)
"#
    };
    format!("{head}{tail}")
}

fn args(pipe: &str, interactive: bool) -> Vec<String> {
    let mut v: Vec<String> = vec!["-NoLogo".into(), "-NoProfile".into()];
    if interactive {
        v.push("-NoExit".into());
    } else {
        v.push("-NonInteractive".into());
    }
    v.push("-Command".into());
    v.push(script(pipe, interactive));
    v
}

/// A new pipe name, as PowerShell takes it (without \\.\pipe\).
fn pipe_name() -> String {
    format!("cablebag-winrm-{}", uuid::Uuid::new_v4().simple())
}

/// The login for a session: its vault login, a password saved in Cablebag, a login the vault holds for the
/// server, or a question; with Negotiate or Kerberos and no user name, the Windows login of this PC.
/// Also says whether a typed password should be remembered once it worked. None: the user cancelled.
async fn find_login(app: &AppHandle, session: &Session, out: &TermOut) -> Option<(Login, bool)> {
    let state = app.state::<AppState>();
    let host = session.host.trim().to_string();
    let mut user = session.username.trim().to_string();
    let item = s(session, "vault_item");
    if !item.is_empty() {
        match state.vw.login(app, &item).await {
            Ok((u, pw)) => {
                if user.is_empty() {
                    user = u;
                }
                if !pw.is_empty() {
                    out.info("Using the login from the vault.");
                    return Some((Login { user, password: pw }, false));
                }
            }
            Err(e) => out.error(&format!("Vault: {e}")),
        }
    }
    if !user.is_empty() {
        let saved = { state.vault.password(&state.store.lock(), "winrm", &host, session.port, &user).ok().flatten() };
        if let Some(pw) = saved {
            return Some((Login { user, password: pw }, false));
        }
    }
    if item.is_empty() {
        match state.vw.pick_login(app, "winrm", &host, &user, &session.name).await {
            Ok(Some((found, name, u, pw))) if !pw.is_empty() => {
                if user.is_empty() {
                    user = u;
                }
                out.info(&format!("Using the vault login \"{name}\"."));
                crate::bitwarden::link_session(app, &session.id, &found);
                return Some((Login { user, password: pw }, false));
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    if user.is_empty() {
        if !needs_password(auth(session)) {
            out.info("Using your Windows login.");
            return Some((Login { user, password: String::new() }, false));
        }
        let ans = state
            .prompts
            .ask(
                app,
                crate::prompt::PromptRequest {
                    id: String::new(),
                    title: format!("Log in to {host}"),
                    message: "Which user name should be used?".into(),
                    details: vec![],
                    fields: vec![crate::prompt::field("user", "User name", "text", serde_json::Value::String(String::new()))],
                    buttons: vec![crate::prompt::button("ok", "Continue", "primary"), crate::prompt::button("cancel", "Cancel", "normal")],
                    warning: false,
                },
            )
            .await;
        user = ans.text("user").trim().to_string();
        if ans.button != "ok" || user.is_empty() {
            return None;
        }
    }
    let (pw, save) = state.prompts.ask_secret(app, &format!("Log in to {host}"), &format!("Password for {user} on {host}"), "Password", true).await?;
    Some((Login { user, password: pw }, save))
}

/// Where PowerShell connects: the server itself, or a local port into the session's WireGuard network. Over
/// HTTPS through WireGuard the certificate is made for the server's name, not for 127.0.0.1, so Cablebag checks
/// that name itself first (Windows still checks who issued it) and PowerShell skips only the name check.
struct Target {
    host: String,
    port: u16,
    skip_cn: bool,
    through_wg: bool,
    _forward: Option<crate::wg::WgForward>,
}

async fn target(app: &AppHandle, session: &Session, info: &(dyn Fn(&str) + Sync)) -> Result<Target> {
    let host = session.host.trim().to_string();
    let port = port(session);
    let Some(wg) = crate::wg::of(session) else {
        return Ok(Target { host, port, skip_cn: false, through_wg: false, _forward: None });
    };
    info("Opening the WireGuard tunnel...");
    let f = crate::wg::local_forward(app, &wg, &host, port).await?;
    let local = f.port;
    let any = flag(session, "any_certificate");
    if https(session) && !any {
        check_certificate(&host, local).await?;
    }
    Ok(Target { host: "127.0.0.1".into(), port: local, skip_cn: https(session), through_wg: true, _forward: Some(f) })
}

/// Checks the server's certificate the way Windows does (trusted issuer, made for this name), through the
/// local port into the tunnel.
async fn check_certificate(host: &str, local: u16) -> Result<()> {
    let tcp = tokio::time::timeout(Duration::from_secs(15), tokio::net::TcpStream::connect(("127.0.0.1", local)))
        .await
        .map_err(|_| anyhow!("No answer from {host} through WireGuard within 15 seconds."))??;
    let tls = tokio_native_tls::TlsConnector::from(native_tls::TlsConnector::new()?);
    match tokio::time::timeout(Duration::from_secs(15), tls.connect(host, tcp)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => bail!(
            "The certificate of {host} was not accepted ({e}). Windows has to trust who issued it, and it has to be made for the name {host}. For a self-signed certificate, turn on \"Accept any certificate\" for this session."
        ),
        Err(_) => bail!("{host} did not finish the HTTPS handshake within 15 seconds."),
    }
}

fn options(session: &Session, t: &Target, login: &Login, command: Option<String>) -> Options {
    let any = https(session) && flag(session, "any_certificate");
    Options {
        host: t.host.clone(),
        port: t.port,
        ssl: https(session),
        auth: auth(session).into(),
        endpoint: s(session, "endpoint"),
        user: login.user.clone(),
        skip_ca: any,
        skip_cn: any || t.skip_cn,
        command,
        shown: format!("{}:{}", session.host.trim(), port(session)),
        through_wg: t.through_wg,
    }
}

#[cfg(windows)]
mod pipe {
    use anyhow::{anyhow, bail, Result};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    pub fn create(name: &str) -> Result<NamedPipeServer> {
        Ok(ServerOptions::new().first_pipe_instance(true).reject_remote_clients(true).max_instances(1).create(format!(r"\\.\pipe\{name}"))?)
    }

    /// Waits for PowerShell to connect and makes sure it is that process and no other.
    pub async fn accept(server: &NamedPipeServer, pid: Option<u32>) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(30), server.connect())
            .await
            .map_err(|_| anyhow!("PowerShell did not start within 30 seconds."))??;
        let mut client = 0u32;
        {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::Foundation::HANDLE;
            use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
            unsafe { GetNamedPipeClientProcessId(HANDLE(server.as_raw_handle()), &mut client) }.map_err(|e| anyhow!("{e}"))?;
        }
        if pid != Some(client) {
            bail!("Another program connected to the pipe meant for PowerShell, so Cablebag sent nothing and stopped.");
        }
        Ok(())
    }

    pub async fn send(server: &mut NamedPipeServer, bytes: &[u8]) -> Result<()> {
        server.write_all(&(bytes.len() as u32).to_le_bytes()).await?;
        server.write_all(bytes).await?;
        server.flush().await?;
        Ok(())
    }

    pub async fn receive(server: &mut NamedPipeServer) -> Result<String> {
        let n = server.read_u32_le().await? as usize;
        if n > 16 * 1024 * 1024 {
            bail!("PowerShell sent too much.");
        }
        let mut b = vec![0u8; n];
        server.read_exact(&mut b).await?;
        Ok(String::from_utf8_lossy(&b).into_owned())
    }
}

/// What came back after the login: Ok, or PowerShell's error message.
#[cfg(windows)]
async fn hand_over(server: &mut tokio::net::windows::named_pipe::NamedPipeServer, pid: Option<u32>, o: &Options, password: &str) -> Result<std::result::Result<(), String>> {
    pipe::accept(server, pid).await?;
    pipe::send(server, serde_json::to_string(o)?.as_bytes()).await?;
    pipe::send(server, password.as_bytes()).await?;
    let reply = pipe::receive(server).await.map_err(|_| anyhow!("Windows PowerShell stopped before it connected."))?;
    Ok(match reply.strip_prefix("error:") {
        Some(e) => Err(e.trim().to_string()),
        None => Ok(()),
    })
}

/// An interactive remote PowerShell in a terminal tab.
pub async fn run(app: AppHandle, id: String, session: Session, mut cols: u16, mut rows: u16, out: TermOut, mut rx: mpsc::UnboundedReceiver<TermCmd>) {
    let fail = |e: &str| {
        out.error(e);
        out.status("failed", e);
    };
    out.status("connecting", "");
    if session.host.trim().is_empty() {
        return fail("This session has no server.");
    }
    if let Err(e) = check(&session, crate::wg::of(&session).is_some()) {
        return fail(&e.to_string());
    }
    // The login and the way there first; the window may resize or close the tab meanwhile.
    let (t, found) = {
        let prepare = async {
            let t = target(&app, &session, &|m| out.info(m)).await?;
            let login = find_login(&app, &session, &out).await;
            Ok::<_, anyhow::Error>((t, login))
        };
        tokio::pin!(prepare);
        loop {
            tokio::select! {
                r = &mut prepare => match r {
                    Ok(v) => break v,
                    Err(e) => return fail(&e.to_string()),
                },
                cmd = rx.recv() => match cmd {
                    Some(TermCmd::Resize(c, r)) => { cols = c; rows = r; }
                    Some(TermCmd::Input(_)) => {}
                    Some(TermCmd::Close) | None => { out.status("closed", ""); return; }
                }
            }
        }
    };
    let Some((login, save)) = found else {
        out.info("Login cancelled.");
        out.status("closed", "Login cancelled.");
        return;
    };
    let o = options(&session, &t, &login, None);
    let shown = format!("{}:{}", session.host.trim(), port(&session));
    out.info(&format!(
        "Connecting to {shown} over {}{}...",
        if o.ssl { "HTTPS" } else { "HTTP" },
        if login.user.is_empty() { String::new() } else { format!(" as {}", login.user) }
    ));
    run_windows(app, id, session, cols, rows, out, rx, t, login, save, o).await;
}

#[cfg(not(windows))]
#[allow(clippy::too_many_arguments)]
async fn run_windows(_: AppHandle, _: String, _: Session, _: u16, _: u16, out: TermOut, _: mpsc::UnboundedReceiver<TermCmd>, _: Target, _: Login, _: bool, _: Options) {
    out.error("PowerShell remoting sessions need Windows.");
    out.status("failed", "PowerShell remoting sessions need Windows.");
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
async fn run_windows(app: AppHandle, id: String, session: Session, cols: u16, rows: u16, out: TermOut, mut rx: mpsc::UnboundedReceiver<TermCmd>, t: Target, login: Login, save: bool, o: Options) {
    let fail = |e: &str| {
        out.error(e);
        out.status("failed", e);
    };
    let name = pipe_name();
    let mut server = match pipe::create(&name) {
        Ok(p) => p,
        Err(e) => return fail(&format!("Could not prepare the login for PowerShell: {e}")),
    };
    let spec = crate::pty::LocalSpec { program: "powershell.exe".into(), args: args(&name, true), cwd: None, startup: None };
    let started = match crate::pty::start(spec, cols, rows, out.clone()) {
        Ok(s) => s,
        Err(e) => return fail(&format!("Could not start Windows PowerShell: {e}")),
    };
    let tx = started.tx.clone();
    let mut ended = started.ended;
    // The tab's input, size and closing go on to PowerShell from now on.
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            let close = matches!(cmd, TermCmd::Close);
            if tx.send(cmd).is_err() || close {
                break;
            }
        }
        let _ = tx.send(TermCmd::Close);
    });

    let result = tokio::select! {
        r = hand_over(&mut server, started.pid, &o, &login.password) => Some(r),
        _ = &mut ended => None,
    };
    drop(server);
    match result {
        None => {
            out.error("Windows PowerShell stopped before it connected.");
            out.status("failed", "PowerShell stopped.");
            return;
        }
        Some(Err(e)) => {
            let _ = started.tx.send(TermCmd::Close);
            return fail(&e.to_string());
        }
        Some(Ok(Err(message))) => {
            let mut text = format!("Could not open the remote session: {message}");
            if let Some(h) = hint(&message, &o) {
                text.push_str(&format!("\n\n{h}"));
            }
            out.error(&text);
            // A long text reaches the window later than a status: the status (with its "press R") goes after it.
            let _ = ended.await;
            tokio::time::sleep(Duration::from_millis(200)).await;
            out.status("failed", &message);
            return;
        }
        Some(Ok(Ok(()))) => {}
    }
    out.status("connected", "");
    if save {
        let state = app.state::<AppState>();
        let _ = state.vault.save_password(&state.store.lock(), "winrm", session.host.trim(), session.port, &login.user, &login.password);
    }
    let generation = uuid::Uuid::new_v4().to_string();
    app.state::<AppState>().winrm.map.lock().insert(id.clone(), (generation.clone(), login));
    let _ = ended.await;
    {
        let state = app.state::<AppState>();
        let mut map = state.winrm.map.lock();
        if map.get(&id).is_some_and(|(g, _)| *g == generation) {
            map.remove(&id);
        }
    }
    out.info("\r\nThe remote session ended.");
    out.status("closed", "");
    // The WireGuard port stays open until here.
    drop(t);
}

/// One command for the assistant, in a PowerShell that is not shown, with the login of the open tab.
pub async fn exec(app: &AppHandle, tab_id: &str, session: &Session, command: &str, timeout: Duration) -> Result<crate::agent::ExecResult> {
    let login = app
        .state::<AppState>()
        .winrm
        .map
        .lock()
        .get(tab_id)
        .map(|(_, l)| l.clone())
        .ok_or_else(|| anyhow!("This PowerShell tab is not connected."))?;
    let t = target(app, session, &|_| {}).await?;
    let o = options(session, &t, &login, Some(command.to_string()));
    exec_windows(&o, &login, timeout).await
}

#[cfg(not(windows))]
async fn exec_windows(_: &Options, _: &Login, _: Duration) -> Result<crate::agent::ExecResult> {
    bail!("PowerShell remoting sessions need Windows.")
}

#[cfg(windows)]
async fn exec_windows(o: &Options, login: &Login, timeout: Duration) -> Result<crate::agent::ExecResult> {
    let name = pipe_name();
    let mut server = pipe::create(&name)?;
    let mut child = tokio::process::Command::new("powershell.exe")
        .args(args(&name, false))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| anyhow!("Could not start Windows PowerShell: {e}"))?;
    let pid = child.id();
    let run = async {
        if let Err(e) = hand_over(&mut server, pid, o, &login.password).await? {
            let mut text = format!("Could not open the remote session: {e}");
            if let Some(h) = hint(&e, o) {
                text.push_str(&format!(" {h}"));
            }
            bail!(text);
        }
        let reply = pipe::receive(&mut server).await?;
        let v: serde_json::Value = serde_json::from_str(&reply).map_err(|_| anyhow!("PowerShell sent an answer Cablebag does not understand."))?;
        let (output, truncated) = crate::agent::limit_output(v["output"].as_str().unwrap_or("").to_string());
        Ok(crate::agent::ExecResult { output, exit: v["exit"].as_i64().map(|e| e as u32), timed_out: false, truncated })
    };
    let res = match tokio::time::timeout(timeout, run).await {
        Ok(r) => r,
        Err(_) => Ok(crate::agent::ExecResult { output: String::new(), exit: None, timed_out: true, truncated: false }),
    };
    let _ = child.kill().await;
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn session(data: serde_json::Value, port: i64, host: &str) -> Session {
        Session {
            id: String::new(),
            folder_id: None,
            name: String::new(),
            kind: "winrm".into(),
            host: host.into(),
            port,
            username: String::new(),
            data,
            comment: String::new(),
            sort: 0,
            last_used: 0,
        }
    }

    #[test]
    fn defaults() {
        let s = session(json!({}), 0, "srv");
        assert_eq!(port(&s), 5985);
        assert_eq!(auth(&s), "Negotiate");
        let s = session(json!({"https": true, "auth": "credssp"}), 0, "srv");
        assert_eq!(port(&s), 5986);
        assert_eq!(auth(&s), "Credssp");
        assert!(needs_password(auth(&s)));
        assert_eq!(port(&session(json!({"https": true}), 15986, "srv")), 15986);
    }

    #[test]
    fn impossible_settings() {
        assert!(check(&session(json!({"auth": "basic"}), 0, "srv"), false).is_err());
        assert!(check(&session(json!({"auth": "basic", "https": true}), 0, "srv"), false).is_ok());
        assert!(check(&session(json!({"auth": "kerberos"}), 0, "srv"), true).is_err());
        assert!(check(&session(json!({"auth": "kerberos"}), 0, "10.0.0.5"), false).is_err());
        assert!(check(&session(json!({"auth": "kerberos"}), 0, "srv.corp"), false).is_ok());
    }

    #[test]
    fn certificate_options() {
        let login = Login { user: "admin".into(), password: "x".into() };
        let direct = Target { host: "srv".into(), port: 5986, skip_cn: false, through_wg: false, _forward: None };
        let o = options(&session(json!({"https": true}), 0, "srv"), &direct, &login, None);
        assert!(o.ssl && !o.skip_ca && !o.skip_cn);
        let o = options(&session(json!({"https": true, "any_certificate": true}), 0, "srv"), &direct, &login, None);
        assert!(o.skip_ca && o.skip_cn);
        // Without HTTPS the certificate setting means nothing.
        let o = options(&session(json!({"any_certificate": true}), 0, "srv"), &direct, &login, None);
        assert!(!o.skip_ca && !o.skip_cn);
        let wg = Target { host: "127.0.0.1".into(), port: 50000, skip_cn: true, through_wg: true, _forward: None };
        let o = options(&session(json!({"https": true}), 0, "srv"), &wg, &login, None);
        assert!(!o.skip_ca && o.skip_cn && o.host == "127.0.0.1" && o.port == 50000);
    }

    #[test]
    fn nothing_secret_on_the_command_line() {
        let a = args(r"\\.\pipe\cablebag-winrm-x", true);
        let line = a.join(" ");
        assert!(!line.contains('"'), "double quotes break Windows PowerShell's command line");
        assert!(line.contains("-NoExit") && line.contains("Enter-PSSession"));
        let a = args(r"\\.\pipe\cablebag-winrm-x", false);
        assert!(a.contains(&"-NonInteractive".to_string()) && !a.join(" ").contains('"'));
        // The options JSON that goes through the pipe has no password field.
        let o = options(&session(json!({}), 0, "srv"), &Target { host: "srv".into(), port: 5985, skip_cn: false, through_wg: false, _forward: None }, &Login { user: "u".into(), password: "secret".into() }, None);
        assert!(!serde_json::to_string(&o).unwrap().contains("secret"));
    }

    /// Both scripts as Windows PowerShell reads them: no syntax errors.
    #[cfg(windows)]
    #[test]
    fn scripts_parse() {
        for interactive in [true, false] {
            let path = std::env::temp_dir().join(format!("cablebag-winrm-test-{}.ps1", uuid::Uuid::new_v4().simple()));
            std::fs::write(&path, script("x", interactive)).unwrap();
            let check = format!(
                "$e = $null; [void][Management.Automation.Language.Parser]::ParseFile('{}', [ref]$null, [ref]$e); if ($e.Count) {{ $e | ForEach-Object {{ $_.Message }}; exit 1 }}",
                path.display()
            );
            let out = std::process::Command::new("powershell.exe").args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &check]).output().unwrap();
            let _ = std::fs::remove_file(&path);
            assert!(out.status.success(), "interactive={interactive}: {}", String::from_utf8_lossy(&out.stdout));
        }
    }

    /// The whole way up to the connection: PowerShell starts, takes the settings and the password from the pipe,
    /// tries to connect where nothing listens, and says so through the pipe.
    #[cfg(windows)]
    #[tokio::test]
    async fn hand_over_to_powershell() {
        let o = Options {
            host: "127.0.0.1".into(),
            port: 1,
            ssl: false,
            auth: "Negotiate".into(),
            endpoint: String::new(),
            user: String::new(),
            skip_ca: false,
            skip_cn: false,
            command: Some("Get-Date".into()),
            shown: "127.0.0.1:1".into(),
            through_wg: false,
        };
        let login = Login { user: String::new(), password: String::new() };
        let e = exec_windows(&o, &login, Duration::from_secs(90)).await.unwrap_err().to_string();
        assert!(e.starts_with("Could not open the remote session:"), "{e}");
        assert!(e.contains("No WinRM service answered at 127.0.0.1:1"), "{e}");
    }

    #[test]
    fn hints() {
        let direct = Target { host: "srv".into(), port: 5985, skip_cn: false, through_wg: false, _forward: None };
        let login = Login { user: String::new(), password: String::new() };
        let o = options(&session(json!({}), 0, "srv"), &direct, &login, None);
        assert!(hint("... the destination machine must be added to the TrustedHosts configuration setting ...", &o).unwrap().contains("-Value srv -Concatenate"));
        assert!(hint("The SSL certificate is signed by an unknown certificate authority.", &o).is_some());
        assert!(hint("Access is denied.", &o).unwrap().contains("Remote Management Users"));
        assert!(hint("WinRM cannot complete the operation. Verify that the specified computer name is valid", &o).unwrap().contains("srv:5985"));
        assert!(hint("something else", &o).is_none());
        // Through WireGuard the messages name the server, not the local port into the tunnel.
        let wg = Target { host: "127.0.0.1".into(), port: 50000, skip_cn: false, through_wg: true, _forward: None };
        let o = options(&session(json!({}), 0, "srv"), &wg, &login, None);
        assert!(hint("... must be added to the TrustedHosts ...", &o).unwrap().contains("HTTPS"));
        let h = hint("WinRM cannot complete the operation.", &o).unwrap();
        assert!(h.contains("srv:5985") && !h.contains("127.0.0.1"), "{h}");
    }
}

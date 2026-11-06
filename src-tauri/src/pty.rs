//! Local terminals (PowerShell, cmd, WSL, Git Bash on Windows; the shells from /etc/shells on
//! Linux) through the platform's pseudo terminal.

use crate::term::{TermCmd, TermOut};
use anyhow::{anyhow, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalShell {
    pub id: String,
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
}

fn exists(p: &str) -> bool {
    Path::new(p).exists()
}

/// Names of installed WSL distributions. `wsl -l -q` prints UTF-16.
#[cfg(windows)]
pub fn wsl_distros() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(out) = std::process::Command::new("wsl.exe")
        .args(["-l", "-q"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return vec![];
    };
    let words: Vec<u16> = out.stdout.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&words)
        .lines()
        .map(|l| l.trim_matches(|c: char| c.is_whitespace() || c == '\0').to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(windows)]
pub fn list_shells() -> Vec<LocalShell> {
    let mut v = Vec::new();
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
    let pwsh = format!(r"{pf}\PowerShell\7\pwsh.exe");
    if exists(&pwsh) {
        v.push(LocalShell { id: "pwsh".into(), name: "PowerShell 7".into(), program: pwsh, args: vec!["-NoLogo".into()] });
    }
    v.push(LocalShell {
        id: "powershell".into(),
        name: "Windows PowerShell".into(),
        program: "powershell.exe".into(),
        args: vec!["-NoLogo".into()],
    });
    v.push(LocalShell { id: "cmd".into(), name: "Command Prompt".into(), program: "cmd.exe".into(), args: vec![] });
    let gitbash = format!(r"{pf}\Git\bin\bash.exe");
    if exists(&gitbash) {
        v.push(LocalShell {
            id: "gitbash".into(),
            name: "Git Bash".into(),
            program: gitbash,
            args: vec!["--login".into(), "-i".into()],
        });
    }
    if exists(r"C:\msys64\usr\bin\bash.exe") {
        v.push(LocalShell {
            id: "msys2".into(),
            name: "MSYS2".into(),
            program: r"C:\msys64\usr\bin\bash.exe".into(),
            args: vec!["--login".into(), "-i".into()],
        });
    }
    for d in wsl_distros() {
        v.push(LocalShell {
            id: format!("wsl:{d}"),
            name: format!("WSL: {d}"),
            program: "wsl.exe".into(),
            args: vec!["-d".into(), d.clone(), "--cd".into(), "~".into()],
        });
    }
    v
}

#[cfg(not(windows))]
pub fn list_shells() -> Vec<LocalShell> {
    // /etc/shells is the system's own list; bash on many distros only appears there via its symlink.
    let mut paths = std::fs::read_to_string("/etc/shells")
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && exists(l))
        .map(str::to_string)
        .collect::<Vec<_>>();
    for p in paths.clone() {
        if let Ok(r) = std::fs::canonicalize(&p) {
            let r = r.to_string_lossy().to_string();
            if !paths.contains(&r) && exists(&r) {
                paths.push(r);
            }
        }
    }
    if let Some(shell) = std::env::var("SHELL").ok().filter(|s| exists(s)) {
        if !paths.contains(&shell) {
            paths.push(shell);
        }
    }
    paths.sort();
    paths.dedup();
    paths.into_iter().filter_map(unix_shell).collect()
}

#[cfg(not(windows))]
fn unix_shell(path: String) -> Option<LocalShell> {
    let name = match path.rsplit('/').next().unwrap_or("") {
        "bash" => "Bash",
        "zsh" => "Zsh",
        "fish" => "Fish",
        "dash" => "Dash",
        "sh" => "Sh",
        "nu" => "Nushell",
        "pwsh" => "PowerShell 7",
        _ => return None, // screens and the like are not terminals to start a session in
    };
    let args = if name.ends_with("PowerShell 7") { vec!["-NoLogo".into()] } else { vec![] };
    Some(LocalShell { id: path.rsplit('/').next().unwrap_or("").into(), name: name.into(), program: path, args })
}

#[cfg(all(test, not(windows)))]
mod unix_tests {
    use super::unix_shell;

    #[test]
    fn names_known_shells_and_skips_the_rest() {
        assert_eq!(unix_shell("/usr/bin/zsh".into()).unwrap().name, "Zsh");
        assert_eq!(unix_shell("/usr/bin/pwsh".into()).unwrap().args, ["-NoLogo"]);
        assert!(unix_shell("/usr/bin/nologin".into()).is_none());
    }
}

pub struct LocalSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub startup: Option<String>,
}

/// A program running in a pseudo console: where to send input, its process id, and its exit code once it ends.
pub struct Started {
    pub tx: mpsc::UnboundedSender<TermCmd>,
    pub pid: Option<u32>,
    pub ended: tokio::sync::oneshot::Receiver<u32>,
}

pub fn spawn(spec: LocalSpec, cols: u16, rows: u16, out: TermOut) -> Result<mpsc::UnboundedSender<TermCmd>> {
    let program = spec.program.clone();
    let started = start(spec, cols, rows, out.clone()).map_err(|e| anyhow!("could not start {program}: {e}"))?;
    out.status("connected", "");
    let ended = started.ended;
    std::thread::spawn(move || {
        let code = ended.blocking_recv().unwrap_or(1);
        out.info(&format!("\r\nProcess ended (exit code {code})."));
        out.status("closed", "");
    });
    Ok(started.tx)
}

/// Starts the program and connects it to the terminal, without any status lines of its own.
pub fn start(spec: LocalSpec, cols: u16, rows: u16, out: TermOut) -> Result<Started> {
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| anyhow!("could not open a pseudo console: {e}"))?;
    let mut cmd = CommandBuilder::new(&spec.program);
    cmd.args(&spec.args);
    let cwd = spec
        .cwd
        .filter(|c| !c.is_empty() && Path::new(c).is_dir())
        .or_else(|| dirs::home_dir().map(|h| h.to_string_lossy().to_string()));
    if let Some(c) = cwd {
        cmd.cwd(c);
    }
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "Cablebag");
    let mut child = pair.slave.spawn_command(cmd).map_err(|e| anyhow!("{e}"))?;
    drop(pair.slave);
    let pid = child.process_id();

    let mut reader = pair.master.try_clone_reader().map_err(|e| anyhow!("{e}"))?;
    let mut writer = pair.master.take_writer().map_err(|e| anyhow!("{e}"))?;
    let master = pair.master;
    let (tx, mut rx) = mpsc::unbounded_channel::<TermCmd>();

    if let Some(s) = spec.startup.filter(|s| !s.trim().is_empty()) {
        let _ = tx.send(TermCmd::Input(format!("{}\r", s.replace('\n', "\r")).into_bytes()));
    }

    std::thread::spawn(move || {
        let mut buf = vec![0u8; 32 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => out.data(&buf[..n]),
            }
        }
    });

    let mut killer = child.clone_killer();
    std::thread::spawn(move || {
        while let Some(cmd) = rx.blocking_recv() {
            match cmd {
                TermCmd::Input(b) => {
                    if writer.write_all(&b).is_err() {
                        break;
                    }
                    let _ = writer.flush();
                }
                TermCmd::Resize(c, r) => {
                    let _ = master.resize(PtySize { rows: r, cols: c, pixel_width: 0, pixel_height: 0 });
                }
                TermCmd::Close => {
                    let _ = killer.kill();
                    break;
                }
            }
        }
    });

    let (done, ended) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let code = child.wait().map(|s| s.exit_code()).unwrap_or(1);
        let _ = done.send(code);
    });
    Ok(Started { tx, pid, ended })
}

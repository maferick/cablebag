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
    // /etc/shells is the system's own list. /bin is often a symlink into /usr/bin there, so an entry
    // is kept per real program: the first path that names it becomes that one entry. $SHELL always
    // ends up first, because that is the default for new tabs, and it stays even when the system
    // does not list it (a login shell someone chose is wanted, whatever /etc/shells says).
    let own = std::env::var("SHELL").ok().filter(|s| exists(s));
    let listed: Vec<String> = std::fs::read_to_string("/etc/shells")
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut rest: Vec<LocalShell> = Vec::new();
    for p in own.iter().chain(listed.iter()) {
        if !exists(p) {
            continue;
        }
        let real = std::fs::canonicalize(p).map(|r| r.to_string_lossy().to_string()).unwrap_or_else(|_| p.clone());
        if !seen.insert(real) {
            continue;
        }
        rest.extend(unix_shell(p.clone()));
    }
    rest.sort_by(|a, b| a.program.cmp(&b.program));
    if let Some(o) = own {
        rest.retain(|s| s.program != o);
        rest.insert(0, own_shell(o));
    }
    rest
}

#[cfg(not(windows))]
fn unix_shell(path: String) -> Option<LocalShell> {
    let file = path.rsplit('/').next().unwrap_or("").to_string();
    let name = match file.as_str() {
        "bash" => "Bash",
        "zsh" => "Zsh",
        "fish" => "Fish",
        "dash" => "Dash",
        "sh" => "Sh",
        "nu" => "Nushell",
        "pwsh" => "PowerShell 7",
        "ksh" | "ksh93" => "Ksh",
        "csh" | "tcsh" => "Tcsh",
        "elvish" => "Elvish",
        "xonsh" => "Xonsh",
        // screens and account tools are not terminals to start a session in.
        "nologin" | "false" | "git-shell" | "tmux" | "screen" | "byobu" | "su" | "sudo" => return None,
        // a shell someone installed under its own name is named by its file name.
        other => return Some(shell_entry(path, pretty(other))),
    };
    Some(shell_entry(path, name.to_string()))
}

/// The entry for a path chosen as someone's login shell, whatever it is called.
#[cfg(not(windows))]
fn own_shell(path: String) -> LocalShell {
    let name = pretty(path.rsplit('/').next().unwrap_or("")).to_string();
    shell_entry(path, name)
}

/// A display name from a file name, capitalised: "ksh" to "Ksh", "my-shell" to "My-shell".
#[cfg(not(windows))]
fn pretty(file: &str) -> String {
    let mut chars = file.chars();
    let mut name: String = chars.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    name.push_str(chars.as_str());
    name
}

#[cfg(not(windows))]
fn shell_entry(path: String, name: String) -> LocalShell {
    let file = path.rsplit('/').next().unwrap_or("").to_string();
    let args = if file == "pwsh" { vec!["-NoLogo".into()] } else { vec![] };
    LocalShell { id: file, name, program: path, args }
}

#[cfg(all(test, not(windows)))]
mod unix_tests {
    use super::*;

    #[test]
    fn names_known_shells_and_skips_the_rest() {
        assert_eq!(unix_shell("/usr/bin/zsh".into()).unwrap().name, "Zsh");
        assert_eq!(unix_shell("/usr/bin/pwsh".into()).unwrap().args, ["-NoLogo"]);
        assert!(unix_shell("/usr/bin/nologin".into()).is_none());
        assert!(unix_shell("/usr/bin/tmux".into()).is_none());
        // Other login shells get named by their file name instead of disappearing.
        assert_eq!(unix_shell("/usr/bin/ksh".into()).unwrap().name, "Ksh");
        assert_eq!(unix_shell("/usr/local/bin/elvish".into()).unwrap().name, "Elvish");
        assert_eq!(unix_shell("/usr/local/bin/myshell".into()).unwrap().name, "Myshell");
    }

    #[test]
    fn one_entry_per_real_program_and_the_login_shell_first() {
        let shells = list_shells();
        let real: Vec<String> = shells.iter().map(|s| std::fs::canonicalize(&s.program).unwrap_or_default().to_string_lossy().to_string()).collect();
        assert_eq!(real.len(), real.iter().collect::<std::collections::HashSet<_>>().len(), "{shells:?}");
        match std::env::var("SHELL").ok().filter(|s| std::path::Path::new(s).exists()) {
            Some(own) => assert_eq!(shells.first().map(|s| std::fs::canonicalize(&s.program).unwrap()), std::fs::canonicalize(own).ok()),
            None => {}
        }
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

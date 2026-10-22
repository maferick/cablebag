//! The assistant's hands on SSH: deciding what a command does, and running it on its own channel of the
//! tab's SSH login so its output and exit code are exact.
//!
//! What a command does is decided here, never by the model: a fixed list of commands that only read, a
//! list of dangerous ones, and "changes" for everything else, including anything this code cannot see
//! through (command substitution, redirection into files). A sudo password is fed to sudo here, from the
//! session's vault login, a saved password or a question to the user; it never goes to the window or the
//! model.

use crate::store::Session;
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use russh::ChannelMsg;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    /// Only looks: runs without asking where reading is allowed by itself.
    Reads,
    /// Changes something, or cannot be told apart from a change: always asks unless an "always" rule says so.
    Changes,
    /// Can destroy data or cut off the server: always asks, never "always", refused in protected folders.
    Dangerous,
}

#[derive(Debug, Serialize)]
pub struct Verdict {
    pub effect: Effect,
    /// Needs root (starts with sudo).
    pub root: bool,
    /// Why, in plain words, for the approval card.
    pub why: String,
    /// The kind of command, for "always approve": the program and its first word, such as "systemctl restart".
    pub kind: String,
}

/// Programs that only read, whatever their arguments (apart from the exceptions in `reads`).
const READ_PROGRAMS: &[&str] = &[
    "ls", "ll", "cat", "head", "tail", "grep", "egrep", "fgrep", "zgrep", "zcat", "wc", "sort", "uniq", "cut", "awk", "column",
    "df", "du", "free", "uptime", "ps", "pgrep", "pstree", "top", "uname", "hostname", "hostnamectl", "whoami", "id", "groups",
    "who", "w", "last", "date", "timedatectl", "stat", "file", "which", "whereis", "type", "lsblk", "blkid", "lscpu", "lsmem",
    "lspci", "lsusb", "lsof", "mount", "findmnt", "ip", "ss", "netstat", "ping", "traceroute", "tracepath", "dig", "nslookup",
    "host", "getent", "env", "printenv", "echo", "printf", "pwd", "realpath", "readlink", "basename", "dirname", "md5sum",
    "sha1sum", "sha256sum", "diff", "cmp", "tree", "nproc", "vmstat", "iostat", "mpstat", "dmesg", "journalctl", "uptime",
    "pveversion", "pvesm", "zfs", "zpool", "nginx", "apachectl", "openssl", "curl", "wget", "jq", "yq", "less", "more", "true",
];

/// Destroys data, cuts off the server or hands over control: never run without a red card.
fn dangerous(words: &[&str], line: &str) -> Option<&'static str> {
    let w0 = words.first().copied().unwrap_or("");
    let has = |x: &str| words.iter().any(|w| *w == x);
    let rec = words.iter().any(|w| w.starts_with('-') && !w.starts_with("--") && (w.contains('r') || w.contains('R')));
    let force = words.iter().any(|w| w.starts_with('-') && !w.starts_with("--") && w.contains('f'));
    match w0 {
        "rm" if rec && force => Some("removes files and folders without asking"),
        "rm" if words.iter().any(|w| matches!(*w, "/" | "/*" | "~" | "/etc" | "/var" | "/home" | "/usr" | "/boot" | "/root")) => Some("removes a system folder"),
        "mkfs" | "mkswap" | "wipefs" | "fdisk" | "sfdisk" | "parted" | "sgdisk" => Some("changes disks or partitions"),
        w if w.starts_with("mkfs.") => Some("formats a disk"),
        "dd" if words.iter().any(|w| w.starts_with("of=/dev/")) => Some("writes straight onto a disk"),
        "shutdown" | "reboot" | "poweroff" | "halt" | "init" => Some("stops or restarts the server"),
        "systemctl" if words.iter().any(|w| matches!(*w, "reboot" | "poweroff" | "halt" | "kexec" | "rescue" | "emergency")) => Some("stops or restarts the server"),
        "systemctl" if words.iter().any(|w| matches!(*w, "stop" | "disable" | "mask")) && words.iter().any(|w| w.starts_with("ssh")) => Some("may cut off this SSH connection"),
        "iptables" | "ip6tables" | "nft" if has("-F") || has("flush") => Some("removes firewall rules, which can cut off this connection"),
        "ufw" if has("enable") || has("reset") => Some("changes the firewall, which can cut off this connection"),
        "userdel" | "deluser" | "passwd" | "chpasswd" | "usermod" => Some("changes user accounts or passwords"),
        "chmod" | "chown" if rec && words.iter().any(|w| *w == "/" || *w == "/etc" || *w == "/usr" || *w == "/var") => Some("changes rights on a whole system folder"),
        "zpool" if has("destroy") || has("labelclear") => Some("destroys a storage pool"),
        "zfs" if has("destroy") || has("rollback") => Some("destroys or rolls back a dataset"),
        "qm" | "pct" if has("destroy") => Some("deletes a virtual machine or container"),
        "lvremove" | "vgremove" | "pvremove" => Some("deletes volumes"),
        "kill" | "pkill" | "killall" if has("1") || has("-9") && has("1") => Some("stops the init process"),
        "crontab" if has("-r") => Some("removes all scheduled jobs"),
        "docker" | "podman" if has("prune") && (has("-a") || has("--all") || has("--volumes")) => Some("deletes containers, images or volumes"),
        _ if line.contains(":(){") => Some("starts processes without end"),
        _ if line.contains("> /dev/sd") || line.contains(">/dev/sd") || line.contains("> /dev/nvme") => Some("writes straight onto a disk"),
        _ => None,
    }
}

/// Whether one simple command (no separators) only reads.
fn reads(words: &[&str]) -> bool {
    let w0 = words.first().copied().unwrap_or("");
    let rest = &words[1.min(words.len())..];
    let has = |x: &str| rest.iter().any(|w| *w == x);
    match w0 {
        "find" => !rest.iter().any(|w| matches!(*w, "-delete" | "-exec" | "-execdir" | "-ok" | "-fprint" | "-fprintf" | "-fls")),
        "sed" => !rest.iter().any(|w| w.starts_with("-i") || *w == "--in-place"),
        "systemctl" => rest.first().is_some_and(|s| matches!(*s, "status" | "is-active" | "is-enabled" | "is-failed" | "list-units" | "list-unit-files" | "list-timers" | "show" | "cat" | "list-dependencies" | "--failed")),
        "journalctl" => !rest.iter().any(|w| w.starts_with("--vacuum") || *w == "--rotate" || *w == "--flush"),
        "ip" => !rest.iter().any(|w| matches!(*w, "add" | "del" | "delete" | "set" | "change" | "replace" | "flush" | "append")),
        "docker" | "podman" => rest.first().is_some_and(|s| matches!(*s, "ps" | "logs" | "inspect" | "images" | "stats" | "top" | "version" | "info" | "port" | "diff" | "history")),
        "docker-compose" => rest.first().is_some_and(|s| matches!(*s, "ps" | "logs" | "config" | "images" | "top")),
        "kubectl" => rest.first().is_some_and(|s| matches!(*s, "get" | "describe" | "logs" | "top" | "version" | "explain" | "api-resources" | "cluster-info")),
        "git" => rest.first().is_some_and(|s| matches!(*s, "status" | "log" | "diff" | "show" | "branch" | "remote" | "rev-parse" | "describe" | "blame")),
        "apt" => rest.first().is_some_and(|s| matches!(*s, "list" | "show" | "search" | "policy")),
        "apt-cache" | "dpkg-query" => true,
        "dpkg" => rest.first().is_some_and(|s| matches!(*s, "-l" | "-L" | "-s" | "--list" | "--status" | "--listfiles")),
        "rpm" => rest.first().is_some_and(|s| s.starts_with("-q")),
        "dnf" | "yum" => rest.first().is_some_and(|s| matches!(*s, "list" | "info" | "search" | "repolist" | "check-update")),
        "qm" | "pct" => rest.first().is_some_and(|s| matches!(*s, "list" | "config" | "status" | "pending" | "listsnapshot")),
        "zpool" => rest.first().is_some_and(|s| matches!(*s, "status" | "list" | "iostat" | "get" | "history")),
        "zfs" => rest.first().is_some_and(|s| matches!(*s, "list" | "get")),
        "pvesm" => rest.first().is_some_and(|s| matches!(*s, "status" | "list")),
        "nginx" => has("-t") || has("-T") || has("-v") || has("-V"),
        "apachectl" => has("-t") || has("-S") || has("configtest"),
        "openssl" => rest.first().is_some_and(|s| matches!(*s, "x509" | "s_client" | "version" | "crl" | "verify")),
        "curl" => !rest.iter().any(|w| matches!(*w, "-X" | "--request" | "-d" | "--data" | "-F" | "--form" | "-T" | "--upload-file" | "-o" | "-O" | "--output")),
        "wget" => has("--spider") || has("-O-") || has("-qO-"),
        "dmesg" => !has("-C") && !has("-c") && !has("--clear"),
        "hostname" | "hostnamectl" | "date" | "timedatectl" => rest.is_empty() || rest.first() == Some(&"status"),
        "top" => has("-b") || has("-bn1"),
        "mount" => rest.is_empty() || rest.iter().all(|w| w.starts_with('-') && (w.contains('l') || *w == "-t")),
        "awk" => !rest.iter().any(|w| w.contains("system(") || w.contains("> ") || w.contains(">\"")),
        // `command -v x` only looks up where a program is; `command x` would run it.
        "command" => rest.first().is_some_and(|s| *s == "-v" || *s == "-V"),
        "test" | "[" | "[[" => true,
        _ => READ_PROGRAMS.contains(&w0),
    }
}

/// Splits on ; && || | and newlines, outside quotes.
fn segments(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut sq, mut dq) = (false, false);
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' if !dq => sq = !sq,
            '"' if !sq => dq = !dq,
            // An & next to > belongs to a redirection (2>&1, &>file), not between commands.
            '&' if i > 0 && chars[i - 1] == '>' || chars.get(i + 1) == Some(&'>') => {}
            ';' | '\n' | '|' | '&' if !sq && !dq => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
                if (c == '|' || c == '&') && chars.get(i + 1) == Some(&c) {
                    i += 1;
                }
                i += 1;
                continue;
            }
            _ => {}
        }
        cur.push(c);
        i += 1;
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// Splits one command into words, keeping quoted parts together (quotes removed).
fn words(seg: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut sq, mut dq, mut any) = (false, false, false);
    for c in seg.chars() {
        match c {
            '\'' if !dq => {
                sq = !sq;
                any = true;
            }
            '"' if !sq => {
                dq = !dq;
                any = true;
            }
            c if c.is_whitespace() && !sq && !dq => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn classify(line: &str) -> Verdict {
    let line = line.trim();
    let mut effect = Effect::Reads;
    let mut root = false;
    let mut why = String::from("only reads");
    let mut kind = String::new();
    // Anything this code cannot see through counts as a change.
    let opaque = line.contains("$(") || line.contains('`') || line.contains("<(") || line.contains(">(");
    let writes = {
        // A redirection into a file (2>&1 and >/dev/null are fine).
        let cleaned = line.replace("2>&1", "").replace(">/dev/null", "").replace("> /dev/null", "").replace("2>/dev/null", "");
        let mut q = (false, false);
        cleaned.chars().any(|c| {
            match c {
                '\'' if !q.1 => q.0 = !q.0,
                '"' if !q.0 => q.1 = !q.1,
                _ => {}
            }
            c == '>' && !q.0 && !q.1
        })
    };
    for seg in segments(line) {
        let ws = words(&seg);
        let mut w: Vec<&str> = ws.iter().map(String::as_str).collect();
        // Leading VAR=value assignments and sudo with its options.
        while w.first().is_some_and(|x| x.contains('=') && !x.starts_with('-') && !x.starts_with('=')) {
            w.remove(0);
        }
        if w.first() == Some(&"sudo") {
            root = true;
            w.remove(0);
            while w.first().is_some_and(|x| x.starts_with('-')) {
                let opt = w.remove(0);
                if matches!(opt, "-u" | "-g" | "-C" | "-p") && !w.is_empty() {
                    w.remove(0);
                }
            }
        }
        if w.is_empty() {
            continue;
        }
        if kind.is_empty() {
            kind = w.iter().take(2).filter(|x| !x.starts_with('-')).copied().collect::<Vec<_>>().join(" ");
        }
        if let Some(reason) = dangerous(&w, line) {
            return Verdict { effect: Effect::Dangerous, root, why: reason.to_string(), kind };
        }
        if effect == Effect::Reads && !reads(&w) {
            effect = Effect::Changes;
            why = format!("{} can change the server", w[0]);
        }
    }
    if opaque || writes {
        if effect == Effect::Reads {
            effect = Effect::Changes;
            why = if writes { "writes into a file".into() } else { "runs a command inside the command, which cannot be checked".into() };
        }
        // What it writes or runs differs every time: no "always" for this one.
        kind.clear();
    }
    Verdict { effect, root, why, kind }
}

// ---- Running ----

#[derive(Debug, Serialize, Default)]
pub struct ExecResult {
    pub output: String,
    pub exit: Option<u32>,
    pub timed_out: bool,
    pub truncated: bool,
}

/// At most this much output goes back to the model.
const MAX_OUTPUT: usize = 48 * 1024;

/// The password sudo asks for: the session's vault login, a saved password, or a question to the user.
async fn sudo_password(app: &AppHandle, session: &Session) -> Result<String> {
    let state = app.state::<AppState>();
    let item = session.data.get("vault_item").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if !item.is_empty() {
        if let Ok((_, pw)) = state.vw.login(app, &item).await {
            if !pw.is_empty() {
                return Ok(pw);
            }
        }
    }
    let saved = { state.vault.password(&state.store.lock(), "ssh", &session.host, session.port, &session.username).ok().flatten() };
    if let Some(pw) = saved {
        return Ok(pw);
    }
    let (pw, _) = state
        .prompts
        .ask_secret(app, &format!("sudo on {}", session.host), &format!("The assistant needs root for a command you approved. Password for {} (goes to sudo only, never to the AI):", session.username), "Password", false)
        .await
        .ok_or_else(|| anyhow!("No password given for sudo."))?;
    Ok(pw)
}

/// Runs one command on its own channel of the tab's SSH login.
/// `session` is the tab's saved session, used only to find the sudo password.
pub async fn exec(app: &AppHandle, tab_id: &str, session: &Session, command: &str, root: bool, timeout: Duration) -> Result<ExecResult> {
    let conn = app.state::<AppState>().ssh.lock().get(tab_id).cloned().ok_or_else(|| anyhow!("This SSH tab is not connected."))?;
    let mut channel = tokio::time::timeout(Duration::from_secs(15), conn.handle.channel_open_session())
        .await
        .map_err(|_| anyhow!("The server did not open a channel for the command in time."))??;
    let quoted = format!("'{}'", command.replace('\'', r"'\''"));
    let line = if root {
        // -S reads the password from the channel, -p '' keeps its question out of the output, -k asks every
        // time so the password always goes the same way.
        format!("sudo -S -k -p '' -- sh -c {quoted}")
    } else {
        format!("sh -c {quoted}")
    };
    tokio::time::timeout(Duration::from_secs(15), channel.exec(true, line.as_bytes()))
        .await
        .map_err(|_| anyhow!("The server did not start the command in time."))??;
    if root {
        let pw = sudo_password(app, session).await?;
        channel.data(format!("{pw}\n").as_bytes()).await?;
    }
    let mut res = ExecResult::default();
    let mut buf: Vec<u8> = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let msg = tokio::select! {
            m = channel.wait() => m,
            _ = tokio::time::sleep_until(deadline) => {
                res.timed_out = true;
                let _ = channel.close().await;
                break;
            }
        };
        match msg {
            Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                if buf.len() < MAX_OUTPUT {
                    buf.extend_from_slice(&data[..data.len().min(MAX_OUTPUT - buf.len())]);
                } else {
                    res.truncated = true;
                }
            }
            Some(ChannelMsg::ExitStatus { exit_status }) => res.exit = Some(exit_status),
            Some(ChannelMsg::Close) | None => break,
            _ => {}
        }
    }
    res.output = String::from_utf8_lossy(&buf).into_owned();
    if root && res.exit == Some(1) && res.output.contains("incorrect password") {
        bail!("sudo did not accept the password.");
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(cmd: &str) -> Effect {
        classify(cmd).effect
    }

    #[test]
    fn reading_commands() {
        for c in [
            "ls -la /var/log",
            "systemctl status nginx",
            "journalctl -u nginx -n 50 --no-pager",
            "df -h && free -m",
            "ps aux | grep java | head",
            "docker ps -a",
            "cat /etc/os-release",
            "sudo journalctl -xe",
            "tail -n 100 /var/log/syslog 2>&1",
            "find /etc -name '*.conf'",
            "sed -n '1,20p' /etc/hosts",
            "qm list",
            "ss -tlnp 2>/dev/null | head -40; command -v nginx caddy haproxy && true",
            "test -f /etc/nginx/nginx.conf && echo yes",
        ] {
            assert_eq!(e(c), Effect::Reads, "{c}");
        }
        assert!(classify("sudo journalctl -xe").root);
    }

    #[test]
    fn changing_commands() {
        for c in [
            "systemctl restart nginx",
            "apt install -y htop",
            "echo hi > /etc/motd",
            "sed -i 's/a/b/' /etc/x.conf",
            "find /tmp -name '*.log' -delete",
            "ls $(cat /tmp/list)",
            "curl -X POST http://x",
            "docker restart web",
            "touch /tmp/x",
            "cat x | sh",
            "command rm -rf /tmp/x",
        ] {
            assert_eq!(e(c), Effect::Changes, "{c}");
        }
        assert_eq!(classify("sudo systemctl restart nginx").kind, "systemctl restart");
        assert_eq!(classify("echo hi > /etc/motd").kind, "");
    }

    #[test]
    fn dangerous_commands() {
        for c in [
            "rm -rf /var/lib/mysql",
            "sudo reboot",
            "dd if=/dev/zero of=/dev/sda bs=1M",
            "mkfs.ext4 /dev/sdb1",
            "systemctl stop sshd",
            "iptables -F",
            "qm destroy 101",
            "zfs destroy -r tank/data",
            "ls; shutdown -h now",
        ] {
            assert_eq!(e(c), Effect::Dangerous, "{c}");
        }
    }
}

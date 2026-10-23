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
    "ls", "ll", "cat", "head", "tail", "grep", "egrep", "fgrep", "zgrep", "zcat", "wc", "cut", "column",
    "df", "du", "free", "uptime", "ps", "pgrep", "pstree", "top", "uname", "hostname", "hostnamectl", "whoami", "id", "groups",
    "who", "w", "last", "date", "timedatectl", "stat", "file", "which", "whereis", "type", "lsblk", "blkid", "lscpu", "lsmem",
    "lspci", "lsusb", "lsof", "mount", "findmnt", "ip", "ss", "netstat", "ping", "traceroute", "tracepath", "dig", "nslookup",
    "host", "getent", "printenv", "echo", "printf", "pwd", "realpath", "readlink", "basename", "dirname", "md5sum",
    "sha1sum", "sha256sum", "diff", "cmp", "tree", "nproc", "vmstat", "iostat", "mpstat", "dmesg", "journalctl", "uptime",
    "pveversion", "pvesm", "zfs", "zpool", "nginx", "apachectl", "openssl", "curl", "wget", "jq", "yq", "less", "more", "true",
];

/// Destroys data, cuts off the server or hands over control: never run without a red card.
fn dangerous(words: &[&str], line: &str) -> Option<&'static str> {
    let w0 = words.first().copied().unwrap_or("");
    let has = |x: &str| words.iter().any(|w| *w == x);
    let rec = words.iter().any(|w| (w.starts_with('-') && !w.starts_with("--") && (w.contains('r') || w.contains('R'))) || *w == "--recursive");
    let force = words.iter().any(|w| (w.starts_with('-') && !w.starts_with("--") && w.contains('f')) || *w == "--force");
    // Only temporary folders may be cleared without a red card; a variable could be empty and hit the root.
    let only_temp = {
        let paths: Vec<&&str> = words.iter().skip(1).filter(|w| !w.starts_with('-')).collect();
        !paths.is_empty()
            && paths.iter().all(|p| {
                (p.starts_with("/tmp/") || p.starts_with("/var/tmp/")) && p.len() > 9 && !p.contains("..") && !p.contains('$')
            })
    };
    match w0 {
        "rm" if rec && force && !only_temp => Some("removes files and folders without asking"),
        "rm" if words.iter().any(|w| matches!(*w, "/" | "/*" | "~" | "/etc" | "/var" | "/home" | "/usr" | "/boot" | "/root")) => Some("removes a system folder"),
        "mkfs" | "mkswap" | "wipefs" | "fdisk" | "sfdisk" | "parted" | "sgdisk" => Some("changes disks or partitions"),
        w if w.starts_with("mkfs.") => Some("formats a disk"),
        "dd" if words.iter().any(|w| w.starts_with("of=/dev/")) => Some("writes straight onto a disk"),
        "shutdown" | "reboot" | "poweroff" | "halt" | "init" => Some("stops or restarts the server"),
        "systemctl" if words.iter().any(|w| matches!(*w, "reboot" | "poweroff" | "halt" | "kexec" | "rescue" | "emergency")) => Some("stops or restarts the server"),
        "systemctl" if words.iter().any(|w| matches!(*w, "stop" | "disable" | "mask")) && words.iter().any(|w| w.starts_with("ssh")) => Some("may cut off this SSH connection"),
        "iptables" | "ip6tables" | "nft" if has("-F") || has("flush") => Some("removes firewall rules, which can cut off this connection"),
        "ufw" if has("enable") || has("reset") => Some("changes the firewall, which can cut off this connection"),
        "passwd" if has("-S") || has("--status") => None,
        "userdel" | "deluser" | "passwd" | "chpasswd" => Some("removes user accounts or changes passwords"),
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
        "find" => !rest.iter().any(|w| *w == "-delete" || w.starts_with("-exec") || w.starts_with("-ok") || w.starts_with("-fprint") || *w == "-fls"),
        "sed" => sed_reads(rest),
        "awk" | "gawk" | "mawk" | "nawk" => awk_reads(rest),
        // env with a program runs that program.
        "env" => rest.iter().all(|w| w.starts_with('-')),
        "sort" => !rest.iter().any(|w| w.starts_with("--output") || w.starts_with("--compress-program") || short_option(w, "o")),
        // uniq's second file is where it writes.
        "uniq" => {
            let mut files = 0;
            let mut skip = false;
            for w in rest {
                if std::mem::take(&mut skip) {
                    continue;
                }
                if matches!(*w, "-f" | "-s" | "-w") {
                    skip = true;
                } else if !w.starts_with('-') {
                    files += 1;
                }
            }
            files <= 1
        }
        "tree" => !has("-o"),
        "yq" => !rest.iter().any(|w| w.starts_with("--inplace") || short_option(w, "i")),
        "less" | "more" => !rest.iter().any(|w| matches!(*w, "-o" | "-O") || w.starts_with("--log-file") || w.starts_with("--LOG-FILE")),
        "ss" => !rest.iter().any(|w| *w == "--kill" || short_option(w, "K")),
        "file" => !rest.iter().any(|w| *w == "--compile" || short_option(w, "C")),
        // Options may come before the subcommand (systemctl --no-pager --failed); without one it lists units.
        "systemctl" => match rest.iter().find(|w| !w.starts_with('-')) {
            Some(s) => matches!(*s, "status" | "is-active" | "is-enabled" | "is-failed" | "list-units" | "list-unit-files" | "list-timers" | "list-sockets" | "show" | "cat" | "list-dependencies"),
            None => true,
        },
        "journalctl" => !rest.iter().any(|w| w.starts_with("--vacuum") || *w == "--rotate" || *w == "--flush"),
        // ip netns exec runs any program; -batch reads its commands from a file.
        "ip" => !rest.iter().any(|w| matches!(*w, "add" | "del" | "delete" | "set" | "change" | "replace" | "flush" | "append" | "exec" | "-b" | "-batch" | "-force")),
        "docker" | "podman" if rest.first() == Some(&"compose") => rest.get(1).is_some_and(|s| matches!(*s, "ps" | "logs" | "config" | "images" | "top" | "ls" | "version")),
        "docker" | "podman" => rest.first().is_some_and(|s| matches!(*s, "ps" | "logs" | "inspect" | "images" | "stats" | "top" | "version" | "info" | "port" | "diff" | "history")),
        "docker-compose" => rest.first().is_some_and(|s| matches!(*s, "ps" | "logs" | "config" | "images" | "top")),
        "kubectl" => rest.first().is_some_and(|s| matches!(*s, "get" | "describe" | "logs" | "top" | "version" | "explain" | "api-resources" | "cluster-info")),
        "git" if rest.iter().any(|w| w.starts_with("--output")) => false,
        // git branch name creates a branch, -d deletes one; git remote add adds one.
        "git" if rest.first() == Some(&"branch") => {
            let opts = &rest[1..];
            let list = opts.iter().any(|w| matches!(*w, "-l" | "--list" | "--contains" | "--no-contains" | "--merged" | "--no-merged" | "--points-at"));
            !opts.iter().any(|w| matches!(*w, "-d" | "-D" | "--delete" | "-m" | "-M" | "--move" | "-c" | "-C" | "--copy" | "-f" | "--force" | "-u" | "--unset-upstream" | "--edit-description" | "-t" | "--track" | "--no-track") || w.starts_with("--set-upstream"))
                && (list || opts.iter().all(|w| w.starts_with('-')))
        }
        "git" if rest.first() == Some(&"remote") => rest.get(1).is_none_or(|s| matches!(*s, "-v" | "--verbose" | "show" | "get-url")),
        "git" => rest.first().is_some_and(|s| matches!(*s, "status" | "log" | "diff" | "show" | "rev-parse" | "describe" | "blame")),
        "apt" => rest.first().is_some_and(|s| matches!(*s, "list" | "show" | "search" | "policy")),
        "apt-cache" | "dpkg-query" => true,
        "dpkg" => rest.first().is_some_and(|s| matches!(*s, "-l" | "-L" | "-s" | "--list" | "--status" | "--listfiles")),
        "rpm" => rest.first().is_some_and(|s| s.starts_with("-q")),
        "dnf" | "yum" => rest.first().is_some_and(|s| matches!(*s, "list" | "info" | "search" | "repolist" | "check-update")),
        "qm" | "pct" => rest.first().is_some_and(|s| matches!(*s, "list" | "config" | "status" | "pending" | "listsnapshot")),
        "zpool" => rest.first().is_some_and(|s| matches!(*s, "status" | "list" | "iostat" | "get" | "history")),
        "zfs" => rest.first().is_some_and(|s| matches!(*s, "list" | "get")),
        "pvesm" => rest.first().is_some_and(|s| matches!(*s, "status" | "list")),
        // Only testing and showing; -s would signal the server and -g adds directives. -c and -p take a path.
        "nginx" => {
            let mut ok = false;
            let mut i = 0;
            while i < rest.len() {
                match rest[i] {
                    "-t" | "-T" | "-v" | "-V" => ok = true,
                    "-q" => {}
                    "-c" | "-p" => i += 1,
                    _ => return false,
                }
                i += 1;
            }
            ok
        }
        "apachectl" => !rest.is_empty() && rest.iter().all(|w| matches!(*w, "-t" | "-S" | "-M" | "-v" | "-V" | "-l" | "-L" | "configtest")),
        "openssl" => {
            rest.first().is_some_and(|s| matches!(*s, "x509" | "s_client" | "version" | "crl" | "verify"))
                && !rest.iter().any(|w| matches!(w.to_ascii_lowercase().as_str(), "-out" | "-keyout" | "-sess_out" | "-msgfile" | "-keylogfile" | "-writerand" | "-cacreateserial"))
        }
        "curl" => curl_reads(rest),
        "wget" => {
            (has("--spider") || has("-O-") || has("-qO-"))
                && !rest.iter().any(|w| {
                    matches!(*w, "-o" | "-a" | "-e") || w.starts_with("--output-file") || w.starts_with("--append-output") || w.starts_with("--execute") || w.starts_with("--save-cookies") || w.starts_with("--post") || w.starts_with("--body") || w.starts_with("--method")
                })
        }
        "dmesg" => !has("-C") && !has("-c") && !has("--clear"),
        "hostname" | "hostnamectl" | "date" | "timedatectl" => rest.is_empty() || rest.first() == Some(&"status"),
        "top" => has("-b") || has("-bn1"),
        "mount" => rest.is_empty() || rest.iter().all(|w| w.starts_with('-') && (w.contains('l') || *w == "-t")),
        // `command -v x` only looks up where a program is; `command x` would run it.
        "command" => rest.first().is_some_and(|s| *s == "-v" || *s == "-V"),
        "test" | "[" | "[[" => true,
        "passwd" => has("-S") || has("--status"),
        "chage" => has("-l") || has("--list"),
        _ => READ_PROGRAMS.contains(&w0),
    }
}

/// Whether a word is a short option (-x, or bundled ones such as -nx) that includes one of the letters.
fn short_option(word: &str, letters: &str) -> bool {
    word.len() > 1 && word.starts_with('-') && !word.starts_with("--") && word[1..].chars().any(|c| letters.contains(c))
}

/// sed only reads unless it edits in place, takes its script from a file, or the script writes or runs.
fn sed_reads(rest: &[&str]) -> bool {
    let sandbox = rest.contains(&"--sandbox");
    let mut scripts: Vec<&str> = Vec::new();
    let mut given = false;
    let mut i = 0;
    while i < rest.len() {
        let w = rest[i];
        if w == "--in-place" || w.starts_with("--in-place=") || w == "--file" || w.starts_with("--file=") {
            return false;
        }
        if let Some(s) = w.strip_prefix("--expression=") {
            given = true;
            scripts.push(s);
        } else if w == "--expression" {
            given = true;
            scripts.extend(rest.get(i + 1));
            i += 1;
        } else if w.starts_with('-') && !w.starts_with("--") && w.len() > 1 {
            // -i edits in place, -f reads the script from a file; -e takes the script, attached or next.
            let letters = &w[1..];
            if let Some(pos) = letters.find(['i', 'f', 'e']) {
                match letters.as_bytes()[pos] {
                    b'e' => {
                        given = true;
                        let attached = &letters[pos + 1..];
                        if attached.is_empty() {
                            scripts.extend(rest.get(i + 1));
                            i += 1;
                        } else {
                            scripts.push(attached);
                        }
                    }
                    _ => return false,
                }
            }
        } else if !w.starts_with('-') && !given && scripts.is_empty() {
            scripts.push(w);
        }
        i += 1;
    }
    sandbox || !scripts.iter().any(|s| sed_script_writes(s))
}

/// Whether a sed script writes files or runs commands: the w, W and e commands and the w and e flags of s.
/// Anything this code does not know counts as writing.
fn sed_script_writes(script: &str) -> bool {
    let c: Vec<char> = script.chars().collect();
    // Past a part that ends with the delimiter d, minding backslash escapes.
    let past = |mut i: usize, d: char| -> usize {
        while i < c.len() && c[i] != d {
            if c[i] == '\\' {
                i += 1;
            }
            i += 1;
        }
        i + 1
    };
    let to_end = |mut i: usize, also_semicolon: bool| -> usize {
        while i < c.len() && c[i] != '\n' && !(also_semicolon && c[i] == ';') {
            i += 1;
        }
        i
    };
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            ' ' | '\t' | '\n' | ';' | '{' | '}' | '!' | ',' | '$' | '~' | '+' | 'I' | 'M' | '0'..='9' => i += 1,
            '/' => i = past(i + 1, '/'),
            '\\' => match c.get(i + 1) {
                Some(&d) => i = past(i + 2, d),
                None => return true,
            },
            's' | 'y' => {
                let Some(&d) = c.get(i + 1).filter(|d| !matches!(d, '\n' | '\\')) else { return true };
                i = past(past(i + 2, d), d);
                if i > c.len() {
                    return true;
                }
                // The flags, up to the end of this command.
                while i < c.len() && !matches!(c[i], ';' | '\n' | '}') {
                    if matches!(c[i], 'w' | 'W' | 'e') {
                        return true;
                    }
                    i += 1;
                }
            }
            'w' | 'W' | 'e' => return true,
            '#' | 'a' | 'i' | 'c' | 'r' | 'R' | 'v' => i = to_end(i + 1, false),
            ':' | 'b' | 't' | 'T' => i = to_end(i + 1, true),
            'p' | 'P' | 'd' | 'D' | 'n' | 'N' | 'g' | 'G' | 'h' | 'H' | 'x' | 'l' | '=' | 'q' | 'Q' | 'z' | 'F' => i += 1,
            _ => return true,
        }
    }
    false
}

/// awk only reads unless it runs commands (system(), pipes into or out of a command), prints into a file,
/// loads code, or takes its program from a file or include (gawk -i inplace edits files).
fn awk_reads(rest: &[&str]) -> bool {
    let mut program = None;
    let mut i = 0;
    while i < rest.len() {
        let w = rest[i];
        if matches!(w, "-F" | "-v") {
            i += 1;
        } else if w.starts_with("-F") || w.starts_with("-v") || w.starts_with("--field-separator") || w.starts_with("--assign") || w == "--" {
        } else if w.starts_with('-') {
            return false;
        } else if program.is_none() {
            program = Some(w);
        }
        i += 1;
    }
    let Some(p) = program else { return false };
    // The code outside strings and regular expressions.
    let mut code = String::new();
    let chars: Vec<char> = p.chars().collect();
    let mut j = 0;
    while j < chars.len() {
        let ch = chars[j];
        let before = code.trim_end().chars().last();
        if ch == '"' || (ch == '/' && before.is_none_or(|b| "(,~!{;&|\n?:".contains(b))) {
            j += 1;
            while j < chars.len() && chars[j] != ch {
                if chars[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            code.push_str(" \"\" ");
        } else {
            code.push(ch);
        }
        j += 1;
    }
    let pipe = code.replace("||", "").contains('|');
    let redirect = code.split([';', '{', '}', '\n']).any(|stmt| stmt.find("print").is_some_and(|at| stmt[at..].contains('>')));
    !(pipe || redirect || code.contains("system") || code.contains("@load") || code.contains("@include"))
}

/// curl only fetches unless it sends data, uses another method, or writes files (-o, -O, cookie jars,
/// header dumps, traces); output to the screen or /dev/null is fine.
fn curl_reads(rest: &[&str]) -> bool {
    const SENDS: &[&str] = &[
        "-X", "--request", "-d", "-F", "-T", "--upload-file", "-K", "--config", "-O", "--remote-name", "--remote-name-all", "-J", "--remote-header-name", "--libcurl", "--output-dir", "--create-dirs",
    ];
    const WRITES: &[&str] = &["-o", "--output", "-c", "--cookie-jar", "-D", "--dump-header", "--trace", "--trace-ascii", "--stderr", "--etag-save", "--hsts", "--alt-svc"];
    let harmless = |t: &str| matches!(t, "/dev/null" | "-");
    let mut i = 0;
    while i < rest.len() {
        let w = rest[i];
        // --name=value, -xVALUE (a value attached to one short option) or bundled short options (-sSo).
        let (name, attached): (String, Option<&str>) = match w.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v)),
            _ if w.starts_with('-') && !w.starts_with("--") && w.len() > 2 => {
                if w[1..].chars().all(|c| c.is_ascii_alphabetic()) {
                    let letters = &w[1..];
                    if letters.chars().any(|c| "OXdFTKJ".contains(c)) || letters[..letters.len() - 1].chars().any(|c| "ocD".contains(c)) {
                        return false;
                    }
                    (format!("-{}", &letters[letters.len() - 1..]), None)
                } else {
                    (w[..2].to_string(), Some(&w[2..]))
                }
            }
            _ => (w.to_string(), None),
        };
        let name = name.as_str();
        if SENDS.contains(&name) || name.starts_with("--data") || name.starts_with("--upload") || name.starts_with("--form") || name.starts_with("--json") {
            return false;
        }
        let takes = WRITES.contains(&name) || matches!(name, "-w" | "--write-out");
        if takes {
            let value = attached.or_else(|| rest.get(i + 1).copied()).unwrap_or("");
            if WRITES.contains(&name) && !harmless(value) || value.contains("%output") {
                return false;
            }
            if attached.is_none() {
                i += 1;
            }
        }
        i += 1;
    }
    true
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

// ---- PowerShell, for Windows guests ----

/// PowerShell verbs whose cmdlets only look (Format-Volume is the exception, see below).
const PS_READ_VERBS: &[&str] = &[
    "get", "test", "select", "measure", "format", "where", "sort", "group", "convertto", "convertfrom", "compare", "find", "resolve",
    "show", "foreach", "join", "split", "out", "write",
];

/// PowerShell's verbs: a word such as qemu-ga is a name, not a cmdlet.
const PS_VERBS: &[&str] = &[
    "add", "approve", "assert", "backup", "block", "clear", "close", "compare", "complete", "compress", "confirm", "connect", "convert",
    "convertfrom", "convertto", "copy", "debug", "deny", "disable", "disconnect", "dismount", "edit", "enable", "enter", "exit", "expand",
    "export", "find", "foreach", "format", "get", "grant", "group", "hide", "import", "initialize", "install", "invoke", "join", "limit",
    "lock", "measure", "merge", "mount", "move", "new", "open", "optimize", "out", "pop", "protect", "publish", "push", "read", "receive",
    "redo", "register", "remove", "rename", "repair", "request", "reset", "resize", "resolve", "restart", "restore", "resume", "revoke",
    "save", "search", "select", "send", "set", "show", "skip", "sort", "split", "start", "step", "stop", "submit", "suspend", "switch",
    "sync", "tee", "test", "trace", "unblock", "undo", "uninstall", "unlock", "unprotect", "unpublish", "unregister", "update", "use",
    "wait", "watch", "where", "write",
];

/// Cmdlets and programs that wipe, format or restart the machine.
const PS_DANGEROUS: &[(&str, &str)] = &[
    ("format-volume", "formats a disk"),
    ("clear-disk", "wipes a disk"),
    ("initialize-disk", "wipes a disk"),
    ("remove-partition", "removes a partition"),
    ("restart-computer", "restarts the computer"),
    ("stop-computer", "shuts the computer down"),
    ("remove-vm", "deletes a virtual machine"),
    ("disable-netadapter", "switches a network card off, which can cut off this connection"),
    ("shutdown", "shuts the computer down or restarts it"),
    ("format", "formats a disk"),
    ("diskpart", "changes disks and partitions"),
    ("bcdedit", "changes how the computer starts"),
    ("cipher", "can wipe free space or encrypt files"),
];

/// Programs that only look.
const PS_READ_PROGRAMS: &[&str] = &[
    "hostname", "whoami", "systeminfo", "tasklist", "netstat", "ping", "nslookup", "tracert", "dir", "ls", "type", "cat", "findstr",
    "where", "where.exe", "query", "quser", "gci", "gc", "gps", "gsv", "gwmi", "echo", "date",
];

/// Whether a program that is not a cmdlet only looks. route, arp and ipconfig also change routes, the ARP
/// table and the network (ipconfig /release), so they count only with their showing arguments.
fn ps_program_reads(w: &[String]) -> bool {
    let first = w.first().map(String::as_str).unwrap_or("");
    let rest = &w[1.min(w.len())..];
    let first = first.strip_suffix(".exe").unwrap_or(first);
    match first {
        "route" => rest.first().is_some_and(|s| s == "print"),
        "arp" => rest.iter().all(|s| !matches!(s.as_str(), "-d" | "/d" | "-s" | "/s")),
        "ipconfig" => rest.iter().all(|s| matches!(s.as_str(), "/all" | "-all" | "/displaydns" | "/showclassid" | "/showclassid6" | "/?")),
        _ => PS_READ_PROGRAMS.contains(&first),
    }
}

/// Methods that only compute or look; any other method on an object (.Kill(), .Terminate(), .Delete(),
/// .Put(), .Invoke...) counts as a change.
fn ps_method_reads(name: &str) -> bool {
    name.starts_with("get") || name.starts_with("to") || name.starts_with("trim") || name.starts_with("pad") || ["adddays", "addhours", "addminutes", "addseconds", "addmilliseconds", "addmonths", "addyears", "addticks"].contains(&name)
        || matches!(
            name,
            "split" | "replace" | "substring" | "contains" | "startswith" | "endswith" | "indexof" | "lastindexof" | "equals" | "compareto" | "where" | "foreach" | "match" | "matches" | "ismatch" | "count" | "normalize" | "format" | "join"
        )
}

/// Kinds of expressions that only compute something: strings, variables, numbers, .NET types such as [math].
fn ps_expression(word: &str) -> bool {
    let w = word.to_ascii_lowercase();
    w.starts_with('"')
        || w.starts_with('\'')
        || (w.starts_with('$') && !w.contains('='))
        || w.starts_with('(')
        || w.starts_with('@')
        || w.chars().next().is_some_and(|c| c.is_ascii_digit())
        || ["[math]", "[datetime]", "[environment]", "[string]", "[int]", "[convert]", "[timespan]", "[system.environment]", "[version]"]
            .iter()
            .any(|t| w.starts_with(t))
}

/// What a PowerShell command line does: the same three answers as `classify`, by verbs instead of programs.
pub fn classify_powershell(line: &str) -> Verdict {
    let line = line.trim();
    let lower = line.to_ascii_lowercase();
    let mut effect = Effect::Reads;
    let mut why = String::from("only reads");
    let mut kind = String::new();
    // Remove-Item with -Recurse removes a whole tree, as rm -rf does.
    if (lower.contains("remove-item") || lower.starts_with("rm ") || lower.contains(" rm ") || lower.starts_with("rd ") || lower.starts_with("rmdir "))
        && (lower.contains("-recurse") || lower.contains(" -r ") || lower.contains(" /s"))
    {
        return Verdict { effect: Effect::Dangerous, root: true, why: "removes files and folders without asking".into(), kind: "remove-item".into() };
    }
    for seg in segments(line) {
        let ws = words(&seg);
        let mut w: Vec<String> = ws.iter().map(|x| x.to_ascii_lowercase()).collect();
        // $x = ... and $x=...: what matters is the right-hand side.
        if w.first().is_some_and(|x| x.starts_with('$') && x.contains('=')) {
            let rest = w[0].split_once('=').map(|(_, r)| r.to_string()).unwrap_or_default();
            if rest.is_empty() {
                w.remove(0);
            } else {
                w[0] = rest;
            }
        } else if w.len() > 1 && w[0].starts_with('$') && w[1] == "=" {
            w.drain(..2);
        }
        let Some(first) = w.first().cloned() else { continue };
        // Expressions are judged on the text as written (words() takes the quotes off); for $x = ..., the right side.
        let mut raw = seg.trim_start();
        if raw.starts_with('$') {
            if let Some((l, r)) = raw.split_once('=') {
                if l.split_whitespace().count() == 1 {
                    raw = r.trim_start();
                }
            }
        }
        let expression = ps_expression(raw);
        // Every cmdlet in the segment counts, also one inside ( ) or $( ).
        let cmdlets: Vec<String> = seg
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '.'))
            .map(|t| t.to_ascii_lowercase())
            .filter(|t| t.split_once('-').is_some_and(|(v, n)| !n.is_empty() && PS_VERBS.contains(&v)))
            .collect();
        for name in cmdlets.iter().chain(std::iter::once(&first)) {
            if let Some((_, reason)) = PS_DANGEROUS.iter().find(|(n, _)| n == name) {
                return Verdict { effect: Effect::Dangerous, root: true, why: reason.to_string(), kind: name.clone() };
            }
        }
        if kind.is_empty() {
            kind = cmdlets.first().cloned().unwrap_or_else(|| first.clone());
        }
        if effect != Effect::Reads {
            continue;
        }
        let changing = cmdlets.iter().find(|c| {
            let verb = c.split('-').next().unwrap_or("");
            !PS_READ_VERBS.contains(&verb) || c.as_str() == "out-file"
        });
        if let Some(c) = changing {
            effect = Effect::Changes;
            why = format!("{c} can change the computer");
        } else if cmdlets.is_empty() && !expression && !ps_program_reads(&w) {
            effect = Effect::Changes;
            why = format!("{first} can change the computer");
        } else if seg.contains("::") && !expression {
            effect = Effect::Changes;
            why = "calls .NET directly, which cannot be checked".into();
        }
        // Methods on objects (.Kill(), .Terminate(), .Delete()...): only the ones known to just look are fine.
        if effect == Effect::Reads {
            let lower = seg.to_ascii_lowercase();
            let b = lower.as_bytes();
            let method = lower.match_indices('.').find_map(|(at, _)| {
                let name: String = lower[at + 1..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                let after = lower[at + 1 + name.len()..].trim_start();
                let is_call = !name.is_empty() && !name.as_bytes()[0].is_ascii_digit() && after.starts_with('(') && (at == 0 || b[at - 1] != b'.');
                (is_call && !ps_method_reads(&name)).then_some(name)
            });
            if let Some(m) = method {
                effect = Effect::Changes;
                why = format!("calls the method {m}, which can change the computer");
            }
        }
    }
    // Writing into a file (but not into $null), and running code that cannot be read first.
    let cleaned = lower.replace("2>&1", "").replace(">$null", "").replace("> $null", "").replace("2>$null", "");
    let opaque = ["invoke-expression", "iex ", "-encodedcommand", "start-process", "[scriptblock]", "invoke-command"].iter().any(|x| lower.contains(x));
    if (cleaned.contains('>') || opaque) && effect == Effect::Reads {
        effect = Effect::Changes;
        why = if opaque { "runs code that cannot be checked".into() } else { "writes into a file".into() };
        kind.clear();
    }
    Verdict { effect, root: true, why, kind }
}

// ---- Running ----

#[derive(Debug, Serialize, Default)]
pub struct ExecResult {
    pub output: String,
    pub exit: Option<u32>,
    pub timed_out: bool,
    pub truncated: bool,
}

/// At most this much output is kept from one command; Settings, Assistant decides how much of it the model gets.
const MAX_OUTPUT: usize = 256 * 1024;

/// The password sudo asks for: the session's vault login, a saved password, a login the vault holds for this
/// server, or a question to the user. Also says whether the user wants a typed password remembered.
async fn sudo_password(app: &AppHandle, session: &Session) -> Result<(String, bool)> {
    let state = app.state::<AppState>();
    let item = session.data.get("vault_item").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if !item.is_empty() {
        if let Ok((_, pw)) = state.creds.login(app, &item).await {
            if !pw.is_empty() {
                return Ok((pw, false));
            }
        }
    }
    let saved = { state.vault.password(&state.store.lock(), "ssh", &session.host, session.port, &session.username).ok().flatten() };
    if let Some(pw) = saved {
        return Ok((pw, false));
    }
    // Logged in with a key: the vault may still hold the password for this server.
    if item.is_empty() {
        if let Ok(Some((_, _, _, pw))) = state.creds.pick_login(app, "ssh", &session.host, &session.username, &session.name).await {
            if !pw.is_empty() {
                return Ok((pw, false));
            }
        }
    }
    let message = format!(
        "The assistant needs root for a command you approved. sudo asks for the password of {} on this server, also when you logged in with a key. It goes to sudo only, never to the AI.",
        session.username
    );
    state
        .prompts
        .ask_secret(app, &format!("sudo on {}", session.host), &message, "Password", true)
        .await
        .ok_or_else(|| anyhow!("No password given for sudo."))
}

/// True when sudo works here without a password (NOPASSWD), so none has to be found or asked.
async fn sudo_without_password(conn: &crate::ssh::SshConn) -> bool {
    let Ok(Ok(mut channel)) = tokio::time::timeout(Duration::from_secs(10), conn.handle.channel_open_session()).await else {
        return false;
    };
    if channel.exec(true, &b"sudo -n -k true"[..]).await.is_err() {
        return false;
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let msg = tokio::select! {
            m = channel.wait() => m,
            _ = tokio::time::sleep_until(deadline) => return false,
        };
        match msg {
            Some(ChannelMsg::ExitStatus { exit_status }) => return exit_status == 0,
            Some(ChannelMsg::Close) | None => return false,
            _ => {}
        }
    }
}

/// The output (at most MAX_OUTPUT) and exit code of a command on a channel, until it ends or the time is up.
async fn collect(channel: &mut russh::Channel<russh::client::Msg>, timeout: Duration) -> ExecResult {
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
    res
}

/// Runs one command line on its own channel of an SSH connection (a Proxmox node, for `pct exec`).
pub async fn exec_line(conn: &crate::ssh::SshConn, line: &str, timeout: Duration) -> Result<ExecResult> {
    let mut channel = tokio::time::timeout(Duration::from_secs(15), conn.handle.channel_open_session())
        .await
        .map_err(|_| anyhow!("The server did not open a channel for the command in time."))??;
    tokio::time::timeout(Duration::from_secs(15), channel.exec(true, line.as_bytes()))
        .await
        .map_err(|_| anyhow!("The server did not start the command in time."))??;
    Ok(collect(&mut channel, timeout).await)
}

/// Text cut to at most MAX_OUTPUT bytes on a character boundary, and whether it was cut.
pub fn limit_output(mut s: String) -> (String, bool) {
    if s.len() <= MAX_OUTPUT {
        return (s, false);
    }
    let mut end = MAX_OUTPUT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    (s, true)
}

/// Runs one command on its own channel of the tab's SSH login.
/// `session` is the tab's saved session, used only to find the sudo password.
pub async fn exec(app: &AppHandle, tab_id: &str, session: &Session, command: &str, root: bool, timeout: Duration) -> Result<ExecResult> {
    let conn = app.state::<AppState>().ssh.lock().get(tab_id).cloned().ok_or_else(|| anyhow!("This SSH tab is not connected."))?;
    let mut channel = tokio::time::timeout(Duration::from_secs(15), conn.handle.channel_open_session())
        .await
        .map_err(|_| anyhow!("The server did not open a channel for the command in time."))??;
    let quoted = format!("'{}'", command.replace('\'', r"'\''"));
    let passwordless = root && sudo_without_password(&conn).await;
    let line = if passwordless {
        format!("sudo -n -- sh -c {quoted}")
    } else if root {
        // -S reads the password from the channel, -p '' keeps its question out of the output, -k asks every
        // time so the password always goes the same way.
        format!("sudo -S -k -p '' -- sh -c {quoted}")
    } else {
        format!("sh -c {quoted}")
    };
    tokio::time::timeout(Duration::from_secs(15), channel.exec(true, line.as_bytes()))
        .await
        .map_err(|_| anyhow!("The server did not start the command in time."))??;
    let mut keep: Option<String> = None;
    if root && !passwordless {
        let (pw, save) = sudo_password(app, session).await?;
        channel.data(format!("{pw}\n").as_bytes()).await?;
        if save {
            keep = Some(pw);
        }
    }
    let res = collect(&mut channel, timeout).await;
    if root && res.exit == Some(1) && res.output.contains("incorrect password") {
        bail!("sudo did not accept the password.");
    }
    // sudo took the typed password: remember it, as asked.
    if let Some(pw) = keep {
        let state = app.state::<AppState>();
        let _ = state.vault.save_password(&state.store.lock(), "ssh", &session.host, session.port, &session.username, &pw);
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
    fn powershell() {
        let p = |c: &str| classify_powershell(c).effect;
        for c in [
            "hostname; [Environment]::OSVersion.VersionString; (Get-Service qemu-ga).Status",
            "$os=Get-CimInstance Win32_OperatingSystem; $u=(Get-Date)-$os.LastBootUpTime; \"Uptime: $($u.Days) days\"",
            "Get-Volume -DriveLetter C | Select-Object Size,SizeRemaining | Format-Table -AutoSize",
            "Get-EventLog -LogName System -Newest 20 | Where-Object { $_.EntryType -eq 'Error' } | Out-String -Width 200",
            "ipconfig /all",
            "Get-ChildItem C:/Windows/Temp | Measure-Object -Property Length -Sum",
        ] {
            assert_eq!(p(c), Effect::Reads, "{c}: {}", classify_powershell(c).why);
        }
        for c in ["Restart-Service Spooler", "Set-ItemProperty HKLM:/X -Name Y -Value 1", "Get-Process | Out-File C:/p.txt", "winget upgrade --all", "Stop-Process -Name notepad", "Invoke-Expression $x"] {
            assert_eq!(p(c), Effect::Changes, "{c}");
        }
        for c in ["Restart-Computer -Force", "Remove-Item C:/Data -Recurse -Force", "Format-Volume -DriveLetter D", "Clear-Disk -Number 1 -RemoveData", "shutdown /r /t 0"] {
            assert_eq!(p(c), Effect::Dangerous, "{c}");
        }
    }

    #[test]
    fn edges() {
        assert_eq!(e("rm -rf /tmp/build"), Effect::Changes);
        assert_eq!(e("rm -rf /var/tmp/cache/x"), Effect::Changes);
        assert_eq!(e("rm -rf /tmp"), Effect::Dangerous);
        assert_eq!(e("rm -rf /tmp/$DIR"), Effect::Dangerous);
        assert_eq!(e("rm -rf /tmp/../etc"), Effect::Dangerous);
        assert_eq!(e("rm --recursive --force /srv/data"), Effect::Dangerous);
        assert_eq!(e("rm -rf build"), Effect::Dangerous);
        assert_eq!(e("docker compose ps"), Effect::Reads);
        assert_eq!(e("docker compose logs --tail 50 web"), Effect::Reads);
        assert_eq!(e("docker compose up -d"), Effect::Changes);
        assert_eq!(e("passwd -S root"), Effect::Reads);
        assert_eq!(e("chage -l root"), Effect::Reads);
        assert_eq!(e("passwd root"), Effect::Dangerous);
        assert_eq!(e("usermod -aG docker ana"), Effect::Changes);
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
            "systemctl --no-pager --failed",
            "systemctl --no-pager status nginx",
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
            "systemctl --no-pager restart nginx",
        ] {
            assert_eq!(e(c), Effect::Changes, "{c}");
        }
        assert_eq!(classify("sudo systemctl restart nginx").kind, "systemctl restart");
        assert_eq!(classify("echo hi > /etc/motd").kind, "");
    }

    /// Commands that look like reading but write files or run other programs (from a review of the classifier).
    #[test]
    fn reading_programs_that_can_write_or_run() {
        for c in [
            "sed -n 'w /tmp/copy' /etc/shadow",
            "sed 's/a/b/w /tmp/out' /etc/hosts",
            "sed -n '1e id' /etc/hosts",
            "sed -e p -e 'w /tmp/x' /etc/hosts",
            "sed -ne 'b end; w /tmp/x' /etc/hosts",
            "sed -f script.sed /etc/hosts",
            "sed -ni p /etc/hosts",
            "awk 'BEGIN { \"id\" | getline x; print x }'",
            "awk '{ print | \"sh\" }' /etc/hosts",
            "awk '{ print $1 > \"/tmp/out\" }' /etc/hosts",
            "awk 'BEGIN { system (\"id\") }'",
            "awk -f prog.awk /etc/hosts",
            "gawk -i inplace '{ print }' /etc/hosts",
            "env rm -rf /tmp/x",
            "env VAR=1 sh -c id",
            "ip netns exec blue sh",
            "ip -batch /tmp/cmds",
            "sort -o /etc/hosts /tmp/x",
            "sort -uo /etc/hosts /tmp/x",
            "sort --compress-program=sh /tmp/x",
            "uniq /tmp/a /etc/hosts",
            "tree -o /tmp/out /etc",
            "yq -i '.a = 1' x.yml",
            "openssl x509 -in a.pem -out /etc/ssl/b.pem",
            "curl -o /tmp/x http://h",
            "curl -sSo /tmp/x http://h",
            "curl -c /tmp/jar http://h",
            "curl -D /tmp/headers http://h",
            "curl --data-binary @f http://h",
            "curl -K /tmp/conf http://h",
            "curl -w '%output{/tmp/x}' http://h",
            "wget -qO- http://h -o /tmp/log",
            "find / -fprint0 /tmp/x",
            "find / -okdir rm {} ;",
            "nginx -t -s stop",
            "nginx -t -g 'pid /tmp/x;'",
            "apachectl -t -k stop",
            "git branch new-branch",
            "git branch -D old",
            "git remote add x http://h",
            "git log --output=/tmp/x",
            "ss -K dst 10.0.0.1",
            "less -o /tmp/log /etc/hosts",
        ] {
            assert_ne!(e(c), Effect::Reads, "{c}");
        }
        for c in [
            "sed -n 's/^#//p' /etc/ssh/sshd_config",
            "sed 's/hello/world/g' /etc/hosts",
            "sed -n '/start/,/end/p' /etc/hosts",
            "sed --sandbox 'w /tmp/x' /etc/hosts",
            "sed -E 's#/usr/#/opt/#g' /etc/profile",
            "awk '{print $1}' /etc/hosts",
            "awk -F: '$3 >= 1000 {print $1}' /etc/passwd",
            "awk '/error|fail/ {n++} END {print n}' /var/log/syslog",
            "awk '{ if ($5 > 80) print $1 }' /tmp/df",
            "env",
            "sort -n -k2 /tmp/x",
            "uniq -c /tmp/x",
            "curl -s http://h",
            "curl -sS -o /dev/null -w '%{http_code}' http://h",
            "curl -I https://h",
            "wget -qO- http://h",
            "openssl x509 -in a.pem -noout -dates",
            "nginx -t",
            "nginx -T -c /etc/nginx/nginx.conf",
            "git branch -a",
            "git branch --list 'feat*'",
            "git remote -v",
            "git log --oneline -5",
        ] {
            assert_eq!(e(c), Effect::Reads, "{c}: {}", classify(c).why);
        }
        let p = |c: &str| classify_powershell(c).effect;
        for c in ["route delete 0.0.0.0", "route add 10.0.0.0 mask 255.0.0.0 10.0.0.1", "arp -d *", "ipconfig /release", "Get-WmiObject Win32_Process -Filter \"Name='x.exe'\" | ForEach-Object { $_.Terminate() }", "(Get-Service spooler).Stop()", "Tee-Object -FilePath C:/x.txt"] {
            assert_ne!(p(c), Effect::Reads, "{c}");
        }
        for c in ["route print", "arp -a", "ipconfig", "(Get-Date).AddDays(-1).ToString('yyyy-MM-dd')", "\"a,b\".Split(',').Count", "(Get-Item C:/x.txt).GetType().Name"] {
            assert_eq!(p(c), Effect::Reads, "{c}: {}", classify_powershell(c).why);
        }
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

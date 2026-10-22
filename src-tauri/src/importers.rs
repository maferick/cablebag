//! Importing sessions from other programs and files: PuTTY and KiTTY saved sessions, WinSCP sites, the
//! OpenSSH config file, Remote Desktop (.rdp) files and MobaXterm (MobaXterm.ini or .mxtsessions, the format with
//! one `Name= #icon#type%field1%field2...#terminal#start mode#comment#colour` line per session and the folder
//! in each `[Bookmarks_N]` section's `SubRep=`).
//!
//! Each source lands in its own top folder (session files keep the folders they define). Importing again
//! updates the same sessions instead of adding copies. Nothing is changed in the other programs.

use crate::store::{Session, Store};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Default)]
pub struct Report {
    pub file: String,
    pub folders: usize,
    pub sessions: usize,
    pub updated: usize,
    pub by_kind: BTreeMap<String, usize>,
}

/// A place sessions can come from, with how many it holds on this PC.
#[derive(Debug, Serialize)]
pub struct Source {
    pub id: &'static str,
    pub name: &'static str,
    pub description: String,
    /// Sessions found on this PC; None when the source is a file you choose.
    pub found: Option<usize>,
    /// Which files to ask for when the source is a file you choose.
    pub extensions: Vec<&'static str>,
}

/// One session read from a source, before it is saved.
struct Item {
    /// Folder path below the source's top folder.
    folder: Vec<String>,
    name: String,
    kind: String,
    host: String,
    port: i64,
    user: String,
    data: Value,
    comment: String,
}

pub fn sources() -> Vec<Source> {
    let count = |r: Result<Vec<Item>>| r.map(|v| v.len()).unwrap_or(0);
    vec![
        Source {
            id: "putty",
            name: "PuTTY",
            description: "Saved sessions: SSH, Telnet and serial, with user name and key file.".into(),
            found: Some(count(putty_like(PUTTY))),
            extensions: vec![],
        },
        Source {
            id: "kitty",
            name: "KiTTY",
            description: "Saved sessions, the same way as PuTTY.".into(),
            found: Some(count(putty_like(KITTY))),
            extensions: vec![],
        },
        Source {
            id: "winscp",
            name: "WinSCP",
            description: "Sites with their folders: SFTP, SCP and FTP.".into(),
            found: Some(count(winscp())),
            extensions: vec![],
        },
        Source {
            id: "sshconfig",
            name: "OpenSSH config",
            description: format!("The Host entries of {}, with user, port, key file and jump host.", ssh_config_path().display()),
            found: Some(count(ssh_config(&ssh_config_path()))),
            extensions: vec![],
        },
        Source {
            id: "rdp",
            name: "Remote Desktop files",
            description: "One or more .rdp files, as saved by Remote Desktop Connection.".into(),
            found: None,
            extensions: vec!["rdp"],
        },
        Source {
            id: "mobaxterm",
            name: "MobaXterm",
            description: match mobaxterm_ini() {
                Some(p) => format!("Sessions and folders from {}, or choose a .mxtsessions export.", p.display()),
                None => "Sessions and folders from MobaXterm.ini or a .mxtsessions export (choose the file).".into(),
            },
            found: Some(mobaxterm_ini().map(|p| session_file(&p).map(|v| v.len()).unwrap_or(0)).unwrap_or(0)),
            extensions: vec!["ini", "mxtsessions"],
        },
    ]
}

/// Imports from a source; `paths` are the chosen files for the file sources.
pub fn import(store: &Store, source: &str, paths: &[String]) -> Result<Report> {
    let (top, items, label) = match source {
        "putty" => (Some("PuTTY"), putty_like(PUTTY)?, "PuTTY".to_string()),
        "kitty" => (Some("KiTTY"), putty_like(KITTY)?, "KiTTY".to_string()),
        "winscp" => (Some("WinSCP"), winscp()?, "WinSCP".to_string()),
        "sshconfig" => {
            let p = ssh_config_path();
            (Some("SSH config"), ssh_config(&p)?, p.display().to_string())
        }
        "rdp" => {
            let mut all = Vec::new();
            for p in paths {
                all.push(rdp_file(Path::new(p))?);
            }
            (Some("Remote Desktop files"), all, paths.join(", "))
        }
        "mobaxterm" | "sessionfile" => {
            let p = match paths.first() {
                Some(p) => PathBuf::from(p),
                None => mobaxterm_ini().context("MobaXterm.ini was not found on this PC; choose the file.")?,
            };
            (None, session_file(&p)?, p.display().to_string())
        }
        _ => bail!("Unknown import source {source}."),
    };
    if items.is_empty() {
        bail!("No sessions were found in {label}.");
    }
    save(store, top, items, label)
}

fn save(store: &Store, top: Option<&str>, items: Vec<Item>, label: String) -> Result<Report> {
    let mut report = Report { file: label, ..Default::default() };
    let mut made = HashSet::new();
    for it in items {
        let mut parent: Option<String> = None;
        for seg in top.into_iter().map(str::to_string).chain(it.folder.iter().cloned()) {
            let seg = seg.trim();
            if seg.is_empty() {
                continue;
            }
            let id = store.ensure_folder(parent.as_deref(), seg)?;
            if made.insert(id.clone()) {
                report.folders += 1;
            }
            parent = Some(id);
        }
        let existing = store.find_session(parent.as_deref(), &it.name)?;
        let updated = existing.is_some();
        store.save_session(&Session {
            id: existing.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            folder_id: parent,
            name: it.name,
            kind: it.kind.clone(),
            host: it.host,
            port: it.port,
            username: it.user,
            data: it.data,
            comment: it.comment,
            sort: 0,
            last_used: 0,
        })?;
        if updated {
            report.updated += 1;
        } else {
            report.sessions += 1;
        }
        *report.by_kind.entry(it.kind).or_default() += 1;
    }
    Ok(report)
}

fn default_port(kind: &str) -> i64 {
    match kind {
        "ssh" | "sftp" | "mosh" => 22,
        "telnet" => 23,
        "rdp" => 3389,
        "vnc" => 5900,
        "ftp" => 21,
        "rsh" => 514,
        "xdmcp" => 177,
        _ => 0,
    }
}

/// Undoes the %XX escapes PuTTY and WinSCP use in session names (and WinSCP in some values).
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(hi), Some(lo)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// "user@host" in a host field becomes the user and the host.
fn split_user(host: &str, user: &str) -> (String, String) {
    match host.rsplit_once('@') {
        Some((u, h)) if user.is_empty() => (h.to_string(), u.to_string()),
        Some((_, h)) => (h.to_string(), user.to_string()),
        None => (host.to_string(), user.to_string()),
    }
}

// ---- PuTTY and KiTTY: HKEY_CURRENT_USER\<path>\<escaped session name> ----

const PUTTY: &str = r"Software\SimonTatham\PuTTY\Sessions";
const KITTY: &str = r"Software\9bis.com\KiTTY\Sessions";

#[cfg(windows)]
fn putty_like(path: &str) -> Result<Vec<Item>> {
    let Ok(root) = windows_registry::CURRENT_USER.open(path) else { return Ok(Vec::new()) };
    let mut items = Vec::new();
    for key in root.keys()? {
        let Ok(k) = root.open(&key) else { continue };
        let s = |n: &str| k.get_string(n).unwrap_or_default();
        let n = |n: &str| k.get_u32(n).ok();
        let proto = s("Protocol").to_lowercase();
        let kind = match proto.as_str() {
            "ssh" | "" => "ssh",
            "telnet" => "telnet",
            "serial" => "serial",
            _ => continue, // raw and rlogin sockets: not offered here
        };
        let name = unescape(&key);
        let mut data = json!({});
        let (host, user) = split_user(&s("HostName"), &s("UserName"));
        if kind == "serial" {
            let line = s("SerialLine");
            if line.is_empty() {
                continue;
            }
            data["serial_port"] = json!(line);
            data["baud"] = json!(n("SerialSpeed").unwrap_or(9600));
        } else if host.trim().is_empty() {
            continue; // "Default Settings" and other empty entries
        }
        let key_file = s("PublicKeyFile");
        if !key_file.is_empty() {
            data["key_path"] = json!(key_file);
        }
        if kind == "ssh" && n("Compression") == Some(1) {
            data["compression"] = json!(true);
        }
        let command = s("RemoteCommand");
        if !command.is_empty() {
            data["command"] = json!(command);
        }
        items.push(Item {
            folder: vec![],
            name,
            kind: kind.into(),
            host: if kind == "serial" { String::new() } else { host },
            port: n("PortNumber").map(i64::from).filter(|p| *p > 0).unwrap_or_else(|| default_port(kind)),
            user: if kind == "serial" { String::new() } else { user },
            data,
            comment: String::new(),
        });
    }
    Ok(items)
}

#[cfg(not(windows))]
fn putty_like(_path: &str) -> Result<Vec<Item>> {
    Ok(Vec::new())
}

// ---- WinSCP: the registry, or WinSCP.ini next to the program or in %APPDATA% ----

const WINSCP: &str = r"Software\Martin Prikryl\WinSCP 2\Sessions";

/// FSProtocol: 0 SCP, 1 SFTP with SCP fallback, 2 SFTP, 5 FTP. Ftps: 0 none, 1 implicit, 2 and 3 explicit.
fn winscp_item(name: &str, get: &dyn Fn(&str) -> Option<String>) -> Option<Item> {
    let full = unescape(name);
    if full == "Default Settings" {
        return None;
    }
    let (host, user) = split_user(&get("HostName").unwrap_or_default(), &get("UserName").unwrap_or_default());
    if host.trim().is_empty() {
        return None;
    }
    let num = |k: &str| get(k).and_then(|v| v.trim().parse::<i64>().ok());
    let kind = match num("FSProtocol").unwrap_or(2) {
        5 => "ftp",
        0 | 1 | 2 => "sftp",
        _ => return None, // WebDAV and S3 sites
    };
    let mut data = json!({});
    if kind == "ftp" {
        match num("Ftps").unwrap_or(0) {
            1 => data["tls"] = json!("implicit"),
            2 | 3 => data["tls"] = json!("explicit"),
            _ => {}
        }
    }
    if let Some(k) = get("PublicKeyFile").filter(|k| !k.is_empty()) {
        data["key_path"] = json!(unescape(&k));
    }
    let mut parts: Vec<String> = full.split('/').map(str::to_string).collect();
    let leaf = parts.pop().unwrap_or_default();
    let port = num("PortNumber").filter(|p| *p > 0).unwrap_or_else(|| if data["tls"] == "implicit" { 990 } else { default_port(kind) });
    Some(Item { folder: parts, name: leaf, kind: kind.into(), host, port, user, data, comment: String::new() })
}

fn winscp() -> Result<Vec<Item>> {
    let mut items = Vec::new();
    #[cfg(windows)]
    if let Ok(root) = windows_registry::CURRENT_USER.open(WINSCP) {
        for key in root.keys()? {
            let Ok(k) = root.open(&key) else { continue };
            let get = |n: &str| k.get_string(n).ok().or_else(|| k.get_u32(n).ok().map(|v| v.to_string()));
            if let Some(it) = winscp_item(&key, &get) {
                items.push(it);
            }
        }
    }
    if items.is_empty() {
        for ini in winscp_ini_paths() {
            if let Ok(text) = read_text(&ini) {
                let mut section: Option<(String, BTreeMap<String, String>)> = None;
                let flush = |sec: Option<(String, BTreeMap<String, String>)>, items: &mut Vec<Item>| {
                    if let Some((name, vals)) = sec {
                        if let Some(it) = winscp_item(&name, &|k: &str| vals.get(k).cloned()) {
                            items.push(it);
                        }
                    }
                };
                for line in text.lines() {
                    let line = line.trim();
                    if let Some(name) = line.strip_prefix("[Sessions\\").and_then(|l| l.strip_suffix(']')) {
                        flush(section.take(), &mut items);
                        section = Some((name.to_string(), BTreeMap::new()));
                    } else if line.starts_with('[') {
                        flush(section.take(), &mut items);
                    } else if let (Some((_, vals)), Some((k, v))) = (section.as_mut(), line.split_once('=')) {
                        vals.insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
                flush(section.take(), &mut items);
                break;
            }
        }
    }
    Ok(items)
}

fn winscp_ini_paths() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = dirs::config_dir() {
        v.push(d.join("WinSCP.ini"));
    }
    v.into_iter().filter(|p| p.exists()).collect()
}

// ---- OpenSSH config ----

fn ssh_config_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".ssh").join("config")
}

fn ssh_config(path: &Path) -> Result<Vec<Item>> {
    let Ok(text) = read_text(path) else { return Ok(Vec::new()) };
    struct Block {
        names: Vec<String>,
        vals: BTreeMap<String, String>,
    }
    let mut blocks: Vec<Block> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = match line.split_once(|c: char| c.is_whitespace() || c == '=') {
            Some((k, v)) => (k.to_lowercase(), v.trim_start_matches(|c: char| c.is_whitespace() || c == '=').trim().trim_matches('"').to_string()),
            None => continue,
        };
        match key.as_str() {
            "host" => blocks.push(Block { names: value.split_whitespace().map(str::to_string).collect(), vals: BTreeMap::new() }),
            "match" => blocks.push(Block { names: vec![], vals: BTreeMap::new() }),
            _ => {
                if let Some(b) = blocks.last_mut() {
                    b.vals.entry(key).or_insert(value);
                }
            }
        }
    }
    let mut items = Vec::new();
    for b in blocks {
        for name in b.names.iter().filter(|n| !n.contains(['*', '?', '!'])) {
            let host = b.vals.get("hostname").cloned().unwrap_or_else(|| name.clone());
            let mut data = json!({});
            if let Some(k) = b.vals.get("identityfile") {
                data["key_path"] = json!(k);
            }
            if let Some(j) = b.vals.get("proxyjump").filter(|j| j.as_str() != "none") {
                let first = j.split(',').next().unwrap_or("").trim();
                let (rest, port) = match first.rsplit_once(':') {
                    Some((h, p)) if p.parse::<u16>().is_ok() => (h, p.parse::<u16>().unwrap()),
                    _ => (first, 22),
                };
                let (jh, ju) = split_user(rest, "");
                data["jump_hosts"] = json!([{ "host": jh, "port": port, "user": ju }]);
            }
            items.push(Item {
                folder: vec![],
                name: name.clone(),
                kind: "ssh".into(),
                host,
                port: b.vals.get("port").and_then(|p| p.parse().ok()).unwrap_or(22),
                user: b.vals.get("user").cloned().unwrap_or_default(),
                data,
                comment: String::new(),
            });
        }
    }
    Ok(items)
}

// ---- Remote Desktop files ----

fn rdp_file(path: &Path) -> Result<Item> {
    let text = read_text(path)?;
    let mut vals = BTreeMap::new();
    for line in text.lines() {
        let mut it = line.trim().splitn(3, ':');
        if let (Some(k), Some(_), Some(v)) = (it.next(), it.next(), it.next()) {
            vals.insert(k.to_lowercase(), v.trim().to_string());
        }
    }
    let address = vals.get("full address").cloned().filter(|a| !a.is_empty()).with_context(|| format!("{} has no computer address.", path.display()))?;
    let (host, port) = match address.rsplit_once(':') {
        Some((h, p)) if p.parse::<i64>().is_ok() && !h.contains(':') => (h.to_string(), p.parse().unwrap()),
        _ => (address.trim_matches(['[', ']']).to_string(), 3389),
    };
    Ok(Item {
        folder: vec![],
        name: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| host.clone()),
        kind: "rdp".into(),
        host,
        port,
        user: vals.get("username").cloned().unwrap_or_default(),
        data: json!({}),
        comment: String::new(),
    })
}

// ---- MobaXterm: MobaXterm.ini (installed or portable copy) and .mxtsessions exports ----

/// Where MobaXterm keeps its settings: %APPDATA%\MobaXterm for the installed version, Documents\MobaXterm for
/// some setups.
fn mobaxterm_ini() -> Option<PathBuf> {
    [dirs::config_dir(), dirs::document_dir()]
        .into_iter()
        .flatten()
        .map(|d| d.join("MobaXterm").join("MobaXterm.ini"))
        .find(|p| p.exists())
}

fn kind_of(t: &str) -> &'static str {
    match t {
        "0" => "ssh",
        "1" => "telnet",
        "2" => "rsh",
        "3" => "xdmcp",
        "4" => "rdp",
        "5" => "vnc",
        "6" => "ftp",
        "7" => "sftp",
        "8" => "serial",
        "9" => "file",
        "10" => "shell",
        "11" => "browser",
        "12" => "mosh",
        "13" => "s3",
        "14" => "wsl",
        _ => "unknown",
    }
}

fn get<'a>(f: &'a [&'a str], i: usize) -> &'a str {
    f.get(i).copied().unwrap_or("")
}

fn on(v: &str) -> bool {
    v == "-1" || v == "1"
}

/// Turns one session line's value into (kind, host, port, user, settings, comment).
fn parse_value(value: &str) -> Option<(String, String, i64, String, Value, String)> {
    let parts: Vec<&str> = value.trim_start().split('#').collect();
    if parts.len() < 3 || !parts[0].is_empty() {
        return None;
    }
    let icon = parts[1];
    let f: Vec<&str> = parts[2].split('%').collect();
    let kind = kind_of(get(&f, 0)).to_string();
    let comment = parts.get(5).map(|c| c.trim().to_string()).unwrap_or_default();
    let mut host = get(&f, 1).to_string();
    let port: i64 = get(&f, 2).parse().unwrap_or_else(|_| default_port(&kind));
    let mut user = get(&f, 3).to_string();
    let mut data = json!({});

    match kind.as_str() {
        "ssh" => {
            data["x11"] = json!(on(get(&f, 5)));
            data["compression"] = json!(on(get(&f, 6)));
            data["command"] = json!(get(&f, 7));
            data["key_path"] = json!(get(&f, 14));
            let gw = get(&f, 8);
            if !gw.is_empty() {
                data["jump_hosts"] = json!([{
                    "host": gw,
                    "port": get(&f, 9).parse::<u16>().unwrap_or(22),
                    "user": get(&f, 10),
                }]);
            }
        }
        "rdp" => {
            let gw = get(&f, 13);
            if !gw.is_empty() {
                data["ssh_gateway"] = json!({
                    "host": gw,
                    "port": get(&f, 14).parse::<u16>().unwrap_or(22),
                    "user": get(&f, 15),
                });
            }
        }
        "sftp" => {
            data["key_path"] = json!(get(&f, 5));
        }
        "vnc" => {
            // VNC lines carry no user name; field 3 is an option flag.
            user.clear();
        }
        "wsl" => {
            data["distro"] = json!(host.clone());
            host.clear();
            user.clear();
        }
        "shell" => {
            data["shell"] = json!(host.clone());
            host.clear();
            user.clear();
        }
        "serial" => {
            data["serial_port"] = json!(host.clone());
            data["baud"] = json!(get(&f, 2).parse::<u32>().unwrap_or(9600));
            host.clear();
            user.clear();
        }
        "file" | "browser" => {
            // The path or address is the session's host, where opening it looks.
            user.clear();
        }
        _ => {}
    }
    // Everything the line said, so settings without a place here yet are not lost.
    data["imported"] = json!({
        "icon": icon,
        "fields": f,
        "terminal": parts.get(3).copied().unwrap_or(""),
        "start_mode": parts.get(4).copied().unwrap_or(""),
        "color": parts.get(6).copied().unwrap_or(""),
    });
    let port = if port == 0 { default_port(&kind) } else { port };
    Some((kind, host, port, user, data, comment))
}

fn session_file(path: &Path) -> Result<Vec<Item>> {
    let text = read_text(path)?;
    let mut items = Vec::new();
    let mut in_bookmarks = false;
    let mut folder: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if line.starts_with('[') {
            let name = line.trim_matches(|c| c == '[' || c == ']');
            in_bookmarks = name == "Bookmarks" || name.starts_with("Bookmarks_");
            folder.clear();
            continue;
        }
        if !in_bookmarks {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if key == "SubRep" {
            folder = value.split('\\').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            continue;
        }
        if key == "ImgNum" {
            continue;
        }
        let Some((kind, host, port, user, data, comment)) = parse_value(value) else { continue };
        items.push(Item { folder: folder.clone(), name: key.trim().to_string(), kind, host, port, user, data, comment });
    }
    Ok(items)
}

/// Reads a text file in UTF-8, UTF-16 (with its byte order mark) or the Windows ANSI code page; the last
/// keeps every ASCII character intact.
fn read_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return Ok(String::from_utf16_lossy(&units));
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).map(<[u8]>::to_vec).unwrap_or(bytes);
    Ok(match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => e.into_bytes().iter().map(|&b| b as char).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_users() {
        assert_eq!(unescape("My%20Server%2Fone"), "My Server/one");
        assert_eq!(unescape("100%"), "100%");
        assert_eq!(split_user("admin@web01", ""), ("web01".to_string(), "admin".to_string()));
        assert_eq!(split_user("web01", "root"), ("web01".to_string(), "root".to_string()));
    }

    #[test]
    fn ssh_config_hosts() {
        let dir = std::env::temp_dir().join(format!("cablebag-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config");
        std::fs::write(
            &p,
            "Host *\n  ServerAliveInterval 30\n\nHost web01 web\n  HostName 10.0.0.5\n  User admin\n  Port 2222\n  IdentityFile ~/.ssh/id_ed25519\n  ProxyJump jump@bastion:2200\n\nHost db01\n  User postgres\n",
        )
        .unwrap();
        let items = ssh_config(&p).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].name, "web01");
        assert_eq!((items[0].host.as_str(), items[0].port, items[0].user.as_str()), ("10.0.0.5", 2222, "admin"));
        assert_eq!(items[0].data["jump_hosts"][0]["host"], "bastion");
        assert_eq!(items[0].data["jump_hosts"][0]["port"], 2200);
        assert_eq!(items[0].data["jump_hosts"][0]["user"], "jump");
        assert_eq!((items[2].host.as_str(), items[2].port), ("db01", 22));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rdp_and_session_files() {
        let dir = std::env::temp_dir().join(format!("cablebag-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let rdp = dir.join("Terminal server.rdp");
        let text: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("screen mode id:i:2\r\nfull address:s:ts01.example.com:3390\r\nusername:s:CORP\\jan\r\n".encode_utf16().flat_map(|u| u.to_le_bytes()))
            .collect();
        std::fs::write(&rdp, text).unwrap();
        let it = rdp_file(&rdp).unwrap();
        assert_eq!((it.name.as_str(), it.host.as_str(), it.port, it.user.as_str()), ("Terminal server", "ts01.example.com", 3390, "CORP\\jan"));

        let ini = dir.join("sessions.ini");
        std::fs::write(&ini, "[Bookmarks_1]\r\nSubRep=Lab\\Linux\r\nImgNum=41\r\nweb01= #109#0%10.0.0.5%22%admin%%-1%-1%%%%%0%0%0%%%-1%0%0%0%%1080%%0%0%1#Consolas%10%0%0%-1%15%236,236,236%30,30,30%180,180,192%0%-1%0%%xterm%-1%-1%_Std_Colors_0_%80%24%0%1%-1%<none>%%0%0%-1#0# #-1\r\n").unwrap();
        let items = session_file(&ini).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].folder, vec!["Lab".to_string(), "Linux".to_string()]);
        assert_eq!((items[0].kind.as_str(), items[0].host.as_str(), items[0].user.as_str()), ("ssh", "10.0.0.5", "admin"));
        std::fs::remove_dir_all(&dir).ok();
    }
}

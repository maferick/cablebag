//! Containers on a server, over the SSH connection of a Containers tab: the docker CLI on channels of their own,
//! so no Docker port is opened anywhere. Compose projects are found through the labels Docker Compose puts on every
//! container it starts (where the project's folder and files are). Stacks made in Portainer keep their files inside
//! Portainer's own data; those are read through Docker and changed in Portainer.

use crate::agent::{exec_line, ExecResult};
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::Value;
use crate::store::Session;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, PartialEq)]
pub struct Port {
    /// The port inside the container.
    pub private: u16,
    pub proto: String,
    /// The port on the server, when the container publishes it.
    pub public: Option<u16>,
}

#[derive(Debug, Serialize)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    /// running, exited, restarting, paused, created, dead
    pub state: String,
    /// healthy, unhealthy, starting, or empty without a health check.
    pub health: String,
    pub started: String,
    pub restarts: u64,
    pub ports: Vec<Port>,
    /// Compose: the project, the service, the project's folder and its compose files ("" and [] outside Compose).
    pub project: String,
    pub service: String,
    pub working_dir: String,
    pub config_files: Vec<String>,
    /// Made by Portainer: its compose files are inside Portainer's data, not in a folder of the server.
    pub portainer: bool,
    /// What the container uses from the server: folders and files (bind), Docker volumes (volume).
    pub mounts: Vec<MountInfo>,
    /// The Docker networks it is on, and host when it uses the server's own network.
    pub networks: Vec<String>,
    pub network_mode: String,
    pub privileged: bool,
    /// Devices of the server it uses (/dev/dri, a USB stick).
    pub devices: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct MountInfo {
    /// bind or volume
    pub kind: String,
    /// The folder on the server, or the volume's name.
    pub source: String,
    pub dest: String,
    pub rw: bool,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Portainer {
    /// The Portainer container, to read stack files from.
    pub container: String,
    /// Where Portainer's /data is on the server (root only, usually a Docker volume).
    pub data: String,
    /// Its web interface on the server: https on 9443, else http on 9000.
    pub url_port: Option<u16>,
    pub https: bool,
}

#[derive(Debug, Serialize)]
pub struct Listing {
    /// direct (the login may use Docker), sudo (through sudo without a password), denied, or none (no Docker).
    pub access: String,
    /// plugin (docker compose), standalone (docker-compose) or none.
    pub compose: String,
    pub containers: Vec<Container>,
    pub portainer: Option<Portainer>,
    /// The compose files in ~/stacks on the server, also of stacks that have no containers now.
    pub stacks: Vec<String>,
    /// Docker said no to the login, but sudo is there (it may want a password).
    pub sudo_possible: bool,
}

/// One line per container, as JSON. Single quotes are not allowed in it: the script sends it inside '...'.
/// .State whole, since .State.Health is missing (not empty) on containers without a health check.
const FORMAT: &str = r#"{"id":{{json .Id}},"name":{{json .Name}},"image":{{json .Config.Image}},"state":{{json .State}},"restarts":{{.RestartCount}},"labels":{{json .Config.Labels}},"ports":{{json .NetworkSettings.Ports}},"mounts":{{json .Mounts}},"hc":{"net":{{json .HostConfig.NetworkMode}},"priv":{{json .HostConfig.Privileged}},"dev":{{json .HostConfig.Devices}}},"nets":{{json .NetworkSettings.Networks}}}"#;

fn list_script() -> String {
    format!(
        r#"D=docker
if docker ps -q >/dev/null 2>&1; then echo "access direct"
elif sudo -n docker ps -q >/dev/null 2>&1; then echo "access sudo"; D="sudo -n docker"
elif command -v docker >/dev/null 2>&1; then echo "access denied"; command -v sudo >/dev/null 2>&1 && echo "sudo yes"; exit 0
else echo "access none"; exit 0; fi
if $D compose version >/dev/null 2>&1; then echo "compose plugin"
elif command -v docker-compose >/dev/null 2>&1; then echo "compose standalone"
else echo "compose none"; fi
{HOME}
for f in "$h"/stacks/*/compose.yml "$h"/stacks/*/compose.yaml "$h"/stacks/*/docker-compose.yml "$h"/stacks/*/docker-compose.yaml; do [ -f "$f" ] && echo "stack $f"; done
ids=$($D ps -aq --no-trunc)
[ -z "$ids" ] || $D inspect --format '{FORMAT}' $ids"#
    )
}

/// The text as one word for a POSIX shell.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn conn(app: &AppHandle, tab: &str) -> Result<Arc<crate::ssh::SshConn>> {
    app.state::<AppState>().ssh.lock().get(tab).cloned().ok_or_else(|| anyhow!("This Containers tab is not connected."))
}

/// Containers tabs whose login may only use Docker through sudo with a password: their scripts run as root,
/// with the password from the session's vault login or asked (as the assistant does).
/// The password sudo took is kept in memory while the tab is open, so it is asked once, not for every refresh.
static AS_ROOT: LazyLock<parking_lot::Mutex<HashMap<String, (Session, Option<String>)>>> = LazyLock::new(Default::default);
/// One password question at a time: commands that start together wait for the first answer.
static ASKING: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(Default::default);

pub fn use_sudo_password(tab: &str, session: Option<Session>) {
    match session {
        Some(s) => AS_ROOT.lock().insert(tab.to_string(), (s, None)),
        None => AS_ROOT.lock().remove(tab),
    };
}

async fn run(app: &AppHandle, tab: &str, script: &str, timeout: Duration) -> Result<ExecResult> {
    if AS_ROOT.lock().contains_key(tab) {
        let mut asking = None;
        if AS_ROOT.lock().get(tab).is_some_and(|(_, pw)| pw.is_none()) {
            asking = Some(ASKING.lock().await);
        }
        let Some((session, known)) = AS_ROOT.lock().get(tab).cloned() else { bail!("This Containers tab is closed.") };
        let why = "Docker on this server only works as root.";
        return match crate::agent::exec_root(app, tab, &session, script, true, timeout, known, Some(why)).await {
            Ok((res, used)) => {
                if let Some(e) = AS_ROOT.lock().get_mut(tab) {
                    e.1 = used.or(e.1.take());
                }
                drop(asking);
                Ok(res)
            }
            Err(err) => {
                // Cancelled or refused: back to without sudo, so the next refresh does not ask again. The tab offers
                // the choice anew.
                let text = err.to_string();
                if text.contains("No password given") || text.contains("did not accept the password") {
                    AS_ROOT.lock().remove(tab);
                } else if let Some(e) = AS_ROOT.lock().get_mut(tab) {
                    e.1 = None;
                }
                Err(err)
            }
        };
    }
    let conn = conn(app, tab)?;
    exec_line(&conn, &format!("sh -c {}", sh_quote(script)), timeout).await
}

/// The login's home, also when the script runs as root through sudo.
const HOME: &str = r#"h="$HOME"; [ -n "$SUDO_USER" ] && h=$(eval echo "~$SUDO_USER")"#;

pub async fn list(app: &AppHandle, tab: &str) -> Result<Listing> {
    let r = run(app, tab, &list_script(), Duration::from_secs(30)).await?;
    if r.timed_out {
        bail!("Docker on the server did not answer within 30 seconds.");
    }
    parse_listing(&r.output)
}

fn parse_listing(output: &str) -> Result<Listing> {
    let mut access = String::new();
    let mut compose = "none".to_string();
    let mut stacks = Vec::new();
    let mut sudo_possible = false;
    let mut raw = Vec::new();
    for line in output.lines().map(str::trim) {
        if let Some(a) = line.strip_prefix("access ") {
            access = a.to_string();
        } else if line == "sudo yes" {
            sudo_possible = true;
        } else if let Some(f) = line.strip_prefix("stack ") {
            stacks.push(f.to_string());
        } else if let Some(c) = line.strip_prefix("compose ") {
            compose = c.to_string();
        } else if line.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                raw.push(v);
            }
        }
    }
    if access.is_empty() {
        bail!("Could not ask Docker on the server: {}", output.trim().lines().last().unwrap_or("no answer"));
    }
    let portainer = raw.iter().find_map(portainer_of);
    let mut containers: Vec<Container> = raw.iter().map(|v| container(v, portainer.as_ref())).collect();
    containers.sort_by(|a, b| (a.project.is_empty(), &a.project, &a.name).cmp(&(b.project.is_empty(), &b.project, &b.name)));
    Ok(Listing { access, compose, containers, portainer, stacks, sudo_possible })
}

/// The Portainer container (the image says so, and it has a /data mount).
fn portainer_of(v: &Value) -> Option<Portainer> {
    let image = v["image"].as_str()?;
    if !image.split('/').any(|p| p.starts_with("portainer")) || image.contains("agent") {
        return None;
    }
    let data = v["mounts"].as_array()?.iter().find(|m| m["Destination"] == "/data")?["Source"].as_str()?.to_string();
    let ports = ports_of(v);
    let public = |private: u16| ports.iter().find(|p| p.private == private).and_then(|p| p.public);
    let (url_port, https) = match (public(9443), public(9000)) {
        (Some(p), _) => (Some(p), true),
        (None, Some(p)) => (Some(p), false),
        _ => (None, true),
    };
    Some(Portainer { container: v["name"].as_str()?.trim_start_matches('/').to_string(), data, url_port, https })
}

fn ports_of(v: &Value) -> Vec<Port> {
    let mut ports: Vec<Port> = Vec::new();
    if let Some(map) = v["ports"].as_object() {
        for (key, binds) in map {
            let (num, proto) = key.split_once('/').unwrap_or((key, "tcp"));
            let Ok(private) = num.parse() else { continue };
            let publics: Vec<u16> = binds.as_array().map(|b| b.iter().filter_map(|x| x["HostPort"].as_str()?.parse().ok()).collect()).unwrap_or_default();
            if publics.is_empty() {
                ports.push(Port { private, proto: proto.into(), public: None });
            }
            // The same port on IPv4 and IPv6 counts once.
            for public in publics {
                let p = Port { private, proto: proto.into(), public: Some(public) };
                if !ports.contains(&p) {
                    ports.push(p);
                }
            }
        }
    }
    ports.sort_by_key(|p| (p.public.is_none(), p.public, p.private));
    ports
}

fn container(v: &Value, portainer: Option<&Portainer>) -> Container {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let labels: BTreeMap<String, String> = serde_json::from_value(v["labels"].clone()).unwrap_or_default();
    let label = |k: &str| labels.get(k).cloned().unwrap_or_default();
    let working_dir = label("com.docker.compose.project.working_dir");
    let config_files: Vec<String> =
        label("com.docker.compose.project.config_files").split(',').map(str::trim).filter(|f| !f.is_empty()).map(String::from).collect();
    // Portainer starts stacks from /data/compose/<id> inside its own container; some older stacks name the same
    // folder by its place on the server.
    let in_portainer = |p: &str| p.starts_with("/data/compose/") || portainer.is_some_and(|x| p.starts_with(&format!("{}/compose/", x.data)));
    Container {
        id: s("id"),
        name: s("name").trim_start_matches('/').to_string(),
        image: s("image"),
        state: v["state"]["Status"].as_str().unwrap_or("").to_string(),
        health: v["state"]["Health"]["Status"].as_str().unwrap_or("").to_string(),
        started: v["state"]["StartedAt"].as_str().unwrap_or("").to_string(),
        restarts: v["restarts"].as_u64().unwrap_or(0),
        ports: ports_of(v),
        project: label("com.docker.compose.project"),
        service: label("com.docker.compose.service"),
        portainer: !working_dir.is_empty() && in_portainer(&working_dir),
        working_dir,
        config_files,
        mounts: v["mounts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| matches!(m["Type"].as_str(), Some("bind" | "volume")))
            .map(|m| MountInfo {
                kind: m["Type"].as_str().unwrap_or("").to_string(),
                source: if m["Type"] == "volume" { m["Name"].as_str() } else { m["Source"].as_str() }.unwrap_or("").to_string(),
                dest: m["Destination"].as_str().unwrap_or("").to_string(),
                rw: m["RW"] != false,
            })
            .collect(),
        networks: v["nets"].as_object().map(|n| n.keys().cloned().collect()).unwrap_or_default(),
        network_mode: v["hc"]["net"].as_str().unwrap_or("").to_string(),
        privileged: v["hc"]["priv"] == true,
        devices: v["hc"]["dev"].as_array().into_iter().flatten().filter_map(|d| d["PathOnHost"].as_str().map(String::from)).collect(),
    }
}

/// A compose file of a Portainer stack, read out of the Portainer container (docker cp gives a tar on stdout).
pub async fn portainer_file(app: &AppHandle, tab: &str, sudo: bool, portainer: &str, data: &str, path: &str) -> Result<String> {
    // The path as Portainer sees it: /data/..., also when the label names the folder on the server.
    let inside = match path.strip_prefix(data).filter(|_| !data.is_empty()) {
        Some(rest) => format!("/data{rest}"),
        None => path.to_string(),
    };
    if !inside.starts_with("/data/compose/") || inside.contains("..") {
        bail!("That is not a file of a Portainer stack.");
    }
    if portainer.is_empty() || !portainer.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        bail!("That is not a container name.");
    }
    let r = run(app, tab, &format!("{} cp {}:{} - | tar -xO", docker(sudo), portainer, sh_quote(&inside)), Duration::from_secs(30)).await?;
    if r.exit != Some(0) {
        bail!("Could not read {inside} from Portainer: {}", r.output.trim());
    }
    Ok(r.output)
}

fn docker(sudo: bool) -> &'static str {
    if sudo { "sudo -n docker" } else { "docker" }
}

/// start, stop, restart, pause or unpause one container.
pub async fn container_action(app: &AppHandle, tab: &str, sudo: bool, id: &str, action: &str) -> Result<ExecResult> {
    if !matches!(action, "start" | "stop" | "restart" | "pause" | "unpause") {
        bail!("Unknown container action {action}.");
    }
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("That is not a container id.");
    }
    run(app, tab, &format!("{} {action} {id} 2>&1", docker(sudo)), Duration::from_secs(120)).await
}

/// The command line for a Compose action on a project, run in the project's folder.
fn compose_line(compose: &str, sudo: bool, project: &str, dir: &str, files: &[String], action: &str) -> Result<String> {
    let args = match action {
        "up" => "up -d",
        "pull" => "pull",
        "restart" => "restart",
        "stop" => "stop",
        "start" => "start",
        // Removes the containers and the project's network; volumes stay.
        "down" => "down",
        _ => bail!("Unknown Compose action {action}."),
    };
    if project.is_empty() || !project.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("That is not a Compose project name.");
    }
    if dir.is_empty() {
        bail!("Docker does not know the folder of this project.");
    }
    let program = compose_program(compose, sudo);
    let files: String = files.iter().map(|f| format!(" -f {}", sh_quote(f))).collect();
    Ok(format!("cd {} && {program} -p {project}{files} {args} 2>&1", sh_quote(dir)))
}

pub async fn compose_action(app: &AppHandle, tab: &str, compose: &str, sudo: bool, project: &str, dir: &str, files: &[String], action: &str) -> Result<ExecResult> {
    let line = compose_line(compose, sudo, project, dir, files, action)?;
    run(app, tab, &line, Duration::from_secs(600)).await
}

/// The last lines a container wrote, for the assistant.
pub async fn logs(app: &AppHandle, tab: &str, sudo: bool, id: &str, lines: u32) -> Result<String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("That is not a container id.");
    }
    let r = run(app, tab, &format!("{} logs --tail {} {id} 2>&1", docker(sudo), lines.clamp(10, 1000)), Duration::from_secs(30)).await?;
    Ok(r.output)
}

#[derive(Debug, Serialize)]
pub struct NewStack {
    /// The stack's folder on the server.
    pub dir: String,
    pub output: String,
    pub exit: Option<u32>,
}

/// A stack of Cablebag's own: the compose file (and .env when given) in ~/stacks/<name> on the server, only
/// readable by the login (they may hold passwords), then docker compose up -d. An existing stack is not overwritten.
pub async fn create_stack(app: &AppHandle, tab: &str, compose: &str, sudo: bool, name: &str, file: &str, env: &str) -> Result<NewStack> {
    use base64::Engine;
    let ok = name.len() <= 63 && name.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit()) && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !ok {
        bail!("A stack name has small letters, digits, - and _ only, and starts with a letter or digit.");
    }
    if file.trim().is_empty() {
        bail!("The compose file is empty.");
    }
    let b64 = |s: &str| base64::engine::general_purpose::STANDARD.encode(s.as_bytes());
    let env_part = if env.trim().is_empty() { String::new() } else { format!(" && printf %s '{}' | base64 -d > \"$d/.env\"", b64(env)) };
    let program = compose_program(compose, sudo);
    let script = format!(
        r#"{HOME}
d="$h/stacks/{name}"
if [ -e "$d/compose.yml" ] || [ -e "$d/docker-compose.yml" ]; then echo "exists $d"; exit 3; fi
umask 077 && mkdir -p "$d" && printf %s '{}' | base64 -d > "$d/compose.yml"{env_part} || exit 4
[ -z "$SUDO_USER" ] || chown "$SUDO_USER" "$h/stacks" "$d" "$d/compose.yml" "$d/.env" 2>/dev/null
echo "dir $d"
cd "$d" && {program} -p {name} -f compose.yml up -d 2>&1"#,
        b64(file)
    );
    let r = run(app, tab, &script, Duration::from_secs(900)).await?;
    let first = r.output.lines().next().unwrap_or("");
    if let Some(d) = first.strip_prefix("exists ") {
        bail!("There is already a stack in {d}. Pick another name, or edit that one.");
    }
    let Some(dir) = first.strip_prefix("dir ") else {
        bail!("Could not write the stack's files: {}", r.output.trim());
    };
    Ok(NewStack { dir: dir.to_string(), output: r.output.lines().skip(1).collect::<Vec<_>>().join("\n"), exit: r.exit })
}

fn compose_program(compose: &str, sudo: bool) -> String {
    match (compose, sudo) {
        ("standalone", false) => "docker-compose".to_string(),
        ("standalone", true) => "sudo -n docker-compose".to_string(),
        _ => format!("{} compose", docker(sudo)),
    }
}

/// A compose file that starts the same container again, made from docker inspect: for a container that was
/// started without Compose. Values of settings that look secret become {{blanks}}.
pub async fn container_compose(app: &AppHandle, tab: &str, sudo: bool, id: &str) -> Result<String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("That is not a container id.");
    }
    let r = run(app, tab, &format!("{} inspect {id}", docker(sudo)), Duration::from_secs(30)).await?;
    let all: Value = serde_json::from_str(&r.output).map_err(|_| anyhow!("Docker did not describe the container: {}", r.output.trim()))?;
    let c = all.get(0).ok_or_else(|| anyhow!("Docker does not know this container."))?;
    // What the image sets itself is left out, so the file holds only what the container added.
    let image = c["Config"]["Image"].as_str().unwrap_or("");
    let img = match run(app, tab, &format!("{} image inspect --format '{{{{json .Config}}}}' {}", docker(sudo), sh_quote(image)), Duration::from_secs(30)).await {
        Ok(r) if r.exit == Some(0) => serde_json::from_str(r.output.trim()).unwrap_or(Value::Null),
        _ => Value::Null,
    };
    Ok(compose_from(c, &img))
}

/// What a server holds of shared storage: NFS and SMB mounts (a NAS folder, mounted here), and, when the server is
/// the NAS itself, its own volumes, with its addresses to recognise it in the other server's mounts.
#[derive(Debug, Serialize, serde::Deserialize, Default, Clone)]
pub struct Mounts {
    pub ips: Vec<String>,
    pub shared: Vec<Shared>,
    /// Folders that are the storage itself (an Asustor or Synology /volume1).
    pub roots: Vec<String>,
    /// The same folder under another name on the NAS: /share/Docker for /volume2/Docker.
    pub aliases: Vec<(String, String)>,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct Shared {
    /// The NAS, as the mount names it (an address or a name).
    pub host: String,
    /// The folder on the NAS; for SMB the share as /share/<name>/...
    pub path: String,
    /// Where it is mounted on this server.
    pub local: String,
}

const MOUNTS: &str = r#"for a in $(hostname -I 2>/dev/null); do echo "ip $a"; done
ip -4 -o addr show 2>/dev/null | while read -r n i f a rest; do echo "ip ${a%/*}"; done
while read -r src dst typ rest; do case "$typ" in nfs|nfs4|cifs|smb3) echo "shared $src $dst";; esac; done < /proc/mounts
for v in /volume[0-9]*; do [ -d "$v" ] && echo "root $v"; done
for s in /share/*; do [ -d "$s" ] || continue; n=${s#/share/}; for v in /volume[0-9]*/"$n"; do [ -d "$v" ] && echo "alias $s $v"; done; done"#;

pub async fn mounts(app: &AppHandle, tab: &str) -> Result<Mounts> {
    let conn = conn(app, tab)?;
    let r = exec_line(&conn, &format!("sh -c {}", sh_quote(MOUNTS)), Duration::from_secs(20)).await?;
    Ok(parse_mounts(&r.output))
}

/// /proc/mounts writes a space as \040.
fn unescape(s: &str) -> String {
    s.replace("\\040", " ").replace("\\011", "\t").replace("\\134", "\\")
}

fn parse_mounts(out: &str) -> Mounts {
    let mut m = Mounts::default();
    for line in out.lines().map(str::trim) {
        let mut w = line.splitn(3, ' ');
        match (w.next(), w.next(), w.next()) {
            (Some("ip"), Some(ip), None) if !ip.is_empty() && !m.ips.contains(&ip.to_string()) && ip != "127.0.0.1" => m.ips.push(ip.to_string()),
            (Some("shared"), Some(src), Some(dst)) => {
                let src = unescape(src);
                let local = unescape(dst);
                // NFS: host:/path. SMB: //host/share/sub, kept as /share/<share>/sub (where an Asustor keeps it).
                let found = if let Some(rest) = src.strip_prefix("//") {
                    rest.split_once('/').map(|(h, p)| (h.to_string(), format!("/share/{p}")))
                } else {
                    src.split_once(":/").map(|(h, p)| (h.to_string(), format!("/{p}")))
                };
                if let Some((host, path)) = found {
                    m.shared.push(Shared { host, path: path.trim_end_matches('/').to_string(), local: local.trim_end_matches('/').to_string() });
                }
            }
            (Some("root"), Some(r), None) => m.roots.push(r.to_string()),
            (Some("alias"), Some(a), Some(b)) => m.aliases.push((a.to_string(), b.to_string())),
            _ => {}
        }
    }
    m
}

/// Where a path of one server is on the other one.
#[derive(Debug, Serialize, PartialEq)]
pub struct Translation {
    pub path: String,
    /// nas: on shared storage; local: only on this server (to copy); host: a file of the server itself
    /// (/etc/localtime), the same path on the other one.
    pub kind: String,
    /// The path on the other server; None when it does not have that storage.
    pub to: Option<String>,
    /// The place on the NAS, as host:/path.
    pub nas: Option<String>,
}

fn under(path: &str, base: &str) -> Option<String> {
    if path == base {
        Some(String::new())
    } else {
        path.strip_prefix(base).filter(|r| r.starts_with('/') || base.ends_with('/')).map(String::from)
    }
}

/// /share/Docker/x -> /volume2/Docker/x with the NAS's own aliases.
fn canonical(path: &str, aliases: &[(String, String)]) -> String {
    for (a, real) in aliases {
        if let Some(rest) = under(path, a) {
            return format!("{real}{rest}");
        }
    }
    path.to_string()
}

/// The NAS and the place on it for a path of a server, through its mounts (the most precise one), or itself
/// when the server is the NAS.
fn to_nas(path: &str, m: &Mounts, aliases: &[(String, String)]) -> Option<(Vec<String>, String)> {
    let best = m.shared.iter().filter_map(|s| under(path, &s.local).map(|rest| (s, rest))).max_by_key(|(s, _)| s.local.len());
    if let Some((s, rest)) = best {
        return Some((vec![s.host.clone()], canonical(&format!("{}{rest}", s.path), aliases)));
    }
    let real = canonical(path, &m.aliases);
    if m.roots.iter().any(|r| under(&real, r).is_some()) {
        return Some((m.ips.clone(), real));
    }
    None
}

fn from_nas(hosts: &[String], nas: &str, m: &Mounts, aliases: &[(String, String)]) -> Option<String> {
    let best = m
        .shared
        .iter()
        .filter(|s| hosts.iter().any(|h| h.eq_ignore_ascii_case(&s.host)))
        .filter_map(|s| under(nas, &canonical(&s.path, aliases)).map(|rest| (s, rest)))
        .max_by_key(|(s, _)| s.path.len());
    if let Some((s, rest)) = best {
        return Some(format!("{}{rest}", s.local));
    }
    // The other server is the NAS itself.
    if hosts.iter().any(|h| m.ips.contains(h)) && m.roots.iter().any(|r| under(nas, r).is_some()) {
        return Some(nas.to_string());
    }
    None
}

/// Translates the bind paths of a stack from one server to another through their shared storage.
pub fn translate(paths: &[String], from: &Mounts, to: &Mounts) -> Vec<Translation> {
    let aliases: Vec<(String, String)> = from.aliases.iter().chain(to.aliases.iter()).cloned().collect();
    paths
        .iter()
        .map(|p| {
            if ["/etc/", "/dev/", "/run/", "/var/run/", "/sys/", "/proc/"].iter().any(|h| p.starts_with(h)) {
                return Translation { path: p.clone(), kind: "host".into(), to: Some(p.clone()), nas: None };
            }
            match to_nas(p, from, &aliases) {
                Some((hosts, nas)) => {
                    // The NAS by any of its names: the address the mount uses, and the NAS's own addresses.
                    let mut ids = hosts.clone();
                    ids.extend(to.shared.iter().filter(|s| from.ips.contains(&s.host)).map(|s| s.host.clone()));
                    let target = from_nas(&ids, &nas, to, &aliases);
                    Translation { path: p.clone(), kind: "nas".into(), to: target, nas: Some(format!("{}:{nas}", hosts.first().cloned().unwrap_or_default())) }
                }
                None => Translation { path: p.clone(), kind: "local".into(), to: None, nas: None },
            }
        })
        .collect()
}

/// What the other server already has: container names, networks, ports in use, images, registries it is logged
/// in to, its processor and whether the stack's folder exists.
#[derive(Debug, Serialize, Default)]
pub struct Target {
    pub arch: String,
    pub names: Vec<String>,
    pub networks: Vec<String>,
    pub ports: Vec<u16>,
    pub images: Vec<String>,
    pub logins: Vec<String>,
    pub stack_exists: bool,
}

pub async fn target(app: &AppHandle, tab: &str, sudo: bool, images: &[String], stack: &str) -> Result<Target> {
    if !stack.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("That is not a stack name.");
    }
    let d = docker(sudo);
    let imgs: String = images.iter().map(|i| sh_quote(i)).collect::<Vec<_>>().join(" ");
    let script = format!(
        r#"{HOME}
echo "arch $(uname -m)"
{d} ps -a --format '{{{{.Names}}}}' | sed 's/^/name /'
{d} network ls --format '{{{{.Name}}}}' | sed 's/^/net /'
{d} ps --format '{{{{.Ports}}}}' | tr ',' '\n' | sed -n 's/.*:\([0-9][0-9]*\)->.*/port \1/p'
(ss -ltnH 2>/dev/null || netstat -ltn 2>/dev/null) | awk '{{print $4}}' | sed -n 's/.*:\([0-9][0-9]*\)$/port \1/p'
for i in {imgs}; do {d} image inspect "$i" >/dev/null 2>&1 && echo "have $i"; done
for c in "$h/.docker/config.json" "$HOME/.docker/config.json"; do [ -f "$c" ] && tr -d ' \n\t' < "$c" | grep -o '"auths":{{.*' | grep -o '"[^"]*":{{"auth' | cut -d'"' -f2 | sed 's/^/login /'; done
[ -e "$h/stacks/{stack}" ] && echo "stack exists""#
    );
    let r = run(app, tab, &script, Duration::from_secs(60)).await?;
    let mut t = Target::default();
    for line in r.output.lines().map(str::trim) {
        if let Some(v) = line.strip_prefix("arch ") {
            t.arch = v.to_string();
        } else if let Some(v) = line.strip_prefix("name ") {
            t.names.push(v.to_string());
        } else if let Some(v) = line.strip_prefix("net ") {
            t.networks.push(v.to_string());
        } else if let Some(v) = line.strip_prefix("port ") {
            if let Ok(p) = v.parse() {
                if !t.ports.contains(&p) {
                    t.ports.push(p);
                }
            }
        } else if let Some(v) = line.strip_prefix("have ") {
            t.images.push(v.to_string());
        } else if let Some(v) = line.strip_prefix("login ") {
            t.logins.push(v.to_string());
        } else if line == "stack exists" {
            t.stack_exists = true;
        }
    }
    Ok(t)
}

/// The exact versions of images as they run now (name@sha256:...), to start the same on the other server.
pub async fn digests(app: &AppHandle, tab: &str, sudo: bool, images: &[String]) -> Result<HashMap<String, String>> {
    let d = docker(sudo);
    let imgs: String = images.iter().map(|i| sh_quote(i)).collect::<Vec<_>>().join(" ");
    let r = run(app, tab, &format!("for i in {imgs}; do printf '%s ' \"$i\"; {d} image inspect --format '{{{{json .RepoDigests}}}}' \"$i\" 2>/dev/null || echo; done"), Duration::from_secs(30)).await?;
    let mut out = HashMap::new();
    for line in r.output.lines() {
        if let Some((img, json)) = line.split_once(' ') {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(json.trim()) {
                if let Some(first) = list.first().and_then(|d| d.split_once('@')) {
                    out.insert(img.to_string(), first.1.to_string());
                }
            }
        }
    }
    Ok(out)
}

/// Whether an environment variable's name says it holds a secret (MYSQL_ROOT_PASSWORD, API_KEY, ...).
pub fn secret_name(name: &str) -> bool {
    name.to_ascii_uppercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|p| matches!(p, "PASSWORD" | "PASS" | "PASSWD" | "PWD" | "SECRET" | "TOKEN" | "KEY" | "APIKEY" | "PRIVATE" | "CREDENTIALS"))
}

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

fn compose_from(c: &Value, img: &Value) -> String {
    let name = c["Name"].as_str().unwrap_or("app").trim_start_matches('/');
    let service: String = name.chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch.to_ascii_lowercase() } else { '-' }).collect();
    let cfg = &c["Config"];
    let host = &c["HostConfig"];
    let mut y = format!("services:\n  {service}:\n    image: {}\n    container_name: {}\n", q(cfg["Image"].as_str().unwrap_or("")), q(name));
    match host["RestartPolicy"]["Name"].as_str() {
        Some(p) if !p.is_empty() && p != "no" => y += &format!("    restart: {p}\n"),
        _ => {}
    }
    if host["NetworkMode"] == "host" {
        y += "    network_mode: host\n";
    }
    let strings = |v: &Value| v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect::<Vec<_>>()).unwrap_or_default();
    if cfg["Cmd"] != img["Cmd"] && cfg["Cmd"].is_array() {
        y += &format!("    command: {}\n", cfg["Cmd"]);
    }
    if cfg["Entrypoint"] != img["Entrypoint"] && cfg["Entrypoint"].is_array() {
        y += &format!("    entrypoint: {}\n", cfg["Entrypoint"]);
    }
    let mut ports = Vec::new();
    if let Some(map) = host["PortBindings"].as_object() {
        for (key, binds) in map {
            let (num, proto) = key.split_once('/').unwrap_or((key, "tcp"));
            for b in binds.as_array().into_iter().flatten() {
                let hp = b["HostPort"].as_str().unwrap_or("");
                let ip = b["HostIp"].as_str().unwrap_or("");
                let mut p = if ip.is_empty() || ip == "0.0.0.0" || ip == "::" { format!("{hp}:{num}") } else { format!("{ip}:{hp}:{num}") };
                if proto != "tcp" {
                    p += &format!("/{proto}");
                }
                if !hp.is_empty() && !ports.contains(&p) {
                    ports.push(p);
                }
            }
        }
    }
    ports.sort();
    if !ports.is_empty() {
        y += "    ports:\n";
        for p in &ports {
            y += &format!("      - {}\n", q(p));
        }
    }
    let image_env = strings(&img["Env"]);
    let env: Vec<String> = strings(&cfg["Env"]).into_iter().filter(|e| !image_env.contains(e)).collect();
    if !env.is_empty() {
        y += "    environment:\n";
        for e in &env {
            let (k, v) = e.split_once('=').unwrap_or((e, ""));
            let v = if secret_name(k) && !v.is_empty() { format!("{{{{{k}}}}}") } else { v.to_string() };
            y += &format!("      {k}: {}\n", q(&v));
        }
    }
    let mut named = Vec::new();
    let mut vols = Vec::new();
    for m in c["Mounts"].as_array().into_iter().flatten() {
        let dst = m["Destination"].as_str().unwrap_or("");
        let ro = if m["RW"] == false { ":ro" } else { "" };
        match m["Type"].as_str() {
            Some("bind") => vols.push(format!("{}:{dst}{ro}", m["Source"].as_str().unwrap_or(""))),
            Some("volume") => {
                let n = m["Name"].as_str().unwrap_or("");
                // A volume Docker named itself (a long hex string) belongs to this container only: leave it out.
                if n.len() == 64 && n.chars().all(|c| c.is_ascii_hexdigit()) {
                    continue;
                }
                vols.push(format!("{n}:{dst}{ro}"));
                named.push(n.to_string());
            }
            _ => {}
        }
    }
    if !vols.is_empty() {
        y += "    volumes:\n";
        for v in &vols {
            y += &format!("      - {}\n", q(v));
        }
    }
    if !named.is_empty() {
        // The same data as now: the volumes exist already.
        y += "volumes:\n";
        for n in &named {
            y += &format!("  {n}:\n    external: true\n");
        }
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_from_docker() {
        let out = concat!(
            "access direct\n",
            "compose plugin\n",
            r#"{"id":"aa11","name":"/web-1","image":"nginx:1.27","state":{"Status":"running","StartedAt":"2026-10-24T08:00:00Z","Health":{"Status":"healthy"}},"restarts":0,"labels":{"com.docker.compose.project":"web","com.docker.compose.service":"web","com.docker.compose.project.working_dir":"/srv/web","com.docker.compose.project.config_files":"/srv/web/compose.yml,/srv/web/compose.override.yml"},"ports":{"80/tcp":[{"HostIp":"0.0.0.0","HostPort":"8080"},{"HostIp":"::","HostPort":"8080"}],"443/tcp":null},"mounts":[]}"#,
            "\n",
            r#"{"id":"bb22","name":"/loose","image":"alpine","state":{"Status":"exited","StartedAt":""},"restarts":3,"labels":null,"ports":{},"mounts":null}"#,
            "\n"
        );
        let l = parse_listing(out).unwrap();
        assert_eq!((l.access.as_str(), l.compose.as_str(), l.containers.len()), ("direct", "plugin", 2));
        let web = &l.containers[0];
        assert_eq!((web.name.as_str(), web.project.as_str(), web.working_dir.as_str()), ("web-1", "web", "/srv/web"));
        assert_eq!((web.state.as_str(), web.health.as_str(), web.portainer), ("running", "healthy", false));
        assert_eq!(web.config_files, vec!["/srv/web/compose.yml", "/srv/web/compose.override.yml"]);
        assert_eq!(web.ports, vec![Port { private: 80, proto: "tcp".into(), public: Some(8080) }, Port { private: 443, proto: "tcp".into(), public: None }]);
        let loose = &l.containers[1];
        assert_eq!((loose.project.as_str(), loose.state.as_str(), loose.health.as_str(), loose.restarts, loose.ports.len()), ("", "exited", "", 3, 0));
        assert_eq!(l.portainer, None);
        assert!(l.stacks.is_empty());
        let l = parse_listing("access direct\ncompose plugin\nstack /home/me/stacks/web/compose.yml\n").unwrap();
        assert_eq!(l.stacks, vec!["/home/me/stacks/web/compose.yml"]);
    }

    #[test]
    fn portainer_stacks() {
        let out = concat!(
            "access direct\ncompose plugin\n",
            r#"{"id":"cc33","name":"/portainer","image":"portainer/portainer-ce:2.21","state":{"Status":"running"},"restarts":0,"labels":{},"ports":{"9443/tcp":[{"HostIp":"0.0.0.0","HostPort":"9443"}],"8000/tcp":[{"HostIp":"0.0.0.0","HostPort":"8000"}]},"mounts":[{"Source":"/var/run/docker.sock","Destination":"/var/run/docker.sock"},{"Source":"/var/lib/docker/volumes/portainer_data/_data","Destination":"/data"}]}"#,
            "\n",
            r#"{"id":"dd44","name":"/keycloak","image":"keycloak","state":{"Status":"running"},"restarts":0,"labels":{"com.docker.compose.project":"keycloak","com.docker.compose.project.working_dir":"/data/compose/346/v2","com.docker.compose.project.config_files":"/data/compose/346/v2/docker-compose.yml"},"ports":{},"mounts":[]}"#,
            "\n",
            r#"{"id":"ee55","name":"/registry","image":"registry:2","state":{"Status":"running"},"restarts":0,"labels":{"com.docker.compose.project":"registry","com.docker.compose.project.working_dir":"/var/lib/docker/volumes/portainer_data/_data/compose/278/v1"},"ports":{},"mounts":[]}"#,
            "\n",
            r#"{"id":"ff66","name":"/recoder","image":"recoder","state":{"Status":"running"},"restarts":0,"labels":{"com.docker.compose.project":"recoder","com.docker.compose.project.working_dir":"/home/me/recoder"},"ports":{},"mounts":[]}"#,
            "\n"
        );
        let l = parse_listing(out).unwrap();
        let p = l.portainer.as_ref().unwrap();
        assert_eq!((p.container.as_str(), p.data.as_str(), p.url_port, p.https), ("portainer", "/var/lib/docker/volumes/portainer_data/_data", Some(9443), true));
        let by = |n: &str| l.containers.iter().find(|c| c.name == n).unwrap().portainer;
        assert!(by("keycloak") && by("registry") && !by("recoder") && !by("portainer"));
    }

    #[test]
    fn listing_without_docker() {
        let l = parse_listing("access none\n").unwrap();
        assert_eq!((l.access.as_str(), l.containers.len()), ("none", 0));
        assert!(parse_listing("sh: 1: syntax error\n").is_err());
    }

    #[test]
    fn compose_lines() {
        let files = vec!["/srv/my app/compose.yml".to_string()];
        assert_eq!(
            compose_line("plugin", false, "web", "/srv/my app", &files, "up").unwrap(),
            "cd '/srv/my app' && docker compose -p web -f '/srv/my app/compose.yml' up -d 2>&1"
        );
        assert_eq!(compose_line("standalone", true, "web", "/srv/web", &[], "pull").unwrap(), "cd '/srv/web' && sudo -n docker-compose -p web pull 2>&1");
        assert!(compose_line("plugin", false, "web; rm -rf /", "/srv", &[], "up").is_err());
        // down keeps the volumes (no -v); anything else is refused.
        assert_eq!(compose_line("plugin", false, "web", "/srv", &[], "down").unwrap(), "cd '/srv' && docker compose -p web down 2>&1");
        assert!(compose_line("plugin", false, "web", "/srv", &[], "rm").is_err());
    }

    #[test]
    fn compose_from_a_container() {
        let c = serde_json::json!({
            "Name": "/My DB",
            "Config": {"Image": "postgres:16", "Env": ["PATH=/usr/bin", "POSTGRES_PASSWORD=hunter2", "POSTGRES_DB=app", "KEYCLOAK_ADMIN=admin"], "Cmd": ["postgres"], "Entrypoint": ["docker-entrypoint.sh"]},
            "HostConfig": {"RestartPolicy": {"Name": "unless-stopped"}, "NetworkMode": "bridge",
                "PortBindings": {"5432/tcp": [{"HostIp": "", "HostPort": "5433"}, {"HostIp": "::", "HostPort": "5433"}], "53/udp": [{"HostIp": "127.0.0.1", "HostPort": "53"}]}},
            "Mounts": [
                {"Type": "volume", "Name": "pgdata", "Destination": "/var/lib/postgresql/data", "RW": true},
                {"Type": "bind", "Source": "/srv/init", "Destination": "/docker-entrypoint-initdb.d", "RW": false},
                {"Type": "volume", "Name": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "Destination": "/tmp/x", "RW": true}
            ]
        });
        let img = serde_json::json!({"Env": ["PATH=/usr/bin"], "Cmd": ["postgres"], "Entrypoint": ["docker-entrypoint.sh"]});
        let y = compose_from(&c, &img);
        assert_eq!(
            y,
            concat!(
                "services:\n  my-db:\n    image: \"postgres:16\"\n    container_name: \"My DB\"\n    restart: unless-stopped\n",
                "    ports:\n      - \"127.0.0.1:53:53/udp\"\n      - \"5433:5432\"\n",
                "    environment:\n      POSTGRES_PASSWORD: \"{{POSTGRES_PASSWORD}}\"\n      POSTGRES_DB: \"app\"\n      KEYCLOAK_ADMIN: \"admin\"\n",
                "    volumes:\n      - \"pgdata:/var/lib/postgresql/data\"\n      - \"/srv/init:/docker-entrypoint-initdb.d:ro\"\n",
                "volumes:\n  pgdata:\n    external: true\n"
            )
        );
    }

    fn nfs(path: &str, local: &str) -> Shared {
        Shared { host: "192.0.2.116".into(), path: path.into(), local: local.into() }
    }

    #[test]
    fn mounts_from_the_server() {
        let m = parse_mounts(concat!(
            "ip 192.0.2.92\nip 172.17.0.1\nip 192.0.2.92\nip 127.0.0.1\n",
            "shared 192.0.2.116:/volume2/Docker /mnt/dockers\n",
            "shared //nas/Media/sub /mnt/my\\040media\n",
            "root /volume1\nalias /share/Docker /volume2/Docker\n"
        ));
        assert_eq!(m.ips, vec!["192.0.2.92", "172.17.0.1"]);
        assert_eq!(m.shared, vec![nfs("/volume2/Docker", "/mnt/dockers"), Shared { host: "nas".into(), path: "/share/Media/sub".into(), local: "/mnt/my media".into() }]);
        assert_eq!((m.roots.len(), m.aliases.len()), (1, 1));
    }

    #[test]
    fn paths_between_servers() {
        let docker92 = Mounts { ips: vec!["192.0.2.92".into()], shared: vec![nfs("/volume2/Docker", "/mnt/dockers"), nfs("/volume1/Photo-Archive", "/mnt/Photo-Archive")], ..Default::default() };
        let nas = Mounts {
            ips: vec!["192.0.2.116".into()],
            roots: vec!["/volume1".into(), "/volume2".into()],
            aliases: vec![("/share/Docker".into(), "/volume2/Docker".into())],
            ..Default::default()
        };
        let other_name = Mounts { ips: vec!["192.0.2.54".into()], shared: vec![nfs("/volume2/Docker", "/mnt/docker-nas")], ..Default::default() };
        let deeper = Mounts { shared: vec![nfs("/volume2/Docker", "/mnt/dockers"), nfs("/volume2/Docker/streamonitor", "/mnt/streamonitor")], ..Default::default() };
        let shallower = Mounts { shared: vec![nfs("/volume2", "/mnt/vol2")], ..Default::default() };
        let one = |p: &str, from: &Mounts, to: &Mounts| translate(&[p.to_string()], from, to).remove(0);

        // A Docker machine to the NAS, where the same folder is local, and back (also by its /share name).
        let t = one("/mnt/dockers/streamonitor/config.json", &docker92, &nas);
        assert_eq!((t.kind.as_str(), t.to.as_deref()), ("nas", Some("/volume2/Docker/streamonitor/config.json")));
        assert_eq!(t.nas.as_deref(), Some("192.0.2.116:/volume2/Docker/streamonitor/config.json"));
        assert_eq!(one("/volume2/Docker/registry/data", &nas, &docker92).to.as_deref(), Some("/mnt/dockers/registry/data"));
        assert_eq!(one("/share/Docker/registry/data", &nas, &docker92).to.as_deref(), Some("/mnt/dockers/registry/data"));
        // Another mount name, a deeper mount (the most precise wins) and a shallower one.
        assert_eq!(one("/mnt/dockers/cam-backup", &docker92, &other_name).to.as_deref(), Some("/mnt/docker-nas/cam-backup"));
        assert_eq!(one("/mnt/dockers/streamonitor/data", &docker92, &deeper).to.as_deref(), Some("/mnt/streamonitor/data"));
        assert_eq!(one("/mnt/dockers/streamonitor", &docker92, &deeper).to.as_deref(), Some("/mnt/streamonitor"));
        assert_eq!(one("/mnt/dockers/cam-backup", &docker92, &shallower).to.as_deref(), Some("/mnt/vol2/Docker/cam-backup"));
        // On the NAS, but not mounted on the other machine.
        let t = one("/mnt/Photo-Archive/2024", &docker92, &other_name);
        assert_eq!((t.kind.as_str(), t.to), ("nas", None));
        // Only on this server, and files of the server itself.
        assert_eq!(one("/opt/recoder/state", &docker92, &nas).kind, "local");
        assert_eq!(one("/mnt/dockersX/a", &docker92, &nas).kind, "local");
        let t = one("/etc/localtime", &docker92, &nas);
        assert_eq!((t.kind.as_str(), t.to.as_deref()), ("host", Some("/etc/localtime")));
    }

    #[test]
    fn secret_names() {
        for s in ["MYSQL_ROOT_PASSWORD", "API_KEY", "GITEA_RUNNER_REGISTRATION_TOKEN", "db_pass", "JWT_SECRET"] {
            assert!(secret_name(s), "{s}");
        }
        for s in ["KEYCLOAK_ADMIN", "PASSAGE", "TZ", "MONKEY", "PUID"] {
            assert!(!secret_name(s), "{s}");
        }
    }

    #[test]
    fn the_format_fits_in_single_quotes() {
        assert!(!FORMAT.contains('\''));
    }
}

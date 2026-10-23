//! Sync through git: folders, sessions, macros, tunnels, Proxmox servers and credential sources kept in a private git
//! repository (Gitea or Forgejo, GitHub, GitLab, or any other through the git program), so every PC has
//! the same set and the repository keeps the history. The files are encrypted by default. Passwords
//! never go there: they stay in the vault or in Cablebag's own encrypted store on each PC.
//!
//! A sync reads the repository, merges it with this PC's data against the state of the last sync
//! (so a deletion on one PC carries over and changes on two PCs combine), applies the result here and
//! writes it back as one commit.

use crate::store::{Folder, Session};
use crate::AppState;
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use tauri::{AppHandle, Emitter, Manager};

/// What is synced: collection name, file in the repository.
const FILES: [(&str, &str); 10] = [
    ("folders", "folders.json"),
    ("sessions", "sessions.json"),
    ("macros", "macros.json"),
    ("tunnels", "tunnels.json"),
    ("proxmox", "proxmox.json"),
    ("memory", "memory.json"),
    ("workspaces", "workspaces.json"),
    ("wireguard", "wireguard.json"),
    ("sources", "sources.json"),
    ("browserprofiles", "browser-profiles.json"),
];

/// Settings that hold the synced lists.
fn setting_key(collection: &str) -> Option<&'static str> {
    match collection {
        "macros" => Some("macros"),
        "tunnels" => Some("tunnels"),
        "proxmox" => Some("proxmox.sources"),
        "memory" => Some("ai.memory"),
        "workspaces" => Some("workspaces"),
        "wireguard" => Some("wireguard.profiles"),
        "sources" => Some(crate::sources::SETTING),
        "browserprofiles" => Some("browser.profiles"),
        _ => None,
    }
}

type Data = BTreeMap<String, Vec<Value>>;

#[derive(Serialize, Clone, Default)]
pub struct SyncResult {
    /// Unix time of the sync.
    pub at: i64,
    /// Items that came in from the repository and changed something here.
    pub received: usize,
    /// Items this PC sent to the repository.
    pub sent: usize,
    /// Items changed on both sides; this PC's version was kept.
    pub conflicts: usize,
    /// Which items those were.
    pub conflict_list: Vec<Conflict>,
    pub sessions: usize,
    pub folders: usize,
    pub error: String,
}

/// An item changed on this PC and elsewhere since the last sync.
#[derive(Serialize, Clone, Debug)]
pub struct Conflict {
    /// What kind of item: session, folder, macro, tunnel, Proxmox server, memory note, workspace.
    pub what: String,
    pub name: String,
    /// Whose version is kept now.
    pub kept: String,
}

fn item_kind(collection: &str) -> &'static str {
    match collection {
        "folders" => "folder",
        "sessions" => "session",
        "macros" => "macro",
        "tunnels" => "tunnel",
        "proxmox" => "Proxmox server",
        "memory" => "memory note",
        "workspaces" => "workspace",
        "wireguard" => "WireGuard network",
        "sources" => "credential source",
        "browserprofiles" => "browser profile",
        _ => "item",
    }
}

fn setting(app: &AppHandle, key: &str) -> String {
    let state = app.state::<AppState>();
    let v = state.store.lock().setting(key).ok().flatten();
    match v {
        Some(Value::String(s)) => s,
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn id_of(v: &Value) -> String {
    v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn sorted(mut items: Vec<Value>) -> Vec<Value> {
    items.retain(|v| !id_of(v).is_empty());
    items.sort_by_key(id_of);
    items
}

/// This PC's synced data. Sessions leave out when they were last used: that changes on every connect.
fn local(app: &AppHandle) -> Result<Data> {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    local_from(&store)
}

fn local_from(store: &crate::store::Store) -> Result<Data> {
    let mut d = Data::new();
    d.insert("folders".into(), sorted(store.folders()?.into_iter().map(|f| serde_json::to_value(f).unwrap_or(Value::Null)).collect()));
    d.insert(
        "sessions".into(),
        sorted(
            store
                .sessions()?
                .into_iter()
                .map(|s| {
                    let mut v = serde_json::to_value(s).unwrap_or(Value::Null);
                    if let Some(o) = v.as_object_mut() {
                        o.remove("last_used");
                    }
                    v
                })
                .collect(),
        ),
    );
    for (name, _) in FILES.iter().skip(2) {
        let key = setting_key(name).unwrap();
        let list = store.setting(key)?.and_then(|v| v.as_array().cloned()).unwrap_or_default();
        d.insert(name.to_string(), sorted(list));
    }
    Ok(d)
}

/// Three-way merge per item id. Without a base (the first sync) both sides are combined, this PC first.
fn merge(base: Option<&Data>, mine: &Data, theirs: &Data) -> (Data, Vec<Conflict>) {
    let mut out = Data::new();
    let mut conflicts = Vec::new();
    for (name, _) in FILES {
        let index = |d: Option<&Data>| -> BTreeMap<String, Value> {
            d.and_then(|d| d.get(name)).map(|l| l.iter().map(|v| (id_of(v), v.clone())).collect()).unwrap_or_default()
        };
        let (b, l, r) = (index(base), index(Some(mine)), index(Some(theirs)));
        let ids: BTreeSet<&String> = l.keys().chain(r.keys()).collect();
        let mut list = Vec::new();
        for id in ids {
            let (lv, rv) = (l.get(id), r.get(id));
            let pick = if base.is_none() {
                lv.or(rv)
            } else {
                let bv = b.get(id);
                if lv == rv {
                    lv
                } else if lv == bv {
                    rv
                } else if rv == bv {
                    lv
                } else {
                    // Changed on both sides: keep this PC's version, or the other one when this PC deleted it.
                    let v = lv.or(rv).cloned().unwrap_or(Value::Null);
                    let label = ["name", "label", "text"].iter().find_map(|k| v.get(*k).and_then(|x| x.as_str())).unwrap_or(id.as_str());
                    conflicts.push(Conflict {
                        what: item_kind(name).to_string(),
                        name: label.chars().take(80).collect(),
                        kept: if lv.is_some() { "this PC" } else { "the other PC (this PC had deleted it)" }.to_string(),
                    });
                    lv.or(rv)
                }
            };
            if let Some(v) = pick {
                list.push(v.clone());
            }
        }
        out.insert(name.to_string(), list);
    }
    (out, conflicts)
}

/// How many items differ between two versions of the data.
fn differences(a: &Data, b: &Data) -> usize {
    let mut n = 0;
    for (name, _) in FILES {
        let ia: BTreeMap<String, &Value> = a.get(name).map(|l| l.iter().map(|v| (id_of(v), v)).collect()).unwrap_or_default();
        let ib: BTreeMap<String, &Value> = b.get(name).map(|l| l.iter().map(|v| (id_of(v), v)).collect()).unwrap_or_default();
        let ids: BTreeSet<&String> = ia.keys().chain(ib.keys()).collect();
        n += ids.into_iter().filter(|id| ia.get(*id) != ib.get(*id)).count();
    }
    n
}

/// Writes the merged data into this PC's database.
fn apply(app: &AppHandle, merged: &Data, mine: &Data) -> Result<()> {
    let state = app.state::<AppState>();
    let store = state.store.lock();

    // Folders: parents before children.
    let folders: Vec<Folder> = merged["folders"].iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect();
    let mut done: BTreeSet<String> = BTreeSet::new();
    let mut left = folders.clone();
    while !left.is_empty() {
        let before = left.len();
        left.retain(|f| {
            let ready = f.parent_id.as_ref().map_or(true, |p| done.contains(p) || !folders.iter().any(|x| &x.id == p));
            if ready {
                let _ = store.save_folder(f);
                done.insert(f.id.clone());
            }
            !ready
        });
        if left.len() == before {
            for f in &left {
                store.save_folder(f)?;
            }
            break;
        }
    }

    // Sessions: keep this PC's "last used".
    let keep: BTreeSet<String> = merged["sessions"].iter().map(id_of).collect();
    for v in &merged["sessions"] {
        let mut v = v.clone();
        let last = store.session(&id_of(&v))?.map(|s| s.last_used).unwrap_or(0);
        v["last_used"] = json!(last);
        let s: Session = serde_json::from_value(v).context("a session in the repository could not be read")?;
        store.save_session(&s)?;
    }
    for v in &mine["sessions"] {
        let id = id_of(v);
        if !keep.contains(&id) {
            store.delete_session(&id)?;
        }
    }
    let keep_folders: BTreeSet<String> = merged["folders"].iter().map(id_of).collect();
    for v in &mine["folders"] {
        let id = id_of(v);
        if !keep_folders.contains(&id) {
            store.delete_folder(&id)?;
        }
    }

    for (name, _) in FILES.iter().skip(2) {
        if merged[*name] != mine[*name] {
            store.set_setting(setting_key(name).unwrap(), &Value::Array(merged[*name].clone()))?;
        }
    }
    Ok(())
}

// ---------- encryption ----------

/// Files written encrypted: every item on its own with AES-256-GCM, only the random item ids readable,
/// so the history still shows which item changed but not what it holds.
// These carry the app's former name on purpose: repositories synced before the rename must stay readable.
const ENCRYPTED: &str = "traxterm-encrypted-1";
const CHECK: &[u8] = b"TraxTerm sync key check";

fn seal(key: &[u8; 32], plain: &[u8]) -> Result<String> {
    use aes_gcm::aead::{Aead, KeyInit};
    use rand::RngExt;
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(key).map_err(|e| anyhow!("{e}"))?;
    let mut nonce = [0u8; 12];
    rand::rng().fill(&mut nonce);
    let ct = cipher
        .encrypt(&aes_gcm::Nonce::try_from(&nonce[..]).map_err(|_| anyhow!("bad nonce"))?, plain)
        .map_err(|_| anyhow!("encryption failed"))?;
    Ok(B64.encode([nonce.to_vec(), ct].concat()))
}

fn unseal(key: &[u8; 32], sealed: &str) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit};
    let raw = B64.decode(sealed)?;
    if raw.len() < 13 {
        bail!("too short");
    }
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(key).map_err(|e| anyhow!("{e}"))?;
    cipher
        .decrypt(&aes_gcm::Nonce::try_from(&raw[..12]).map_err(|_| anyhow!("bad nonce"))?, &raw[12..])
        .map_err(|_| anyhow!("does not decrypt"))
}

/// A file as read from the repository.
#[derive(Default, Debug)]
struct Stored {
    items: Vec<Value>,
    /// id -> (item, its sealed text), to write unchanged items exactly as they were.
    sealed: BTreeMap<String, (Value, String)>,
    encrypted: bool,
    check: Option<String>,
}

fn decode(text: &str, key: Option<&[u8; 32]>, file: &str) -> Result<Stored> {
    let v: Value = serde_json::from_str(text).with_context(|| format!("{file} in the repository is not valid"))?;
    if let Value::Array(list) = v {
        return Ok(Stored { items: sorted(list), ..Default::default() });
    }
    if v["format"].as_str() != Some(ENCRYPTED) {
        bail!("{file} in the repository has a format this Cablebag does not know.");
    }
    let key = key.ok_or_else(|| anyhow!("The repository is encrypted. Choose the same encryption under Settings, Sync."))?;
    let check = v["check"].as_str().unwrap_or("").to_string();
    unseal(key, &check).map_err(|_| anyhow!("The sync key does not open this repository: it was encrypted with another vault account or another passphrase."))?;
    let mut out = Stored { encrypted: true, check: Some(check), ..Default::default() };
    for it in v["items"].as_array().cloned().unwrap_or_default() {
        let data = it["data"].as_str().unwrap_or("").to_string();
        let plain: Value = serde_json::from_slice(&unseal(key, &data).with_context(|| format!("an item in {file} does not decrypt"))?)?;
        out.sealed.insert(id_of(&plain), (plain.clone(), data));
        out.items.push(plain);
    }
    out.items = sorted(out.items);
    Ok(out)
}

fn encode(items: &[Value], key: Option<&[u8; 32]>, before: Option<&Stored>) -> Result<String> {
    let Some(key) = key else {
        return Ok(serde_json::to_string_pretty(items)? + "\n");
    };
    let before = before.filter(|b| b.encrypted);
    let check = match before.and_then(|b| b.check.clone()) {
        Some(c) => c,
        None => seal(key, CHECK)?,
    };
    let mut list = Vec::new();
    for v in items {
        let id = id_of(v);
        let data = match before.and_then(|b| b.sealed.get(&id)).filter(|(p, _)| p == v) {
            Some((_, d)) => d.clone(),
            None => seal(key, &serde_json::to_vec(v)?)?,
        };
        list.push(json!({ "id": id, "data": data }));
    }
    let doc = json!({
        "format": ENCRYPTED,
        "note": "Encrypted by Cablebag (AES-256-GCM, one item at a time); only the item ids can be read here.",
        "check": check,
        "items": list,
    });
    Ok(serde_json::to_string_pretty(&doc)? + "\n")
}

/// The key the files are encrypted with, or None when encryption is off.
/// "vault": derived from the account key of a Vaultwarden source, the same on every PC that unlocks that vault.
/// "passphrase": derived from a passphrase typed on each PC.
async fn sync_key(app: &AppHandle) -> Result<Option<[u8; 32]>> {
    let state = app.state::<AppState>();
    match setting(app, "sync.encrypt").as_str() {
        "off" => Ok(None),
        "passphrase" => {
            let pass = { state.vault.password(&state.store.lock(), "git-sync", "passphrase", 0, "").ok().flatten() };
            let pass = pass.ok_or_else(|| anyhow!("Type the sync passphrase under Settings, Sync."))?;
            let key = tokio::task::spawn_blocking(move || -> Result<[u8; 32]> {
                let mut out = [0u8; 32];
                let params = argon2::Params::new(64 * 1024, 3, 4, Some(32)).map_err(|e| anyhow!("{e}"))?;
                argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
                    .hash_password_into(pass.as_bytes(), b"TraxTerm sync v1", &mut out)
                    .map_err(|e| anyhow!("{e}"))?;
                Ok(out)
            })
            .await??;
            Ok(Some(key))
        }
        _ => {
            let key = state.creds.derived_key(app, "traxterm-sync-v1").await;
            key.map(Some).map_err(|e| anyhow!("The sync is encrypted with a key from the vault, so it needs the vault: {e}"))
        }
    }
}

// ---------- tokens and passphrase ----------

async fn token(app: &AppHandle, url: &str) -> Result<String> {
    let state = app.state::<AppState>();
    let item = setting(app, "sync.vault_item");
    if !item.is_empty() {
        let (_, pw) = state.creds.login(app, &item).await?;
        if !pw.is_empty() {
            return Ok(pw);
        }
    }
    let saved = { state.vault.password(&state.store.lock(), "git-sync", url, 0, "token").ok().flatten() };
    saved.ok_or_else(|| anyhow!("No access token yet: add one under Settings, Sync."))
}

/// Keeps the token in Cablebag's encrypted store.
pub fn save_token(app: &AppHandle, token: &str) -> Result<()> {
    let state = app.state::<AppState>();
    let url = setting(app, "sync.url");
    state.vault.save_password(&state.store.lock(), "git-sync", &url, 0, "token", token.trim())
}

pub fn has_token(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let url = setting(app, "sync.url");
    let store = state.store.lock();
    matches!(store.find_secret("password", "", "git-sync", &url, 0, "token"), Ok(Some(_)))
}

pub fn save_passphrase(app: &AppHandle, pass: &str) -> Result<()> {
    let state = app.state::<AppState>();
    state.vault.save_password(&state.store.lock(), "git-sync", "passphrase", 0, "", pass)
}

pub fn has_passphrase(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    matches!(store.find_secret("password", "", "git-sync", "passphrase", 0, ""), Ok(Some(_)))
}

// ---------- the sync ----------

/// One sync; a second try when someone else wrote to the repository in between.
pub async fn sync(app: &AppHandle) -> Result<SyncResult> {
    let state = app.state::<AppState>();
    let _one = state.sync_lock.lock().await;
    // The old single vault becomes a source before the list of sources is synced.
    crate::sources::configs(app);
    let mut last = Err(anyhow!("not run"));
    for _ in 0..2 {
        last = sync_once(app).await;
        match &last {
            Err(e) if e.to_string().contains(crate::gitremote::RACE) => continue,
            _ => break,
        }
    }
    let result = match &last {
        Ok(r) => r.clone(),
        Err(e) => SyncResult { at: crate::store::now(), error: e.to_string(), ..Default::default() },
    };
    let _ = state.store.lock().set_setting("sync.last", &serde_json::to_value(&result)?);
    last
}

/// Cablebag's own working copy for the git program, one per repository.
fn workdir(url: &str) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    let h = hex::encode(&Sha256::digest(url.as_bytes())[..6]);
    crate::store::data_dir().join("sync").join(h)
}

async fn sync_once(app: &AppHandle) -> Result<SyncResult> {
    let url = setting(app, "sync.url").trim().trim_end_matches('/').to_string();
    let repo = setting(app, "sync.repo").trim().trim_matches('/').to_string();
    if url.is_empty() {
        bail!("Fill in the git server or repository address under Settings, Sync.");
    }
    let kind = match setting(app, "sync.kind").as_str() {
        "" | "auto" => crate::gitremote::detect(&url).await?.to_string(),
        k => k.to_string(),
    };
    let token = if kind == "git" { String::new() } else { token(app, &url).await? };
    let mut remote = crate::gitremote::Remote::new(&kind, &url, &repo, token, workdir(&url))?;
    remote.open().await?;
    let key = sync_key(app).await?;

    let mine = local(app)?;
    let base: Option<Data> = {
        let state = app.state::<AppState>();
        let v = state.store.lock().setting("sync.base").ok().flatten();
        v.and_then(|v| serde_json::from_value(v).ok())
    };
    let pc = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "this PC".into());
    let x = exchange(&remote, key.as_ref(), &mine, base.as_ref(), &pc).await?;
    if x.received > 0 {
        apply(app, &x.merged, &mine)?;
    }
    {
        let state = app.state::<AppState>();
        state.store.lock().set_setting("sync.base", &serde_json::to_value(&x.merged)?)?;
    }
    if x.received > 0 {
        let _ = app.emit("sync-applied", ());
    }
    Ok(SyncResult {
        at: crate::store::now(),
        received: x.received,
        sent: x.sent,
        conflicts: x.conflicts.len(),
        conflict_list: x.conflicts,
        sessions: x.merged["sessions"].len(),
        folders: x.merged["folders"].len(),
        error: String::new(),
    })
}

/// What one exchange with the repository came to.
struct Exchange {
    merged: Data,
    /// Items that changed here.
    received: usize,
    /// Items that changed in the repository.
    sent: usize,
    conflicts: Vec<Conflict>,
}

/// Reads the repository, merges it with this PC's data against the last synced state, and writes the
/// result back as one commit. Applying the result here is up to the caller.
async fn exchange(remote: &crate::gitremote::Remote, key: Option<&[u8; 32]>, mine: &Data, base: Option<&Data>, pc: &str) -> Result<Exchange> {
    let mut theirs = Data::new();
    let mut stored: BTreeMap<String, Stored> = BTreeMap::new();
    let mut versions: BTreeMap<String, String> = BTreeMap::new();
    for (name, file) in FILES {
        match remote.read(file).await? {
            Some((text, version)) => {
                let s = decode(&text, key, file)?;
                theirs.insert(name.into(), s.items.clone());
                stored.insert(name.into(), s);
                versions.insert(name.into(), version);
            }
            None => {
                theirs.insert(name.into(), vec![]);
            }
        }
    }
    // A repository without any of the files (new, emptied or made again) is a first sync: both sides are
    // combined. Merging against the last synced state would read it as "everything was deleted".
    let base = if stored.is_empty() { None } else { base };
    let (merged, conflicts) = merge(base, mine, &theirs);
    let received = differences(mine, &merged);

    // Write what changed, what is missing, and files whose encryption is not the chosen one.
    let sent = differences(&theirs, &merged);
    let mut writes = Vec::new();
    for (name, file) in FILES {
        let before = stored.get(name);
        let format_differs = before.is_some_and(|b| b.encrypted != key.is_some());
        if theirs[name] != merged[name] || before.is_none() || format_differs {
            writes.push((file.to_string(), encode(&merged[name], key, before)?, versions.get(name).cloned()));
        }
    }
    if !writes.is_empty() {
        let what = if sent > 0 {
            format!("{sent} change{}", if sent == 1 { "" } else { "s" })
        } else if key.is_some() && stored.values().any(|s| !s.encrypted) {
            "encrypted".to_string()
        } else if key.is_none() && stored.values().any(|s| s.encrypted) {
            "no longer encrypted".to_string()
        } else {
            "first sync".to_string()
        };
        remote.write(writes, &format!("Cablebag on {pc}: {what}")).await?;
    }
    Ok(Exchange { merged, received, sent, conflicts })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(sessions: &[(&str, &str)]) -> Data {
        let mut d = Data::new();
        for (name, _) in FILES {
            d.insert(name.into(), vec![]);
        }
        d.insert("sessions".into(), sessions.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect());
        d
    }

    fn names(d: &Data) -> Vec<String> {
        d["sessions"].iter().map(|v| v["name"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn first_sync_combines() {
        let (m, c) = merge(None, &data(&[("a", "mine"), ("b", "only mine")]), &data(&[("a", "theirs"), ("c", "only theirs")]));
        assert_eq!(names(&m), vec!["mine", "only mine", "only theirs"]);
        assert!(c.is_empty());
    }

    #[test]
    fn three_way() {
        let base = data(&[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")]);
        let mine = data(&[("a", "a2"), ("c", "c"), ("d", "d")]);
        let theirs = data(&[("a", "a"), ("b", "b"), ("c", "c3"), ("e", "e")]);
        let (m, c) = merge(Some(&base), &mine, &theirs);
        assert_eq!(names(&m), vec!["a2", "c3", "e"]);
        assert!(c.is_empty());
        assert_eq!(differences(&mine, &m), 3);
        assert_eq!(differences(&theirs, &m), 2);
    }

    #[test]
    fn both_changed_keeps_mine() {
        let base = data(&[("a", "a"), ("b", "b")]);
        let (m, c) = merge(Some(&base), &data(&[("a", "mine")]), &data(&[("a", "theirs"), ("b", "b2")]));
        assert_eq!(names(&m), vec!["mine", "b2"]);
        assert_eq!(c.len(), 2);
        // Each one named, with whose version is kept.
        assert_eq!((c[0].what.as_str(), c[0].name.as_str(), c[0].kept.as_str()), ("session", "mine", "this PC"));
        assert_eq!((c[1].name.as_str(), c[1].kept.as_str()), ("b2", "the other PC (this PC had deleted it)"));
    }

    /// Two PCs against one real repository, encrypted:
    /// GIT_TEST_KIND=gitea GIT_TEST_URL=https://host GIT_TEST_REPO=owner/name GIT_TEST_TOKEN=... cargo test --lib two_pcs -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn two_pcs() {
        let env = |k: &str| std::env::var(k).unwrap();
        let dir = std::env::temp_dir().join("cablebag-two-pcs");
        let mut remote = crate::gitremote::Remote::new(&env("GIT_TEST_KIND"), &env("GIT_TEST_URL"), &env("GIT_TEST_REPO"), std::env::var("GIT_TEST_TOKEN").unwrap_or_default(), dir).unwrap();
        remote.open().await.unwrap();
        let key = [9u8; 32];
        let session = |id: &str, name: &str| json!({ "id": id, "name": name, "host": "10.0.0.1" });
        let set = |list: Vec<Value>| -> Data {
            let mut d = Data::new();
            for (n, _) in FILES {
                d.insert(n.into(), vec![]);
            }
            d.insert("sessions".into(), sorted(list));
            d
        };
        let names = |d: &Data| d["sessions"].iter().map(|v| v["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();

        // Each PC: its data and its memory of the last sync. A sync replaces both with the result.
        struct Pc {
            data: Data,
            base: Option<Data>,
        }
        async fn sync(remote: &crate::gitremote::Remote, key: &[u8; 32], pc: &mut Pc, name: &str) -> (usize, usize, usize) {
            let x = exchange(remote, Some(key), &pc.data, pc.base.as_ref(), name).await.unwrap();
            pc.data = x.merged.clone();
            pc.base = Some(x.merged);
            (x.received, x.sent, x.conflicts.len())
        }

        let mut a = Pc { data: set(vec![session("s1", "one"), session("s2", "two")]), base: None };
        let mut b = Pc { data: set(vec![]), base: None };

        sync(&remote, &key, &mut a, "PC-A").await;
        let r = sync(&remote, &key, &mut b, "PC-B").await;
        assert_eq!(names(&b.data), vec!["one", "two"], "B receives A's sessions");
        println!("B got A's sessions: received {}", r.0);

        // A renames one and adds one; B deletes one. Both directions arrive.
        a.data.get_mut("sessions").unwrap()[0]["name"] = json!("one, renamed on A");
        a.data.get_mut("sessions").unwrap().push(session("s3", "three"));
        a.data.insert("sessions".into(), sorted(a.data["sessions"].clone()));
        sync(&remote, &key, &mut a, "PC-A").await;
        b.data.get_mut("sessions").unwrap().retain(|v| v["id"] != "s2");
        sync(&remote, &key, &mut b, "PC-B").await;
        sync(&remote, &key, &mut a, "PC-A").await;
        assert_eq!(names(&a.data), vec!["one, renamed on A", "three"]);
        assert_eq!(a.data, b.data, "both PCs hold the same set");
        println!("rename, add and delete crossed over: {:?}", names(&a.data));

        // Both change the same session: the later sync keeps its own version, and both end up with it.
        a.data.get_mut("sessions").unwrap()[1]["name"] = json!("three, by A");
        sync(&remote, &key, &mut a, "PC-A").await;
        b.data.get_mut("sessions").unwrap()[1]["name"] = json!("three, by B");
        let r = sync(&remote, &key, &mut b, "PC-B").await;
        assert_eq!(r.2, 1, "one conflict seen");
        sync(&remote, &key, &mut a, "PC-A").await;
        assert_eq!(a.data, b.data);
        println!("conflict settled the same on both: {:?}", names(&a.data));

        // Nothing readable in the repository.
        let (text, _) = remote.read("sessions.json").await.unwrap().unwrap();
        assert!(!text.contains("10.0.0.1") && !text.contains("three"));
        println!("repository holds only ciphertext");
    }

    #[test]
    fn encryption() {
        let key = [7u8; 32];
        let other = [8u8; 32];
        let items = vec![json!({ "id": "a", "host": "10.0.0.1", "username": "root" }), json!({ "id": "b", "host": "secret.example" })];

        // Encrypted text reveals the ids only.
        let text = encode(&items, Some(&key), None).unwrap();
        assert!(text.contains("\"a\"") && !text.contains("10.0.0.1") && !text.contains("secret.example") && !text.contains("root"));

        // Reads back with the key; refused with another key or without one.
        let back = decode(&text, Some(&key), "x.json").unwrap();
        assert_eq!(back.items, items);
        assert!(decode(&text, Some(&other), "x.json").unwrap_err().to_string().contains("does not open"));
        assert!(decode(&text, None, "x.json").unwrap_err().to_string().contains("encrypted"));

        // Writing again keeps unchanged items byte for byte, so the history shows only real changes.
        let mut changed = items.clone();
        changed[1]["host"] = json!("other.example");
        let again = decode(&encode(&changed, Some(&key), Some(&back)).unwrap(), Some(&key), "x.json").unwrap();
        assert_eq!(again.sealed["a"].1, back.sealed["a"].1);
        assert_ne!(again.sealed["b"].1, back.sealed["b"].1);
        assert_eq!(again.check, back.check);

        // A readable file is read without a key, and turned encrypted on the next write.
        let plain = encode(&items, None, None).unwrap();
        let p = decode(&plain, Some(&key), "x.json").unwrap();
        assert!(!p.encrypted && p.items == items);
    }
}

//! Credential sources: where logins come from besides Cablebag's own store. Any number of them, each with
//! a name and a kind: Vaultwarden or Bitwarden ("vaultwarden"), CyberArk Privilege Cloud or a self-hosted
//! PVWA ("cyberark") and Delinea Secret Server ("delinea").
//!
//! Sessions, folders, Proxmox servers, WireGuard networks, AI hosts and the sync refer to one login by a
//! reference: "source:item", or a bare item id for the first vault (from before there were several
//! sources), so every reference saved then still works. The list of sources syncs; what a source keeps
//! on this PC (tokens, keys) and every secret it hands out do not.

pub mod cyberark;
pub mod delinea;
pub mod matching;
pub mod vaultwarden;
pub mod webauth;

use crate::prompt::{button, field, PromptRequest};
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// The id of the vault that was set up before there were several sources. Its items keep bare references.
pub const LEGACY: &str = "vaultwarden";
/// The setting that holds the list of sources (synced).
pub const SETTING: &str = "credentials.sources";

/// One source as the person set it up. No secrets: this syncs.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SourceConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// "vaultwarden", "cyberark" or "delinea".
    pub kind: String,
    /// The server: the vault, the PVWA or Privilege Cloud address, the Secret Server address.
    #[serde(default)]
    pub url: String,
    /// The account: the vault's email address, or the user name to sign in with (may be empty for single sign-on).
    #[serde(default)]
    pub account: String,
    /// What else the kind needs, such as the sign-in method; each kind documents its own keys.
    #[serde(default)]
    pub options: Value,
}

impl SourceConfig {
    pub fn opt(&self, key: &str) -> String {
        match self.options.get(key) {
            Some(Value::String(s)) => s.trim().to_string(),
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::Bool(b)) => b.to_string(),
            _ => String::new(),
        }
    }

    pub fn opt_bool(&self, key: &str, default: bool) -> bool {
        self.options.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    /// What decides who is signed in where: when it changes, the sign-in this PC kept no longer fits.
    fn identity(&self) -> (String, String, String, String) {
        (self.kind.clone(), self.url.trim().to_lowercase(), self.account.trim().to_lowercase(), self.opt("auth"))
    }
}

/// One login, without its secret. `id` is local to the source inside a backend; what leaves this module
/// carries the full reference in `id` and the source's id in `source`.
#[derive(Clone, Debug, Serialize)]
pub struct Item {
    pub id: String,
    pub source: String,
    pub name: String,
    pub username: String,
    /// Addresses the login is for (Vaultwarden URIs, the CyberArk account address, the Delinea machine).
    pub uris: Vec<String>,
    /// Where it lives: the vault folder, the CyberArk safe, the Secret Server folder.
    pub folder: String,
    /// The Bitwarden organization, the CyberArk platform, the Secret Server template.
    pub organization: String,
    pub has_password: bool,
}

/// A login this PC has checked out (CyberArk exclusive accounts, Delinea check-out), to check in again.
#[derive(Clone, Debug, Serialize, Default)]
pub struct CheckedOut {
    /// The full reference.
    pub id: String,
    pub name: String,
}

/// The state of one source, for the window. No secrets.
#[derive(Clone, Debug, Serialize, Default)]
pub struct SourceStatus {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub url: String,
    pub account: String,
    pub options: Value,
    /// Enough is filled in to sign in.
    pub configured: bool,
    /// Open (Vaultwarden) or signed in (CyberArk, Delinea).
    pub unlocked: bool,
    /// This PC keeps something that opens it again after a restart.
    pub remembered: bool,
    /// Logins loaded.
    pub items: usize,
    /// Unix time the logins were loaded.
    pub synced: i64,
    /// More logins exist than were loaded: the picker asks the server when searching.
    pub partial: bool,
    pub checked_out: Vec<CheckedOut>,
    /// One line about the sign-in, such as who is signed in and until when.
    pub detail: String,
}

pub fn make_ref(source: &str, item: &str) -> String {
    if source == LEGACY || item.is_empty() { item.to_string() } else { format!("{source}:{item}") }
}

/// (source id or None for a bare reference, item id). Bitwarden ids, CyberArk account ids ("12_3") and
/// Secret Server ids (numbers) hold no colon; source ids never do.
pub fn parse_ref(r: &str) -> (Option<&str>, &str) {
    let r = r.trim();
    match r.split_once(':') {
        Some((s, i)) if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') => (Some(s), i),
        _ => (None, r),
    }
}

/// Logins whose name, user, place or address hold the words of `query`.
pub fn filter(items: Vec<Item>, query: &str) -> Vec<Item> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|i| [&i.name, &i.username, &i.folder, &i.organization].iter().any(|v| v.to_lowercase().contains(&q)) || i.uris.iter().any(|u| u.to_lowercase().contains(&q)))
        .collect()
}

fn read_list(app: &AppHandle) -> Option<Vec<SourceConfig>> {
    let v = app.state::<AppState>().store.lock().setting(SETTING).ok().flatten()?;
    serde_json::from_value::<Vec<SourceConfig>>(v).ok()
}

fn write_list(app: &AppHandle, list: &[SourceConfig]) -> Result<()> {
    app.state::<AppState>().store.lock().set_setting(SETTING, &serde_json::to_value(list)?)
}

fn old_setting(app: &AppHandle, key: &str) -> String {
    app.state::<AppState>().store.lock().setting(key).ok().flatten().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// The vault set up before there were sources becomes the first source; its old settings stay, so an
/// older Cablebag still finds them.
fn migrate(server: &str, email: &str) -> Vec<SourceConfig> {
    if server.trim().is_empty() {
        return Vec::new();
    }
    vec![SourceConfig { id: LEGACY.into(), name: "Vaultwarden".into(), kind: "vaultwarden".into(), url: server.trim().into(), account: email.trim().into(), options: json!({}) }]
}

/// Every source, in the order the person put them. The first call after an update moves the old vault in.
pub fn configs(app: &AppHandle) -> Vec<SourceConfig> {
    if let Some(list) = read_list(app) {
        return list;
    }
    let list = migrate(&old_setting(app, "vault.server"), &old_setting(app, "vault.email"));
    let _ = write_list(app, &list);
    list
}

pub fn config(app: &AppHandle, id: &str) -> Option<SourceConfig> {
    configs(app).into_iter().find(|c| c.id == id)
}

/// The source of a reference. A bare one belongs to the first vault, or else to the first Vaultwarden source.
fn config_of_ref(app: &AppHandle, r: &str) -> Result<(SourceConfig, String)> {
    let list = configs(app);
    let (source, item) = parse_ref(r);
    let cfg = match source {
        Some(s) => list.iter().find(|c| c.id == s),
        None => list.iter().find(|c| c.id == LEGACY).or_else(|| list.iter().find(|c| c.kind == "vaultwarden")),
    };
    let cfg = cfg.cloned().ok_or_else(|| anyhow!("The credential source of this login is no longer set up."))?;
    Ok((cfg, item.to_string()))
}

pub enum Backend {
    Vaultwarden(vaultwarden::Vaultwarden),
    CyberArk(cyberark::CyberArk),
    Delinea(delinea::Delinea),
}

impl Backend {
    fn new(kind: &str) -> Result<Self> {
        Ok(match kind {
            "vaultwarden" => Backend::Vaultwarden(Default::default()),
            "cyberark" => Backend::CyberArk(Default::default()),
            "delinea" => Backend::Delinea(Default::default()),
            other => bail!("Cablebag does not know credential sources of the kind \"{other}\"."),
        })
    }
}

/// Calls the same method on whichever kind the backend is.
macro_rules! each {
    ($b:expr, $x:ident => $e:expr) => {
        match &*$b {
            Backend::Vaultwarden($x) => $e,
            Backend::CyberArk($x) => $e,
            Backend::Delinea($x) => $e,
        }
    };
}

/// The open sources of this run, by source id.
#[derive(Default)]
pub struct Sources {
    open: Mutex<HashMap<String, (SourceConfig, Arc<Backend>)>>,
}

impl Sources {
    /// The backend of a source, made when first needed. A source whose server or account changed starts over.
    fn backend(&self, cfg: &SourceConfig) -> Result<Arc<Backend>> {
        let mut open = self.open.lock();
        if let Some((old, b)) = open.get(&cfg.id) {
            if old.identity() == cfg.identity() {
                let b = b.clone();
                open.insert(cfg.id.clone(), (cfg.clone(), b.clone()));
                return Ok(b);
            }
        }
        let b = Arc::new(Backend::new(&cfg.kind)?);
        open.insert(cfg.id.clone(), (cfg.clone(), b.clone()));
        Ok(b)
    }

    fn opened(&self) -> Vec<(SourceConfig, Arc<Backend>)> {
        self.open.lock().values().cloned().collect()
    }

    pub fn status(&self, app: &AppHandle) -> Vec<SourceStatus> {
        configs(app)
            .into_iter()
            .map(|cfg| {
                let mut st = match self.backend(&cfg) {
                    Ok(b) => each!(b, x => x.status(app, &cfg)),
                    Err(e) => SourceStatus { detail: e.to_string(), ..Default::default() },
                };
                for c in st.checked_out.iter_mut() {
                    c.id = make_ref(&cfg.id, &c.id);
                }
                st.id = cfg.id;
                st.name = cfg.name;
                st.kind = cfg.kind;
                st.url = cfg.url;
                st.account = cfg.account;
                st.options = cfg.options;
                st
            })
            .collect()
    }

    /// Adds or changes a source. A new one gets an id; a changed server or account signs the old one out.
    pub async fn save(&self, app: &AppHandle, mut cfg: SourceConfig) -> Result<SourceConfig> {
        Backend::new(&cfg.kind)?;
        cfg.name = cfg.name.trim().to_string();
        cfg.url = normalise_url(&cfg.url);
        cfg.account = cfg.account.trim().to_string();
        if !cfg.options.is_object() {
            cfg.options = json!({});
        }
        let mut list = configs(app);
        if cfg.id.is_empty() {
            cfg.id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        }
        if cfg.name.is_empty() {
            cfg.name = kind_name(&cfg.kind).to_string();
        }
        match list.iter_mut().find(|c| c.id == cfg.id) {
            Some(old) => {
                if old.identity() != cfg.identity() {
                    self.close(app, old).await;
                }
                *old = cfg.clone();
            }
            None => list.push(cfg.clone()),
        }
        write_list(app, &list)?;
        let _ = app.emit("vault-changed", ());
        Ok(cfg)
    }

    pub async fn delete(&self, app: &AppHandle, id: &str) -> Result<()> {
        let mut list = configs(app);
        if let Some(old) = list.iter().find(|c| c.id == id).cloned() {
            self.close(app, &old).await;
        }
        list.retain(|c| c.id != id);
        write_list(app, &list)?;
        let _ = app.emit("vault-changed", ());
        Ok(())
    }

    /// Puts the sources in this order (ids); ones not named keep their place at the end.
    pub fn reorder(&self, app: &AppHandle, ids: &[String]) -> Result<()> {
        let mut list = configs(app);
        list.sort_by_key(|c| ids.iter().position(|i| i == &c.id).unwrap_or(usize::MAX));
        write_list(app, &list)?;
        let _ = app.emit("vault-changed", ());
        Ok(())
    }

    /// Signs out, checks in, and forgets what this PC kept for it.
    async fn close(&self, app: &AppHandle, cfg: &SourceConfig) {
        let b = self.open.lock().remove(&cfg.id).map(|(_, b)| b);
        if let Some(b) = b {
            each!(b, x => x.lock(app, cfg).await);
        }
        self.forget(app, cfg);
    }

    fn forget(&self, app: &AppHandle, cfg: &SourceConfig) {
        match cfg.kind.as_str() {
            "vaultwarden" => vaultwarden::forget(app, cfg),
            "cyberark" => cyberark::forget(app, cfg),
            "delinea" => delinea::forget(app, cfg),
            _ => {}
        }
    }

    pub async fn unlock(&self, app: &AppHandle, id: &str) -> Result<()> {
        let cfg = config(app, id).ok_or_else(|| anyhow!("That credential source is no longer set up."))?;
        let b = self.backend(&cfg)?;
        let r = each!(b, x => x.unlock_interactive(app, &cfg).await);
        let _ = app.emit("vault-changed", ());
        r
    }

    pub async fn lock(&self, app: &AppHandle, id: &str) {
        if let Some(cfg) = config(app, id) {
            if let Ok(b) = self.backend(&cfg) {
                each!(b, x => x.lock(app, &cfg).await);
            }
        }
        let _ = app.emit("vault-changed", ());
    }

    /// Locks and signs out of every source (Cablebag closing, or the person asked).
    pub async fn lock_all(&self, app: &AppHandle) {
        for (cfg, b) in self.opened() {
            each!(b, x => x.lock(app, &cfg).await);
        }
        let _ = app.emit("vault-changed", ());
    }

    pub async fn sync(&self, app: &AppHandle, id: &str) -> Result<()> {
        let cfg = config(app, id).ok_or_else(|| anyhow!("That credential source is no longer set up."))?;
        let b = self.backend(&cfg)?;
        let r = each!(b, x => x.sync(app, &cfg).await);
        let _ = app.emit("vault-changed", ());
        r
    }

    /// Stops keeping what opens a source on this PC; the next unlock asks again.
    pub fn forget_kept(&self, app: &AppHandle, id: &str) {
        if let Some(cfg) = config(app, id) {
            self.forget(app, &cfg);
        }
    }

    /// The loaded logins of every open source, in the order of the sources.
    pub fn items(&self, app: &AppHandle) -> Vec<Item> {
        let mut out = Vec::new();
        for cfg in configs(app) {
            let Some((_, b)) = self.open.lock().get(&cfg.id).cloned() else { continue };
            out.extend(each!(b, x => x.items()).into_iter().map(|i| qualify(&cfg, i)));
        }
        out
    }

    /// Logins that match `query` in every open source; sources that did not load everything ask their server.
    pub async fn search(&self, app: &AppHandle, query: &str) -> Vec<Item> {
        let mut out = Vec::new();
        for cfg in configs(app) {
            let Some((_, b)) = self.open.lock().get(&cfg.id).cloned() else { continue };
            if !each!(b, x => x.is_unlocked()) {
                continue;
            }
            let found = match each!(b, x => x.search(app, &cfg, query).await) {
                Ok(f) => f,
                Err(_) => filter(each!(b, x => x.items()), query),
            };
            out.extend(found.into_iter().map(|i| qualify(&cfg, i)));
        }
        out
    }

    /// Which loaded logins fit a host, best first (names and user names only; nothing is unlocked).
    pub fn candidates(&self, app: &AppHandle, protocol: &str, host: &str, user: &str, name: &str) -> Vec<Item> {
        matching::rank(&self.items(app), protocol, host, user, name)
    }

    /// The login (user, password) of a reference. Unlocks or signs in to its source first when needed.
    pub async fn login(&self, app: &AppHandle, r: &str) -> Result<(String, String)> {
        let (cfg, item) = config_of_ref(app, r)?;
        let b = self.backend(&cfg)?;
        let was_open = each!(b, x => x.is_unlocked());
        let out = each!(b, x => x.login(app, &cfg, &item).await);
        if !was_open || matches!(cfg.kind.as_str(), "cyberark" | "delinea") {
            // Signed in on the way, or a check-out happened: the window shows it.
            let _ = app.emit("vault-changed", ());
        }
        out
    }

    pub async fn check_in(&self, app: &AppHandle, r: &str) -> Result<()> {
        let (cfg, item) = config_of_ref(app, r)?;
        let b = self.backend(&cfg)?;
        let out = each!(b, x => x.check_in(app, &cfg, &item).await);
        let _ = app.emit("vault-changed", ());
        out
    }

    /// Finds a login for a host that has none linked, in every open source. When none is open, the first
    /// source is opened, as the single vault was before. The only match, or one that fits clearly better,
    /// is used; several are offered in a list. Some((reference, item name, user, password)), or None when
    /// there is nothing (or the person prefers to type). Errors only when the person cancels.
    pub async fn pick_login(&self, app: &AppHandle, protocol: &str, host: &str, user: &str, name: &str) -> Result<Option<(String, String, String, String)>> {
        let list: Vec<SourceConfig> = configs(app).into_iter().filter(|c| self.status_of(app, c).configured).collect();
        if list.is_empty() || host.trim().is_empty() {
            return Ok(None);
        }
        let any_open = list.iter().any(|c| self.backend(c).map(|b| each!(b, x => x.is_unlocked())).unwrap_or(false));
        if !any_open {
            let first = &list[0];
            let b = self.backend(first)?;
            let opened = each!(b, x => x.unlock_interactive(app, first).await);
            let _ = app.emit("vault-changed", ());
            if opened.is_err() {
                return Ok(None);
            }
        }
        let mut pool = Vec::new();
        for cfg in &list {
            let b = self.backend(cfg)?;
            if !each!(b, x => x.is_unlocked()) {
                continue;
            }
            if let Ok(found) = each!(b, x => x.find(app, cfg, host).await) {
                pool.extend(found.into_iter().map(|i| qualify(cfg, i)));
            }
        }
        let scored = matching::rank_scored(&pool, protocol, host, user, name);
        let found: Vec<Item> = scored.iter().map(|(_, i)| i.clone()).collect();
        // One login, or one that fits clearly better than the rest (made for this kind of login, or for
        // this user), is used without asking; logins that fit equally well are offered in a list.
        let clear = scored.len() == 1 || (scored.len() > 1 && scored[0].0 > scored[1].0);
        let several_sources = list.len() > 1;
        let chosen = match found.len() {
            0 => return Ok(None),
            _ if clear => found[0].clone(),
            _ => {
                let names: HashMap<String, String> = list.iter().map(|c| (c.id.clone(), c.name.clone())).collect();
                let options: Vec<Value> = found
                    .iter()
                    .take(12)
                    .map(|i| {
                        let source = if several_sources { names.get(&i.source).cloned().unwrap_or_default() } else { String::new() };
                        let place = [source.as_str(), i.organization.as_str(), i.folder.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" / ");
                        let label = [i.name.as_str(), i.username.as_str(), place.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
                        json!({ "id": i.id, "label": label })
                    })
                    .collect();
                let state = app.state::<AppState>();
                let ans = state
                    .prompts
                    .ask(
                        app,
                        PromptRequest {
                            id: String::new(),
                            title: format!("Log in to {host}"),
                            message: format!("There are {} logins that fit {host}. Which one?", found.len()),
                            details: vec![],
                            fields: vec![field("item", "Login", "choice", Value::Array(options))],
                            buttons: vec![button("use", "Use this login", "primary"), button("type", "Type a password", "normal"), button("cancel", "Cancel", "normal")],
                            warning: false,
                        },
                    )
                    .await;
                match ans.button.as_str() {
                    "use" => match found.iter().find(|i| i.id == ans.text("item")) {
                        Some(i) => i.clone(),
                        None => return Ok(None),
                    },
                    "type" => return Ok(None),
                    _ => bail!("Login cancelled."),
                }
            }
        };
        let (u, pw) = self.login(app, &chosen.id).await?;
        Ok(Some((chosen.id, chosen.name, u, pw)))
    }

    fn status_of(&self, app: &AppHandle, cfg: &SourceConfig) -> SourceStatus {
        match self.backend(cfg) {
            Ok(b) => each!(b, x => x.status(app, cfg)),
            Err(_) => SourceStatus::default(),
        }
    }

    /// Every secret any open source handed out or holds, so the assistant can hide them from what it sends.
    pub fn known_passwords(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (_, b) in self.opened() {
            out.extend(each!(b, x => x.known_passwords()));
        }
        out
    }

    /// The vault whose account key encrypts the sync: the one chosen under Settings, Sync, or the first Vaultwarden source.
    pub fn key_source(&self, app: &AppHandle) -> Option<SourceConfig> {
        let list = configs(app);
        let chosen = old_setting(app, "sync.keySource");
        list.iter().find(|c| c.id == chosen && c.kind == "vaultwarden").or_else(|| list.iter().find(|c| c.kind == "vaultwarden")).cloned()
    }

    /// A key for one purpose derived from the key source's account key, opening that vault first.
    pub async fn derived_key(&self, app: &AppHandle, purpose: &str) -> Result<[u8; 32]> {
        let cfg = self.key_source(app).ok_or_else(|| anyhow!("There is no Vaultwarden source to take the key from."))?;
        let b = self.backend(&cfg)?;
        let Backend::Vaultwarden(vw) = &*b else { bail!("The sync key comes from a Vaultwarden source.") };
        if !vw.is_unlocked() {
            vw.unlock_interactive(app, &cfg).await?;
            let _ = app.emit("vault-changed", ());
        }
        vw.derived_key(purpose).ok_or_else(|| anyhow!("{} is locked.", cfg.name))
    }

    /// The CyberArk source with this id and its backend, for connecting through PSM.
    pub fn cyberark(&self, app: &AppHandle, id: &str) -> Result<(SourceConfig, Arc<Backend>)> {
        let cfg = config(app, id).filter(|c| c.kind == "cyberark").ok_or_else(|| anyhow!("The CyberArk source for this session is no longer set up."))?;
        let b = self.backend(&cfg)?;
        Ok((cfg, b))
    }
}

fn qualify(cfg: &SourceConfig, mut i: Item) -> Item {
    i.id = make_ref(&cfg.id, &i.id);
    i.source = cfg.id.clone();
    i
}

fn normalise_url(url: &str) -> String {
    let u = url.trim().trim_end_matches('/').to_string();
    if u.is_empty() || u.starts_with("http://") || u.starts_with("https://") { u } else { format!("https://{u}") }
}

pub fn kind_name(kind: &str) -> &'static str {
    match kind {
        "vaultwarden" => "Vaultwarden",
        "cyberark" => "CyberArk",
        "delinea" => "Delinea Secret Server",
        _ => "Credential source",
    }
}

/// Links a saved session to the login it logged in with, so the next connect does not search.
pub fn link_session(app: &AppHandle, session_id: &str, item_ref: &str) {
    if session_id.is_empty() || item_ref.is_empty() {
        return;
    }
    let state = app.state::<AppState>();
    {
        let store = state.store.lock();
        let Ok(Some(mut s)) = store.session(session_id) else { return };
        if !s.data.is_object() {
            s.data = json!({});
        }
        s.data["vault_item"] = Value::String(item_ref.to_string());
        if store.save_session(&s).is_err() {
            return;
        }
    }
    let _ = app.emit("library-changed", ());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references() {
        assert_eq!(make_ref(LEGACY, "0f1e2d3c-aaaa-bbbb-cccc-1234567890ab"), "0f1e2d3c-aaaa-bbbb-cccc-1234567890ab");
        assert_eq!(make_ref("work", "12_3"), "work:12_3");
        assert_eq!(parse_ref("work:12_3"), (Some("work"), "12_3"));
        assert_eq!(parse_ref("0f1e2d3c-aaaa-bbbb-cccc-1234567890ab"), (None, "0f1e2d3c-aaaa-bbbb-cccc-1234567890ab"));
        assert_eq!(parse_ref("a1b2c3d4e5f6:42"), (Some("a1b2c3d4e5f6"), "42"));
        assert_eq!(parse_ref(""), (None, ""));
    }

    #[test]
    fn the_old_vault_becomes_the_first_source() {
        let list = migrate("https://vault.example.com", "me@example.com");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, LEGACY);
        assert_eq!(list[0].kind, "vaultwarden");
        assert_eq!(list[0].url, "https://vault.example.com");
        assert_eq!(list[0].account, "me@example.com");
        assert!(migrate("", "").is_empty());
    }

    #[test]
    fn a_saved_list_reads_back() {
        let v = json!([{ "id": "vaultwarden", "name": "Home", "kind": "vaultwarden", "url": "https://a", "account": "x@y" }, { "id": "w1", "kind": "cyberark", "url": "https://b", "options": { "auth": "saml" } }]);
        let list: Vec<SourceConfig> = serde_json::from_value(v).unwrap();
        assert_eq!(list[1].opt("auth"), "saml");
        assert_eq!(list[1].account, "");
        assert_ne!(list[0].identity(), list[1].identity());
    }

    #[test]
    fn urls() {
        assert_eq!(normalise_url("vault.example.com/"), "https://vault.example.com");
        assert_eq!(normalise_url("http://10.0.0.5:8080"), "http://10.0.0.5:8080");
        assert_eq!(normalise_url(""), "");
    }
}

//! Proton Pass, through Proton's own command-line tool pass-cli (the "protonpass" kind of credential source).
//! pass-cli keeps its own sign-in on this PC (`pass-cli login`, which signs in through the web browser); Cablebag
//! never sees the Proton password and only reads.
//!
//! What it calls (pass-cli 2.1):
//! - Signed in: `pass-cli info` succeeds. A session that ran out answers other calls with nothing at all (exit 0),
//!   so an empty answer counts as signed out.
//! - Vaults: `pass-cli vault list --output json` gives {"vaults":[{"name","vault_id","share_id"}]}.
//! - Logins: per vault `pass-cli item list --share-id S --filter-type login --output json --show-secrets`, for the
//!   titles, user names and addresses; the passwords in that answer are dropped at once. Sessions signed in with a
//!   personal access token ("agent" sessions) may not use --show-secrets: then only titles come, and logins are
//!   matched by title.
//! - A login: `pass-cli item view --share-id SHARE --item-id ITEM --output json` gives
//!   item.content.content.Login.{username, email, password, urls}. Ids are URL-safe base64 (letters, digits, - _ =).
//!
//! Every call says why in PROTON_PASS_AGENT_REASON, which agent sessions require and log.
//!
//! The source's `options` (synced):
//! - `vaults`: the names of the vaults to use, separated by commas; empty uses them all.
//!
//! Where pass-cli is, when it is not on the PATH, is a setting of this PC only (`protonpass.cli.<source id>`): a
//! program path that came in through the sync would be a program another PC chose to run here.

use super::{Item, SourceConfig, SourceStatus};
use crate::prompt::{button, PromptRequest};
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use serde_json::Value;
use tauri::{AppHandle, Manager};

/// What is loaded while the source is open.
struct Loaded {
    items: Vec<Item>,
    synced: i64,
    /// Only titles came (an agent session): logins are matched by title.
    titles_only: bool,
}

#[derive(Default)]
pub struct ProtonPass {
    open: Mutex<Option<Loaded>>,
    /// One sign-in at a time.
    asking: tokio::sync::Mutex<()>,
    /// Passwords handed out while open, so the assistant can hide them; dropped when it closes.
    handed_out: Mutex<Vec<String>>,
}

/// The setting of this PC that says where pass-cli is for a source (never synced).
pub fn cli_setting(id: &str) -> String {
    format!("protonpass.cli.{id}")
}

/// Checks a pass-cli path given in the settings: empty (found on the PATH), or a program file on this PC.
pub fn check_cli(path: &str) -> Result<String> {
    let p = path.trim().trim_matches('"').to_string();
    if p.is_empty() {
        return Ok(p);
    }
    if p.starts_with("\\\\") || p.starts_with("//") {
        bail!("pass-cli has to be on this PC, not on a network share.");
    }
    let path = std::path::Path::new(&p);
    if !path.is_absolute() || !path.is_file() {
        bail!("{p} is not a program on this PC; fill in the full path of pass-cli, or leave it empty when it is on the PATH.");
    }
    Ok(p)
}

fn cli(app: &AppHandle, cfg: &SourceConfig) -> String {
    let set = app.state::<AppState>().store.lock().setting(&cli_setting(&cfg.id)).ok().flatten().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    check_cli(&set).ok().filter(|p| !p.is_empty()).unwrap_or_else(|| "pass-cli".into())
}

/// Runs pass-cli without a console window. `why` goes into Proton's audit log for agent sessions.
async fn run(app: &AppHandle, cfg: &SourceConfig, args: &[&str], why: &str) -> Result<String> {
    let mut cmd = tokio::process::Command::new(cli(app, cfg));
    cmd.args(args).env("PROTON_PASS_AGENT_REASON", why).stdin(std::process::Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // no console window
    let out = match tokio::time::timeout(std::time::Duration::from_secs(90), cmd.output()).await {
        Err(_) => bail!("Proton Pass's command-line tool did not answer within 90 seconds."),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            bail!("Proton Pass's command-line tool (pass-cli) was not found. Install it from Proton (proton.me/pass/download), or fill in where it is in the source's settings (Settings, Credential sources).")
        }
        Ok(r) => r?,
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("it ended with an error");
        bail!("pass-cli: {line}");
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Only the characters Proton's ids are made of (URL-safe base64), so an id is never taken for anything else.
fn safe_id(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '='))
}

/// An item reference inside the source: "share/item".
fn split_item(item: &str) -> Result<(&str, &str)> {
    match item.split_once('/') {
        Some((s, i)) if safe_id(s) && safe_id(i) => Ok((s, i)),
        _ => bail!("This login does not belong to Proton Pass."),
    }
}

/// The Login part of an item, wherever this version of pass-cli puts it.
fn login_of(v: &Value) -> Option<&Value> {
    let item = if v.get("item").is_some() { &v["item"] } else { v };
    let l = &item["content"]["content"]["Login"];
    (!l.is_null()).then_some(l)
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

/// The logins of one vault's list. Passwords in it are not kept.
fn parse_items(answer: &str, vault: &str) -> Result<Vec<Item>> {
    if answer.trim().is_empty() {
        bail!("pass-cli is signed out; sign in again.");
    }
    let v: Value = serde_json::from_str(answer).map_err(|_| anyhow!("pass-cli gave an answer Cablebag does not understand."))?;
    let list = v.get("items").and_then(Value::as_array).ok_or_else(|| anyhow!("pass-cli gave no list of logins."))?;
    Ok(list
        .iter()
        .filter_map(|i| {
            let (share, id) = (text(i, "share_id"), text(i, "id"));
            if !safe_id(&share) || !safe_id(&id) {
                return None;
            }
            let login = login_of(i);
            let title = {
                let t = text(i, "title");
                if t.is_empty() { i.get("content").map(|c| text(c, "title")).unwrap_or_default() } else { t }
            };
            let username = login.map(|l| Some(text(l, "username")).filter(|u| !u.is_empty()).unwrap_or_else(|| text(l, "email"))).unwrap_or_default();
            let uris = login.and_then(|l| l.get("urls")).and_then(Value::as_array).map(|a| a.iter().filter_map(|u| u.as_str().map(str::to_string)).collect()).unwrap_or_default();
            Some(Item {
                id: format!("{share}/{id}"),
                source: String::new(),
                name: title,
                username,
                uris,
                folder: vault.to_string(),
                organization: String::new(),
                has_password: true,
            })
        })
        .collect())
}

/// Whether pass-cli refused to show secrets in a list (rather than failing).
fn is_not_allowed(message: &str) -> bool {
    let m = message.to_lowercase();
    m.contains("show-secrets") || m.contains("not allowed") || m.contains("agent") || m.contains("permission")
}

/// User name (or email) and password of one item.
fn parse_login(answer: &str) -> Result<(String, String)> {
    let v: Value = serde_json::from_str(answer).map_err(|_| anyhow!("pass-cli gave an answer Cablebag does not understand."))?;
    let l = login_of(&v).ok_or_else(|| anyhow!("This Proton Pass item is not a login."))?;
    let user = Some(text(l, "username")).filter(|u| !u.is_empty()).unwrap_or_else(|| text(l, "email"));
    let password = l.get("password").and_then(Value::as_str).unwrap_or("").to_string();
    if password.is_empty() {
        bail!("This Proton Pass login has no password.");
    }
    Ok((user, password))
}

impl ProtonPass {
    pub fn status(&self, _app: &AppHandle, _cfg: &SourceConfig) -> SourceStatus {
        let o = self.open.lock();
        SourceStatus {
            configured: true,
            unlocked: o.is_some(),
            items: o.as_ref().map(|x| x.items.len()).unwrap_or(0),
            synced: o.as_ref().map(|x| x.synced).unwrap_or(0),
            detail: match o.as_ref() {
                Some(x) if x.titles_only => "Open through pass-cli (signed in with an access token: logins are matched by their title)".into(),
                Some(_) => "Open through pass-cli".into(),
                None => String::new(),
            },
            ..Default::default()
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.open.lock().is_some()
    }

    /// Checks that pass-cli is signed in (helping to sign in when it is not) and loads the logins.
    pub async fn unlock_interactive(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<()> {
        let _one = self.asking.lock().await;
        if self.open.lock().is_some() {
            return Ok(());
        }
        // pass-cli missing or failing is said as it is; only "not signed in" leads to signing in.
        if run(app, cfg, &["info"], "Cablebag: checking the sign-in").await?.trim().is_empty() {
            self.sign_in(app, cfg).await?;
        }
        self.load(app, cfg).await
    }

    /// pass-cli signs in through the web browser, in a window of its own; Cablebag waits for the person.
    async fn sign_in(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<()> {
        let state = app.state::<AppState>();
        let ask = |message: String, buttons| PromptRequest {
            id: String::new(),
            title: format!("Sign in to {}", cfg.name),
            message,
            details: vec![],
            fields: vec![],
            buttons,
            warning: false,
        };
        let first = state
            .prompts
            .ask(
                app,
                ask(
                    "Proton Pass's command-line tool (pass-cli) is not signed in on this PC. Cablebag opens it in a window of its own; it signs you in through your web browser. Come back here when it says you are logged in.".into(),
                    vec![button("open", "Sign in", "primary"), button("cancel", "Cancel", "normal")],
                ),
            )
            .await;
        if first.button != "open" {
            bail!("Cancelled.");
        }
        let mut cmd = std::process::Command::new(cli(app, cfg));
        cmd.arg("login");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0000_0010); // a console window of its own, for its sign-in link
        }
        let mut child = cmd.spawn().map_err(|e| anyhow!("Could not start pass-cli: {e}"))?;
        let mut message = "Sign in in the pass-cli window and your web browser, then click Done.".to_string();
        loop {
            let ans = state.prompts.ask(app, ask(message.clone(), vec![button("done", "Done", "primary"), button("cancel", "Cancel", "normal")])).await;
            if ans.button != "done" {
                let _ = child.kill();
                bail!("Cancelled.");
            }
            match run(app, cfg, &["info"], "Cablebag: checking the sign-in").await {
                Ok(o) if !o.trim().is_empty() => return Ok(()),
                Ok(_) => message = "pass-cli does not say it is signed in yet. Finish the sign-in in the pass-cli window and your web browser, then click Done.".into(),
                Err(e) => message = format!("{e} Finish the sign-in in the pass-cli window, then click Done."),
            }
        }
    }

    /// Loads the logins of the chosen vaults.
    async fn load(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<()> {
        let answer = run(app, cfg, &["vault", "list", "--output", "json"], "Cablebag: listing the vaults").await?;
        if answer.trim().is_empty() {
            bail!("pass-cli is signed out; sign in again.");
        }
        let v: Value = serde_json::from_str(&answer).map_err(|_| anyhow!("pass-cli gave an answer Cablebag does not understand."))?;
        let wanted: Vec<String> = cfg.opt("vaults").split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
        let vaults: Vec<(String, String)> = v
            .get("vaults")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|x| (text(x, "name"), text(x, "share_id"))).filter(|(n, s)| safe_id(s) && (wanted.is_empty() || wanted.contains(&n.to_lowercase()))).collect())
            .unwrap_or_default();
        let mut items = Vec::new();
        let mut titles_only = false;
        for (name, share) in vaults {
            let why = format!("Cablebag: listing the logins of the vault {name}");
            let full = ["item", "list", "--share-id", share.as_str(), "--filter-type", "login", "--output", "json", "--show-secrets"];
            let answer = match run(app, cfg, &full, &why).await {
                Ok(a) => a,
                // Sessions that may not see secrets in lists (as Proton documents for agent sessions): the titles
                // then. Any other failure is said as it is.
                Err(e) if is_not_allowed(&e.to_string()) => {
                    titles_only = true;
                    run(app, cfg, &full[..full.len() - 1], &why).await?
                }
                Err(e) => return Err(e),
            };
            items.extend(parse_items(&answer, &name)?);
        }
        *self.open.lock() = Some(Loaded { items, synced: crate::store::now(), titles_only });
        Ok(())
    }

    /// Closes the source. pass-cli keeps its own sign-in on this PC, except when the person wants every start to
    /// ask (Settings, Credential sources): then it signs out too.
    pub async fn lock(&self, app: &AppHandle, cfg: &SourceConfig) {
        let was_open = self.open.lock().take().is_some();
        self.handed_out.lock().clear();
        if was_open && super::vaultwarden::keep_mode(app) == "password" {
            let _ = run(app, cfg, &["logout"], "Cablebag: signing out").await;
        }
    }

    pub async fn sync(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<()> {
        if self.open.lock().is_none() {
            bail!("{} is closed.", cfg.name);
        }
        self.load(app, cfg).await
    }

    pub fn items(&self) -> Vec<Item> {
        self.open.lock().as_ref().map(|o| o.items.clone()).unwrap_or_default()
    }

    pub async fn search(&self, _app: &AppHandle, _cfg: &SourceConfig, query: &str) -> Result<Vec<Item>> {
        Ok(super::filter(self.items(), query))
    }

    pub async fn find(&self, _app: &AppHandle, _cfg: &SourceConfig, _host: &str) -> Result<Vec<Item>> {
        Ok(self.items())
    }

    /// The user name and password of a login, read from Proton Pass when asked for.
    pub async fn login(&self, app: &AppHandle, cfg: &SourceConfig, item: &str) -> Result<(String, String)> {
        let (share, id) = split_item(item)?;
        if self.open.lock().is_none() {
            self.unlock_interactive(app, cfg).await?;
        }
        let name = self.items().into_iter().find(|i| i.id == item).map(|i| i.name).unwrap_or_else(|| "a login".into());
        let answer = run(app, cfg, &["item", "view", "--share-id", share, "--item-id", id, "--output", "json"], &format!("Cablebag: using the login {name}")).await?;
        if answer.trim().is_empty() {
            bail!("pass-cli is signed out; sign in again.");
        }
        let (user, password) = parse_login(&answer)?;
        let mut known = self.handed_out.lock();
        if !known.contains(&password) {
            known.push(password.clone());
        }
        drop(known);
        Ok((user, password))
    }

    pub async fn check_in(&self, _app: &AppHandle, _cfg: &SourceConfig, _item: &str) -> Result<()> {
        bail!("Proton Pass has no check-out.")
    }

    pub fn known_passwords(&self) -> Vec<String> {
        self.handed_out.lock().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_with_and_without_secrets() {
        let full = r#"{"items":[{"id":"aXRlbQ==","share_id":"c2hhcmU=","title":"Azure portal","item_type":"login",
            "content":{"title":"Azure portal","content":{"Login":{"email":"me@example.com","username":"","password":"s3cret","urls":["https://portal.azure.com"]}}}}]}"#;
        let items = parse_items(full, "Work").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "c2hhcmU=/aXRlbQ==");
        assert_eq!(items[0].username, "me@example.com");
        assert_eq!(items[0].uris, vec!["https://portal.azure.com".to_string()]);
        assert_eq!(items[0].folder, "Work");
        // The password in the list is not kept anywhere.
        assert!(!format!("{:?}", items).contains("s3cret"));
        let titles = r#"{"items":[{"id":"aXRlbQ==","share_id":"c2hhcmU=","title":"Teams","item_type":"login"}]}"#;
        let items = parse_items(titles, "Work").unwrap();
        assert_eq!((items[0].name.as_str(), items[0].username.as_str(), items[0].uris.len()), ("Teams", "", 0));
        // Ids that could escape the pass:// address are skipped.
        let bad = r#"{"items":[{"id":"../x","share_id":"s","title":"x"}]}"#;
        assert!(parse_items(bad, "Work").unwrap().is_empty());
    }

    #[test]
    fn a_login() {
        let view = r#"{"item":{"id":"i","share_id":"s","content":{"title":"x","content":{"Login":{"email":"me@example.com","username":"admin","password":"pw","urls":[]}}}},"attachments":[]}"#;
        assert_eq!(parse_login(view).unwrap(), ("admin".to_string(), "pw".to_string()));
        assert!(parse_login(r#"{"item":{"content":{"content":{"Note":{}}}}}"#).is_err());
        assert!(split_item("s/i").is_ok());
        assert!(split_item("s/i/password").is_err());
        assert!(split_item("s").is_err());
        assert!(split_item("a+b/c").is_err());
    }

    #[test]
    fn the_program_path_is_checked() {
        assert_eq!(check_cli("").unwrap(), "");
        assert!(check_cli(r"\\server\share\pass-cli.exe").is_err());
        assert!(check_cli("pass-cli.exe").is_err());
        assert!(check_cli(r"C:\no\such\pass-cli.exe").is_err());
        let me = std::env::current_exe().unwrap();
        assert!(check_cli(&format!("\"{}\"", me.display())).is_ok());
        assert!(is_not_allowed("pass-cli: Error: --show-secrets is not allowed with agent sessions"));
        assert!(!is_not_allowed("pass-cli: Error: network unreachable"));
    }
}

//! Delinea Secret Server, through its REST API (the "delinea" kind of credential source): Secret Server
//! on-premises (https://server/SecretServer) and Secret Server Cloud (https://tenant.secretservercloud.com).
//!
//! Built from Delinea's documentation only; which documented endpoint each part follows:
//! - The REST API reference (Swagger, version 12.1): token at https://updates.thycotic.net/secretserver/restapiguide/OAuth/
//!   (swagger.json there), every other call at https://updates.thycotic.net/secretserver/restapiguide/TokenAuth/,
//!   linked from https://docs.delinea.com/online-help/secret-server/api-scripting/rest-api/rest-api-reference-download/index.htm
//! - Sign-in: POST /oauth2/token, grant_type=password with username ("user" or "domain\user") and password;
//!   a two-factor code in the "OTP" header (https://docs.delinea.com/online-help/secret-server-11-5-x/api-scripting/authenticating/index.htm);
//!   grant_type=refresh_token to stay signed in, where the server allows refresh tokens; one use each, living
//!   the API session timeout plus 15 minutes (https://docs.delinea.com/online-help/secret-server-11-6-x/authentication/enable-refresh-token/index.htm).
//! - Secret Server on the Delinea Platform: a platform service user's token, client_credentials at
//!   <platform>/identity/api/oauth2/token/xpmplatform with scope xpmheadless, refreshed there with
//!   grant_type=refresh_token and the old token as Bearer (https://docs.delinea.com/online-help/platform-api/start.htm,
//!   https://docs.delinea.com/online-help/delinea-platform/api/ss-apis.htm). The Secret Server calls then go to the
//!   Secret Server Cloud address with that token.
//! - Sign-out: POST /api/v1/oauth-expiration ("Expiring a Token" in
//!   https://docs.delinea.com/online-help/secret-server/api-scripting/rest-api/rest-api-powershell-scripts/index.htm).
//! - Who is signed in: GET /api/v1/users/current.
//! - Search: GET /api/v2/secrets with filter.searchText, filter.includeRestricted, filter.extendedFields, skip and
//!   take; the answer's records, total, hasNext and nextSkip page it. Servers before 11.2 have /api/v1/secrets instead.
//! - Reading: GET /api/v1/secrets/{id}/state (secretState says whether a comment, check-out, approval or DoubleLock
//!   is needed), GET /api/v2/secrets/{id} for the fields, GET /api/v1/secrets/{id}/fields/{slug} for the password
//!   (the reference says password values only come that way), and for a secret that needs a comment, ticket number
//!   or DoubleLock password POST /api/v1/secrets/{id}/restricted and /restricted/fields/{slug} with the
//!   SecretRestrictedArgs (comment, ticketNumber, ticketSystemId, doubleLockPassword) and autoCheckout/autoComment.
//! - Check-out and check-in: POST /api/v1/secrets/{id}/check-out and /check-in.
//!
//! Left out because Cablebag cannot do them honestly: Windows integrated sign-in (the
//! /winauthwebservices/api path needs NTLM or Kerberos, which reqwest does not speak; not offered on Secret
//! Server Cloud either), single sign-on (SAML) sign-in for API clients (no documented way to turn a web
//! sign-in into an API token), and requesting access to an approval-required secret (POST
//! /api/v2/secret-access-requests exists, but needs a start, an end and a reviewer's answer; Cablebag says to
//! request it in Secret Server instead).
//!
//! The source's `options`:
//! - `auth`: "password" (default): a Secret Server user, local or from Active Directory, with its password and,
//!   when needed, a two-factor code. "platform": a Delinea Platform service user (its name in `account`).
//! - `domain`: the Active Directory domain to sign in with, for "password" (sent as "domain\user").
//! - `otp`: true to ask for a two-factor code with the password every time. Without it, Cablebag asks for one
//!   when the server's answer mentions a code.
//! - `platformUrl`: the Delinea Platform address for "platform", such as https://tenant.delinea.app.
//! - `ticketSystem`: the number of the ticket system to check ticket numbers against, when the server has
//!   several; empty uses the server's default.
//!
//! The password, the two-factor code and the DoubleLock password are used once and never kept. A refresh
//! token may stay on this PC, encrypted, when the person lets sources keep their sign-in.

mod api;

use super::{CheckedOut, Item, SourceConfig, SourceStatus};
use crate::prompt::{button, field, PromptRequest};
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use api::{Access, Fail, Need, Token};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::future::Future;
use tauri::{AppHandle, Manager};

/// Secrets loaded at sign-in; past this the picker asks the server when searching.
const LOAD_CAP: usize = 2000;
/// Secrets one server search returns.
const SEARCH_CAP: usize = 200;

/// A signed-in Secret Server.
struct Session {
    base: String,
    token: Token,
    user: String,
    user_id: i64,
    items: Vec<Item>,
    synced: i64,
    partial: bool,
}

/// A secret this PC checked out, to check in again (with the comment given, when one was asked for).
#[derive(Clone)]
struct Out {
    name: String,
    comment: String,
}

#[derive(Default)]
pub struct Delinea {
    session: Mutex<Option<Session>>,
    /// One sign-in question at a time: logins that need Secret Server at the same moment wait for it.
    asking: tokio::sync::Mutex<()>,
    /// Passwords handed out while signed in, so the assistant can hide them; dropped on sign-out.
    handed_out: Mutex<Vec<String>>,
    /// Secrets checked out by this PC, by id; all are checked in on sign-out.
    checked_out: Mutex<BTreeMap<String, Out>>,
}

fn now() -> i64 {
    crate::store::now()
}

fn base_of(cfg: &SourceConfig) -> String {
    cfg.url.trim().trim_end_matches('/').to_string()
}

fn is_platform(cfg: &SourceConfig) -> bool {
    cfg.opt("auth") == "platform"
}

fn platform_url(cfg: &SourceConfig) -> String {
    let u = cfg.opt("platformUrl").trim_end_matches('/').to_string();
    if u.is_empty() || u.starts_with("http://") || u.starts_with("https://") { u } else { format!("https://{u}") }
}

fn host_of(url: &str) -> &str {
    url.trim_start_matches("https://").trim_start_matches("http://")
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent("Cablebag").timeout(std::time::Duration::from_secs(30)).build()?)
}

/// The kept sign-in lives in Cablebag's own encrypted store, one per source.
fn save_kept(app: &AppHandle, cfg: &SourceConfig, token: &Token) {
    // Secret Server's refresh needs only the refresh token; the platform's wants the old access token too.
    let kept = Token { access: if is_platform(cfg) { token.access.clone() } else { String::new() }, ..token.clone() };
    let Ok(text) = serde_json::to_string(&kept) else { return };
    let state = app.state::<AppState>();
    let store = state.store.lock();
    if let Ok((n, c)) = state.vault.encrypt(&store, &text) {
        let _ = store.put_secret("token", "refresh", "delinea", &cfg.id, 0, "", &n, &c);
    }
}

fn load_kept(app: &AppHandle, cfg: &SourceConfig) -> Option<Token> {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    let (n, c) = store.find_secret("token", "refresh", "delinea", &cfg.id, 0, "").ok().flatten()?;
    let text = state.vault.decrypt(&store, &n, &c).ok()?;
    serde_json::from_str::<Token>(&text).ok().filter(|t| t.refresh.is_some())
}

/// Keeps the newest refresh token when the person lets sources keep their sign-in, else nothing.
fn keep(app: &AppHandle, cfg: &SourceConfig, token: &Token) {
    if super::vaultwarden::keep_mode(app) == "password" || token.refresh.is_none() {
        forget(app, cfg);
    } else {
        save_kept(app, cfg, token);
    }
}

/// Removes the sign-in this PC kept for the source.
pub fn forget(app: &AppHandle, cfg: &SourceConfig) {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    if let Ok(list) = store.list_secrets() {
        for s in list.iter().filter(|s| s.kind == "token" && s.protocol == "delinea" && s.host == cfg.id) {
            let _ = store.delete_secret(&s.id);
        }
    }
}

/// A secret id goes into the address of each call: only numbers are secret ids.
fn secret_id(item: &str) -> Result<String> {
    let id = item.trim();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        bail!("This login does not belong to Secret Server.");
    }
    Ok(id.to_string())
}

impl Delinea {
    pub fn status(&self, app: &AppHandle, cfg: &SourceConfig) -> SourceStatus {
        let s = self.session.lock();
        SourceStatus {
            configured: !base_of(cfg).is_empty() && !cfg.account.trim().is_empty() && (!is_platform(cfg) || !platform_url(cfg).is_empty()),
            unlocked: s.is_some(),
            remembered: load_kept(app, cfg).is_some(),
            items: s.as_ref().map(|x| x.items.len()).unwrap_or(0),
            synced: s.as_ref().map(|x| x.synced).unwrap_or(0),
            partial: s.as_ref().map(|x| x.partial).unwrap_or(false),
            checked_out: self.checked_out.lock().iter().map(|(id, o)| CheckedOut { id: id.clone(), name: o.name.clone() }).collect(),
            detail: s.as_ref().map(|x| format!("Signed in as {}", x.user)).unwrap_or_default(),
            ..Default::default()
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.session.lock().is_some()
    }

    /// Signs in (with the sign-in this PC kept, or by asking) and loads the secrets the person can see.
    pub async fn unlock_interactive(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<()> {
        let _one = self.asking.lock().await;
        if self.session.lock().is_some() {
            return Ok(());
        }
        let token = self.sign_in(app, cfg).await?;
        self.open(cfg, token).await
    }

    async fn open(&self, cfg: &SourceConfig, token: Token) -> Result<()> {
        let http = client()?;
        let base = base_of(cfg);
        let (user_id, user) = api::current_user(&http, &base, &token.access).await?;
        let (items, partial) = api::search(&http, &base, &token.access, "", LOAD_CAP).await?;
        *self.session.lock() = Some(Session { base, token, user, user_id, items, synced: now(), partial });
        Ok(())
    }

    /// A token: from the sign-in this PC kept, else by asking. Call with `asking` held.
    async fn sign_in(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<Token> {
        if let Some(t) = self.restore(app, cfg).await? {
            return Ok(t);
        }
        let t = self.ask_sign_in(app, cfg).await?;
        keep(app, cfg, &t);
        Ok(t)
    }

    /// The kept refresh token, after Windows Hello when that is the choice. None when nothing is kept or the
    /// server no longer takes it (signed out there, or it timed out), so the password is needed.
    async fn restore(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<Option<Token>> {
        let mode = super::vaultwarden::keep_mode(app);
        if mode == "password" {
            forget(app, cfg);
            return Ok(None);
        }
        let Some(kept) = load_kept(app, cfg) else { return Ok(None) };
        if mode == "hello" && !super::vaultwarden::hello_confirm(app, &format!("Sign in to {} in Cablebag", cfg.name)).await? {
            return Ok(None);
        }
        match self.refreshed(cfg, &kept).await {
            Ok(t) => {
                keep(app, cfg, &t);
                Ok(Some(t))
            }
            Err(e) if matches!(e.downcast_ref::<Fail>(), Some(Fail::Rejected(_))) => {
                forget(app, cfg);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    async fn refreshed(&self, cfg: &SourceConfig, token: &Token) -> Result<Token> {
        let refresh = token.refresh.as_deref().ok_or_else(|| anyhow!("The Secret Server sign-in has expired. Sign in again."))?;
        let http = client()?;
        if is_platform(cfg) {
            api::platform_refresh(&http, &platform_url(cfg), &token.access, refresh, now()).await
        } else {
            api::refresh_token(&http, &base_of(cfg), refresh, now()).await
        }
    }

    /// Asks for the password (and a two-factor code when needed) and signs in with it.
    async fn ask_sign_in(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<Token> {
        let base = base_of(cfg);
        let account = cfg.account.trim().to_string();
        let platform = is_platform(cfg);
        let platform_at = platform_url(cfg);
        if base.is_empty() || account.is_empty() || (platform && platform_at.is_empty()) {
            bail!("Set up {} first under Settings, Credential sources.", cfg.name);
        }
        let user = api::sign_in_name(&account, &cfg.opt("domain"));
        let mut with_code = !platform && cfg.opt_bool("otp", false);
        let mut message = if platform {
            format!("Password of the service user {account} on {}", host_of(&platform_at))
        } else {
            format!("Password for {user} on {}", host_of(&base))
        };
        let state = app.state::<AppState>();
        for _ in 0..3 {
            let mut fields = vec![field("password", "Password", "password", json!(""))];
            if with_code {
                fields.push(field("code", "Two-factor code", "text", json!("")));
            }
            let ans = state
                .prompts
                .ask(
                    app,
                    PromptRequest {
                        id: String::new(),
                        title: format!("Sign in to {}", cfg.name),
                        message: message.clone(),
                        details: vec![],
                        fields,
                        buttons: vec![button("ok", "Sign in", "primary"), button("cancel", "Cancel", "normal")],
                        warning: false,
                    },
                )
                .await;
            if ans.button != "ok" {
                bail!("Sign-in cancelled.");
            }
            let code = ans.text("code").replace(' ', "");
            let http = client()?;
            let got = if platform {
                api::platform_token(&http, &platform_at, &account, &ans.text("password"), now()).await
            } else {
                api::password_token(&http, &base, &user, &ans.text("password"), with_code.then_some(code.as_str()), now()).await
            };
            match got {
                Ok(t) => return Ok(t),
                Err(e) => match e.downcast_ref::<Fail>() {
                    Some(Fail::Rejected(m)) if !with_code && !platform && api::wants_code(m) => {
                        with_code = true;
                        message = "Secret Server asks for a two-factor code as well. Enter your password and the code.".into();
                    }
                    Some(Fail::Rejected(_)) => message = format!("{e} Try again."),
                    _ => return Err(e),
                },
            }
        }
        bail!("Signing in to {} did not work.", cfg.name)
    }

    /// (address, access token, user id) of the session, signing in or refreshing first when needed.
    async fn access(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<(String, String, i64)> {
        if self.session.lock().is_none() {
            self.unlock_interactive(app, cfg).await?;
        }
        let (base, token, me) = {
            let s = self.session.lock();
            let s = s.as_ref().ok_or_else(|| anyhow!("{} is signed out.", cfg.name))?;
            (s.base.clone(), s.token.clone(), s.user_id)
        };
        if token.stale(now()) {
            let access = self.renew(app, cfg, &token.access).await?;
            return Ok((base, access, me));
        }
        Ok((base, token.access, me))
    }

    /// A working access token after `failed` stopped working: one refresh, else a new sign-in.
    async fn renew(&self, app: &AppHandle, cfg: &SourceConfig, failed: &str) -> Result<String> {
        let _one = self.asking.lock().await;
        let old = self.session.lock().as_ref().map(|s| s.token.clone()).ok_or_else(|| anyhow!("{} is signed out.", cfg.name))?;
        // Another call renewed it while this one waited.
        if old.access != failed && !old.stale(now()) {
            return Ok(old.access);
        }
        let fresh = match self.refreshed(cfg, &old).await {
            Ok(t) => {
                keep(app, cfg, &t);
                t
            }
            Err(_) => self.sign_in(app, cfg).await?,
        };
        let access = fresh.access.clone();
        if let Some(s) = self.session.lock().as_mut() {
            s.token = fresh;
        }
        Ok(access)
    }

    /// Runs one call with the session's token; when the token turns out expired, renews it once and runs again.
    async fn call<T, F, Fut>(&self, app: &AppHandle, cfg: &SourceConfig, f: F) -> Result<T>
    where
        F: Fn(reqwest::Client, String, String, i64) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let (base, token, me) = self.access(app, cfg).await?;
        let http = client()?;
        match f(http.clone(), base.clone(), token.clone(), me).await {
            Err(e) if api::is_expired(&e) => {
                let token = self.renew(app, cfg, &token).await?;
                f(http, base, token, me).await
            }
            other => other,
        }
    }

    /// Checks in what this PC checked out and signs out. When this PC keeps the sign-in, the server session
    /// is left to run out by itself (its own timeout), so the kept refresh token still works after a restart.
    pub async fn lock(&self, app: &AppHandle, cfg: &SourceConfig) {
        let session = self.session.lock().take();
        let outs: Vec<(String, Out)> = std::mem::take(&mut *self.checked_out.lock()).into_iter().collect();
        self.handed_out.lock().clear();
        let Some(s) = session else { return };
        let Ok(http) = client() else { return };
        let token = if s.token.stale(now()) {
            match self.refreshed(cfg, &s.token).await {
                Ok(t) => {
                    keep(app, cfg, &t);
                    t
                }
                Err(_) => s.token,
            }
        } else {
            s.token
        };
        for (id, out) in outs {
            let _ = api::check_in(&http, &s.base, &token.access, &id, &out.comment).await;
        }
        if load_kept(app, cfg).is_none() {
            let _ = api::expire_session(&http, &s.base, &token.access).await;
        }
    }

    /// Loads the list of secrets again with the current sign-in.
    pub async fn sync(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<()> {
        if self.session.lock().is_none() {
            bail!("{} is signed out.", cfg.name);
        }
        let (items, partial) = self.call(app, cfg, |h, b, t, _| async move { api::search(&h, &b, &t, "", LOAD_CAP).await }).await?;
        if let Some(s) = self.session.lock().as_mut() {
            s.items = items;
            s.partial = partial;
            s.synced = now();
        }
        Ok(())
    }

    pub fn items(&self) -> Vec<Item> {
        self.session.lock().as_ref().map(|s| s.items.clone()).unwrap_or_default()
    }

    fn partial(&self) -> bool {
        self.session.lock().as_ref().map(|s| s.partial).unwrap_or(false)
    }

    /// Filters what was loaded; when not everything was, asks the server.
    pub async fn search(&self, app: &AppHandle, cfg: &SourceConfig, query: &str) -> Result<Vec<Item>> {
        if !self.partial() || query.trim().chars().count() < 2 {
            return Ok(super::filter(self.items(), query));
        }
        let q = query.trim().to_string();
        let (found, _) = self.call(app, cfg, |h, b, t, _| {
            let q = q.clone();
            async move { api::search(&h, &b, &t, &q, SEARCH_CAP).await }
        })
        .await?;
        Ok(found)
    }

    /// The candidates for a host: every loaded secret (the ranking picks), plus what the server finds for
    /// the host when not everything was loaded.
    pub async fn find(&self, app: &AppHandle, cfg: &SourceConfig, host: &str) -> Result<Vec<Item>> {
        let mut out = self.items();
        if self.partial() && !host.trim().is_empty() {
            let h = host.trim().to_string();
            let (found, _) = self.call(app, cfg, |c, b, t, _| {
                let h = h.clone();
                async move { api::search(&c, &b, &t, &h, SEARCH_CAP).await }
            })
            .await?;
            for i in found {
                if !out.iter().any(|o| o.id == i.id) {
                    out.push(i);
                }
            }
        }
        Ok(out)
    }

    /// The user name and password of a secret. Asks for what Secret Server wants first (a reason, a ticket
    /// number, a check-out, the DoubleLock password), and signs in first when needed.
    pub async fn login(&self, app: &AppHandle, cfg: &SourceConfig, item: &str) -> Result<(String, String)> {
        let id = secret_id(item)?;
        let state = {
            let id = id.clone();
            self.call(app, cfg, |h, b, t, _| {
                let id = id.clone();
                async move { api::secret_state(&h, &b, &t, &id).await }
            })
            .await?
        };
        let listed = self.items().into_iter().find(|i| i.id == id).map(|i| i.name);
        let name = state
            .as_ref()
            .and_then(|s| s.get("secretName").and_then(Value::as_str).map(str::to_string))
            .filter(|n| !n.is_empty())
            .or(listed)
            .unwrap_or_else(|| "This secret".into());
        let minutes = state.as_ref().and_then(api::check_out_minutes);
        let need = state.as_ref().map(api::need_of).unwrap_or(Need::Nothing);
        let mut access = self.access_for(app, cfg, &id, &name, &need, minutes).await?;
        let fetch = |access: Access| {
            let id = id.clone();
            self.call(app, cfg, move |h, b, t, me| {
                let (id, access) = (id.clone(), access.clone());
                async move { api::fetch_login(&h, &b, &t, &id, &access, me).await }
            })
        };
        let login = match fetch(access.clone()).await {
            // A server without the state call says what it wants when the secret is read.
            Err(e) if state.is_none() => match api::need_of_error(&e) {
                Some(need) => {
                    access = self.access_for(app, cfg, &id, &name, &need, None).await?;
                    fetch(access.clone()).await?
                }
                None => return Err(e),
            },
            other => other?,
        };
        if login.checked_out {
            let name = if login.name.is_empty() { name } else { login.name.clone() };
            self.checked_out.lock().insert(id.clone(), Out { name, comment: access.comment.clone() });
        }
        if !login.password.is_empty() {
            let mut known = self.handed_out.lock();
            if !known.contains(&login.password) {
                known.push(login.password.clone());
            }
        }
        Ok((login.username, login.password))
    }

    /// What to send for a secret that needs something first; asks the person, or says plainly why it cannot open.
    async fn access_for(&self, app: &AppHandle, cfg: &SourceConfig, id: &str, name: &str, need: &Need, minutes: Option<i64>) -> Result<Access> {
        let ticket_system = cfg.opt("ticketSystem").parse::<i64>().ok();
        // Checked out by this PC already: no new check-out, and the reason given then still stands.
        let kept = self.checked_out.lock().get(id).map(|o| o.comment.clone());
        if let Some(reason) = kept
            && (matches!(need, Need::Nothing | Need::CheckOut) || (!reason.is_empty() && matches!(need, Need::Comment | Need::CheckOutAndComment)))
        {
            return Ok(Access { comment: reason, ticket_system, ..Default::default() });
        }
        let (comment, check_out, double_lock) = match need {
            Need::Nothing => return Ok(Access::default()),
            Need::Approval => bail!("{name} needs approval before you can use it. Request access to it in Secret Server, then try again once it is approved."),
            Need::TakenBy(who) => {
                let who = if who.trim().is_empty() { "someone else" } else { who.as_str() };
                bail!("{name} is checked out by {who} right now. Try again once it is checked in.")
            }
            Need::Mfa => bail!("{name} asks for an extra confirmation that Cablebag cannot show. Open it once in Secret Server, then try again."),
            Need::CannotView => bail!("Your account can see that {name} exists, but not open it. Ask its owner in Secret Server for access."),
            Need::Inactive => bail!("{name} has been deleted in Secret Server."),
            Need::Changing => bail!("The password of {name} is being changed right now. Try again in a minute."),
            Need::Comment => (true, false, false),
            Need::CheckOut => (false, true, false),
            Need::CheckOutAndComment => (true, true, false),
            Need::DoubleLock => (false, false, true),
        };
        let mut lines = Vec::new();
        if comment {
            lines.push(format!("Secret Server asks why you need {name}."));
        }
        if check_out {
            let time = minutes.map(|m| format!(" for {m} minutes")).unwrap_or_default();
            lines.push(format!("Using it checks it out to you{time}: nobody else can use it until it is checked in. Cablebag checks it in when you sign out."));
        }
        if double_lock {
            lines.push(format!("{name} is protected with DoubleLock. Enter your DoubleLock password."));
        }
        let mut message = lines.join(" ");
        let state = app.state::<AppState>();
        for _ in 0..3 {
            let mut fields = Vec::new();
            if comment {
                fields.push(field("comment", "Reason", "text", json!("")));
                fields.push(field("ticket", "Ticket number, if your organisation uses them", "text", json!("")));
            }
            if double_lock {
                fields.push(field("doublelock", "DoubleLock password", "password", json!("")));
            }
            let ans = state
                .prompts
                .ask(
                    app,
                    PromptRequest {
                        id: String::new(),
                        title: format!("Open {name}"),
                        message: message.clone(),
                        details: vec![],
                        fields,
                        buttons: vec![button("ok", if check_out { "Check out" } else { "Continue" }, "primary"), button("cancel", "Cancel", "normal")],
                        warning: false,
                    },
                )
                .await;
            if ans.button != "ok" {
                bail!("Cancelled.");
            }
            let access = Access { comment: ans.text("comment").trim().to_string(), ticket: ans.text("ticket").trim().to_string(), ticket_system, double_lock: ans.text("doublelock"), check_out };
            if comment && access.comment.is_empty() {
                message = format!("Secret Server needs a reason before it shows {name}.");
                continue;
            }
            if double_lock && access.double_lock.is_empty() {
                message = format!("Enter the DoubleLock password to open {name}.");
                continue;
            }
            return Ok(access);
        }
        bail!("Cancelled.")
    }

    /// Checks a secret in that this PC checked out.
    pub async fn check_in(&self, app: &AppHandle, cfg: &SourceConfig, item: &str) -> Result<()> {
        let id = secret_id(item)?;
        let comment = self.checked_out.lock().get(&id).map(|o| o.comment.clone()).unwrap_or_default();
        {
            let (id, comment) = (id.clone(), comment.clone());
            self.call(app, cfg, move |h, b, t, _| {
                let (id, comment) = (id.clone(), comment.clone());
                async move { api::check_in(&h, &b, &t, &id, &comment).await }
            })
            .await?;
        }
        self.checked_out.lock().remove(&id);
        Ok(())
    }

    /// Every password handed out since signing in, so the assistant can hide them from what it sends.
    pub fn known_passwords(&self) -> Vec<String> {
        self.handed_out.lock().clone()
    }
}

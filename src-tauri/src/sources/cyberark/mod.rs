//! CyberArk: Privilege Cloud or a self-hosted PVWA, through the PAM REST API (the "cyberark" kind of
//! credential source). Read-only: Cablebag lists the accounts the person may see, retrieves a password
//! when a session needs it, checks exclusive accounts in again, and connects through PSM.
//!
//! Which documented endpoint each part follows (docs.cyberark.com, PAM Self-Hosted 15.2 unless noted;
//! Privilege Cloud documents the same calls under /privilege-cloud-standard/ and
//! /privilege-cloud-shared-services/):
//! - CyberArk, LDAP, RADIUS and Windows sign-in: POST /PasswordVault/API/auth/{type}/Logon/
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/sdk/cyberark%20authentication%20-%20logon_v10.htm
//!   (Privilege Cloud Standard: .../privilege-cloud-standard/latest/en/content/sdk/cyberark%20authentication%20-%20logon_v10.htm).
//!   RADIUS challenge-response is documented for the Vault but not how the REST logon shows it; the
//!   ITATS542I answer handled here is what PVWA sends (the psPAS module relies on it).
//! - SAML: the identity provider's response, posted to the assertion consumer address
//!   /PasswordVault/api/auth/saml/logon, is caught in a Cablebag window and handed to
//!   POST /PasswordVault/API/auth/SAML/Logon/ (form: concurrentSession, apiUse=true, SAMLResponse)
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/sdk/saml_%20authentication_%20logon_newgen.htm
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/pas%20inst/saml-authentication.htm
//!   https://docs.cyberark.com/privilege-cloud-standard/latest/en/content/privilege%20cloud/privcloud-config-saml-auth.htm
//!   (needs "IdP initiated SSO" enabled on the PVWA). OIDC is documented for the PVWA's own pages only
//!   (pas inst/oidc-authentication.htm) and OAuth 2.0 for machine-to-machine clients only
//!   (pas inst/oauth2-authentication.htm), so neither is offered.
//! - Privilege Cloud on Shared Services (*.privilegecloud.cyberark.cloud): the person signs in to
//!   CyberArk Identity (which may hand them on to Entra ID) in a Cablebag window; its session token, the
//!   .ASPXAUTH cookie, is then sent as "Authorization: Bearer" to the Privilege Cloud API, and
//!   POST /Security/Logout ends it.
//!   https://docs.cyberark.com/identity/latest/en/content/developer/authentication/use-auth-token.htm
//!   https://docs.cyberark.com/identity/latest/en/content/developer/authentication/auth-cookies-overview.htm
//!   https://docs.cyberark.com/identity/latest/en/content/developer/authentication/log-out.htm
//!   https://docs.cyberark.com/privilege-cloud-shared-services/latest/en/content/webservices/implementing%20privileged%20account%20security%20web%20services%20.htm
//! - Who is signed in: GET /PasswordVault/WebServices/PIMServices.svc/User/
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/sdk/user%20web%20services%20-%20logged%20on%20user%20details.htm
//! - Log off: POST /PasswordVault/API/Auth/Logoff/
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/sdk/cyberark%20authentication%20-%20logoff_v10.htm
//! - Accounts: GET /PasswordVault/API/Accounts?search=&offset=&limit= (limit at most 1000), and
//!   savedFilter=Locked for what is checked out to the person; GET /PasswordVault/API/Accounts/{id}/
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/sdk/getaccounts.htm
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/webservices/get%20account%20details.htm
//! - Platform rules (reason, exclusive, one-time, dual control): GET /PasswordVault/API/Platforms/Targets/,
//!   else GET /PasswordVault/API/Platforms/
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/sdk/rest-api-get-target-platforms.htm
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/sdk/rest-api-get-platforms.htm
//! - Password: POST /PasswordVault/API/Accounts/{id}/Password/Retrieve/ (reason, TicketingSystemName,
//!   TicketId, ActionType, Machine)
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/webservices/getpasswordvaluev10.htm
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/pasimp/specifying-a-reason-for-accessing-accounts.htm
//! - Check in: POST /PasswordVault/API/Accounts/{id}/CheckIn/
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/webservices/checkin-account.htm
//! - PSM for Windows through the API: POST /PasswordVault/API/Accounts/{id}/PSMConnect/ (an RDP file)
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/webservices/connectthroughpsm.htm
//!   and with an RDP client: PSM server address, start program "psm /u <user> /a <address> /c <component>"
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/pasimp/psso-connectingdirectly.htm
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/pasimp/psso-configurerdpstart.htm
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/pasimp/psso-authenticate.htm
//! - PSM for SSH: the login name "<vault user>@<target user>[#<domain>]@<target>[#<port>]" or the
//!   key-value form, and the MFA caching key: POST /PasswordVault/API/Users/Secret/SSHKeys/Cache/
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/pasimp/psso-pmsp.htm
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/pasimp/psm_for_ssh_upn.htm
//!   https://docs.cyberark.com/privilege-cloud-shared-services/latest/en/content/privilege%20cloud/privcloud-connect-using-ssh.htm
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/pasimp/mfa-caching.htm
//!   https://docs.cyberark.com/pam-self-hosted/latest/en/content/webservices/generate%20mfa%20caching%20ssh%20key.htm
//!
//! The source's `options` (all optional):
//! - `auth`: how to sign in: "cyberark" (CyberArk password), "ldap", "radius", "windows", "saml" (single
//!   sign-on through the PVWA's SAML identity provider) or "identity" (CyberArk Identity, for Privilege
//!   Cloud on Shared Services). Default: "identity" for *.privilegecloud.cyberark.cloud, else "cyberark".
//! - `sso_url`: where the single sign-on window starts, instead of the default (the PVWA's sign-in page
//!   for "saml", the Identity tenant or the Privilege Cloud portal for "identity"); for example the
//!   identity provider's own link to CyberArk.
//! - `identity_url`: the CyberArk Identity tenant, such as https://abc1234.id.cyberark.cloud.
//! - `ticketing_system`: the ticketing system name filled in when CyberArk asks for a ticket.
//! - `ask_reason` (bool): ask for a reason every time a password is retrieved.
//! - `psmp_host`, `psmp_port` (default 22): PSM for SSH. `psmp_user`: the vault user in the PSMP login
//!   name when it differs from the signed-in one. `psmp_key` (bool, default true): get an MFA caching SSH
//!   key after signing in, so PSMP does not ask for a password.
//! - `psm_host`, `psm_port` (default 3389): the PSM server (or its load balancer) for RDP clients.
//!   `psm_component` (default "PSM-RDP"): the connection component. `psm_rdp_mode`: "connect" (the
//!   PSMConnect API hands out the connection), "client" (connect to the PSM server with the start program)
//!   or "auto" (the default: PSMConnect when there is an account, else the RDP client way). `psm_user`:
//!   the vault user for the RDP client way.
//!
//! Nothing is kept on disk: the session token, the accounts, the passwords handed out and the SSH key
//! live in memory until the source is locked or Cablebag closes. The single sign-on window keeps the
//! identity provider's cookies in its own profile (none when the unlock setting is "password every time"),
//! and removes CyberArk Identity's session cookie from it once read.

mod api;
mod psm;

use super::webauth::{self, Capture, Captured};
use super::{CheckedOut, Item, SourceConfig, SourceStatus};
use crate::prompt::{button, field, PromptRequest};
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use api::{Access, Need, Rules};
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use tauri::{AppHandle, Manager, Url};

/// How many accounts are loaded at sign-in; past that the picker asks the server.
const LOAD_CAP: usize = 2000;
/// The largest page the Accounts call gives.
const PAGE: usize = 1000;
/// Accounts asked for per search.
const SEARCH_LIMIT: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Auth {
    CyberArk,
    Ldap,
    Radius,
    Windows,
    Saml,
    Identity,
}

impl Auth {
    fn of(cfg: &SourceConfig) -> Self {
        match cfg.opt("auth").to_lowercase().as_str() {
            "cyberark" => Auth::CyberArk,
            "ldap" => Auth::Ldap,
            "radius" => Auth::Radius,
            "windows" => Auth::Windows,
            "saml" => Auth::Saml,
            "identity" => Auth::Identity,
            _ => match flavour(&root_of(&cfg.url)) {
                Flavour::SharedServices => Auth::Identity,
                _ => Auth::CyberArk,
            },
        }
    }

    /// The type in the logon address.
    fn path(self) -> &'static str {
        match self {
            Auth::Ldap => "LDAP",
            Auth::Radius => "RADIUS",
            Auth::Windows => "Windows",
            _ => "Cyberark",
        }
    }

    fn single_sign_on(self) -> bool {
        matches!(self, Auth::Saml | Auth::Identity)
    }

    fn label(self) -> &'static str {
        match self {
            Auth::CyberArk => "CyberArk password",
            Auth::Ldap => "LDAP",
            Auth::Radius => "RADIUS",
            Auth::Windows => "Windows",
            Auth::Saml => "single sign-on",
            Auth::Identity => "CyberArk Identity",
        }
    }
}

#[derive(Debug, PartialEq)]
enum Flavour {
    SelfHosted,
    /// Privilege Cloud Standard, <subdomain>.privilegecloud.cyberark.com: the PVWA's own sign-in.
    PrivilegeCloud,
    /// Privilege Cloud on Shared Services, <subdomain>.privilegecloud.cyberark.cloud: CyberArk Identity.
    SharedServices,
}

fn host_of(root: &str) -> String {
    Url::parse(root).ok().and_then(|u| u.host_str().map(str::to_lowercase)).unwrap_or_default()
}

fn flavour(root: &str) -> Flavour {
    let h = host_of(root);
    if h.ends_with(".privilegecloud.cyberark.cloud") {
        Flavour::SharedServices
    } else if h.ends_with(".privilegecloud.cyberark.com") {
        Flavour::PrivilegeCloud
    } else {
        Flavour::SelfHosted
    }
}

/// The server's root ("https://host[:port]") from whatever was filled in: a bare host, the PVWA address
/// with /PasswordVault/..., or the Shared Services portal (<subdomain>.cyberark.cloud/privilegecloud/),
/// whose API lives on <subdomain>.privilegecloud.cyberark.cloud.
fn root_of(url: &str) -> String {
    let t = url.trim();
    if t.is_empty() {
        return String::new();
    }
    let with_scheme = if t.contains("://") { t.to_string() } else { format!("https://{t}") };
    let Ok(u) = Url::parse(&with_scheme) else { return String::new() };
    let Some(host) = u.host_str().map(str::to_lowercase) else { return String::new() };
    let path = u.path().to_lowercase();
    let host = match host.strip_suffix(".cyberark.cloud") {
        Some(sub) if !sub.contains('.') && path.starts_with("/privilegecloud") => format!("{sub}.privilegecloud.cyberark.cloud"),
        _ => host,
    };
    match u.port() {
        Some(p) => format!("{}://{host}:{p}", u.scheme()),
        None => format!("{}://{host}", u.scheme()),
    }
}

/// The Identity tenant's root, from the option.
fn identity_root(cfg: &SourceConfig) -> Option<String> {
    let r = root_of(&cfg.opt("identity_url"));
    (!r.is_empty()).then_some(r)
}

/// Where the Shared Services portal is, which sends the person to Identity to sign in.
fn portal_of(root: &str) -> Option<String> {
    let h = host_of(root);
    h.strip_suffix(".privilegecloud.cyberark.cloud").map(|sub| format!("https://{sub}.cyberark.cloud/privilegecloud/"))
}

/// A readable name for an account. CyberArk names them like "Operating System-WinServerLocal-10.0.0.5-admin";
/// "admin @ 10.0.0.5" says the same.
fn readable_name(name: &str, user: &str, address: &str) -> String {
    match (user.trim(), address.trim()) {
        (u, a) if !u.is_empty() && !a.is_empty() => format!("{u} @ {a}"),
        (u, _) if !name.trim().is_empty() || u.is_empty() => name.trim().to_string(),
        (u, _) => u.to_string(),
    }
}

/// What a login needs of an account besides its Item.
#[derive(Clone, Debug, Default)]
struct Meta {
    name: String,
    user: String,
    address: String,
    platform: String,
    has_password: bool,
}

fn to_item(v: &Value) -> Option<(Item, Meta)> {
    let id = api::get_str(v, "id");
    if id.is_empty() {
        return None;
    }
    let user = api::get_str(v, "userName");
    let address = api::get_str(v, "address");
    let platform = api::get_str(v, "platformId");
    let secret = api::get_str(v, "secretType");
    // Only password accounts hand out something to type; SSH keys are for PSM for SSH.
    let has_password = secret.is_empty() || secret.eq_ignore_ascii_case("password");
    let name = readable_name(&api::get_str(v, "name"), &user, &address);
    let item = Item {
        id,
        source: String::new(),
        name: name.clone(),
        username: user.clone(),
        uris: if address.is_empty() { vec![] } else { vec![address.clone()] },
        folder: api::get_str(v, "safeName"),
        organization: platform.clone(),
        has_password,
    };
    Some((item, Meta { name, user, address, platform, has_password }))
}

#[derive(Clone)]
struct Session {
    root: String,
    /// The Authorization header value: the PVWA's session token, or "Bearer <Identity token>".
    auth: String,
    /// The Identity tenant to sign out of (Shared Services).
    identity: Option<String>,
    user: String,
    method: Auth,
}

#[derive(Default)]
struct State {
    session: Option<Session>,
    items: Vec<Item>,
    meta: HashMap<String, Meta>,
    synced: i64,
    partial: bool,
    checked_out: Vec<CheckedOut>,
    /// Every password handed out since signing in, so the assistant can hide them.
    passwords: Vec<String>,
    /// Platform rules, when the person may read them.
    rules: Option<HashMap<String, Rules>>,
    /// The MFA caching key and when it stops working.
    key: Option<(String, i64)>,
}

impl State {
    /// Puts the loaded accounts in place of the ones before.
    fn load(&mut self, raw: &[Value], partial: bool) {
        self.items.clear();
        self.meta.clear();
        for v in raw {
            if let Some((item, meta)) = to_item(v) {
                self.meta.insert(item.id.clone(), meta);
                self.items.push(item);
            }
        }
        self.items.sort_by_key(|i| i.name.to_lowercase());
        self.partial = partial;
        self.synced = crate::store::now();
    }
}

#[derive(Default)]
pub struct CyberArk {
    st: Mutex<State>,
    /// One sign-in at a time: logins that need CyberArk at the same moment wait for it.
    asking: tokio::sync::Mutex<()>,
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent("Cablebag").timeout(std::time::Duration::from_secs(30)).build()?)
}

/// Nothing at all is kept when the person chose to type their password every time.
fn private_window(app: &AppHandle) -> bool {
    super::vaultwarden::keep_mode(app) == "password"
}

impl CyberArk {
    pub fn status(&self, _app: &AppHandle, cfg: &SourceConfig) -> SourceStatus {
        let st = self.st.lock();
        let detail = match &st.session {
            Some(s) if !s.user.is_empty() => format!("Signed in as {} ({}).", s.user, s.method.label()),
            Some(s) => format!("Signed in with {}.", s.method.label()),
            None => String::new(),
        };
        SourceStatus {
            configured: !root_of(&cfg.url).is_empty(),
            unlocked: st.session.is_some(),
            remembered: false,
            items: st.items.len(),
            synced: st.synced,
            partial: st.partial,
            checked_out: st.checked_out.clone(),
            detail,
            ..Default::default()
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.st.lock().session.is_some()
    }

    fn session(&self) -> Option<Session> {
        self.st.lock().session.clone()
    }

    /// Signs in (asking for a password, or opening the single sign-on window) and loads the accounts.
    pub async fn unlock_interactive(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<()> {
        let _one = self.asking.lock().await;
        if self.is_unlocked() {
            return Ok(());
        }
        let root = root_of(&cfg.url);
        if root.is_empty() {
            bail!("Set up {} first under Settings, Credential sources.", cfg.name);
        }
        let http = client()?;
        let session = match Auth::of(cfg) {
            Auth::Saml => self.saml_sign_in(app, cfg, &http, &root).await?,
            Auth::Identity => self.identity_sign_in(app, cfg, &http, &root).await?,
            m => self.password_sign_in(app, cfg, &http, &root, m).await?,
        };
        self.open(&http, session).await
    }

    async fn password_sign_in(&self, app: &AppHandle, cfg: &SourceConfig, http: &reqwest::Client, root: &str, method: Auth) -> Result<Session> {
        let state = app.state::<AppState>();
        let title = format!("Sign in to {}", cfg.name);
        let mut user = cfg.account.trim().to_string();
        let mut message = format!("Sign in to {} with your {} account.", host_of(root), method.label());
        if method == Auth::Radius {
            message.push_str(" If your RADIUS server takes a code with the password, type them as your server expects.");
        }
        for _ in 0..3 {
            let ans = state
                .prompts
                .ask(
                    app,
                    PromptRequest {
                        id: String::new(),
                        title: title.clone(),
                        message: message.clone(),
                        details: vec![],
                        fields: vec![field("user", "User name", "text", Value::String(user.clone())), field("password", "Password", "password", Value::String(String::new()))],
                        buttons: vec![button("ok", "Sign in", "primary"), button("cancel", "Cancel", "normal")],
                        warning: false,
                    },
                )
                .await;
            if ans.button != "ok" {
                bail!("Sign-in cancelled.");
            }
            user = ans.text("user").trim().to_string();
            if user.is_empty() {
                message = "Fill in your user name.".into();
                continue;
            }
            let mut answer = api::logon(http, root, method.path(), &user, &ans.text("password")).await;
            // A RADIUS challenge: the next logon carries the answer instead of the password.
            for _ in 0..3 {
                let Ok(api::Logon::Challenge(question)) = &answer else { break };
                let q = if question.is_empty() { "Your RADIUS server asks for more, such as a code from your authenticator.".to_string() } else { question.clone() };
                let Some((reply, _)) = state.prompts.ask_secret(app, &title, &q, "Answer", false).await else { bail!("Sign-in cancelled.") };
                answer = api::logon(http, root, method.path(), &user, &reply).await;
            }
            match answer {
                Ok(api::Logon::Token(token)) => return Ok(Session { root: root.to_string(), auth: token, identity: None, user, method }),
                Ok(api::Logon::Challenge(_)) => bail!("The RADIUS server kept asking. Try again."),
                Err(e) if e.downcast_ref::<api::ApiError>().map(|a| matches!(a.status, 400 | 401 | 403)).unwrap_or(false) => {
                    let said = e.downcast_ref::<api::ApiError>().map(|a| a.message.trim().trim_end_matches('.').to_string()).unwrap_or_default();
                    message = if said.is_empty() { "CyberArk did not accept that user name and password. Try again.".into() } else { format!("CyberArk did not accept that: {said}. Try again.") };
                }
                Err(e) => return Err(e),
            }
        }
        bail!("You are not signed in to {}.", cfg.name)
    }

    async fn saml_sign_in(&self, app: &AppHandle, cfg: &SourceConfig, http: &reqwest::Client, root: &str) -> Result<Session> {
        let mut start = cfg.opt("sso_url");
        if start.is_empty() {
            start = format!("{root}/PasswordVault/");
        }
        let what = Capture::SamlPost { acs_host: host_of(root), acs_path: "/auth/saml".into() };
        let Captured::Saml { response, .. } = webauth::capture(app, &format!("Sign in to {}", cfg.name), &start, what, private_window(app)).await? else {
            bail!("The sign-in ended without an answer for CyberArk.");
        };
        let token = api::saml_logon(http, root, &response).await?;
        Ok(Session { root: root.to_string(), auth: token, identity: None, user: cfg.account.trim().to_string(), method: Auth::Saml })
    }

    /// Signs in to CyberArk Identity in the window and takes its session token. A token Privilege Cloud
    /// does not accept (one left from before, say) is set aside and the window opens once more.
    async fn identity_sign_in(&self, app: &AppHandle, cfg: &SourceConfig, http: &reqwest::Client, root: &str) -> Result<Session> {
        let identity = identity_root(cfg);
        let start = Some(cfg.opt("sso_url")).filter(|s| !s.is_empty()).or_else(|| identity.clone()).or_else(|| portal_of(root));
        let Some(start) = start else { bail!("Fill in the CyberArk Identity address of {} under Settings, Credential sources.", cfg.name) };
        let hosts = match &identity {
            Some(i) => vec![host_of(i)],
            None => vec![".id.cyberark.cloud".to_string(), ".idaptive.app".to_string()],
        };
        let mut ignore = None;
        for _ in 0..2 {
            let what = Capture::Cookie { name: ".ASPXAUTH".into(), hosts: hosts.clone(), ignore: ignore.clone() };
            let Captured::Cookie { value, host } = webauth::capture(app, &format!("Sign in to {}", cfg.name), &start, what, private_window(app)).await? else {
                bail!("The sign-in ended without an answer for CyberArk.");
            };
            let auth = format!("Bearer {value}");
            match api::accounts(http, root, &auth, "", 0, 1).await {
                Err(e) if api::is_unauthorised(&e) => ignore = Some(value),
                Err(e) => return Err(e),
                Ok(_) => {
                    return Ok(Session { root: root.to_string(), auth, identity: Some(format!("https://{host}")), user: cfg.account.trim().to_string(), method: Auth::Identity });
                }
            }
        }
        bail!("CyberArk Identity signed you in, but Privilege Cloud did not accept the sign-in.")
    }

    /// Learns who signed in, the platform rules and the accounts, and keeps the session.
    async fn open(&self, http: &reqwest::Client, mut s: Session) -> Result<()> {
        if let Ok(u) = api::whoami(http, &s.root, &s.auth).await
            && !u.is_empty()
        {
            s.user = u;
        }
        let rules = api::platform_rules(http, &s.root, &s.auth).await.ok();
        let (raw, partial) = api::all_accounts(http, &s.root, &s.auth, "", PAGE, LOAD_CAP).await?;
        let mut st = self.st.lock();
        st.load(&raw, partial);
        st.rules = rules;
        st.session = Some(s);
        Ok(())
    }

    /// Runs a call with the session, signing in first when needed and once more when the token expired.
    async fn call<T, F, Fut>(&self, app: &AppHandle, cfg: &SourceConfig, f: F) -> Result<T>
    where
        F: Fn(reqwest::Client, Session) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let http = client()?;
        if !self.is_unlocked() {
            self.unlock_interactive(app, cfg).await?;
        }
        let s = self.session().ok_or_else(|| anyhow!("You are not signed in to {}.", cfg.name))?;
        match f(http.clone(), s).await {
            Err(e) if api::is_unauthorised(&e) => {
                self.st.lock().session = None;
                self.st.lock().key = None;
                self.unlock_interactive(app, cfg).await?;
                let s = self.session().ok_or_else(|| anyhow!("You are not signed in to {}.", cfg.name))?;
                f(http, s).await
            }
            r => r,
        }
    }

    /// Checks in what this PC checked out, signs out and forgets everything.
    pub async fn lock(&self, _app: &AppHandle, _cfg: &SourceConfig) {
        let (session, out) = {
            let mut st = self.st.lock();
            let out: Vec<String> = st.checked_out.iter().map(|c| c.id.clone()).collect();
            let s = st.session.take();
            *st = State::default();
            (s, out)
        };
        let Some(s) = session else { return };
        let Ok(http) = client() else { return };
        for id in out {
            let _ = api::check_in(&http, &s.root, &s.auth, &id).await;
        }
        match &s.identity {
            Some(i) => {
                let _ = api::identity_logout(&http, i, &s.auth).await;
            }
            None => {
                let _ = api::logoff(&http, &s.root, &s.auth).await;
            }
        }
    }

    /// Loads the accounts again with the current sign-in.
    pub async fn sync(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<()> {
        if !self.is_unlocked() {
            bail!("Sign in to {} first.", cfg.name);
        }
        let (raw, partial) = self.call(app, cfg, |http, s| async move { api::all_accounts(&http, &s.root, &s.auth, "", PAGE, LOAD_CAP).await }).await?;
        self.st.lock().load(&raw, partial);
        Ok(())
    }

    pub fn items(&self) -> Vec<Item> {
        self.st.lock().items.clone()
    }

    /// Keeps what a search found, so a login picked from it works.
    fn remember(&self, raw: &[Value]) -> Vec<Item> {
        let mut st = self.st.lock();
        let mut out = Vec::new();
        for v in raw {
            if let Some((item, meta)) = to_item(v) {
                st.meta.insert(item.id.clone(), meta);
                out.push(item);
            }
        }
        out
    }

    async fn server_search(&self, app: &AppHandle, cfg: &SourceConfig, words: &str) -> Result<Vec<Item>> {
        let page = self.call(app, cfg, |http, s| async move { api::accounts(&http, &s.root, &s.auth, words, 0, SEARCH_LIMIT).await }).await?;
        Ok(self.remember(&page.accounts))
    }

    /// Everything is loaded: filtering. Otherwise CyberArk searches.
    pub async fn search(&self, app: &AppHandle, cfg: &SourceConfig, query: &str) -> Result<Vec<Item>> {
        let partial = self.st.lock().partial;
        if !partial || query.trim().is_empty() {
            return Ok(super::filter(self.items(), query));
        }
        self.server_search(app, cfg, query).await
    }

    /// The accounts for a host: all of them when everything is loaded (the ranking picks), else what
    /// CyberArk finds for the address together with what is loaded.
    pub async fn find(&self, app: &AppHandle, cfg: &SourceConfig, host: &str) -> Result<Vec<Item>> {
        let partial = self.st.lock().partial;
        if !partial {
            return Ok(self.items());
        }
        let mut out = super::filter(self.items(), host);
        for i in self.server_search(app, cfg, host).await? {
            if !out.iter().any(|o| o.id == i.id) {
                out.push(i);
            }
        }
        Ok(out)
    }

    fn rules_of(&self, platform: &str) -> Option<Rules> {
        self.st.lock().rules.as_ref().and_then(|r| r.get(platform).copied())
    }

    async fn meta(&self, app: &AppHandle, cfg: &SourceConfig, id: &str) -> Result<Meta> {
        if let Some(m) = self.st.lock().meta.get(id).cloned() {
            return Ok(m);
        }
        let v = self.call(app, cfg, |http, s| async move { api::account(&http, &s.root, &s.auth, id).await }).await?;
        let (_, meta) = to_item(&v).ok_or_else(|| anyhow!("The CyberArk account linked to this session no longer exists."))?;
        self.st.lock().meta.insert(id.to_string(), meta.clone());
        Ok(meta)
    }

    /// Asks why the person needs an account (and the ticket, when CyberArk wants one).
    async fn ask_access(&self, app: &AppHandle, cfg: &SourceConfig, what: &str, need: Need, said: Option<String>, prev: Access) -> Result<Access> {
        let state = app.state::<AppState>();
        let ticket = need.ticket || !prev.ticketing_system.is_empty() || !prev.ticket_id.is_empty();
        let mut fields = vec![field("reason", "Reason", "text", Value::String(prev.reason.clone()))];
        if ticket {
            fields.push(field("system", "Ticketing system", "text", Value::String(prev.ticketing_system.clone())));
            fields.push(field("ticket", "Ticket number", "text", Value::String(prev.ticket_id.clone())));
        }
        let message = match said {
            Some(s) => s,
            None if need.ticket => format!("CyberArk asks for a reason and a ticket before it hands out {what}."),
            None => format!("CyberArk asks why you need {what}."),
        };
        let ans = state
            .prompts
            .ask(
                app,
                PromptRequest {
                    id: String::new(),
                    title: format!("Access through {}", cfg.name),
                    message,
                    details: vec![],
                    fields,
                    buttons: vec![button("ok", "Continue", "primary"), button("cancel", "Cancel", "normal")],
                    warning: false,
                },
            )
            .await;
        if ans.button != "ok" {
            bail!("Login cancelled.");
        }
        let mut out = Access { reason: ans.text("reason").trim().to_string(), ..prev };
        if ticket {
            out.ticketing_system = ans.text("system").trim().to_string();
            out.ticket_id = ans.text("ticket").trim().to_string();
        }
        Ok(out)
    }

    /// Runs `f` with what the account needs: a reason asked first when the platform or the settings say
    /// so, and asked again (with CyberArk's own words) when CyberArk refuses for want of one.
    async fn with_access<T, F, Fut>(&self, app: &AppHandle, cfg: &SourceConfig, meta: &Meta, f: F) -> Result<T>
    where
        F: Fn(Access) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut access = Access { ticketing_system: cfg.opt("ticketing_system"), ..Default::default() };
        if cfg.opt_bool("ask_reason", false) || self.rules_of(&meta.platform).and_then(|r| r.reason) == Some(true) {
            access = self.ask_access(app, cfg, &meta.name, Need { reason: true, ticket: false }, None, access).await?;
        }
        let mut tries = 0;
        loop {
            match f(access.clone()).await {
                Ok(v) => return Ok(v),
                Err(e) => match api::needs_access(&e) {
                    Some(need) if tries < 2 => {
                        tries += 1;
                        access = self.ask_access(app, cfg, &meta.name, need, Some(e.to_string()), access).await?;
                    }
                    _ => return Err(e),
                },
            }
        }
    }

    /// Notes an account as checked out to this PC when its platform makes using it exclusive or one-time
    /// (or, when the rules can not be read, when CyberArk lists it as locked to the person).
    async fn note_checkout(&self, app: &AppHandle, cfg: &SourceConfig, id: &str, meta: &Meta) {
        let out = match self.rules_of(&meta.platform) {
            Some(r) => r.checks_out(),
            None => self.call(app, cfg, |http, s| async move { api::locked(&http, &s.root, &s.auth).await }).await.map(|l| l.iter().any(|x| x == id)).unwrap_or(false),
        };
        let mut st = self.st.lock();
        if out && !st.checked_out.iter().any(|c| c.id == id) {
            st.checked_out.push(CheckedOut { id: id.to_string(), name: meta.name.clone() });
        }
    }

    /// The login of one account: its user name and the password CyberArk hands out.
    pub async fn login(&self, app: &AppHandle, cfg: &SourceConfig, item: &str) -> Result<(String, String)> {
        if !self.is_unlocked() {
            self.unlock_interactive(app, cfg).await?;
        }
        let meta = self.meta(app, cfg, item).await?;
        if !meta.has_password {
            bail!("{} holds an SSH key, not a password. Connect to it through PSM for SSH.", meta.name);
        }
        let password = self
            .with_access(app, cfg, &meta, |access| {
                let machine = meta.address.as_str();
                async move {
                    let a = &access;
                    self.call(app, cfg, |http, s| async move { api::retrieve(&http, &s.root, &s.auth, item, a, machine).await }).await
                }
            })
            .await?;
        {
            let mut st = self.st.lock();
            if !password.is_empty() && !st.passwords.contains(&password) {
                st.passwords.push(password.clone());
            }
        }
        self.note_checkout(app, cfg, item, &meta).await;
        Ok((meta.user, password))
    }

    pub async fn check_in(&self, app: &AppHandle, cfg: &SourceConfig, item: &str) -> Result<()> {
        self.call(app, cfg, |http, s| async move { api::check_in(&http, &s.root, &s.auth, item).await }).await?;
        self.st.lock().checked_out.retain(|c| c.id != item);
        Ok(())
    }

    pub fn known_passwords(&self) -> Vec<String> {
        self.st.lock().passwords.clone()
    }

    /// An MFA caching key for PSM for SSH, made once and used until shortly before it expires. When
    /// CyberArk wants such keys protected by a passphrase, a random one is used and the key is opened here.
    async fn mfa_key(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<String> {
        if let Some((k, until)) = self.st.lock().key.clone()
            && (until == 0 || until > crate::store::now() + 60)
        {
            return Ok(k);
        }
        let key = match self.call(app, cfg, |http, s| async move { api::mfa_key(&http, &s.root, &s.auth, None).await }).await {
            Ok(k) => k,
            Err(e) if e.to_string().to_lowercase().contains("passphrase") || e.to_string().to_lowercase().contains("password") => {
                let pass = format!("Cb{}9!", uuid::Uuid::new_v4().simple());
                let p = pass.as_str();
                let mut k = self.call(app, cfg, |http, s| async move { api::mfa_key(&http, &s.root, &s.auth, Some(p)).await }).await?;
                k.private_key = open_key(&k.private_key, &pass)?;
                k
            }
            Err(e) => return Err(e),
        };
        self.st.lock().key = Some((key.private_key.clone(), key.expires));
        Ok(key.private_key)
    }

    /// The account for a user on a host, asking when several fit.
    async fn account_for(&self, app: &AppHandle, cfg: &SourceConfig, user: &str, host: &str) -> Result<String> {
        let mut found: Vec<Item> = self.items().into_iter().filter(|i| i.uris.iter().any(|u| u.eq_ignore_ascii_case(host))).collect();
        if found.is_empty() {
            found = self.server_search(app, cfg, host).await?.into_iter().filter(|i| i.uris.iter().any(|u| u.eq_ignore_ascii_case(host))).collect();
        }
        let short = |u: &str| u.rsplit('\\').next().unwrap_or(u).split('@').next().unwrap_or(u).to_lowercase();
        if !user.trim().is_empty() {
            let exact: Vec<Item> = found.iter().filter(|i| i.username.eq_ignore_ascii_case(user.trim())).cloned().collect();
            found = if exact.is_empty() { found.into_iter().filter(|i| short(&i.username) == short(user)).collect() } else { exact };
        }
        match found.len() {
            0 if user.trim().is_empty() => bail!("There is no CyberArk account for {host} that you may use."),
            0 => bail!("There is no CyberArk account for {user} on {host} that you may use."),
            1 => Ok(found[0].id.clone()),
            _ => {
                let options: Vec<Value> = found.iter().take(12).map(|i| serde_json::json!({ "id": i.id, "label": format!("{} · {}", i.name, i.folder) })).collect();
                let state = app.state::<AppState>();
                let ans = state
                    .prompts
                    .ask(
                        app,
                        PromptRequest {
                            id: String::new(),
                            title: format!("Connect to {host}"),
                            message: format!("There are {} CyberArk accounts for {host}. Which one?", found.len()),
                            details: vec![],
                            fields: vec![field("item", "Account", "choice", Value::Array(options))],
                            buttons: vec![button("use", "Connect", "primary"), button("cancel", "Cancel", "normal")],
                            warning: false,
                        },
                    )
                    .await;
                if ans.button != "use" || ans.text("item").is_empty() {
                    bail!("Connection cancelled.");
                }
                Ok(ans.text("item"))
            }
        }
    }
}

/// Opens a passphrase-protected OpenSSH key, for memory only.
fn open_key(pem: &str, pass: &str) -> Result<String> {
    let k = ssh_key::PrivateKey::from_openssh(pem).map_err(|_| anyhow!("CyberArk sent an SSH key Cablebag could not read."))?;
    let k = if k.is_encrypted() { k.decrypt(pass).map_err(|_| anyhow!("CyberArk sent an SSH key Cablebag could not open."))? } else { k };
    Ok(k.to_openssh(ssh_key::LineEnding::LF).map_err(|_| anyhow!("CyberArk sent an SSH key Cablebag could not read."))?.to_string())
}

/// CyberArk keeps nothing on this PC: the sign-in, the accounts and the passwords live in memory only.
pub fn forget(_app: &AppHandle, _cfg: &SourceConfig) {}

/// How an SSH session reaches its target through PSM for SSH (PSMP).
pub struct PsmpLogin {
    pub host: String,
    pub port: u16,
    /// The combined login name PSMP takes: vault user, target user and target in one.
    pub user: String,
    /// An OpenSSH private key PSMP accepts for the vault user (MFA caching), when there is one.
    pub key: Option<String>,
}

/// How a Remote Desktop session reaches its target through PSM.
#[derive(Default)]
pub struct PsmRdp {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub domain: String,
    pub password: String,
    /// The program PSM starts ("alternate shell").
    pub remote_program: String,
    pub load_balance_info: String,
}

/// The vault user for PSM: the option, else who signed in, else the source's account.
fn vault_user(ca: &CyberArk, cfg: &SourceConfig, option: &str) -> String {
    let o = cfg.opt(option);
    if !o.is_empty() {
        return o;
    }
    ca.session().map(|s| s.user).filter(|u| !u.is_empty()).unwrap_or_else(|| cfg.account.trim().to_string())
}

/// PSMP's address, port and the login name for `target_user` on the target. With single sign-on there is
/// no password PSMP could take, so Cablebag signs in first and brings an MFA caching key.
pub async fn psmp_login(ca: &CyberArk, app: &AppHandle, cfg: &SourceConfig, target_user: &str, target_host: &str, target_port: u16) -> Result<PsmpLogin> {
    let host = cfg.opt("psmp_host");
    if host.is_empty() {
        bail!("Fill in the PSM for SSH server of {} under Settings, Credential sources.", cfg.name);
    }
    if target_host.trim().is_empty() {
        bail!("There is no target address to connect to through PSM for SSH.");
    }
    let port = cfg.opt("psmp_port").parse::<u16>().ok().filter(|p| *p != 0).unwrap_or(22);
    let want_key = cfg.opt_bool("psmp_key", true);
    if want_key && Auth::of(cfg).single_sign_on() && !ca.is_unlocked() {
        ca.unlock_interactive(app, cfg).await?;
    }
    // A key is a convenience: without one PSMP asks for the password.
    let key = if want_key && ca.is_unlocked() { ca.mfa_key(app, cfg).await.ok() } else { None };
    let vault = vault_user(ca, cfg, "psmp_user");
    if vault.is_empty() {
        bail!("Fill in your CyberArk user name for {} under Settings, Credential sources.", cfg.name);
    }
    let user = psm::psmp_user(&psm::PsmpTarget { vault_user: vault, target_user: target_user.to_string(), target_address: target_host.to_string(), target_port, ..Default::default() });
    Ok(PsmpLogin { host, port, user, key })
}

/// How to reach a Windows target through PSM: from the PSMConnect API (an RDP file for the account), or
/// with an RDP client to the PSM server and the documented start program.
pub async fn psm_rdp(ca: &CyberArk, app: &AppHandle, cfg: &SourceConfig, account: Option<&str>, target_user: &str, target_host: &str) -> Result<PsmRdp> {
    let component = Some(cfg.opt("psm_component")).filter(|c| !c.is_empty()).unwrap_or_else(|| "PSM-RDP".into());
    let method = Auth::of(cfg);
    let use_api = match cfg.opt("psm_rdp_mode").as_str() {
        "connect" => true,
        "client" => false,
        // People who sign in with single sign-on have no password the PSM sign-in screen could take.
        _ => account.is_some() || method.single_sign_on() || cfg.opt("psm_host").is_empty(),
    };
    if use_api {
        return psm_connect(ca, app, cfg, account, target_user, target_host, &component).await;
    }
    let host = cfg.opt("psm_host");
    if host.is_empty() {
        bail!("Fill in the PSM server of {} under Settings, Credential sources.", cfg.name);
    }
    if target_host.trim().is_empty() || target_user.trim().is_empty() {
        bail!("Connecting through PSM with an RDP client needs the target's user name and address.");
    }
    let port = cfg.opt("psm_port").parse::<u16>().unwrap_or(0);
    let user = vault_user(ca, cfg, "psm_user");
    let mut password = String::new();
    if !method.single_sign_on() {
        let state = app.state::<AppState>();
        let who = if user.is_empty() { "you".to_string() } else { user.clone() };
        let msg = format!("Your CyberArk password for {who}, for the PSM server. Leave it empty to type it on the PSM sign-in screen.");
        let Some((pw, _)) = state.prompts.ask_secret(app, &format!("Connect through {}", cfg.name), &msg, "Password", false).await else { bail!("Connection cancelled.") };
        password = pw;
    }
    Ok(PsmRdp { host, port, user, domain: String::new(), password, remote_program: psm::psm_start_program(target_user, target_host, &component, "", ""), load_balance_info: String::new() })
}

async fn psm_connect(ca: &CyberArk, app: &AppHandle, cfg: &SourceConfig, account: Option<&str>, target_user: &str, target_host: &str, component: &str) -> Result<PsmRdp> {
    if !ca.is_unlocked() {
        ca.unlock_interactive(app, cfg).await?;
    }
    let id = match account {
        Some(a) => a.to_string(),
        None => ca.account_for(app, cfg, target_user, target_host).await?,
    };
    let meta = ca.meta(app, cfg, &id).await?;
    let idr = id.as_str();
    let answer = ca
        .with_access(app, cfg, &meta, |access| async move {
            let a = &access;
            ca.call(app, cfg, |http, s| async move { api::psm_connect(&http, &s.root, &s.auth, idr, component, a).await }).await
        })
        .await?;
    ca.note_checkout(app, cfg, &id, &meta).await;
    let text = match answer {
        api::PsmAnswer::Rdp(t) => t,
        api::PsmAnswer::Gateway => bail!("CyberArk hands out connections through its HTML5 gateway here, which Cablebag can not use. Choose \"With an RDP client\" under Connecting through PSM in the source settings."),
    };
    let f = psm::parse_rdp(&text);
    if f.host.is_empty() {
        bail!("CyberArk sent a connection without a PSM server address.");
    }
    Ok(PsmRdp { host: f.host, port: f.port, user: f.user, domain: f.domain, password: String::new(), remote_program: f.alternate_shell, load_balance_info: f.load_balance_info })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg(url: &str, options: Value) -> SourceConfig {
        SourceConfig { id: "ca1".into(), name: "Work".into(), kind: "cyberark".into(), url: url.into(), account: String::new(), options }
    }

    #[test]
    fn server_roots() {
        assert_eq!(root_of("pvwa.example.com"), "https://pvwa.example.com");
        assert_eq!(root_of("https://pvwa.example.com/PasswordVault/v10/logon"), "https://pvwa.example.com");
        assert_eq!(root_of("https://pvwa.example.com:8443/"), "https://pvwa.example.com:8443");
        assert_eq!(root_of("https://Acme.privilegecloud.cyberark.com"), "https://acme.privilegecloud.cyberark.com");
        assert_eq!(root_of("https://acme.cyberark.cloud/privilegecloud/"), "https://acme.privilegecloud.cyberark.cloud");
        assert_eq!(root_of("acme.privilegecloud.cyberark.cloud"), "https://acme.privilegecloud.cyberark.cloud");
        assert_eq!(root_of(""), "");
        assert_eq!(flavour("https://acme.privilegecloud.cyberark.cloud"), Flavour::SharedServices);
        assert_eq!(flavour("https://acme.privilegecloud.cyberark.com"), Flavour::PrivilegeCloud);
        assert_eq!(flavour("https://pvwa.example.com"), Flavour::SelfHosted);
        assert_eq!(portal_of("https://acme.privilegecloud.cyberark.cloud").as_deref(), Some("https://acme.cyberark.cloud/privilegecloud/"));
        assert_eq!(portal_of("https://pvwa.example.com"), None);
    }

    #[test]
    fn sign_in_methods() {
        assert_eq!(Auth::of(&cfg("https://pvwa.example.com", json!({}))), Auth::CyberArk);
        assert_eq!(Auth::of(&cfg("https://acme.privilegecloud.cyberark.cloud", json!({}))), Auth::Identity);
        assert_eq!(Auth::of(&cfg("https://acme.privilegecloud.cyberark.com", json!({ "auth": "saml" }))), Auth::Saml);
        assert_eq!(Auth::of(&cfg("https://pvwa.example.com", json!({ "auth": "LDAP" }))).path(), "LDAP");
        assert_eq!(Auth::of(&cfg("https://pvwa.example.com", json!({ "auth": "radius" }))).path(), "RADIUS");
        assert_eq!(Auth::of(&cfg("https://pvwa.example.com", json!({ "auth": "windows" }))).path(), "Windows");
        assert_eq!(Auth::CyberArk.path(), "Cyberark");
        assert!(Auth::Identity.single_sign_on() && Auth::Saml.single_sign_on() && !Auth::Ldap.single_sign_on());
        let c = cfg("https://acme.privilegecloud.cyberark.cloud", json!({ "identity_url": "abc1234.id.cyberark.cloud/" }));
        assert_eq!(identity_root(&c).as_deref(), Some("https://abc1234.id.cyberark.cloud"));
    }

    #[test]
    fn accounts_become_items() {
        let p1: Value = serde_json::from_str(include_str!("../../../tests/cyberark/accounts_page1.json")).unwrap();
        let list: Vec<(Item, Meta)> = p1["value"].as_array().unwrap().iter().filter_map(to_item).collect();
        let (a, m) = &list[0];
        assert_eq!(a.id, "12_3");
        assert_eq!(a.name, "admin @ 10.0.0.5");
        assert_eq!(a.username, "admin");
        assert_eq!(a.uris, vec!["10.0.0.5".to_string()]);
        assert_eq!(a.folder, "Windows-Servers");
        assert_eq!(a.organization, "WinServerLocal");
        assert!(a.has_password && m.has_password);
        assert_eq!(m.address, "10.0.0.5");
        let (k, _) = &list[1];
        assert_eq!(k.name, "root @ web01.example.com");
        assert!(!k.has_password);
        assert_eq!(readable_name("Some account", "", ""), "Some account");
        assert_eq!(readable_name("", "svc", ""), "svc");
        assert_eq!(readable_name("Named", "svc", ""), "Named");
        assert!(to_item(&json!({ "name": "no id" })).is_none());
    }

    #[test]
    fn nothing_kept_says_so() {
        let ca = CyberArk::default();
        ca.st.lock().passwords.push("pw-test".into());
        assert_eq!(ca.known_passwords(), vec!["pw-test".to_string()]);
        assert!(!ca.is_unlocked());
    }

    #[test]
    fn a_passphrase_key_that_is_not_one_is_refused_plainly() {
        let e = open_key("not a key", "x").unwrap_err().to_string();
        assert!(e.contains("could not read"));
    }
}

//! The Secret Server REST API calls, each a plain function of a base address, a `reqwest::Client` and a
//! token, so the tests run them against a mock server without an AppHandle. Which documented endpoint each
//! one follows is listed at the top of `mod.rs`.

use crate::sources::Item;
use anyhow::{anyhow, Result};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

/// Secrets asked for per page when loading or searching.
pub const PAGE: usize = 500;

/// Template fields asked for with every search ("extended fields"), so the user name and the machine come
/// back with the list. Secret Server returns only the ones its templates expose for display; names it does
/// not know are left out of the answer.
pub const EXTENDED: [&str; 8] = ["Username", "Machine", "Server", "Host", "URL", "Domain", "Resource", "Hostname"];

/// Fields whose value is the address a secret is for.
const PLACES: [&str; 8] = ["machine", "server", "host", "hostname", "url", "resource", "computer", "ip address"];

/// Why a call failed, in a form the caller can act on. The text never holds a token or a password.
#[derive(Debug)]
pub enum Fail {
    /// The access token expired or was signed out on the server: refresh it or sign in again.
    Expired,
    /// The sign-in itself was not accepted, with the server's own message.
    Rejected(String),
    /// Secret Server refused a request: its status, its error code and its message.
    Server { status: u16, code: String, message: String },
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fail::Expired => write!(f, "The Secret Server sign-in has expired. Sign in again."),
            Fail::Rejected(m) if m.is_empty() => write!(f, "Secret Server did not accept the sign-in."),
            Fail::Rejected(m) => write!(f, "Secret Server did not accept the sign-in: {}", sentence(m)),
            Fail::Server { message, .. } if !message.is_empty() => write!(f, "Secret Server said: {}", sentence(message)),
            Fail::Server { status: 403, .. } => write!(f, "Secret Server refused this: your account may not have access to it."),
            Fail::Server { status: 404, .. } => write!(f, "Secret Server does not know this. It may have been removed, or the address is not a Secret Server."),
            Fail::Server { status, .. } if *status >= 500 => write!(f, "Secret Server ran into a problem. Try again later."),
            Fail::Server { .. } => write!(f, "Secret Server did not accept the request."),
        }
    }
}

impl std::error::Error for Fail {}

fn sentence(m: &str) -> String {
    let m = m.trim();
    if m.ends_with(['.', '!', '?']) { m.to_string() } else { format!("{m}.") }
}

/// The access token expired: one refresh or a new sign-in fixes it.
pub fn is_expired(e: &anyhow::Error) -> bool {
    matches!(e.downcast_ref::<Fail>(), Some(Fail::Expired))
}

fn status_of(e: &anyhow::Error) -> Option<u16> {
    match e.downcast_ref::<Fail>() {
        Some(Fail::Server { status, .. }) => Some(*status),
        _ => None,
    }
}

/// The server's message in a sign-in answer that was not accepted, when it hints that a two-factor code is
/// needed as well. The documentation names the OTP header but not the message, so this looks for the words.
pub fn wants_code(message: &str) -> bool {
    let m = message.to_lowercase();
    ["otp", "two factor", "two-factor", "2fa", "one time", "one-time", "multi-factor", "multifactor", "mfa", "verification code"].iter().any(|w| m.contains(w))
}

fn text(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn int(v: &Value, key: &str) -> Option<i64> {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    }
}

/// The error message in a Secret Server answer (BadRequestResponse, AuthenticationFailedResponse, the
/// token errors), whichever field it uses.
fn message_of(body: &Value) -> String {
    for k in ["message", "error_description", "error", "messageDetail"] {
        let m = text(body, k);
        if !m.is_empty() {
            return m;
        }
    }
    String::new()
}

/// A 401, or the 403 that the documentation shows after a token was expired ("Authentication failed").
fn looks_expired(status: u16, message: &str) -> bool {
    let m = message.to_lowercase();
    status == 401 || (status == 403 && (m.contains("authentication failed") || m.contains("expired token") || m.contains("access token")))
}

fn unreachable(base: &str) -> anyhow::Error {
    let host = base.trim_start_matches("https://").trim_start_matches("http://");
    anyhow!("Could not reach Secret Server at {host}. Check the address and your connection.")
}

async fn send(req: RequestBuilder, base: &str) -> Result<reqwest::Response> {
    let resp = req.send().await.map_err(|_| unreachable(base))?;
    let status = resp.status().as_u16();
    if resp.status().is_success() {
        return Ok(resp);
    }
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    let message = message_of(&body);
    if looks_expired(status, &message) {
        return Err(Fail::Expired.into());
    }
    Err(Fail::Server { status, code: text(&body, "errorCode"), message }.into())
}

async fn send_json(req: RequestBuilder, base: &str) -> Result<Value> {
    let resp = send(req, base).await?;
    resp.json().await.map_err(|_| anyhow!("Secret Server sent an answer Cablebag cannot read. Is the address that of a Secret Server?"))
}

fn url(base: &str, path: &str) -> Result<Url> {
    Url::parse(&format!("{}{path}", base.trim_end_matches('/'))).map_err(|_| anyhow!("The Secret Server address is not valid."))
}

/// A token from Secret Server or the Delinea Platform, with the Unix time it stops working.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub access: String,
    #[serde(default)]
    pub refresh: Option<String>,
    #[serde(default)]
    pub expires: i64,
}

impl Token {
    /// Expired, or about to: a refresh first saves a failed call.
    pub fn stale(&self, now: i64) -> bool {
        self.expires > 0 && now >= self.expires
    }
}

/// The documented TokenResponse: access_token, expires_in (a string in the reference, a number in the
/// examples) and refresh_token when the server allows refresh tokens.
fn token_of(v: &Value, now: i64) -> Result<Token> {
    let access = text(v, "access_token");
    if access.is_empty() {
        return Err(anyhow!("Secret Server sent no sign-in token."));
    }
    let refresh = Some(text(v, "refresh_token")).filter(|r| !r.is_empty());
    // A little early, so a call does not start with a token that runs out on the way.
    let expires = int(v, "expires_in").filter(|s| *s > 0).map(|s| now + (s - 30).max(1)).unwrap_or(0);
    Ok(Token { access, refresh, expires })
}

async fn token_call(req: RequestBuilder, base: &str, now: i64) -> Result<Token> {
    let resp = req.send().await.map_err(|_| unreachable(base))?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return token_of(&body, now);
    }
    if status.is_client_error() {
        return Err(Fail::Rejected(message_of(&body)).into());
    }
    Err(Fail::Server { status: status.as_u16(), code: String::new(), message: message_of(&body) }.into())
}

/// The user name as Secret Server expects it: "domain\user" when a domain is set (the reference lists both
/// "username" and "domain\username"). A name that already holds a domain stays as it is.
pub fn sign_in_name(account: &str, domain: &str) -> String {
    let (a, d) = (account.trim(), domain.trim());
    if d.is_empty() || a.contains('\\') || a.contains('@') { a.to_string() } else { format!("{d}\\{a}") }
}

/// POST /oauth2/token with grant_type=password. A two-factor code goes in the OTP header.
pub async fn password_token(http: &Client, base: &str, username: &str, password: &str, otp: Option<&str>, now: i64) -> Result<Token> {
    let mut req = http.post(url(base, "/oauth2/token")?).form(&[("grant_type", "password"), ("username", username), ("password", password)]);
    if let Some(code) = otp.map(str::trim).filter(|c| !c.is_empty()) {
        req = req.header("OTP", code);
    }
    token_call(req, base, now).await
}

/// POST /oauth2/token with grant_type=refresh_token. A refresh token works once; the answer holds the next.
pub async fn refresh_token(http: &Client, base: &str, refresh: &str, now: i64) -> Result<Token> {
    let req = http.post(url(base, "/oauth2/token")?).form(&[("grant_type", "refresh_token"), ("refresh_token", refresh)]);
    token_call(req, base, now).await
}

/// The Delinea Platform token for a service user: client_credentials at the platform's identity endpoint,
/// with the service user's name and password as client id and secret and the scope "xpmheadless".
pub async fn platform_token(http: &Client, platform: &str, client_id: &str, secret: &str, now: i64) -> Result<Token> {
    let req = http
        .post(url(platform, "/identity/api/oauth2/token/xpmplatform")?)
        .form(&[("grant_type", "client_credentials"), ("client_id", client_id), ("client_secret", secret), ("scope", "xpmheadless")]);
    token_call(req, platform, now).await
}

/// A new platform token from the refresh token, sent with the (expired) access token as the documentation shows.
pub async fn platform_refresh(http: &Client, platform: &str, access: &str, refresh: &str, now: i64) -> Result<Token> {
    let req = http
        .post(url(platform, "/identity/api/oauth2/token/xpmplatform")?)
        .bearer_auth(access)
        .form(&[("grant_type", "refresh_token"), ("refresh_token", refresh)]);
    token_call(req, platform, now).await
}

/// GET /api/v1/users/current: (user id, the name to show).
pub async fn current_user(http: &Client, base: &str, token: &str) -> Result<(i64, String)> {
    let v = send_json(http.get(url(base, "/api/v1/users/current")?).bearer_auth(token), base).await?;
    let name = Some(text(&v, "displayName")).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| text(&v, "userName"));
    Ok((int(&v, "id").unwrap_or(0), name))
}

/// POST /api/v1/oauth-expiration: signs this token out on the server.
pub async fn expire_session(http: &Client, base: &str, token: &str) -> Result<()> {
    send(http.post(url(base, "/api/v1/oauth-expiration")?).bearer_auth(token), base).await?;
    Ok(())
}

/// One page of a secret search.
#[derive(Debug, Default)]
pub struct Page {
    pub items: Vec<Item>,
    pub total: Option<usize>,
    /// The `skip` of the next page, when there is one.
    pub next: Option<usize>,
}

/// GET /api/v2/secrets (the search, with paging and extended fields). Servers from before version 11.2
/// only know /api/v1/secrets, which takes the same filter.
pub async fn search_page(http: &Client, base: &str, token: &str, text_: &str, skip: usize, take: usize, extended: bool) -> Result<Page> {
    let query = |u: &mut Url| {
        let mut q = u.query_pairs_mut();
        if !text_.trim().is_empty() {
            q.append_pair("filter.searchText", text_.trim());
        }
        // Secrets that need a comment, a check-out or approval are listed too: opening one asks for it.
        q.append_pair("filter.includeRestricted", "true");
        if extended {
            for f in EXTENDED {
                q.append_pair("filter.extendedFields", f);
            }
        }
        q.append_pair("skip", &skip.to_string());
        q.append_pair("take", &take.to_string());
        q.append_pair("sortBy[0].name", "name");
        q.append_pair("sortBy[0].direction", "asc");
    };
    let mut u = url(base, "/api/v2/secrets")?;
    query(&mut u);
    let v = match send_json(http.get(u).bearer_auth(token), base).await {
        Err(e) if status_of(&e) == Some(404) => {
            let mut u = url(base, "/api/v1/secrets")?;
            query(&mut u);
            send_json(http.get(u).bearer_auth(token), base).await?
        }
        other => other?,
    };
    let records = v.get("records").and_then(Value::as_array).cloned().unwrap_or_default();
    let items: Vec<Item> = records.iter().filter_map(item_of).collect();
    let next = if flag(&v, "hasNext") { Some(int(&v, "nextSkip").map(|n| n as usize).unwrap_or(skip + records.len())) } else { None };
    // An empty page never leads anywhere, whatever the server says.
    let next = next.filter(|_| !records.is_empty());
    Ok(Page { items, total: int(&v, "total").map(|t| t as usize), next })
}

/// Secrets that match `text_` (all of them when empty), up to `cap`; true when there were more.
/// When the server will not return the extended fields (an older version, or a field it refuses), the
/// search runs again without them: the list then has names, folders and templates only.
pub async fn search(http: &Client, base: &str, token: &str, text_: &str, cap: usize) -> Result<(Vec<Item>, bool)> {
    let mut out: Vec<Item> = Vec::new();
    let mut skip = 0;
    let mut extended = true;
    loop {
        let take = PAGE.min(cap - out.len());
        let page = match search_page(http, base, token, text_, skip, take, extended).await {
            Err(e) if extended && status_of(&e) == Some(400) => {
                extended = false;
                continue;
            }
            other => other?,
        };
        out.extend(page.items);
        match page.next {
            // A next page that does not move on would loop forever: stop there.
            Some(n) if n <= skip => return Ok((out, false)),
            Some(n) if out.len() < cap => skip = n,
            Some(_) => return Ok((out, true)),
            None => {
                let more = page.total.map(|t| t > out.len()).unwrap_or(false) && out.len() >= cap;
                return Ok((out, more));
            }
        }
    }
}

/// A SecretSummary from a search as an Item. Whether a secret has a password is not in the summary, so
/// every secret counts as one with a password until it is opened.
pub fn item_of(s: &Value) -> Option<Item> {
    let id = int(s, "id")?;
    let mut username = String::new();
    let mut domain = String::new();
    let mut uris = Vec::new();
    for f in s.get("extendedFields").and_then(Value::as_array).into_iter().flatten() {
        let name = text(f, "name").to_lowercase();
        let value = text(f, "value").trim().to_string();
        if value.is_empty() {
            continue;
        }
        if name == "username" || name == "user name" {
            username = value;
        } else if name == "domain" {
            domain = value;
        } else if PLACES.contains(&name.as_str()) && !uris.contains(&value) {
            uris.push(value);
        }
    }
    let folder = text(s, "folderPath").trim_start_matches('\\').to_string();
    Some(Item {
        id: id.to_string(),
        source: String::new(),
        name: text(s, "name"),
        username: with_domain(&username, &domain),
        uris,
        folder,
        organization: text(s, "secretTemplateName"),
        has_password: true,
    })
}

/// "DOMAIN\user" for an Active Directory account, so Windows logins get their domain.
fn with_domain(user: &str, domain: &str) -> String {
    if user.is_empty() || domain.is_empty() || user.contains('\\') || user.contains('@') { user.to_string() } else { format!("{domain}\\{user}") }
}

/// What Secret Server wants before it shows a secret, from GET /api/v1/secrets/{id}/state.
#[derive(Debug, Clone, PartialEq)]
pub enum Need {
    Nothing,
    Comment,
    CheckOut,
    CheckOutAndComment,
    Approval,
    DoubleLock,
    /// Someone else has it checked out (their name).
    TakenBy(String),
    Mfa,
    CannotView,
    Inactive,
    /// The password is being changed after a check-in; it can be checked out once that is done.
    Changing,
}

/// The documented SecretAccessRequired values, in their documented order (for a server that sends the number).
const STATES: [&str; 12] = [
    "None",
    "RequiresApproval",
    "RequiresCheckout",
    "RequiresComment",
    "RequiresDoubleLockPassword",
    "CreateDoubleLockPassword",
    "DoubleLockNoAccess",
    "CannotView",
    "RequiresUndelete",
    "RequiresCheckoutPendingRPC",
    "RequiresCheckoutAndComment",
    "RequiresMfaChallenge",
];

/// The SecretDetailStateViewModel's secretState as a Need.
pub fn need_of(state: &Value) -> Need {
    let s = match state.get("secretState") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.as_u64().and_then(|i| STATES.get(i as usize)).map(|s| s.to_string()).unwrap_or_default(),
        _ => String::new(),
    };
    let taken = flag(state, "isCheckedOut") && !flag(state, "isCheckedOutByCurrentUser");
    match s.to_lowercase().as_str() {
        "requirescheckout" | "requirescheckoutandcomment" if taken => Need::TakenBy(text(state, "checkedOutUserDisplayName")),
        "requiresapproval" => Need::Approval,
        "requirescheckout" => Need::CheckOut,
        "requirescomment" => Need::Comment,
        "requirescheckoutandcomment" => Need::CheckOutAndComment,
        "requiresdoublelockpassword" => Need::DoubleLock,
        "createdoublelockpassword" | "doublelocknoaccess" | "cannotview" => Need::CannotView,
        "requiresundelete" => Need::Inactive,
        "requirescheckoutpendingrpc" => Need::Changing,
        "requiresmfachallenge" => Need::Mfa,
        _ => Need::Nothing,
    }
}

/// What an error on reading a secret says it wants, for a server without the state call. The reference
/// lists no error codes for this, so the words of the code and the message decide.
pub fn need_of_error(e: &anyhow::Error) -> Option<Need> {
    let Some(Fail::Server { code, message, .. }) = e.downcast_ref::<Fail>() else { return None };
    let w = format!("{code} {message}").to_lowercase().replace(['-', '_'], " ");
    let checkout = w.contains("checkout") || w.contains("check out") || w.contains("checked out");
    let comment = w.contains("comment") || w.contains("ticket");
    if w.contains("approval") {
        Some(Need::Approval)
    } else if w.contains("doublelock") || w.contains("double lock") {
        Some(Need::DoubleLock)
    } else if checkout && comment {
        Some(Need::CheckOutAndComment)
    } else if comment {
        Some(Need::Comment)
    } else if checkout {
        Some(Need::CheckOut)
    } else {
        None
    }
}

/// GET /api/v1/secrets/{id}/state. None when the server does not have it (before version 11).
pub async fn secret_state(http: &Client, base: &str, token: &str, id: &str) -> Result<Option<Value>> {
    match send_json(http.get(url(base, &format!("/api/v1/secrets/{id}/state"))?).bearer_auth(token), base).await {
        Ok(v) => Ok(Some(v)),
        Err(e) if status_of(&e) == Some(404) => Ok(None),
        Err(e) => Err(e),
    }
}

/// What the person gave for a restricted secret (the documented SecretRestrictedArgs) and whether to check it out.
#[derive(Clone, Debug, Default)]
pub struct Access {
    pub comment: String,
    pub ticket: String,
    pub ticket_system: Option<i64>,
    pub double_lock: String,
    pub check_out: bool,
}

impl Access {
    /// Something to send with the restricted calls.
    pub fn restricted(&self) -> bool {
        !self.comment.trim().is_empty() || !self.ticket.trim().is_empty() || !self.double_lock.is_empty()
    }

    fn args(&self) -> Value {
        let mut a = json!({});
        if !self.comment.trim().is_empty() {
            a["comment"] = json!(self.comment.trim());
        }
        if !self.ticket.trim().is_empty() {
            a["ticketNumber"] = json!(self.ticket.trim());
            if let Some(t) = self.ticket_system {
                a["ticketSystemId"] = json!(t);
            }
        }
        if !self.double_lock.is_empty() {
            a["doubleLockPassword"] = json!(self.double_lock);
        }
        a
    }

    /// autoCheckout and autoComment, the query options of the restricted calls.
    fn query(&self, u: &mut Url) {
        let mut q = u.query_pairs_mut();
        if self.check_out {
            q.append_pair("autoCheckout", "true");
        }
        if !self.comment.trim().is_empty() {
            q.append_pair("autoComment", self.comment.trim());
        }
    }
}

/// POST /api/v1/secrets/{id}/check-out.
pub async fn check_out(http: &Client, base: &str, token: &str, id: &str) -> Result<()> {
    send(http.post(url(base, &format!("/api/v1/secrets/{id}/check-out"))?).bearer_auth(token), base).await?;
    Ok(())
}

/// POST /api/v1/secrets/{id}/check-in, with the comment given at check-out when there was one.
pub async fn check_in(http: &Client, base: &str, token: &str, id: &str, comment: &str) -> Result<()> {
    let body = if comment.trim().is_empty() { json!({}) } else { json!({ "comment": comment.trim() }) };
    send(http.post(url(base, &format!("/api/v1/secrets/{id}/check-in"))?).bearer_auth(token).json(&body), base).await?;
    Ok(())
}

/// The secret with its fields: POST /api/v1/secrets/{id}/restricted when there is a comment, a ticket
/// or a DoubleLock password to give, else GET /api/v2/secrets/{id} (or /api/v1 on older servers).
pub async fn read_secret(http: &Client, base: &str, token: &str, id: &str, access: &Access) -> Result<Value> {
    if access.restricted() {
        let mut u = url(base, &format!("/api/v1/secrets/{id}/restricted"))?;
        access.query(&mut u);
        return send_json(http.post(u).bearer_auth(token).json(&access.args()), base).await;
    }
    match send_json(http.get(url(base, &format!("/api/v2/secrets/{id}"))?).bearer_auth(token), base).await {
        Err(e) if status_of(&e) == Some(404) => send_json(http.get(url(base, &format!("/api/v1/secrets/{id}"))?).bearer_auth(token), base).await,
        other => other,
    }
}

/// One field's value: GET /api/v1/secrets/{id}/fields/{slug}, or its POST .../restricted/fields/{slug}
/// twin. The reference says password values only come this way. The answer is a JSON string, or plain text.
pub async fn read_field(http: &Client, base: &str, token: &str, id: &str, slug: &str, access: &Access) -> Result<String> {
    let req = if access.restricted() {
        let mut u = url(base, &format!("/api/v1/secrets/{id}/restricted/fields/{slug}"))?;
        access.query(&mut u);
        http.post(u).bearer_auth(token).json(&access.args())
    } else {
        http.get(url(base, &format!("/api/v1/secrets/{id}/fields/{slug}"))?).bearer_auth(token)
    };
    let body = send(req, base).await?.text().await.map_err(|_| anyhow!("Secret Server sent an answer Cablebag cannot read."))?;
    Ok(serde_json::from_str::<String>(&body).unwrap_or(body))
}

/// (user name with its domain, the slug of the password field) from a secret's items (RestSecretItem).
pub fn fields_of(secret: &Value) -> (String, Option<String>) {
    let items = secret.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    let value = |want: &dyn Fn(&str, &str) -> bool| {
        items
            .iter()
            .find(|i| want(&text(i, "slug").to_lowercase(), &text(i, "fieldName").to_lowercase()))
            .map(|i| text(i, "itemValue").trim().to_string())
            .unwrap_or_default()
    };
    let user = value(&|slug, name| slug == "username" || name == "username" || name == "user name");
    let domain = value(&|slug, name| slug == "domain" || name == "domain");
    // The field called password; else the first password field (a passphrase or PIN comes later on templates).
    let secret_fields: Vec<&Value> = items.iter().filter(|i| flag(i, "isPassword")).collect();
    let password = secret_fields
        .iter()
        .find(|i| text(i, "slug").eq_ignore_ascii_case("password"))
        .or(secret_fields.first())
        .map(|i| text(i, "slug"))
        .filter(|s| !s.is_empty());
    (with_domain(&user, &domain), password)
}

/// The login of a secret, read.
#[derive(Debug, Default)]
pub struct Login {
    pub name: String,
    pub username: String,
    pub password: String,
    /// Checked out to this user now (by this call, or the server did it on reading).
    pub checked_out: bool,
}

/// Checks the secret out when asked, reads it and its password. `me` is the signed-in user's id, to see
/// whether the server checked the secret out to us on reading it.
pub async fn fetch_login(http: &Client, base: &str, token: &str, id: &str, access: &Access, me: i64) -> Result<Login> {
    if access.check_out && !access.restricted() {
        check_out(http, base, token, id).await?;
    }
    let secret = read_secret(http, base, token, id, access).await?;
    let (username, slug) = fields_of(&secret);
    let password = match &slug {
        Some(s) => read_field(http, base, token, id, s, access).await?,
        None => String::new(),
    };
    let ours = flag(&secret, "checkedOut") && me != 0 && int(&secret, "checkOutUserId") == Some(me);
    Ok(Login { name: text(&secret, "name"), username, password, checked_out: access.check_out || ours })
}

/// Minutes a check-out lasts, from the state, for the question.
pub fn check_out_minutes(state: &Value) -> Option<i64> {
    int(state, "checkOutIntervalMinutes").filter(|m| *m > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{bearer_token, body_partial_json, body_string_contains, header, method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TOKEN: &str = include_str!("../../../tests/delinea/token.json");
    const PAGE1: &str = include_str!("../../../tests/delinea/search_page1.json");
    const PAGE2: &str = include_str!("../../../tests/delinea/search_page2.json");
    const SECRET: &str = include_str!("../../../tests/delinea/secret.json");
    const STATE: &str = include_str!("../../../tests/delinea/state.json");
    const PLATFORM: &str = include_str!("../../../tests/delinea/platform_token.json");

    fn http() -> Client {
        Client::new()
    }

    fn json_of(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    fn state(secret_state: &str) -> Value {
        let mut v = json_of(STATE);
        v["secretState"] = json!(secret_state);
        v
    }

    #[tokio::test]
    async fn password_grant_gives_a_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token"))
            .and(body_string_contains("grant_type=password"))
            .and(body_string_contains("username=CORP%5Cjdoe"))
            .and(body_string_contains("password=not-a-real-password"))
            .respond_with(ResponseTemplate::new(200).set_body_string(TOKEN).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        let t = password_token(&http(), &server.uri(), &sign_in_name("jdoe", "CORP"), "not-a-real-password", None, 1000).await.unwrap();
        assert_eq!(t.access, "test-access-token");
        assert_eq!(t.refresh.as_deref(), Some("test-refresh-token"));
        assert_eq!(t.expires, 1000 + 1199 - 30);
        assert!(!t.stale(1000) && t.stale(3000));
    }

    #[tokio::test]
    async fn the_two_factor_code_goes_in_the_otp_header() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token"))
            .and(header("OTP", "123456"))
            .respond_with(ResponseTemplate::new(200).set_body_string(TOKEN).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        let t = password_token(&http(), &server.uri(), "jdoe", "pw", Some(" 123456 "), 0).await.unwrap();
        assert_eq!(t.access, "test-access-token");
    }

    #[tokio::test]
    async fn refresh_grant_gives_the_next_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains("refresh_token=old-refresh"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "access_token": "next-access", "token_type": "bearer", "expires_in": "1199", "refresh_token": "next-refresh" })))
            .expect(1)
            .mount(&server)
            .await;
        let t = refresh_token(&http(), &server.uri(), "old-refresh", 0).await.unwrap();
        assert_eq!((t.access.as_str(), t.refresh.as_deref()), ("next-access", Some("next-refresh")));
        assert_eq!(t.expires, 1199 - 30);
    }

    #[tokio::test]
    async fn a_wrong_password_is_rejected_with_the_servers_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth2/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({ "error": "Login failed." })))
            .mount(&server)
            .await;
        let e = password_token(&http(), &server.uri(), "jdoe", "wrong", None, 0).await.unwrap_err();
        match e.downcast_ref::<Fail>() {
            Some(Fail::Rejected(m)) => assert_eq!(m, "Login failed."),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(e.to_string(), "Secret Server did not accept the sign-in: Login failed.");
        assert!(!e.to_string().contains("wrong"));
        assert!(wants_code("Please enter your OTP code"));
        assert!(!wants_code("Login failed."));
    }

    #[tokio::test]
    async fn platform_service_user_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/identity/api/oauth2/token/xpmplatform"))
            .and(body_string_contains("grant_type=client_credentials"))
            .and(body_string_contains("client_id=svc-cablebag"))
            .and(body_string_contains("scope=xpmheadless"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PLATFORM).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/identity/api/oauth2/token/xpmplatform"))
            .and(bearer_token("platform-access"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PLATFORM).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        let t = platform_token(&http(), &server.uri(), "svc-cablebag", "not-a-real-secret", 0).await.unwrap();
        assert_eq!(t.access, "platform-access");
        assert_eq!(t.refresh.as_deref(), Some("platform-refresh"));
        platform_refresh(&http(), &server.uri(), &t.access, t.refresh.as_deref().unwrap(), 0).await.unwrap();
    }

    #[tokio::test]
    async fn search_pages_and_maps_secrets() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/secrets"))
            .and(query_param("skip", "0"))
            .and(query_param("filter.includeRestricted", "true"))
            .and(query_param("filter.extendedFields", "Machine"))
            .and(bearer_token("tok"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PAGE1).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/secrets"))
            .and(query_param("skip", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PAGE2).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        let (items, more) = search(&http(), &server.uri(), "tok", "", 100).await.unwrap();
        assert!(!more);
        assert_eq!(items.len(), 3);
        let a = &items[0];
        assert_eq!((a.id.as_str(), a.name.as_str(), a.username.as_str()), ("12", "db01 root", "root"));
        assert_eq!(a.uris, vec!["db01.example.test".to_string()]);
        assert_eq!(a.folder, "Servers\\Linux");
        assert_eq!(a.organization, "Unix Account (SSH)");
        assert!(a.has_password);
        // An Active Directory account carries its domain; a web login its URL.
        assert_eq!(items[1].username, "EXAMPLE\\svc-backup");
        assert_eq!(items[2].uris, vec!["https://intranet.example.test".to_string()]);
    }

    #[tokio::test]
    async fn search_stops_at_the_cap_and_says_there_is_more() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/secrets"))
            .and(query_param("take", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PAGE1).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        let (items, more) = search(&http(), &server.uri(), "tok", "", 2).await.unwrap();
        assert_eq!(items.len(), 2);
        assert!(more);
    }

    #[tokio::test]
    async fn search_without_extended_fields_when_the_server_refuses_them() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/secrets"))
            .and(query_param("filter.searchText", "db01"))
            .and(query_param("filter.extendedFields", "Username"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({ "message": "The request is invalid." })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/secrets"))
            .and(query_param_is_missing("filter.extendedFields"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PAGE2).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        let (items, _) = search(&http(), &server.uri(), "tok", "db01", 50).await.unwrap();
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn reading_a_secret_gives_user_and_password() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/secrets/12"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SECRET).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/secrets/12/fields/password"))
            .respond_with(ResponseTemplate::new(200).set_body_string("\"not-a-real-password\"").insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        let l = fetch_login(&http(), &server.uri(), "tok", "12", &Access::default(), 7).await.unwrap();
        assert_eq!((l.name.as_str(), l.username.as_str(), l.password.as_str()), ("db01 root", "root", "not-a-real-password"));
        assert!(!l.checked_out);
    }

    #[tokio::test]
    async fn a_comment_required_secret_is_read_through_the_restricted_calls() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/secrets/12/state"))
            .respond_with(ResponseTemplate::new(200).set_body_json(state("RequiresComment")))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v1/secrets/12/restricted"))
            .and(body_partial_json(json!({ "comment": "Patching db01", "ticketNumber": "CHG-1", "ticketSystemId": 2 })))
            .and(query_param("autoComment", "Patching db01"))
            .and(query_param_is_missing("autoCheckout"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SECRET).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v1/secrets/12/restricted/fields/password"))
            .and(body_partial_json(json!({ "comment": "Patching db01" })))
            .respond_with(ResponseTemplate::new(200).set_body_string("\"not-a-real-password\""))
            .expect(1)
            .mount(&server)
            .await;
        let st = secret_state(&http(), &server.uri(), "tok", "12").await.unwrap().unwrap();
        assert_eq!(need_of(&st), Need::Comment);
        let access = Access { comment: "Patching db01".into(), ticket: "CHG-1".into(), ticket_system: Some(2), ..Default::default() };
        let l = fetch_login(&http(), &server.uri(), "tok", "12", &access, 7).await.unwrap();
        assert_eq!(l.password, "not-a-real-password");
    }

    #[tokio::test]
    async fn a_check_out_required_secret_is_checked_out_first() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/secrets/12/state"))
            .respond_with(ResponseTemplate::new(200).set_body_json(state("RequiresCheckout")))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v1/secrets/12/check-out"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "responseCodes": [] })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/secrets/12"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SECRET).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/secrets/12/fields/password"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not-a-real-password"))
            .mount(&server)
            .await;
        let st = secret_state(&http(), &server.uri(), "tok", "12").await.unwrap().unwrap();
        assert_eq!(need_of(&st), Need::CheckOut);
        assert_eq!(check_out_minutes(&st), Some(30));
        let l = fetch_login(&http(), &server.uri(), "tok", "12", &Access { check_out: true, ..Default::default() }, 7).await.unwrap();
        assert!(l.checked_out);
        assert_eq!(l.password, "not-a-real-password");
    }

    #[tokio::test]
    async fn check_out_with_a_comment_goes_through_restricted_with_auto_checkout() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/secrets/12/restricted"))
            .and(query_param("autoCheckout", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SECRET).insert_header("content-type", "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v1/secrets/12/restricted/fields/password"))
            .respond_with(ResponseTemplate::new(200).set_body_string("\"pw\""))
            .mount(&server)
            .await;
        Mock::given(method("POST")).and(path("/api/v1/secrets/12/check-out")).respond_with(ResponseTemplate::new(500)).expect(0).mount(&server).await;
        let access = Access { comment: "Maintenance".into(), check_out: true, ..Default::default() };
        let l = fetch_login(&http(), &server.uri(), "tok", "12", &access, 7).await.unwrap();
        assert!(l.checked_out);
    }

    #[tokio::test]
    async fn check_in_and_sign_out() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/secrets/12/check-in"))
            .and(body_partial_json(json!({ "comment": "Maintenance" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": 12, "checkedOut": false })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v1/oauth-expiration"))
            .and(bearer_token("tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
            .expect(1)
            .mount(&server)
            .await;
        check_in(&http(), &server.uri(), "tok", "12", "Maintenance").await.unwrap();
        expire_session(&http(), &server.uri(), "tok").await.unwrap();
    }

    #[tokio::test]
    async fn an_expired_token_is_told_apart_from_a_refusal() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/users/current"))
            .and(bearer_token("old"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "message": "Authentication failed." })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/users/current"))
            .and(bearer_token("new"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": 7, "userName": "jdoe", "displayName": "J Doe" })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/secrets/99"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "message": "Access Denied" })))
            .mount(&server)
            .await;
        assert!(is_expired(&current_user(&http(), &server.uri(), "old").await.unwrap_err()));
        assert_eq!(current_user(&http(), &server.uri(), "new").await.unwrap(), (7, "J Doe".to_string()));
        let e = read_secret(&http(), &server.uri(), "new", "99", &Access::default()).await.unwrap_err();
        assert!(!is_expired(&e));
        assert_eq!(e.to_string(), "Secret Server said: Access Denied.");
    }

    #[test]
    fn states_and_errors_map_to_what_to_ask() {
        assert_eq!(need_of(&state("None")), Need::Nothing);
        assert_eq!(need_of(&state("RequiresApproval")), Need::Approval);
        assert_eq!(need_of(&state("RequiresCheckoutAndComment")), Need::CheckOutAndComment);
        assert_eq!(need_of(&state("RequiresDoubleLockPassword")), Need::DoubleLock);
        assert_eq!(need_of(&state("RequiresMfaChallenge")), Need::Mfa);
        assert_eq!(need_of(&state("RequiresCheckoutPendingRPC")), Need::Changing);
        assert_eq!(need_of(&json!({ "secretState": 3 })), Need::Comment);
        let mut taken = state("RequiresCheckout");
        taken["isCheckedOut"] = json!(true);
        taken["checkedOutUserDisplayName"] = json!("Someone Else");
        assert_eq!(need_of(&taken), Need::TakenBy("Someone Else".into()));
        let e: anyhow::Error = Fail::Server { status: 400, code: "API_CommentRequired".into(), message: String::new() }.into();
        assert_eq!(need_of_error(&e), Some(Need::Comment));
        let e: anyhow::Error = Fail::Server { status: 400, code: String::new(), message: "This secret requires check out.".into() }.into();
        assert_eq!(need_of_error(&e), Some(Need::CheckOut));
        let e: anyhow::Error = Fail::Server { status: 400, code: String::new(), message: "Something else.".into() }.into();
        assert_eq!(need_of_error(&e), None);
    }

    #[test]
    fn user_names_with_domains() {
        assert_eq!(sign_in_name("jdoe", "CORP"), "CORP\\jdoe");
        assert_eq!(sign_in_name("CORP\\jdoe", "OTHER"), "CORP\\jdoe");
        assert_eq!(sign_in_name("jdoe@corp.example.test", "CORP"), "jdoe@corp.example.test");
        assert_eq!(sign_in_name(" jdoe ", ""), "jdoe");
        let (user, slug) = fields_of(&json_of(SECRET));
        assert_eq!((user.as_str(), slug.as_deref()), ("root", Some("password")));
    }
}

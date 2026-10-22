//! The CyberArk REST calls Cablebag makes. Each is a plain function of the server root (scheme and host,
//! such as "https://pvwa.example.com"), an HTTP client and the session's Authorization value, so the tests
//! run them against a mock server without a window. Nothing here logs or puts a secret in an error.

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;

/// What a call that failed answered. Shown as a plain sentence: the server's own message where it gave
/// one, never its error code (the code is kept for telling cases apart).
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let m = self.message.as_str();
        let text = match self.status {
            401 => "Your CyberArk sign-in is no longer valid.".to_string(),
            403 if m.is_empty() => "CyberArk did not allow this. Ask your CyberArk administrator for access.".to_string(),
            403 => format!("CyberArk did not allow this: {m}"),
            404 if m.is_empty() => "CyberArk could not find this.".to_string(),
            429 => "CyberArk is busy at the moment. Try again in a little while.".to_string(),
            500..=599 if m.is_empty() => "The CyberArk server ran into a problem.".to_string(),
            _ if m.is_empty() => "CyberArk did not accept the request.".to_string(),
            _ => m.to_string(),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for ApiError {}

impl ApiError {
    pub fn unauthorised(&self) -> bool {
        self.status == 401
    }
}

/// True when an error is CyberArk saying the session token is no longer valid.
pub fn is_unauthorised(e: &anyhow::Error) -> bool {
    e.downcast_ref::<ApiError>().map(|a| a.unauthorised()).unwrap_or(false)
}

/// Looks a field up as camelCase or PascalCase; CyberArk's own APIs differ.
pub fn get<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    if let Some(x) = v.get(name) {
        return Some(x);
    }
    let mut c = name.chars();
    let other: String = match c.next() {
        Some(f) if f.is_lowercase() => f.to_uppercase().chain(c).collect(),
        Some(f) => f.to_lowercase().chain(c).collect(),
        None => return None,
    };
    v.get(&other)
}

pub fn get_str(v: &Value, name: &str) -> String {
    match get(v, name) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// The server's message without its error code ("ITATS542I ...") and in plain characters.
fn clean_message(m: &str) -> String {
    let re = regex::Regex::new(r"^\s*[A-Z]{3,8}\d{3}[A-Z]?\b[\s:.-]*").expect("pattern");
    re.replace(m.trim(), "").replace('\u{2014}', "-").trim().to_string()
}

/// Reads an error body: the documented {"ErrorCode", "ErrorMessage"} pair, or the "Details" some calls add.
pub fn error_from(status: u16, body: &str) -> ApiError {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let code = get_str(&v, "ErrorCode");
    let mut message = get_str(&v, "ErrorMessage");
    if message.is_empty() {
        message = get_str(&v, "Message");
    }
    if message.is_empty()
        && let Some(d) = get(&v, "Details").and_then(|d| d.as_array())
    {
        message = d.iter().map(|x| get_str(x, "ErrorMessage")).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
    }
    ApiError { status, code, message: clean_message(&message) }
}

async fn fail(resp: reqwest::Response) -> anyhow::Error {
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    anyhow::Error::new(error_from(status, &body))
}

fn reach(root: &str) -> impl FnOnce(reqwest::Error) -> anyhow::Error + '_ {
    move |e| {
        if e.is_timeout() {
            anyhow!("CyberArk at {root} did not answer in time.")
        } else {
            anyhow!("Could not reach CyberArk at {root}. Check the address and your network.")
        }
    }
}

/// An address with its query, encoded.
fn with_query(base: &str, q: &[(&str, String)]) -> Result<tauri::Url> {
    tauri::Url::parse_with_params(base, q.iter().map(|(k, v)| (*k, v.as_str()))).map_err(|_| anyhow!("{base} is not a web address."))
}

pub fn api(root: &str) -> String {
    format!("{root}/PasswordVault/API")
}

/// The session token in a logon answer: a JSON string ("<session token>"), the first generation's
/// {"CyberArkLogonResult": ...}, or bare text.
pub fn token_from(body: &str) -> Result<String> {
    let t = body.trim();
    let token = match serde_json::from_str::<Value>(t) {
        Ok(Value::String(s)) => s,
        Ok(v @ Value::Object(_)) => get_str(&v, "CyberArkLogonResult"),
        _ => t.trim_matches('"').to_string(),
    };
    if token.trim().is_empty() {
        return Err(anyhow!("CyberArk signed you in but sent no session."));
    }
    Ok(token.trim().to_string())
}

pub enum Logon {
    Token(String),
    /// The RADIUS server asks a question (a code from a token, say); its text, to show the person.
    Challenge(String),
}

/// The documented RADIUS challenge answer. The PVWA documentation says challenge-response works but not
/// how the REST logon shows it; this code is what PVWA sends (as the psPAS PowerShell module relies on).
pub fn is_radius_challenge(e: &ApiError) -> bool {
    e.code.eq_ignore_ascii_case("ITATS542I")
}

/// CyberArk, LDAP, RADIUS or Windows logon (`method` as in the URL: "Cyberark", "LDAP", "RADIUS",
/// "Windows"). A concurrent session, so signing in here does not end the person's PVWA tab.
pub async fn logon(http: &reqwest::Client, root: &str, method: &str, user: &str, password: &str) -> Result<Logon> {
    let resp = http
        .post(format!("{}/auth/{method}/Logon/", api(root)))
        .json(&json!({ "username": user, "password": password, "concurrentSession": true }))
        .send()
        .await
        .map_err(reach(root))?;
    if resp.status().is_success() {
        return Ok(Logon::Token(token_from(&resp.text().await?)?));
    }
    let status = resp.status().as_u16();
    let e = error_from(status, &resp.text().await.unwrap_or_default());
    if is_radius_challenge(&e) {
        return Ok(Logon::Challenge(e.message));
    }
    Err(anyhow::Error::new(e))
}

/// Hands the SAML response the identity provider gave the person to the PVWA for a session token.
pub async fn saml_logon(http: &reqwest::Client, root: &str, saml_response: &str) -> Result<String> {
    let resp = http
        .post(format!("{}/auth/SAML/Logon/", api(root)))
        .form(&[("concurrentSession", "true"), ("apiUse", "true"), ("SAMLResponse", saml_response)])
        .send()
        .await
        .map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    token_from(&resp.text().await?)
}

pub async fn logoff(http: &reqwest::Client, root: &str, auth: &str) -> Result<()> {
    let resp = http.post(format!("{}/Auth/Logoff/", api(root))).header("Authorization", auth).json(&json!({})).send().await.map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    Ok(())
}

/// Ends a CyberArk Identity session (Shared Services).
pub async fn identity_logout(http: &reqwest::Client, identity_root: &str, bearer: &str) -> Result<()> {
    let resp = http
        .post(format!("{identity_root}/Security/Logout"))
        .header("Authorization", bearer)
        .header("X-IDAP-NATIVE-CLIENT", "true")
        .json(&json!({}))
        .send()
        .await
        .map_err(reach(identity_root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    Ok(())
}

/// The signed-in Vault user's name ("Get logged on user details").
pub async fn whoami(http: &reqwest::Client, root: &str, auth: &str) -> Result<String> {
    let resp = http.get(format!("{root}/PasswordVault/WebServices/PIMServices.svc/User/")).header("Authorization", auth).send().await.map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    let v: Value = resp.json().await?;
    Ok(get_str(&v, "UserName"))
}

pub struct Page {
    pub accounts: Vec<Value>,
    /// How many accounts match in all, when the server says.
    pub total: Option<u64>,
}

/// Characters the Accounts search does not support in URL values.
fn search_words(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, '+' | '&' | '%')).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One page of the accounts the person may see. `search` is keywords (empty for all).
pub async fn accounts(http: &reqwest::Client, root: &str, auth: &str, search: &str, offset: usize, limit: usize) -> Result<Page> {
    let mut q: Vec<(&str, String)> = vec![("offset", offset.to_string()), ("limit", limit.to_string())];
    let words = search_words(search);
    if !words.is_empty() {
        q.push(("search", words));
    }
    let resp = http.get(with_query(&format!("{}/Accounts", api(root)), &q)?).header("Authorization", auth).send().await.map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    let v: Value = resp.json().await?;
    let accounts = get(&v, "value").and_then(|a| a.as_array()).cloned().unwrap_or_default();
    let total = get(&v, "count").and_then(|c| c.as_u64());
    Ok(Page { accounts, total })
}

/// Up to `cap` accounts, a page of `page` at a time; true when there were more than that.
pub async fn all_accounts(http: &reqwest::Client, root: &str, auth: &str, search: &str, page: usize, cap: usize) -> Result<(Vec<Value>, bool)> {
    let mut out: Vec<Value> = Vec::new();
    loop {
        let want = page.min(cap - out.len());
        let p = accounts(http, root, auth, search, out.len(), want).await?;
        let got = p.accounts.len();
        out.extend(p.accounts);
        let total = p.total.map(|t| t as usize);
        let more = match total {
            Some(t) => out.len() < t,
            None => got == want,
        };
        if !more || got == 0 {
            return Ok((out, false));
        }
        if out.len() >= cap {
            return Ok((out, true));
        }
    }
}

/// One account's details.
pub async fn account(http: &reqwest::Client, root: &str, auth: &str, id: &str) -> Result<Value> {
    let resp = http.get(format!("{}/Accounts/{id}/", api(root))).header("Authorization", auth).send().await.map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    Ok(resp.json().await?)
}

/// Why the person wants the account, when the safe or platform asks for it.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Access {
    pub reason: String,
    pub ticketing_system: String,
    pub ticket_id: String,
}

impl Access {
    fn put(&self, body: &mut Value) {
        if !self.reason.is_empty() {
            body["reason"] = Value::String(self.reason.clone());
        }
        if !self.ticketing_system.is_empty() {
            body["TicketingSystemName"] = Value::String(self.ticketing_system.clone());
        }
        if !self.ticket_id.is_empty() {
            body["TicketId"] = Value::String(self.ticket_id.clone());
        }
    }
}

/// What a refusal asks for before CyberArk hands the password out.
#[derive(Debug, PartialEq, Default, Clone, Copy)]
pub struct Need {
    pub reason: bool,
    pub ticket: bool,
}

/// The documentation says the password is not handed out without a reason when the Master Policy asks
/// for one, and that an error comes back, without naming it; ticketing integrations refuse in the same way.
/// So a refusal that talks about a reason or a ticket is taken as the question.
pub fn needs_access(e: &anyhow::Error) -> Option<Need> {
    let a = e.downcast_ref::<ApiError>()?;
    if a.unauthorised() {
        return None;
    }
    let m = a.message.to_lowercase();
    let need = Need { reason: m.contains("reason"), ticket: m.contains("ticket") };
    (need.reason || need.ticket).then_some(need)
}

/// The password of an account ("Get password value"), for connecting to `machine`.
pub async fn retrieve(http: &reqwest::Client, root: &str, auth: &str, id: &str, access: &Access, machine: &str) -> Result<String> {
    let mut body = json!({ "ActionType": "connect" });
    if !machine.is_empty() {
        body["Machine"] = Value::String(machine.to_string());
    }
    access.put(&mut body);
    let resp = http.post(format!("{}/Accounts/{id}/Password/Retrieve/", api(root))).header("Authorization", auth).json(&body).send().await.map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    let t = resp.text().await?;
    Ok(match serde_json::from_str::<Value>(t.trim()) {
        Ok(Value::String(s)) => s,
        _ => t,
    })
}

pub async fn check_in(http: &reqwest::Client, root: &str, auth: &str, id: &str) -> Result<()> {
    let resp = http.post(format!("{}/Accounts/{id}/CheckIn/", api(root))).header("Authorization", auth).json(&json!({})).send().await.map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    Ok(())
}

/// The access rules of a platform, from its Master Policy settings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rules {
    /// The person must give a reason (only "Get target platforms" says so).
    pub reason: Option<bool>,
    pub exclusive: bool,
    pub one_time: bool,
    pub dual_control: bool,
}

impl Rules {
    /// The account is checked out to the person once its password is retrieved.
    pub fn checks_out(&self) -> bool {
        self.exclusive || self.one_time
    }
}

fn active(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(o @ Value::Object(_)) => get(o, "IsActive").and_then(|b| b.as_bool()).unwrap_or(false),
        _ => false,
    }
}

/// "Get target platforms": {"Platforms": [{"PlatformID", "PrivilegedAccessWorkflows": {"...": {"IsActive"}}}]}.
pub fn parse_target_platforms(v: &Value) -> HashMap<String, Rules> {
    let mut out = HashMap::new();
    for p in get(v, "Platforms").and_then(|a| a.as_array()).cloned().unwrap_or_default() {
        let id = get_str(&p, "PlatformID");
        if id.is_empty() {
            continue;
        }
        let w = get(&p, "PrivilegedAccessWorkflows").cloned().unwrap_or(Value::Null);
        out.insert(
            id,
            Rules {
                reason: Some(active(get(&w, "RequireUsersToSpecifyReasonForAccess"))),
                exclusive: active(get(&w, "EnforceCheckinCheckoutExclusiveAccess")),
                one_time: active(get(&w, "EnforceOnetimePasswordAccess")),
                dual_control: active(get(&w, "RequireDualControlPasswordAccessApproval")),
            },
        );
    }
    out
}

/// "Get platforms": {"Platforms": [{"general": {"id"}, "privilegedAccessWorkflows": {"...": bool}}]}.
pub fn parse_platforms(v: &Value) -> HashMap<String, Rules> {
    let mut out = HashMap::new();
    for p in get(v, "Platforms").and_then(|a| a.as_array()).cloned().unwrap_or_default() {
        let id = get(&p, "general").map(|g| get_str(g, "id")).unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let w = get(&p, "privilegedAccessWorkflows").cloned().unwrap_or(Value::Null);
        out.insert(
            id,
            Rules {
                reason: None,
                exclusive: active(get(&w, "enforceCheckinCheckoutExclusiveAccess")),
                one_time: active(get(&w, "enforceOnetimePasswordAccess")),
                dual_control: active(get(&w, "requireDualControlPasswordAccessApproval")),
            },
        );
    }
    out
}

/// The access rules of every platform the person can read: the target platforms first (they include the
/// reason rule), else the general list. Either may be refused to people who are not administrators.
pub async fn platform_rules(http: &reqwest::Client, root: &str, auth: &str) -> Result<HashMap<String, Rules>> {
    let resp = http.get(format!("{}/Platforms/Targets/", api(root))).header("Authorization", auth).send().await.map_err(reach(root))?;
    if resp.status().is_success() {
        return Ok(parse_target_platforms(&resp.json().await?));
    }
    if resp.status().as_u16() == 401 {
        return Err(fail(resp).await);
    }
    let resp = http.get(format!("{}/Platforms/", api(root))).header("Authorization", auth).send().await.map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    Ok(parse_platforms(&resp.json().await?))
}

/// The ids of the accounts locked to the person (checked out), from the saved filter "Locked".
pub async fn locked(http: &reqwest::Client, root: &str, auth: &str) -> Result<Vec<String>> {
    let q = [("savedFilter", "Locked".to_string()), ("limit", "1000".to_string())];
    let resp = http.get(with_query(&format!("{}/Accounts", api(root)), &q)?).header("Authorization", auth).send().await.map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    let v: Value = resp.json().await?;
    Ok(get(&v, "value").and_then(|a| a.as_array()).map(|a| a.iter().map(|x| get_str(x, "id")).filter(|s| !s.is_empty()).collect()).unwrap_or_default())
}

pub enum PsmAnswer {
    /// The text of an RDP file.
    Rdp(String),
    /// The PVWA hands out HTML5 gateway connections instead, which Cablebag cannot use.
    Gateway,
}

/// The RDP file in a PSMConnect answer: raw text, a JSON string of it, or that string in base64.
pub fn rdp_text(body: &str) -> String {
    let t = body.trim();
    let s = match serde_json::from_str::<Value>(t) {
        Ok(Value::String(s)) => s,
        _ => t.to_string(),
    };
    if s.to_lowercase().contains("full address") {
        return s;
    }
    match B64.decode(s.trim()) {
        Ok(b) => String::from_utf8_lossy(&b).to_string(),
        Err(_) => s,
    }
}

/// Connects to an account through PSM ("Connect using PSM", PSMConnect) and returns the RDP file.
pub async fn psm_connect(http: &reqwest::Client, root: &str, auth: &str, id: &str, component: &str, access: &Access) -> Result<PsmAnswer> {
    let mut body = json!({ "ConnectionComponent": component });
    access.put(&mut body);
    let resp = http
        .post(format!("{}/Accounts/{id}/PSMConnect/", api(root)))
        .header("Authorization", auth)
        .header("Accept", "application/octet-stream")
        .json(&body)
        .send()
        .await
        .map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    let method = resp.headers().get("ConnectionMethod").and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
    let text = resp.text().await?;
    if method.eq_ignore_ascii_case("PSMGW") || text.contains("PSMGWURL") {
        return Ok(PsmAnswer::Gateway);
    }
    Ok(PsmAnswer::Rdp(rdp_text(&text)))
}

pub struct MfaKey {
    /// OpenSSH private key text.
    pub private_key: String,
    /// Unix time it stops working (0 when the server did not say).
    pub expires: i64,
}

/// Times in CyberArk answers come in seconds or milliseconds.
fn unix_seconds(v: Option<&Value>) -> i64 {
    let n = v.and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)).or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0);
    if n > 100_000_000_000 { n / 1000 } else { n }
}

/// The OpenSSH key of a "Generate an MFA caching SSH key" answer.
pub fn parse_mfa_key(v: &Value) -> Result<MfaKey> {
    let list = get(v, "value").and_then(|a| a.as_array()).cloned().unwrap_or_default();
    let key = list
        .iter()
        .find(|k| get_str(k, "format").eq_ignore_ascii_case("openssh"))
        .or_else(|| list.iter().find(|k| get_str(k, "privateKey").contains("OPENSSH PRIVATE KEY")))
        .map(|k| get_str(k, "privateKey"))
        .filter(|k| !k.is_empty())
        .ok_or_else(|| anyhow!("CyberArk sent no SSH key in OpenSSH format."))?;
    Ok(MfaKey { private_key: key, expires: unix_seconds(get(v, "expirationTime")) })
}

/// A short-lived SSH key PSM for SSH accepts for the signed-in person (MFA caching).
pub async fn mfa_key(http: &reqwest::Client, root: &str, auth: &str, key_password: Option<&str>) -> Result<MfaKey> {
    let mut body = json!({ "formats": ["OpenSSH"] });
    if let Some(p) = key_password {
        body["keyPassword"] = Value::String(p.to_string());
    }
    let resp = http.post(format!("{}/Users/Secret/SSHKeys/Cache/", api(root))).header("Authorization", auth).json(&body).send().await.map_err(reach(root))?;
    if !resp.status().is_success() {
        return Err(fail(resp).await);
    }
    parse_mfa_key(&resp.json().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, body_partial_json, body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn http() -> reqwest::Client {
        reqwest::Client::new()
    }

    #[tokio::test]
    async fn logon_for_each_password_method() {
        let server = MockServer::start().await;
        for m in ["Cyberark", "LDAP", "RADIUS", "Windows"] {
            Mock::given(method("POST"))
                .and(path(format!("/PasswordVault/API/auth/{m}/Logon/")))
                .and(body_json(json!({ "username": "jdoe", "password": "pw-test", "concurrentSession": true })))
                .respond_with(ResponseTemplate::new(200).set_body_string(format!("\"token-{m}\"")))
                .mount(&server)
                .await;
        }
        for m in ["Cyberark", "LDAP", "RADIUS", "Windows"] {
            match logon(&http(), &server.uri(), m, "jdoe", "pw-test").await.unwrap() {
                Logon::Token(t) => assert_eq!(t, format!("token-{m}")),
                Logon::Challenge(_) => panic!("no challenge expected"),
            }
        }
    }

    #[tokio::test]
    async fn a_wrong_password_reads_as_a_plain_sentence() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/PasswordVault/API/auth/Cyberark/Logon/"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "ErrorCode": "ITATS127E", "ErrorMessage": "Authentication failure for User [jdoe]." })))
            .mount(&server)
            .await;
        let e = match logon(&http(), &server.uri(), "Cyberark", "jdoe", "nope").await {
            Err(e) => e,
            Ok(_) => panic!("should fail"),
        };
        let text = e.to_string();
        assert!(text.contains("Authentication failure"));
        assert!(!text.contains("ITATS"));
        assert!(!is_unauthorised(&e));
    }

    #[tokio::test]
    async fn radius_challenge_comes_back_as_a_question() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/PasswordVault/API/auth/RADIUS/Logon/"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({ "ErrorCode": "ITATS542I", "ErrorMessage": "Enter the code from your token." })))
            .mount(&server)
            .await;
        match logon(&http(), &server.uri(), "RADIUS", "jdoe", "pw-test").await.unwrap() {
            Logon::Challenge(m) => assert_eq!(m, "Enter the code from your token."),
            Logon::Token(_) => panic!("expected a challenge"),
        }
    }

    #[tokio::test]
    async fn saml_logon_posts_the_response_as_a_form() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/PasswordVault/API/auth/SAML/Logon/"))
            .and(header("content-type", "application/x-www-form-urlencoded"))
            .and(body_string_contains("apiUse=true"))
            .and(body_string_contains("concurrentSession=true"))
            .and(body_string_contains("SAMLResponse=PHNhbWxwOlJlc3BvbnNl"))
            .respond_with(ResponseTemplate::new(200).set_body_string("\"saml-token\""))
            .mount(&server)
            .await;
        assert_eq!(saml_logon(&http(), &server.uri(), "PHNhbWxwOlJlc3BvbnNl").await.unwrap(), "saml-token");
    }

    #[test]
    fn tokens_in_every_documented_shape() {
        assert_eq!(token_from("\"abc\"").unwrap(), "abc");
        assert_eq!(token_from("{\"CyberArkLogonResult\":\"def\"}").unwrap(), "def");
        assert_eq!(token_from("ghi\n").unwrap(), "ghi");
        assert!(token_from("\"\"").is_err());
    }

    #[tokio::test]
    async fn accounts_are_read_page_by_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/PasswordVault/API/Accounts"))
            .and(query_param("offset", "0"))
            .and(header("Authorization", "tok"))
            .respond_with(ResponseTemplate::new(200).set_body_string(include_str!("../../../tests/cyberark/accounts_page1.json")))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/PasswordVault/API/Accounts"))
            .and(query_param("offset", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_string(include_str!("../../../tests/cyberark/accounts_page2.json")))
            .mount(&server)
            .await;
        let (all, capped) = all_accounts(&http(), &server.uri(), "tok", "", 2, 100).await.unwrap();
        assert_eq!(all.len(), 3);
        assert!(!capped);
        let (some, capped) = all_accounts(&http(), &server.uri(), "tok", "", 2, 2).await.unwrap();
        assert_eq!(some.len(), 2);
        assert!(capped);
    }

    #[tokio::test]
    async fn search_sends_keywords_without_unsupported_characters() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/PasswordVault/API/Accounts"))
            .and(query_param("search", "web01.example.com root"))
            .respond_with(ResponseTemplate::new(200).set_body_string(include_str!("../../../tests/cyberark/accounts_page2.json")))
            .mount(&server)
            .await;
        let p = accounts(&http(), &server.uri(), "tok", " web01.example.com + root& ", 0, 50).await.unwrap();
        assert_eq!(p.accounts.len(), 1);
        assert_eq!(p.total, Some(3));
    }

    #[tokio::test]
    async fn retrieve_without_and_with_a_reason() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/PasswordVault/API/Accounts/12_3/Password/Retrieve/"))
            .and(body_partial_json(json!({ "reason": "Patch night", "TicketingSystemName": "ServiceNow", "TicketId": "CHG0001" })))
            .respond_with(ResponseTemplate::new(200).set_body_string("\"pw-with-reason\""))
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/PasswordVault/API/Accounts/12_3/Password/Retrieve/"))
            .and(body_json(json!({ "ActionType": "connect", "Machine": "10.0.0.5" })))
            .respond_with(ResponseTemplate::new(200).set_body_string("\"pw-plain\""))
            .with_priority(2)
            .mount(&server)
            .await;
        let plain = retrieve(&http(), &server.uri(), "tok", "12_3", &Access::default(), "10.0.0.5").await.unwrap();
        assert_eq!(plain, "pw-plain");
        let a = Access { reason: "Patch night".into(), ticketing_system: "ServiceNow".into(), ticket_id: "CHG0001".into() };
        assert_eq!(retrieve(&http(), &server.uri(), "tok", "12_3", &a, "10.0.0.5").await.unwrap(), "pw-with-reason");
    }

    #[tokio::test]
    async fn a_missing_reason_is_recognised() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/PasswordVault/API/Accounts/12_3/Password/Retrieve/"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "ErrorCode": "PASWS999E", "ErrorMessage": "You must specify a reason to access this password." })))
            .mount(&server)
            .await;
        let e = retrieve(&http(), &server.uri(), "tok", "12_3", &Access::default(), "").await.unwrap_err();
        assert_eq!(needs_access(&e), Some(Need { reason: true, ticket: false }));
        assert!(!e.to_string().contains("PASWS"));

        let ticket = anyhow::Error::new(error_from(400, r#"{"ErrorCode":"X","ErrorMessage":"A valid ticket ID is required."}"#));
        assert_eq!(needs_access(&ticket), Some(Need { reason: false, ticket: true }));
        let expired = anyhow::Error::new(error_from(401, r#"{"ErrorMessage":"reason"}"#));
        assert_eq!(needs_access(&expired), None);
        assert!(is_unauthorised(&expired));
    }

    #[tokio::test]
    async fn check_in_and_log_off() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/PasswordVault/API/Accounts/12_3/CheckIn/")).and(header("Authorization", "tok")).respond_with(ResponseTemplate::new(200)).expect(1).mount(&server).await;
        Mock::given(method("POST")).and(path("/PasswordVault/API/Auth/Logoff/")).and(header("Authorization", "tok")).respond_with(ResponseTemplate::new(200).set_body_string("{}")).expect(1).mount(&server).await;
        check_in(&http(), &server.uri(), "tok", "12_3").await.unwrap();
        logoff(&http(), &server.uri(), "tok").await.unwrap();
    }

    #[test]
    fn platform_rules_from_both_lists() {
        let t: Value = serde_json::from_str(include_str!("../../../tests/cyberark/platforms_targets.json")).unwrap();
        let r = parse_target_platforms(&t);
        assert_eq!(r["WinServerLocal"], Rules { reason: Some(true), exclusive: true, one_time: false, dual_control: false });
        assert!(r["UnixSSH"].checks_out());
        let g = json!({ "Platforms": [{ "general": { "id": "WinDomain" }, "privilegedAccessWorkflows": { "requireDualControlPasswordAccessApproval": false, "enforceCheckinCheckoutExclusiveAccess": false, "enforceOnetimePasswordAccess": true } }], "Total": 1 });
        let r = parse_platforms(&g);
        assert_eq!(r["WinDomain"], Rules { reason: None, exclusive: false, one_time: true, dual_control: false });
    }

    #[tokio::test]
    async fn psm_connect_returns_the_rdp_file() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/PasswordVault/API/Accounts/12_3/PSMConnect/"))
            .and(body_partial_json(json!({ "ConnectionComponent": "PSM-RDP" })))
            .respond_with(ResponseTemplate::new(200).insert_header("ConnectionMethod", "RDPFile").set_body_string(include_str!("../../../tests/cyberark/psm_connect.rdp")))
            .mount(&server)
            .await;
        match psm_connect(&http(), &server.uri(), "tok", "12_3", "PSM-RDP", &Access::default()).await.unwrap() {
            PsmAnswer::Rdp(t) => assert!(t.contains("alternate shell:s:psm /u admin")),
            PsmAnswer::Gateway => panic!("expected an RDP file"),
        }
        let quoted = serde_json::to_string(include_str!("../../../tests/cyberark/psm_connect.rdp")).unwrap();
        assert!(rdp_text(&quoted).starts_with("full address:s:psm.example.com"));
        let b64 = B64.encode(include_str!("../../../tests/cyberark/psm_connect.rdp"));
        assert!(rdp_text(&b64).starts_with("full address:s:psm.example.com"));
    }

    #[tokio::test]
    async fn the_html5_gateway_answer_is_recognised() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/PasswordVault/API/Accounts/12_3/PSMConnect/"))
            .respond_with(ResponseTemplate::new(200).insert_header("ConnectionMethod", "PSMGW").set_body_json(json!({ "PSMGWURL": "https://gw.example.com/guac/direct", "PSMGWRequest": "e30=" })))
            .mount(&server)
            .await;
        assert!(matches!(psm_connect(&http(), &server.uri(), "tok", "12_3", "PSM-RDP", &Access::default()).await.unwrap(), PsmAnswer::Gateway));
    }

    #[tokio::test]
    async fn mfa_caching_key_in_openssh_format() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/PasswordVault/API/Users/Secret/SSHKeys/Cache/"))
            .and(body_json(json!({ "formats": ["OpenSSH"] })))
            .respond_with(ResponseTemplate::new(200).set_body_string(include_str!("../../../tests/cyberark/mfa_key.json")))
            .mount(&server)
            .await;
        let k = mfa_key(&http(), &server.uri(), "tok", None).await.unwrap();
        assert!(k.private_key.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert_eq!(k.expires, 1729003600);
        assert_eq!(unix_seconds(Some(&json!(1729003600000i64))), 1729003600);
    }

    #[tokio::test]
    async fn whoami_and_locked_accounts() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/PasswordVault/WebServices/PIMServices.svc/User/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "FirstName": "J", "LastName": "D", "UserName": "jdoe@example.com", "Source": "LDAP" })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/PasswordVault/API/Accounts"))
            .and(query_param("savedFilter", "Locked"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "value": [{ "id": "12_3" }], "count": 1 })))
            .mount(&server)
            .await;
        assert_eq!(whoami(&http(), &server.uri(), "tok").await.unwrap(), "jdoe@example.com");
        assert_eq!(locked(&http(), &server.uri(), "tok").await.unwrap(), vec!["12_3".to_string()]);
    }

    #[test]
    fn error_messages_lose_their_codes() {
        assert_eq!(error_from(400, r#"{"ErrorCode":"PASWS013E","ErrorMessage":"PASWS013E Missing mandatory parameter."}"#).message, "Missing mandatory parameter.");
        assert_eq!(error_from(500, "not json").to_string(), "The CyberArk server ran into a problem.");
        assert_eq!(error_from(429, "").to_string(), "CyberArk is busy at the moment. Try again in a little while.");
        assert_eq!(error_from(400, r#"{"Details":[{"ErrorMessage":"Bad one."}]}"#).message, "Bad one.");
    }
}

//! Vaultwarden (Bitwarden-compatible) vault, read-only.
//!
//! Cablebag logs in the way the Bitwarden apps do: the master password is stretched locally
//! (PBKDF2 or Argon2id), only a hash of it is sent, and the vault arrives encrypted. The user key
//! that unlocks it is kept in memory only, until the vault is locked or Cablebag closes. Item
//! passwords are decrypted one at a time, when a session needs them.

use crate::prompt::{button, field, PromptRequest};
use crate::AppState;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD as B64URL};
use base64::Engine;
use hmac::{Hmac, Mac};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager};

type HmacSha256 = Hmac<Sha256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// A symmetric Bitwarden key: 32 bytes for encryption, 32 for authentication.
#[derive(Clone)]
struct SymKey {
    enc: [u8; 32],
    mac: Option<[u8; 32]>,
}

impl SymKey {
    fn to_64(&self) -> Vec<u8> {
        let mut v = self.enc.to_vec();
        if let Some(m) = self.mac {
            v.extend_from_slice(&m);
        }
        v
    }

    fn from_64(b: &[u8]) -> Result<Self> {
        if b.len() != 64 {
            bail!("unexpected key length {}", b.len());
        }
        let mut enc = [0u8; 32];
        let mut mac = [0u8; 32];
        enc.copy_from_slice(&b[..32]);
        mac.copy_from_slice(&b[32..]);
        Ok(Self { enc, mac: Some(mac) })
    }
}

/// Looks a field up as camelCase or PascalCase; servers differ.
fn get<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    if let Some(x) = v.get(name) {
        return Some(x);
    }
    let mut c = name.chars();
    let pascal: String = match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => return None,
    };
    v.get(&pascal)
}

fn get_str(v: &Value, name: &str) -> Option<String> {
    get(v, name).and_then(|x| x.as_str()).map(str::to_string)
}

/// Decrypts a Bitwarden "cipher string" such as "2.iv|data|mac" with a symmetric key.
fn decrypt_sym(s: &str, key: &SymKey) -> Result<Vec<u8>> {
    let (t, rest) = s.split_once('.').ok_or_else(|| anyhow!("not an encrypted value"))?;
    let parts: Vec<&str> = rest.split('|').collect();
    match t {
        "2" => {
            if parts.len() < 3 {
                bail!("damaged encrypted value");
            }
            let iv = B64.decode(parts[0])?;
            let ct = B64.decode(parts[1])?;
            let mac = B64.decode(parts[2])?;
            let mac_key = key.mac.ok_or_else(|| anyhow!("the key has no authentication part"))?;
            let mut m = <HmacSha256 as Mac>::new_from_slice(&mac_key).map_err(|e| anyhow!("{e}"))?;
            m.update(&iv);
            m.update(&ct);
            m.verify_slice(&mac).map_err(|_| anyhow!("wrong key or damaged data"))?;
            aes_cbc(&key.enc, &iv, &ct)
        }
        "0" => {
            if parts.len() < 2 {
                bail!("damaged encrypted value");
            }
            aes_cbc(&key.enc, &B64.decode(parts[0])?, &B64.decode(parts[1])?)
        }
        other => bail!("unsupported encryption type {other}"),
    }
}

fn aes_cbc(key: &[u8; 32], iv: &[u8], ct: &[u8]) -> Result<Vec<u8>> {
    let dec = Aes256CbcDec::new_from_slices(key, iv).map_err(|e| anyhow!("{e}"))?;
    dec.decrypt_padded_vec_mut::<Pkcs7>(ct).map_err(|_| anyhow!("wrong key or damaged data"))
}

fn decrypt_text(s: &str, key: &SymKey) -> Result<String> {
    Ok(String::from_utf8(decrypt_sym(s, key)?)?)
}

/// Decrypts an RSA-wrapped organization key ("4.data" or "3.data").
fn decrypt_rsa(s: &str, private_der: &[u8]) -> Result<Vec<u8>> {
    use rsa::pkcs8::DecodePrivateKey;
    let (t, rest) = s.split_once('.').ok_or_else(|| anyhow!("not an encrypted value"))?;
    let data = B64.decode(rest.split('|').next().unwrap_or(""))?;
    let key = rsa::RsaPrivateKey::from_pkcs8_der(private_der).map_err(|e| anyhow!("private key: {e}"))?;
    let out = match t {
        "4" | "6" => key.decrypt(rsa::Oaep::new::<sha1::Sha1>(), &data),
        "3" | "5" => key.decrypt(rsa::Oaep::new::<sha2::Sha256>(), &data),
        other => bail!("unsupported key wrapping type {other}"),
    };
    out.map_err(|e| anyhow!("organization key: {e}"))
}

fn master_key(password: &str, email: &str, kdf: &Value) -> Result<[u8; 32]> {
    let kind = get(kdf, "kdf").and_then(|v| v.as_i64()).unwrap_or(0);
    let iterations = get(kdf, "kdfIterations").and_then(|v| v.as_u64()).unwrap_or(600_000) as u32;
    let mut out = [0u8; 32];
    match kind {
        0 => pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), email.as_bytes(), iterations, &mut out),
        1 => {
            let memory = get(kdf, "kdfMemory").and_then(|v| v.as_u64()).unwrap_or(64) as u32;
            let parallel = get(kdf, "kdfParallelism").and_then(|v| v.as_u64()).unwrap_or(4) as u32;
            let salt = Sha256::digest(email.as_bytes());
            let params = argon2::Params::new(memory * 1024, iterations, parallel, Some(32)).map_err(|e| anyhow!("{e}"))?;
            argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
                .hash_password_into(password.as_bytes(), &salt, &mut out)
                .map_err(|e| anyhow!("{e}"))?;
        }
        other => bail!("The vault uses an unknown key derivation ({other})."),
    }
    Ok(out)
}

fn stretch(master: &[u8; 32]) -> Result<SymKey> {
    let hk = hkdf::Hkdf::<Sha256>::from_prk(master).map_err(|e| anyhow!("{e}"))?;
    let mut enc = [0u8; 32];
    let mut mac = [0u8; 32];
    hk.expand(b"enc", &mut enc).map_err(|e| anyhow!("{e}"))?;
    hk.expand(b"mac", &mut mac).map_err(|e| anyhow!("{e}"))?;
    Ok(SymKey { enc, mac: Some(mac) })
}

fn password_hash(master: &[u8; 32], password: &str) -> String {
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(master, password.as_bytes(), 1, &mut out);
    B64.encode(out)
}

/// One login item, without its password.
#[derive(Clone, Serialize)]
pub struct VaultItem {
    pub id: String,
    pub name: String,
    pub username: String,
    pub uris: Vec<String>,
    pub folder: String,
    pub organization: String,
    pub has_password: bool,
}

struct ItemSecret {
    password: Option<String>,
    key: SymKey,
}

struct Unlocked {
    items: Vec<VaultItem>,
    secrets: HashMap<String, ItemSecret>,
    synced: i64,
    user_key: SymKey,
    access: String,
    server: String,
}

#[derive(Default)]
pub struct Vaultwarden {
    unlocked: Mutex<Option<Unlocked>>,
    /// One master password question at a time: logins that need the vault at the same moment wait for it.
    asking: tokio::sync::Mutex<()>,
}

#[derive(Serialize)]
pub struct VaultStatus {
    pub configured: bool,
    pub unlocked: bool,
    /// This PC keeps the vault key, so a restart does not need the master password.
    pub remembered: bool,
    /// Windows Hello (PIN, fingerprint or face) can be used here.
    pub hello: bool,
    pub server: String,
    pub email: String,
    pub items: usize,
    pub synced: i64,
}

fn setting(app: &AppHandle, key: &str) -> String {
    app.state::<AppState>()
        .store
        .lock()
        .setting(key)
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn set_setting(app: &AppHandle, key: &str, value: &str) {
    let _ = app.state::<AppState>().store.lock().set_setting(key, &Value::String(value.into()));
}

/// Small helpers to keep tokens encrypted at rest in Cablebag's own (DPAPI-protected) vault.
fn save_token(app: &AppHandle, label: &str, value: &str) {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    if let Ok((n, c)) = state.vault.encrypt(&store, value) {
        let _ = store.put_secret("token", label, "vaultwarden", "", 0, "", &n, &c);
    }
}

fn load_token(app: &AppHandle, label: &str) -> Option<String> {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    let (n, c) = store.find_secret("token", label, "vaultwarden", "", 0, "").ok().flatten()?;
    state.vault.decrypt(&store, &n, &c).ok()
}

/// The host part of a vault URI: "ssh://root@10.0.0.5:22/x" gives "10.0.0.5".
fn uri_host(uri: &str) -> String {
    let u = uri.trim().to_lowercase();
    let u = u.split_once("://").map(|(_, r)| r).unwrap_or(&u);
    let u = u.split(['/', '?', '#']).next().unwrap_or("");
    let u = u.rsplit_once('@').map(|(_, h)| h).unwrap_or(u);
    if let Some(rest) = u.strip_prefix('[') {
        return rest.split(']').next().unwrap_or("").to_string();
    }
    match u.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') && p.chars().all(|c| c.is_ascii_digit()) => h.to_string(),
        _ => u.to_string(),
    }
}

/// "DOMAIN\user" and "user@domain.tld" both compare as "user". "root@pam!token" (a Proxmox API token)
/// stays whole, so it never passes for the user root.
fn bare_user(u: &str) -> String {
    let u = u.trim().to_lowercase();
    let u = u.rsplit_once('\\').map(|(_, n)| n.to_string()).unwrap_or(u);
    match u.split_once('@') {
        Some((n, d)) if d.contains('.') && d.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-') => n.to_string(),
        _ => u,
    }
}

fn scheme(uri: &str) -> String {
    uri.trim().to_lowercase().split_once("://").map(|(s, _)| s.to_string()).unwrap_or_default()
}

/// Vault logins that belong to a host, best first: the address in the item's URIs, or the item named
/// after the host or the session. With a user name, logins for another user are left out.
fn rank(items: &[VaultItem], protocol: &str, host: &str, user: &str, name: &str) -> Vec<VaultItem> {
    rank_scored(items, protocol, host, user, name).into_iter().map(|(_, i)| i).collect()
}

/// rank, with each login's score: higher fits better.
fn rank_scored(items: &[VaultItem], protocol: &str, host: &str, user: &str, name: &str) -> Vec<(i32, VaultItem)> {
    let host = host.trim().to_lowercase();
    let name = name.trim().to_lowercase();
    let short = if host.parse::<std::net::IpAddr>().is_ok() { String::new() } else { host.split('.').next().unwrap_or("").to_string() };
    let want = bare_user(user);
    let mut scored: Vec<(i32, &VaultItem)> = items
        .iter()
        .filter(|i| i.has_password)
        .filter_map(|i| {
            let iname = i.name.trim().to_lowercase();
            let at_host: Vec<&String> = i.uris.iter().filter(|u| !host.is_empty() && uri_host(u) == host).collect();
            let mut score = if !at_host.is_empty() {
                // A website at the same address (http://host) is most likely not the login for SSH or RDP.
                if at_host.iter().all(|u| matches!(scheme(u).as_str(), "http" | "https")) { 3 } else { 6 }
            } else if !host.is_empty() && (iname == host || iname.split([' ', '(', ')', ',', '/']).any(|w| w == host)) {
                5
            } else if !name.is_empty() && iname == name {
                4
            } else if short.len() >= 3 && (iname == short || iname.split([' ', '(', ')', ',', '/', '.']).any(|w| w == short) || i.uris.iter().any(|u| uri_host(u).split('.').next() == Some(short.as_str()))) {
                3
            } else {
                return None;
            };
            // Items made for this kind of login: "ssh: pve-main (...)" or an ssh:// address.
            if iname.starts_with(&format!("{protocol}:")) || iname.starts_with(&format!("{protocol} ")) || i.uris.iter().any(|u| scheme(u) == protocol) {
                score += 2;
            }
            if !want.is_empty() {
                let have = bare_user(&i.username);
                if have == want {
                    score += 2;
                } else if !have.is_empty() {
                    return None;
                }
            }
            Some((score, i))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase())));
    scored.into_iter().map(|(s, i)| (s, i.clone())).collect()
}

/// Links a saved session to the vault login it logged in with, so the next connect does not search.
pub fn link_session(app: &AppHandle, session_id: &str, item_id: &str) {
    if session_id.is_empty() || item_id.is_empty() {
        return;
    }
    let state = app.state::<AppState>();
    {
        let store = state.store.lock();
        let Ok(Some(mut s)) = store.session(session_id) else { return };
        if !s.data.is_object() {
            s.data = json!({});
        }
        s.data["vault_item"] = Value::String(item_id.to_string());
        if store.save_session(&s).is_err() {
            return;
        }
    }
    let _ = app.emit("library-changed", ());
}

/// How the vault opens after Cablebag starts: "password" (the master password every time), "hello"
/// (Windows Hello, then the key kept on this PC) or "remember" (the kept key without a question).
fn unlock_mode(app: &AppHandle) -> String {
    match setting(app, "vault.unlock").as_str() {
        "" => "hello".into(),
        m => m.to_string(),
    }
}

/// Removes the vault key and login this PC kept.
pub fn forget(app: &AppHandle) {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    if let Ok(list) = store.list_secrets() {
        for s in list.iter().filter(|s| s.kind == "token" && s.protocol == "vaultwarden" && (s.label == "user_key" || s.label == "refresh")) {
            let _ = store.delete_secret(&s.id);
        }
    }
}

#[cfg(windows)]
mod hello {
    //! Windows Hello as a check before the kept vault key is used.
    use anyhow::Result;
    use windows::core::{factory, HSTRING};
    use windows::Security::Credentials::UI::{UserConsentVerificationResult as R, UserConsentVerifier, UserConsentVerifierAvailability as A};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;

    pub enum Outcome {
        Verified,
        Cancelled,
        Unavailable,
    }

    pub fn available() -> bool {
        UserConsentVerifier::CheckAvailabilityAsync().and_then(|op| op.join()).map(|a| a == A::Available).unwrap_or(false)
    }

    pub fn verify(hwnd: isize, message: &str) -> Result<Outcome> {
        if !available() {
            return Ok(Outcome::Unavailable);
        }
        // The window handle puts the Windows Hello dialog in front of Cablebag.
        let interop = factory::<UserConsentVerifier, IUserConsentVerifierInterop>()?;
        let op: windows_future::IAsyncOperation<R> = unsafe { interop.RequestVerificationForWindowAsync(HWND(hwnd as *mut _), &HSTRING::from(message))? };
        Ok(match op.join()? {
            R::Verified => Outcome::Verified,
            R::DeviceNotPresent | R::NotConfiguredForUser | R::DisabledByPolicy => Outcome::Unavailable,
            _ => Outcome::Cancelled,
        })
    }
}

#[cfg(not(windows))]
mod hello {
    pub enum Outcome {
        Verified,
        Cancelled,
        Unavailable,
    }
    pub fn available() -> bool {
        false
    }
    pub fn verify(_hwnd: isize, _message: &str) -> anyhow::Result<Outcome> {
        Ok(Outcome::Unavailable)
    }
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent("Cablebag").timeout(std::time::Duration::from_secs(30)).build()?)
}

fn device_id(app: &AppHandle) -> String {
    let id = setting(app, "vault.deviceId");
    if !id.is_empty() {
        return id;
    }
    let id = uuid::Uuid::new_v4().to_string();
    set_setting(app, "vault.deviceId", &id);
    id
}

async fn error_text(resp: reqwest::Response) -> String {
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    let msg = get_str(&body, "errorModel")
        .or_else(|| get(&body, "errorModel").and_then(|m| get_str(m, "message")))
        .or_else(|| get_str(&body, "error_description"))
        .or_else(|| get_str(&body, "message"))
        .unwrap_or_default();
    if msg.is_empty() { format!("The vault server answered {status}.") } else { msg }
}

impl Vaultwarden {
    pub fn status(&self, app: &AppHandle) -> VaultStatus {
        let u = self.unlocked.lock();
        VaultStatus {
            configured: !setting(app, "vault.server").is_empty() && !setting(app, "vault.email").is_empty(),
            unlocked: u.is_some(),
            remembered: load_token(app, "user_key").is_some(),
            hello: hello::available(),
            server: setting(app, "vault.server"),
            email: setting(app, "vault.email"),
            items: u.as_ref().map(|x| x.items.len()).unwrap_or(0),
            synced: u.as_ref().map(|x| x.synced).unwrap_or(0),
        }
    }

    /// Downloads the vault again with the current login; no master password needed.
    pub async fn sync(&self) -> Result<()> {
        let (server, access, key) = {
            let u = self.unlocked.lock();
            let u = u.as_ref().ok_or_else(|| anyhow!("The vault is locked."))?;
            (u.server.clone(), u.access.clone(), u.user_key.clone())
        };
        let resp = client()?.get(format!("{server}/api/sync?excludeDomains=true")).bearer_auth(&access).send().await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            bail!("The vault login has expired. Lock and unlock the vault to sync.");
        }
        let sync: Value = resp.error_for_status()?.json().await?;
        let fresh = decode_vault(&sync, &key, &access, &server)?;
        *self.unlocked.lock() = Some(fresh);
        Ok(())
    }

    pub fn lock(&self) {
        *self.unlocked.lock() = None;
    }

    pub fn items(&self) -> Vec<VaultItem> {
        self.unlocked.lock().as_ref().map(|u| u.items.clone()).unwrap_or_default()
    }

    /// Finds the vault login for a host that has none linked. The only match is used; several are offered
    /// in a list. Some((item id, item name, user, password)), or None when there is nothing (or the person
    /// prefers to type). Errors only when the person cancels.
    pub async fn pick_login(&self, app: &AppHandle, protocol: &str, host: &str, user: &str, name: &str) -> Result<Option<(String, String, String, String)>> {
        if setting(app, "vault.server").is_empty() || setting(app, "vault.email").is_empty() || host.trim().is_empty() {
            return Ok(None);
        }
        if self.unlocked.lock().is_none() && self.unlock_interactive(app).await.is_err() {
            return Ok(None);
        }
        let scored = rank_scored(&self.items(), protocol, host, user, name);
        let found: Vec<VaultItem> = scored.iter().map(|(_, i)| i.clone()).collect();
        // One login, or one that fits clearly better than the rest (made for this kind of login, or for
        // this user), is used without asking; logins that fit equally well are offered in a list.
        let clear = scored.len() == 1 || (scored.len() > 1 && scored[0].0 > scored[1].0);
        let chosen = match found.len() {
            0 => return Ok(None),
            _ if clear => found[0].clone(),
            _ => {
                let options: Vec<Value> = found
                    .iter()
                    .take(12)
                    .map(|i| {
                        let place = [i.organization.as_str(), i.folder.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" / ");
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
                            message: format!("The vault has {} logins that fit {host}. Which one?", found.len()),
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

    pub fn is_unlocked(&self) -> bool {
        self.unlocked.lock().is_some()
    }

    /// A key for one purpose derived from the account key: the same on every PC that unlocks this vault,
    /// and unknown to the vault server.
    pub fn derived_key(&self, purpose: &str) -> Option<[u8; 32]> {
        let u = self.unlocked.lock();
        let ikm = u.as_ref()?.user_key.to_64();
        // The app's former name: keys derived before the rename must stay the same.
        let hk = hkdf::Hkdf::<Sha256>::new(Some(b"TraxTerm"), &ikm);
        let mut out = [0u8; 32];
        hk.expand(purpose.as_bytes(), &mut out).ok()?;
        Some(out)
    }

    /// The logins pick_login would consider, without unlocking or reading secrets (empty when locked).
    pub fn candidates(&self, protocol: &str, host: &str, user: &str, name: &str) -> Vec<VaultItem> {
        rank(&self.items(), protocol, host, user, name)
    }

    /// The login of one item. Unlocks the vault first (asking for the master password) when needed.
    pub async fn login(&self, app: &AppHandle, item_id: &str) -> Result<(String, String)> {
        if self.unlocked.lock().is_none() {
            self.unlock_interactive(app).await?;
        }
        let u = self.unlocked.lock();
        let u = u.as_ref().ok_or_else(|| anyhow!("The vault is locked."))?;
        let item = u.items.iter().find(|i| i.id == item_id).ok_or_else(|| anyhow!("The vault item linked to this session no longer exists."))?;
        let secret = u.secrets.get(item_id).ok_or_else(|| anyhow!("vault item missing"))?;
        let password = match &secret.password {
            Some(enc) => decrypt_text(enc, &secret.key)?,
            None => String::new(),
        };
        Ok((item.username.clone(), password))
    }

    /// Every password of the open vault, so the assistant can hide them from what it sends; empty when locked.
    pub fn known_passwords(&self) -> Vec<String> {
        let u = self.unlocked.lock();
        let Some(u) = u.as_ref() else { return Vec::new() };
        u.secrets.values().filter_map(|s| s.password.as_ref().and_then(|enc| decrypt_text(enc, &s.key).ok())).collect()
    }

    /// Asks for the master password (and a two-step code when needed) and unlocks.
    pub async fn unlock_interactive(&self, app: &AppHandle) -> Result<()> {
        let _one = self.asking.lock().await;
        if self.unlocked.lock().is_some() {
            return Ok(());
        }
        // A key this PC kept from an earlier unlock opens the vault without the master password.
        if self.restore(app).await? {
            let _ = app.emit("vault-changed", ());
            return Ok(());
        }
        let state = app.state::<AppState>();
        let server = setting(app, "vault.server");
        let email = setting(app, "vault.email");
        if server.is_empty() || email.is_empty() {
            bail!("Set up the vault first under Settings, Vault.");
        }
        let mut message = format!("Master password for {email} on {}", server.trim_start_matches("https://"));
        for _ in 0..3 {
            let Some((pw, _)) = state.prompts.ask_secret(app, "Unlock the vault", &message, "Master password", false).await else {
                bail!("Vault unlock cancelled.");
            };
            match self.unlock(app, &pw).await {
                Ok(()) => {
                    // The window shows the vault state too; tell it the vault opened here.
                    let _ = app.emit("vault-changed", ());
                    return Ok(());
                }
                Err(e) if e.to_string().contains("Username or password is incorrect") || e.to_string().contains("invalid_grant") => {
                    message = "That master password was not accepted. Try again.".into();
                }
                Err(e) => return Err(e),
            }
        }
        bail!("The vault stayed locked.")
    }

    pub async fn unlock(&self, app: &AppHandle, password: &str) -> Result<()> {
        let server = setting(app, "vault.server").trim_end_matches('/').to_string();
        let email = setting(app, "vault.email").trim().to_lowercase();
        let http = client()?;

        let kdf: Value = http
            .post(format!("{server}/identity/accounts/prelogin"))
            .json(&json!({ "email": email }))
            .send()
            .await
            .with_context(|| format!("Could not reach {server}"))?
            .json()
            .await
            .context("Unexpected answer from the vault server")?;
        let master = tokio::task::spawn_blocking({
            let (pw, em, k) = (password.to_string(), email.clone(), kdf.clone());
            move || master_key(&pw, &em, &k)
        })
        .await??;
        let hash = password_hash(&master, password);
        let device = device_id(app);

        let mut form: Vec<(&str, String)> = vec![
            ("grant_type", "password".into()),
            ("username", email.clone()),
            ("password", hash),
            ("scope", "api offline_access".into()),
            ("client_id", "desktop".into()),
            ("deviceType", "6".into()),
            ("deviceIdentifier", device),
            ("deviceName", "Cablebag".into()),
        ];
        if let Some(remember) = load_token(app, "remember") {
            form.push(("twoFactorProvider", "5".into()));
            form.push(("twoFactorToken", remember));
        }

        let mut token = self.token(&http, &server, &email, &form).await;
        if let Err(TokenError::TwoFactor(providers)) = &token {
            // Remembered device was not accepted or there was none: ask for a code.
            form.retain(|(k, _)| *k != "twoFactorProvider" && *k != "twoFactorToken");
            let provider = if providers.contains(&0) {
                0
            } else if providers.contains(&1) {
                1
            } else {
                return Err(anyhow!("This vault account uses a two-step login Cablebag does not support yet."));
            };
            if provider == 1 {
                let _ = http
                    .post(format!("{server}/api/two-factor/send-email-login"))
                    .json(&json!({ "email": email, "masterPasswordHash": form.iter().find(|(k, _)| *k == "password").map(|(_, v)| v.clone()) }))
                    .send()
                    .await;
            }
            let state = app.state::<AppState>();
            let ans = state
                .prompts
                .ask(
                    app,
                    PromptRequest {
                        id: String::new(),
                        title: "Two-step login".into(),
                        message: if provider == 0 { "Enter the code from your authenticator app.".into() } else { "Enter the code the vault sent to your email.".into() },
                        details: vec![],
                        fields: vec![
                            field("code", "Code", "text", Value::String(String::new())),
                            field("remember", "Remember this PC", "checkbox", Value::Bool(true)),
                        ],
                        buttons: vec![button("ok", "Unlock", "primary"), button("cancel", "Cancel", "normal")],
                        warning: false,
                    },
                )
                .await;
            if ans.button != "ok" {
                bail!("Vault unlock cancelled.");
            }
            form.push(("twoFactorProvider", provider.to_string()));
            form.push(("twoFactorToken", ans.text("code").replace(' ', "")));
            form.push(("twoFactorRemember", if ans.flag("remember") { "1".into() } else { "0".into() }));
            token = self.token(&http, &server, &email, &form).await;
        }
        let token = match token {
            Ok(t) => t,
            Err(TokenError::TwoFactor(_)) => bail!("The two-step code was not accepted."),
            Err(TokenError::Other(e)) => return Err(e),
        };
        if let Some(r) = get_str(&token, "twoFactorToken") {
            save_token(app, "remember", &r);
        }

        let access = get_str(&token, "access_token").ok_or_else(|| anyhow!("The vault server sent no access token."))?;
        let protected_key = get_str(&token, "key").ok_or_else(|| anyhow!("The vault server sent no account key."))?;
        let stretched = stretch(&master)?;
        let user_key = if protected_key.starts_with("0.") {
            SymKey::from_64(&decrypt_sym(&protected_key, &SymKey { enc: master, mac: None })?)?
        } else {
            SymKey::from_64(&decrypt_sym(&protected_key, &stretched).map_err(|_| anyhow!("That master password does not open this vault."))?)?
        };

        let sync: Value = http
            .get(format!("{server}/api/sync?excludeDomains=true"))
            .bearer_auth(&access)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let unlocked = decode_vault(&sync, &user_key, &access, &server)?;
        *self.unlocked.lock() = Some(unlocked);
        if unlock_mode(app) != "password" {
            if let Some(refresh) = get_str(&token, "refresh_token") {
                save_token(app, "user_key", &hex::encode(user_key.to_64()));
                save_token(app, "refresh", &refresh);
            }
        }
        Ok(())
    }

    /// Opens the vault with the key kept on this PC: Windows Hello first when that is the choice, then
    /// a fresh login with the kept refresh token. False when there is nothing kept or it no longer works,
    /// so the master password is needed.
    async fn restore(&self, app: &AppHandle) -> Result<bool> {
        let mode = unlock_mode(app);
        if mode == "password" {
            forget(app);
            return Ok(false);
        }
        let (Some(key_hex), Some(refresh)) = (load_token(app, "user_key"), load_token(app, "refresh")) else {
            return Ok(false);
        };
        if mode == "hello" {
            let hwnd = app.get_webview_window("main").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize).unwrap_or(0);
            let email = setting(app, "vault.email");
            let outcome = tokio::task::spawn_blocking(move || hello::verify(hwnd, &format!("Unlock the Cablebag vault ({email})"))).await??;
            match outcome {
                hello::Outcome::Verified => {}
                hello::Outcome::Cancelled => bail!("Vault unlock cancelled."),
                // No PIN or fingerprint set up (any more): fall back to the master password.
                hello::Outcome::Unavailable => return Ok(false),
            }
        }
        let user_key = SymKey::from_64(&hex::decode(key_hex)?)?;
        let server = setting(app, "vault.server").trim_end_matches('/').to_string();
        let http = client()?;
        let resp = http
            .post(format!("{server}/identity/connect/token"))
            .form(&[("grant_type", "refresh_token"), ("client_id", "desktop"), ("refresh_token", refresh.as_str())])
            .send()
            .await
            .with_context(|| format!("Could not reach {server}"))?;
        if !resp.status().is_success() {
            // Signed out on the server, or the login expired: start over with the master password.
            forget(app);
            return Ok(false);
        }
        let token: Value = resp.json().await?;
        let access = get_str(&token, "access_token").ok_or_else(|| anyhow!("The vault server sent no access token."))?;
        if let Some(r) = get_str(&token, "refresh_token") {
            save_token(app, "refresh", &r);
        }
        let sync: Value = http
            .get(format!("{server}/api/sync?excludeDomains=true"))
            .bearer_auth(&access)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let unlocked = decode_vault(&sync, &user_key, &access, &server).map_err(|_| {
            forget(app);
            anyhow!("The key this PC kept no longer opens the vault; unlock it with the master password.")
        })?;
        *self.unlocked.lock() = Some(unlocked);
        Ok(true)
    }

    async fn token(&self, http: &reqwest::Client, server: &str, email: &str, form: &[(&str, String)]) -> Result<Value, TokenError> {
        let resp = http
            .post(format!("{server}/identity/connect/token"))
            .header("Auth-Email", B64URL.encode(email))
            .header("Device-Type", "6")
            .form(form)
            .send()
            .await
            .map_err(|e| TokenError::Other(anyhow!("Could not reach the vault server: {e}")))?;
        if resp.status().is_success() {
            return resp.json().await.map_err(|e| TokenError::Other(anyhow!("{e}")));
        }
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        if let Some(p) = get(&body, "twoFactorProviders").and_then(|v| v.as_array()) {
            let list: Vec<i64> = p
                .iter()
                .filter_map(|x| x.as_i64().or_else(|| x.as_str().and_then(|s| s.parse().ok())))
                .collect();
            return Err(TokenError::TwoFactor(list));
        }
        let msg = get_str(&body, "error_description")
            .or_else(|| get(&body, "errorModel").and_then(|m| get_str(m, "message")))
            .unwrap_or_else(|| format!("The vault server answered {status}."));
        Err(TokenError::Other(anyhow!(msg)))
    }
}

enum TokenError {
    TwoFactor(Vec<i64>),
    Other(anyhow::Error),
}

fn decode_vault(sync: &Value, user_key: &SymKey, access: &str, server: &str) -> Result<Unlocked> {
    let profile = get(sync, "profile").cloned().unwrap_or(Value::Null);
    let private_der = get_str(&profile, "privateKey")
        .and_then(|pk| decrypt_sym(&pk, user_key).ok());

    let mut org_keys: HashMap<String, SymKey> = HashMap::new();
    let mut org_names: HashMap<String, String> = HashMap::new();
    if let Some(orgs) = get(&profile, "organizations").and_then(|v| v.as_array()) {
        for o in orgs {
            let (Some(id), Some(k)) = (get_str(o, "id"), get_str(o, "key")) else { continue };
            org_names.insert(id.clone(), get_str(o, "name").unwrap_or_default());
            if let Some(der) = &private_der {
                if let Ok(raw) = decrypt_rsa(&k, der) {
                    if let Ok(key) = SymKey::from_64(&raw) {
                        org_keys.insert(id, key);
                    }
                }
            }
        }
    }

    let mut folders: HashMap<String, String> = HashMap::new();
    if let Some(list) = get(sync, "folders").and_then(|v| v.as_array()) {
        for f in list {
            if let (Some(id), Some(n)) = (get_str(f, "id"), get_str(f, "name")) {
                folders.insert(id, decrypt_text(&n, user_key).unwrap_or_default());
            }
        }
    }

    let mut items = Vec::new();
    let mut secrets = HashMap::new();
    for c in get(sync, "ciphers").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        if get(&c, "type").and_then(|v| v.as_i64()) != Some(1) {
            continue;
        }
        if get(&c, "deletedDate").map(|v| !v.is_null()).unwrap_or(false) {
            continue;
        }
        let org = get_str(&c, "organizationId");
        let base = match &org {
            Some(o) => match org_keys.get(o) {
                Some(k) => k.clone(),
                None => continue,
            },
            None => user_key.clone(),
        };
        let key = match get_str(&c, "key") {
            Some(k) => match decrypt_sym(&k, &base).and_then(|raw| SymKey::from_64(&raw)) {
                Ok(k) => k,
                Err(_) => continue,
            },
            None => base,
        };
        let Some(id) = get_str(&c, "id") else { continue };
        let dec = |v: Option<String>| v.and_then(|s| decrypt_text(&s, &key).ok()).unwrap_or_default();
        let login = get(&c, "login").cloned().unwrap_or(Value::Null);
        let uris = get(&login, "uris")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|u| get_str(u, "uri")).map(|u| decrypt_text(&u, &key).unwrap_or_default()).filter(|u| !u.is_empty()).collect())
            .unwrap_or_default();
        let password = get_str(&login, "password");
        items.push(VaultItem {
            id: id.clone(),
            name: dec(get_str(&c, "name")),
            username: dec(get_str(&login, "username")),
            uris,
            folder: get_str(&c, "folderId").and_then(|f| folders.get(&f).cloned()).unwrap_or_default(),
            organization: org.as_ref().and_then(|o| org_names.get(o).cloned()).unwrap_or_default(),
            has_password: password.is_some(),
        });
        secrets.insert(id, ItemSecret { password, key });
    }
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(Unlocked { items, secrets, synced: crate::store::now(), user_key: user_key.clone(), access: access.to_string(), server: server.to_string() })
}

#[allow(dead_code)]
pub async fn check_server(server: &str) -> Result<()> {
    let http = client()?;
    let resp = http.get(format!("{}/api/config", server.trim_end_matches('/'))).send().await?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(anyhow!(error_text(resp).await))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::BlockEncryptMut;

    type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

    fn encrypt(plain: &[u8], key: &SymKey, iv: [u8; 16]) -> String {
        let ct = Aes256CbcEnc::new_from_slices(&key.enc, &iv).unwrap().encrypt_padded_vec_mut::<Pkcs7>(plain);
        let mut m = <HmacSha256 as Mac>::new_from_slice(&key.mac.unwrap()).unwrap();
        m.update(&iv);
        m.update(&ct);
        format!("2.{}|{}|{}", B64.encode(iv), B64.encode(&ct), B64.encode(m.finalize().into_bytes()))
    }

    #[test]
    fn cipher_string_round_trip_and_tamper_check() {
        let key = SymKey { enc: [7u8; 32], mac: Some([9u8; 32]) };
        let s = encrypt(b"correct horse", &key, [3u8; 16]);
        assert_eq!(decrypt_text(&s, &key).unwrap(), "correct horse");
        let wrong = SymKey { enc: [7u8; 32], mac: Some([8u8; 32]) };
        assert!(decrypt_text(&s, &wrong).is_err());
    }

    /// The documented Bitwarden derivation for a PBKDF2 account:
    /// master key = PBKDF2-SHA256(password, lowercase email, iterations), login hash = PBKDF2-SHA256(master key, password, 1).
    #[test]
    fn pbkdf2_master_key_and_login_hash_match_reference() {
        let kdf = json!({ "kdf": 0, "kdfIterations": 5000 });
        let mk = master_key("p4ssw0rd", "user@example.com", &kdf).unwrap();
        assert_eq!(hex::encode(mk), include_str!("../tests/bw_pbkdf2_master.hex").trim());
        assert_eq!(password_hash(&mk, "p4ssw0rd"), include_str!("../tests/bw_pbkdf2_hash.txt").trim());
        let st = stretch(&mk).unwrap();
        assert_eq!(hex::encode(st.enc), include_str!("../tests/bw_stretch_enc.hex").trim());
        assert_eq!(hex::encode(st.mac.unwrap()), include_str!("../tests/bw_stretch_mac.hex").trim());
    }

    #[test]
    fn argon2_master_key_matches_reference() {
        let kdf = json!({ "Kdf": 1, "KdfIterations": 3, "KdfMemory": 16, "KdfParallelism": 2 });
        let mk = master_key("p4ssw0rd", "user@example.com", &kdf).unwrap();
        assert_eq!(hex::encode(mk), include_str!("../tests/bw_argon2_master.hex").trim());
    }
}

#[cfg(test)]
mod match_tests {
    use super::*;

    fn item(id: &str, name: &str, user: &str, uris: &[&str]) -> VaultItem {
        VaultItem { id: id.into(), name: name.into(), username: user.into(), uris: uris.iter().map(|s| s.to_string()).collect(), folder: String::new(), organization: String::new(), has_password: true }
    }

    #[test]
    fn hosts_from_uris() {
        assert_eq!(uri_host("ssh://root@10.0.0.5:22/x"), "10.0.0.5");
        assert_eq!(uri_host("https://Proxmox2.example.net"), "proxmox2.example.net");
        assert_eq!(uri_host("10.20.30.53:8006"), "10.20.30.53");
        assert_eq!(uri_host("rdp://[fe80::1]:3389"), "fe80::1");
    }

    #[test]
    fn users() {
        assert_eq!(bare_user("CORP\\admin.jan"), "admin.jan");
        assert_eq!(bare_user("admin.jan@corp.example"), "admin.jan");
        assert_eq!(bare_user("root@pam!dashboard-readonly"), "root@pam!dashboard-readonly");
    }

    #[test]
    fn ranking() {
        let items = vec![
            item("a", "pve root", "root", &["https://10.20.30.53:8006"]),
            item("b", "dc01", "CORP\\admin.jan", &[]),
            item("c", "something else", "root", &["https://example.com"]),
            item("d", "docker01", "jan", &["ssh://10.20.30.54"]),
            item("e", "docker01 old", "root", &["ssh://10.20.30.54"]),
        ];
        let ids = |v: Vec<VaultItem>| v.into_iter().map(|i| i.id).collect::<Vec<_>>();
        assert_eq!(ids(rank(&items, "ssh", "10.20.30.53", "root", "")), vec!["a"]);
        assert_eq!(ids(rank(&items, "rdp", "dc01.corp.example", "admin.jan", "")), vec!["b"]);
        assert_eq!(ids(rank(&items, "ssh", "10.20.30.54", "jan", "")), vec!["d"]);
        assert_eq!(ids(rank(&items, "ssh", "10.20.30.54", "", "")), vec!["d", "e"]);
        assert_eq!(ids(rank(&items, "ssh", "10.9.9.9", "", "docker01")), vec!["d"]);
        assert!(rank(&items, "ssh", "10.9.9.9", "root", "").is_empty());
        // The pve case from the real vault: the SSH login wins; the API token and the website do not count.
        let pve = vec![
            item("web", "http://10.20.30.200", "jan", &["http://10.20.30.200"]),
            item("tok", "proxmox-dashboard", "root@pam!dashboard-readonly", &["https://10.20.30.200:8006"]),
            item("ssh", "ssh: pve-main (10.20.30.200)", "root", &["10.20.30.200"]),
        ];
        assert_eq!(ids(rank(&pve, "ssh", "10.20.30.200", "root", "")), vec!["ssh"]);
        assert_eq!(ids(rank(&pve, "ssh", "10.20.30.200", "", ""))[0], "ssh");
        // The test case that asked needlessly: a Telnet and an FTP login for the same host and user.
        let same = vec![
            item("t", "telnet: test (10.0.0.9)", "ttest", &["telnet://10.0.0.9:2323"]),
            item("f", "ftp: test (10.0.0.9)", "ttest", &["ftp://10.0.0.9:2121"]),
        ];
        let s = rank_scored(&same, "telnet", "10.0.0.9", "ttest", "");
        assert_eq!(s[0].1.id, "t");
        assert!(s[0].0 > s[1].0, "the Telnet login fits clearly better, so no list");
    }
}

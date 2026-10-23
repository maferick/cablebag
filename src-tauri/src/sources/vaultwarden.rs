//! Vaultwarden (Bitwarden-compatible) vault, read-only: the "vaultwarden" kind of credential source.
//!
//! Cablebag logs in the way the Bitwarden apps do: the master password is stretched locally
//! (PBKDF2 or Argon2id), only a hash of it is sent, and the vault arrives encrypted. The user key
//! that unlocks it is kept in memory only, until the vault is locked or Cablebag closes. Item
//! passwords are decrypted one at a time, when a session needs them.

use super::{Item, SourceConfig, SourceStatus};
use crate::prompt::{button, field, PromptRequest};
use crate::AppState;
use aes::cipher::{block_padding::Pkcs7, BlockModeDecrypt, KeyIvInit};
use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD as B64URL};
use base64::Engine;
use hmac::{Hmac, Mac};
use parking_lot::Mutex;
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
            let mut m = <HmacSha256 as hmac::KeyInit>::new_from_slice(&mac_key).map_err(|e| anyhow!("{e}"))?;
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
    dec.decrypt_padded_vec::<Pkcs7>(ct).map_err(|_| anyhow!("wrong key or damaged data"))
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
        "4" | "6" => key.decrypt(rsa::Oaep::new::<rsa_sha1::Sha1>(), &data),
        "3" | "5" => key.decrypt(rsa::Oaep::new::<rsa_sha2::Sha256>(), &data),
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

struct ItemSecret {
    password: Option<String>,
    key: SymKey,
}

struct Unlocked {
    items: Vec<Item>,
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

/// Small helpers to keep tokens encrypted at rest in Cablebag's own (DPAPI-protected) vault, one set per
/// source. The first vault (from before there were several sources) keeps its tokens where they were.
fn token_host(cfg: &SourceConfig) -> &str {
    if cfg.id == super::LEGACY { "" } else { &cfg.id }
}

fn save_token(app: &AppHandle, cfg: &SourceConfig, label: &str, value: &str) {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    if let Ok((n, c)) = state.vault.encrypt(&store, value) {
        let _ = store.put_secret("token", label, "vaultwarden", token_host(cfg), 0, "", &n, &c);
    }
}

fn load_token(app: &AppHandle, cfg: &SourceConfig, label: &str) -> Option<String> {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    let (n, c) = store.find_secret("token", label, "vaultwarden", token_host(cfg), 0, "").ok().flatten()?;
    state.vault.decrypt(&store, &n, &c).ok()
}

fn server_of(cfg: &SourceConfig) -> String {
    cfg.url.trim().trim_end_matches('/').to_string()
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
pub fn forget(app: &AppHandle, cfg: &SourceConfig) {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    if let Ok(list) = store.list_secrets() {
        for s in list.iter().filter(|s| s.kind == "token" && s.protocol == "vaultwarden" && s.host == token_host(cfg) && (s.label == "user_key" || s.label == "refresh")) {
            let _ = store.delete_secret(&s.id);
        }
    }
}

/// Whether Windows Hello (PIN, fingerprint or face) can be used on this PC.
pub fn hello_available() -> bool {
    hello::available()
}

/// How a source opens after Cablebag starts, for every source that can keep its sign-in: "password" (ask
/// every time, keep nothing), "hello" (Windows Hello, then what this PC kept) or "remember" (without asking).
pub fn keep_mode(app: &AppHandle) -> String {
    unlock_mode(app)
}

/// Asks Windows Hello to confirm it is you before something this PC kept is used. Ok(true) when confirmed,
/// Ok(false) when Hello is not set up here, an error when the person cancels.
pub async fn hello_confirm(app: &AppHandle, message: &str) -> Result<bool> {
    #[cfg(windows)]
    let hwnd = app.get_webview_window("main").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize).unwrap_or(0);
    // No Hello on other platforms: the stub verify gets no window and answers Unavailable.
    #[cfg(not(windows))]
    let hwnd = {
        let _ = app;
        0isize
    };
    let message = message.to_string();
    match tokio::task::spawn_blocking(move || hello::verify(hwnd, &message)).await?? {
        hello::Outcome::Verified => Ok(true),
        hello::Outcome::Cancelled => bail!("Unlock cancelled."),
        hello::Outcome::Unavailable => Ok(false),
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
    pub fn status(&self, app: &AppHandle, cfg: &SourceConfig) -> SourceStatus {
        let u = self.unlocked.lock();
        SourceStatus {
            configured: !cfg.url.trim().is_empty() && !cfg.account.trim().is_empty(),
            unlocked: u.is_some(),
            remembered: load_token(app, cfg, "user_key").is_some(),
            items: u.as_ref().map(|x| x.items.len()).unwrap_or(0),
            synced: u.as_ref().map(|x| x.synced).unwrap_or(0),
            ..Default::default()
        }
    }

    /// Downloads the vault again with the current login; no master password needed.
    pub async fn sync(&self, _app: &AppHandle, _cfg: &SourceConfig) -> Result<()> {
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

    pub async fn lock(&self, _app: &AppHandle, _cfg: &SourceConfig) {
        *self.unlocked.lock() = None;
    }

    pub fn items(&self) -> Vec<Item> {
        self.unlocked.lock().as_ref().map(|u| u.items.clone()).unwrap_or_default()
    }

    /// The whole vault is in memory, so searching is filtering.
    pub async fn search(&self, _app: &AppHandle, _cfg: &SourceConfig, query: &str) -> Result<Vec<Item>> {
        Ok(super::filter(self.items(), query))
    }

    /// Every login is a candidate; the ranking picks.
    pub async fn find(&self, _app: &AppHandle, _cfg: &SourceConfig, _host: &str) -> Result<Vec<Item>> {
        Ok(self.items())
    }

    /// Vaultwarden has nothing to check out.
    pub async fn check_in(&self, _app: &AppHandle, _cfg: &SourceConfig, _item: &str) -> Result<()> {
        Ok(())
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

    /// The login of one item. Unlocks the vault first (asking for the master password) when needed.
    pub async fn login(&self, app: &AppHandle, cfg: &SourceConfig, item_id: &str) -> Result<(String, String)> {
        if self.unlocked.lock().is_none() {
            self.unlock_interactive(app, cfg).await?;
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
    pub async fn unlock_interactive(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<()> {
        let _one = self.asking.lock().await;
        if self.unlocked.lock().is_some() {
            return Ok(());
        }
        // A key this PC kept from an earlier unlock opens the vault without the master password.
        if self.restore(app, cfg).await? {
            let _ = app.emit("vault-changed", ());
            return Ok(());
        }
        let state = app.state::<AppState>();
        let server = server_of(cfg);
        let email = cfg.account.trim().to_string();
        if server.is_empty() || email.is_empty() {
            bail!("Set up {} first under Settings, Credential sources.", cfg.name);
        }
        let mut message = format!("Master password for {email} on {}", server.trim_start_matches("https://"));
        let title = format!("Unlock {}", cfg.name);
        for _ in 0..3 {
            let Some((pw, _)) = state.prompts.ask_secret(app, &title, &message, "Master password", false).await else {
                bail!("Vault unlock cancelled.");
            };
            match self.unlock(app, cfg, &pw).await {
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

    pub async fn unlock(&self, app: &AppHandle, cfg: &SourceConfig, password: &str) -> Result<()> {
        let server = server_of(cfg);
        let email = cfg.account.trim().to_lowercase();
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
        if let Some(remember) = load_token(app, cfg, "remember") {
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
            save_token(app, cfg, "remember", &r);
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
                save_token(app, cfg, "user_key", &hex::encode(user_key.to_64()));
                save_token(app, cfg, "refresh", &refresh);
            }
        }
        Ok(())
    }

    /// Opens the vault with the key kept on this PC: Windows Hello first when that is the choice, then
    /// a fresh login with the kept refresh token. False when there is nothing kept or it no longer works,
    /// so the master password is needed.
    async fn restore(&self, app: &AppHandle, cfg: &SourceConfig) -> Result<bool> {
        let mode = unlock_mode(app);
        if mode == "password" {
            forget(app, cfg);
            return Ok(false);
        }
        let (Some(key_hex), Some(refresh)) = (load_token(app, cfg, "user_key"), load_token(app, cfg, "refresh")) else {
            return Ok(false);
        };
        if mode == "hello" {
            let what = format!("{} ({})", cfg.name, cfg.account.trim());
            // No PIN or fingerprint set up (any more): fall back to the master password.
            if !hello_confirm(app, &format!("Unlock {what} in Cablebag")).await? {
                return Ok(false);
            }
        }
        let user_key = SymKey::from_64(&hex::decode(key_hex)?)?;
        let server = server_of(cfg);
        let http = client()?;
        let resp = http
            .post(format!("{server}/identity/connect/token"))
            .form(&[("grant_type", "refresh_token"), ("client_id", "desktop"), ("refresh_token", refresh.as_str())])
            .send()
            .await
            .with_context(|| format!("Could not reach {server}"))?;
        if !resp.status().is_success() {
            // Signed out on the server, or the login expired: start over with the master password.
            forget(app, cfg);
            return Ok(false);
        }
        let token: Value = resp.json().await?;
        let access = get_str(&token, "access_token").ok_or_else(|| anyhow!("The vault server sent no access token."))?;
        if let Some(r) = get_str(&token, "refresh_token") {
            save_token(app, cfg, "refresh", &r);
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
            forget(app, cfg);
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
        items.push(Item {
            id: id.clone(),
            source: String::new(),
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
    use aes::cipher::BlockModeEncrypt;

    type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

    fn encrypt(plain: &[u8], key: &SymKey, iv: [u8; 16]) -> String {
        let ct = Aes256CbcEnc::new_from_slices(&key.enc, &iv).unwrap().encrypt_padded_vec::<Pkcs7>(plain);
        let mut m = <HmacSha256 as hmac::KeyInit>::new_from_slice(&key.mac.unwrap()).unwrap();
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
        assert_eq!(hex::encode(mk), include_str!("../../tests/bw_pbkdf2_master.hex").trim());
        assert_eq!(password_hash(&mk, "p4ssw0rd"), include_str!("../../tests/bw_pbkdf2_hash.txt").trim());
        let st = stretch(&mk).unwrap();
        assert_eq!(hex::encode(st.enc), include_str!("../../tests/bw_stretch_enc.hex").trim());
        assert_eq!(hex::encode(st.mac.unwrap()), include_str!("../../tests/bw_stretch_mac.hex").trim());
    }

    #[test]
    fn argon2_master_key_matches_reference() {
        let kdf = json!({ "Kdf": 1, "KdfIterations": 3, "KdfMemory": 16, "KdfParallelism": 2 });
        let mk = master_key("p4ssw0rd", "user@example.com", &kdf).unwrap();
        assert_eq!(hex::encode(mk), include_str!("../../tests/bw_argon2_master.hex").trim());
    }
}

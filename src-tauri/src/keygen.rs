//! SSH key generator: create a key, show its public half, save it, and install it on a server.

use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use serde::Serialize;
use ssh_key::{Algorithm, EcdsaCurve, HashAlg, LineEnding, PrivateKey};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default)]
pub struct Keys {
    /// Generated or loaded keys, kept in memory until saved or forgotten.
    keys: Mutex<HashMap<String, PrivateKey>>,
}

#[derive(Serialize)]
pub struct KeyInfo {
    pub id: String,
    pub algorithm: String,
    pub comment: String,
    pub public_key: String,
    pub fingerprint: String,
}

fn info(id: &str, k: &PrivateKey) -> KeyInfo {
    KeyInfo {
        id: id.to_string(),
        algorithm: k.algorithm().as_str().to_string(),
        comment: k.comment().to_string(),
        public_key: k.public_key().to_openssh().unwrap_or_default(),
        fingerprint: k.public_key().fingerprint(HashAlg::Sha256).to_string(),
    }
}

impl Keys {
    pub fn generate(&self, kind: &str, comment: &str) -> Result<KeyInfo> {
        let mut rng = rand::rng();
        let mut key = match kind {
            "ed25519" => PrivateKey::random(&mut rng, Algorithm::Ed25519),
            "ecdsa-256" => PrivateKey::random(&mut rng, Algorithm::Ecdsa { curve: EcdsaCurve::NistP256 }),
            "ecdsa-384" => PrivateKey::random(&mut rng, Algorithm::Ecdsa { curve: EcdsaCurve::NistP384 }),
            "rsa-3072" | "rsa-4096" => {
                let bits = if kind == "rsa-4096" { 4096 } else { 3072 };
                let pair = ssh_key::private::RsaKeypair::random(&mut rng, bits).map_err(|e| anyhow!("{e}"))?;
                PrivateKey::new(ssh_key::private::KeypairData::Rsa(pair), "")
            }
            other => bail!("Unknown key type {other}."),
        }
        .map_err(|e| anyhow!("Key generation failed: {e}"))?;
        key.set_comment(comment);
        let id = uuid::Uuid::new_v4().to_string();
        let i = info(&id, &key);
        self.keys.lock().insert(id, key);
        Ok(i)
    }

    /// Loads an existing key (OpenSSH or PuTTY format) to show it or save it in OpenSSH format.
    pub fn load(&self, path: &str, passphrase: Option<&str>) -> Result<KeyInfo> {
        let key = russh::keys::load_secret_key(path, passphrase.filter(|p| !p.is_empty())).map_err(|e| match e {
            russh::keys::Error::KeyIsEncrypted => anyhow!("This key has a passphrase; enter it to open the key."),
            e => anyhow!("Could not read the key: {e}"),
        })?;
        let id = uuid::Uuid::new_v4().to_string();
        let i = info(&id, &key);
        self.keys.lock().insert(id, key);
        Ok(i)
    }

    /// Writes the private key in OpenSSH format (encrypted when a passphrase is given) and the
    /// public key next to it with ".pub" appended.
    pub fn save(&self, id: &str, path: &str, passphrase: &str, comment: &str) -> Result<(String, String)> {
        let mut key = self.keys.lock().get(id).cloned().ok_or_else(|| anyhow!("That key is no longer open."))?;
        key.set_comment(comment);
        let out = if passphrase.is_empty() {
            key.clone()
        } else {
            key.encrypt(&mut rand::rng(), passphrase).map_err(|e| anyhow!("Encryption failed: {e}"))?
        };
        let pem = out.to_openssh(LineEnding::LF).map_err(|e| anyhow!("{e}"))?;
        let private = PathBuf::from(path);
        if let Some(dir) = private.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&private, pem.as_bytes())?;
        let public = PathBuf::from(format!("{path}.pub"));
        std::fs::write(&public, format!("{}\n", key.public_key().to_openssh().map_err(|e| anyhow!("{e}"))?))?;
        Ok((private.display().to_string(), public.display().to_string()))
    }

    pub fn public_key(&self, id: &str) -> Option<String> {
        self.keys.lock().get(id).and_then(|k| k.public_key().to_openssh().ok())
    }

    pub fn forget(&self, id: &str) {
        self.keys.lock().remove(id);
    }
}

struct Quiet;

impl crate::term::Sink for Quiet {
    fn info(&self, _: &str) {}
    fn error(&self, _: &str) {}
}

/// Adds the public key to ~/.ssh/authorized_keys on a server, once (what ssh-copy-id does).
pub async fn install(app: &tauri::AppHandle, public_key: &str, hops: &[crate::ssh::Hop]) -> Result<()> {
    let key = public_key.trim();
    if key.contains('\'') || key.contains('\n') {
        bail!("Unexpected characters in the public key.");
    }
    let conn = crate::ssh::connect_chain(app, hops, false, &Quiet).await?;
    let mut ch = conn.handle.channel_open_session().await?;
    let cmd = format!(
        "umask 077; mkdir -p ~/.ssh && touch ~/.ssh/authorized_keys && \
         (grep -qxF '{key}' ~/.ssh/authorized_keys || echo '{key}' >> ~/.ssh/authorized_keys) && \
         chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys && echo CABLEBAG_KEY_OK"
    );
    ch.exec(true, cmd).await?;
    let mut out = Vec::new();
    let mut code = None;
    while let Some(msg) = ch.wait().await {
        match msg {
            russh::ChannelMsg::Data { data } | russh::ChannelMsg::ExtendedData { data, .. } => out.extend_from_slice(&data),
            russh::ChannelMsg::ExitStatus { exit_status } => code = Some(exit_status),
            russh::ChannelMsg::Eof | russh::ChannelMsg::Close => break,
            _ => {}
        }
    }
    let _ = conn.handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
    let text = String::from_utf8_lossy(&out);
    if code == Some(0) && text.contains("CABLEBAG_KEY_OK") {
        Ok(())
    } else {
        bail!("The server did not accept the key: {}", text.trim())
    }
}

pub fn default_key_dir() -> String {
    dirs::home_dir().unwrap_or_default().join(".ssh").display().to_string()
}

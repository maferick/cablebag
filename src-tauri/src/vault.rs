//! Password vault. Every secret is encrypted with AES-256-GCM under one random vault key.
//! The vault key itself is protected per platform: Windows DPAPI, or a key file readable only by
//! this account, so only this account on this PC can unwrap it.

use crate::store::Store;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use rand::RngExt;

pub struct Vault {
    key: Mutex<Option<[u8; 32]>>,
}

impl Vault {
    pub fn new() -> Self {
        Self { key: Mutex::new(None) }
    }

    fn key(&self, store: &Store) -> Result<[u8; 32]> {
        let mut guard = self.key.lock();
        if let Some(k) = *guard {
            return Ok(k);
        }
        let key = match store.vault_get("dpapi_key")? {
            Some(wrapped) => {
                let raw = dpapi::unprotect(&wrapped)?;
                let key: [u8; 32] = raw.try_into().map_err(|_| anyhow!("vault key has the wrong length"))?;
                #[cfg(not(windows))]
                if wrapped.len() == 32 {
                    // A build from before the key file kept the plaintext key here: put it under the key file once.
                    store.vault_put("dpapi_key", &dpapi::protect(&key)?)?;
                }
                key
            }
            None => {
                let mut k = [0u8; 32];
                rand::rng().fill(&mut k);
                store.vault_put("dpapi_key", &dpapi::protect(&k)?)?;
                k
            }
        };
        *guard = Some(key);
        Ok(key)
    }

    pub fn encrypt(&self, store: &Store, plain: &str) -> Result<(Vec<u8>, Vec<u8>)> {
        let cipher = Aes256Gcm::new_from_slice(&self.key(store)?).map_err(|e| anyhow!("{e}"))?;
        let mut nonce = [0u8; 12];
        rand::rng().fill(&mut nonce);
        let ct = cipher
            .encrypt(&Nonce::try_from(&nonce[..]).map_err(|_| anyhow!("bad nonce"))?, plain.as_bytes())
            .map_err(|_| anyhow!("encryption failed"))?;
        Ok((nonce.to_vec(), ct))
    }

    pub fn decrypt(&self, store: &Store, nonce: &[u8], ct: &[u8]) -> Result<String> {
        let cipher = Aes256Gcm::new_from_slice(&self.key(store)?).map_err(|e| anyhow!("{e}"))?;
        let plain = cipher
            .decrypt(&Nonce::try_from(nonce).map_err(|_| anyhow!("bad nonce"))?, ct)
            .map_err(|_| anyhow!("stored secret could not be decrypted"))?;
        Ok(String::from_utf8(plain)?)
    }

    /// Stores a password for a protocol/host/port/user combination.
    pub fn save_password(&self, store: &Store, protocol: &str, host: &str, port: i64, user: &str, pw: &str) -> Result<()> {
        let (n, c) = self.encrypt(store, pw)?;
        store.put_secret("password", "", protocol, host, port, user, &n, &c)?;
        Ok(())
    }

    pub fn password(&self, store: &Store, protocol: &str, host: &str, port: i64, user: &str) -> Result<Option<String>> {
        match store.find_secret("password", "", protocol, host, port, user)? {
            Some((n, c)) => Ok(Some(self.decrypt(store, &n, &c)?)),
            None => Ok(None),
        }
    }

    pub fn save_passphrase(&self, store: &Store, key_path: &str, pw: &str) -> Result<()> {
        let (n, c) = self.encrypt(store, pw)?;
        store.put_secret("passphrase", key_path, "", "", 0, "", &n, &c)?;
        Ok(())
    }

    pub fn passphrase(&self, store: &Store, key_path: &str) -> Result<Option<String>> {
        match store.find_secret("passphrase", key_path, "", "", 0, "")? {
            Some((n, c)) => Ok(Some(self.decrypt(store, &n, &c)?)),
            None => Ok(None),
        }
    }
}

#[cfg(windows)]
mod dpapi {
    use anyhow::{anyhow, Result};
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn run(data: &[u8], protect: bool) -> Result<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            if protect {
                CryptProtectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            } else {
                CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            }
            .map_err(|e| anyhow!("Windows data protection failed: {e}"))?;
            let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
            let _ = LocalFree(Some(HLOCAL(out.pbData as _)));
            Ok(bytes)
        }
    }

    pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
        run(data, true)
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
        run(data, false)
    }
}

#[cfg(not(windows))]
mod dpapi {
    //! Stand-in for DPAPI: the vault key is wrapped under a random key kept in its own file, mode
    //! 0600, so only this account can unwrap it and a copy of the database alone reveals nothing.
    use crate::store::data_dir;
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};
    use anyhow::{anyhow, bail, Result};
    use rand::RngExt;
    use std::fs::{OpenOptions, Permissions};
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::path::PathBuf;

    fn key_file() -> PathBuf {
        data_dir().join("vault.key")
    }

    fn kek() -> Result<[u8; 32]> {
        let path = key_file();
        if let Ok(raw) = std::fs::read(&path) {
            let _ = std::fs::set_permissions(&path, Permissions::from_mode(0o600));
            return raw.try_into().map_err(|_| anyhow!("{} does not hold a 32-byte key", path.display()));
        }
        let mut k = [0u8; 32];
        rand::rng().fill(&mut k);
        let mut f = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path)?;
        f.write_all(&k)?;
        Ok(k)
    }

    pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
        let mut nonce = [0u8; 12];
        rand::rng().fill(&mut nonce);
        let ct = Aes256Gcm::new_from_slice(&kek()?)
            .map_err(|e| anyhow!("{e}"))?
            .encrypt(&Nonce::try_from(&nonce[..]).map_err(|_| anyhow!("bad nonce"))?, data)
            .map_err(|_| anyhow!("vault key protection failed"))?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&ct);
        Ok(out)
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
        // Exactly 32 bytes can only be the plaintext key that builds before the key file stored.
        if data.len() == 32 {
            return Ok(data.to_vec());
        }
        let Some((nonce, ct)) = data.split_at_checked(12) else {
            bail!("the stored vault key is too short to be a wrapped key");
        };
        Aes256Gcm::new_from_slice(&kek()?)
            .map_err(|e| anyhow!("{e}"))?
            .decrypt(&Nonce::try_from(nonce).map_err(|_| anyhow!("bad nonce"))?, ct)
            .map_err(|_| anyhow!("the key file at {} no longer opens the stored vault key", key_file().display()))
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn key_file_protects_the_vault_key() {
        let dir = std::env::temp_dir().join(format!("cablebag-vault-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe { std::env::set_var("CABLEBAG_DATA_DIR", &dir) };
        let store = Store::open().unwrap();
        let v = Vault::new();
        let key = v.key(&store).unwrap();

        // The database holds a wrapped copy (12 nonce + 32 key + 16 tag), never the key itself.
        let wrapped = store.vault_get("dpapi_key").unwrap().unwrap();
        assert_eq!(wrapped.len(), 60);
        assert!(!wrapped.windows(32).any(|w| w == &key[..]), "plaintext key found in the database");
        // The key file is this account's alone.
        assert_eq!(std::fs::read(dir.join("vault.key")).unwrap().len(), 32);
        assert_eq!(std::fs::metadata(dir.join("vault.key")).unwrap().permissions().mode() & 0o777, 0o600);

        // Secrets round-trip under the key.
        let (nonce, ct) = v.encrypt(&store, "hunter2").unwrap();
        assert_eq!(v.decrypt(&store, &nonce, &ct).unwrap(), "hunter2");

        // A database from before the key file: the plaintext key still opens the vault,
        // and the next unlock stores it wrapped.
        store.vault_put("dpapi_key", &key).unwrap();
        *v.key.lock() = None;
        assert_eq!(v.key(&store).unwrap(), key);
        assert_eq!(store.vault_get("dpapi_key").unwrap().unwrap().len(), 60);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

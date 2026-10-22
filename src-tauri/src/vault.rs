//! Password vault. Every secret is encrypted with AES-256-GCM under one random vault key.
//! The vault key itself is protected by Windows DPAPI, so only this Windows account can unwrap it.

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
                raw.try_into().map_err(|_| anyhow!("vault key has the wrong length"))?
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
    use anyhow::Result;
    pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }
    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }
}

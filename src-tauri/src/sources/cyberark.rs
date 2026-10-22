//! CyberArk: Privilege Cloud or a self-hosted PVWA, through its REST API (the "cyberark" kind of
//! credential source).
//!
//! shortcut: a stub that only shows the interface; the client itself is still to be written.

use super::{Item, SourceConfig, SourceStatus};
use anyhow::{bail, Result};
use tauri::AppHandle;

#[derive(Default)]
pub struct CyberArk {}

impl CyberArk {
    pub fn status(&self, _app: &AppHandle, cfg: &SourceConfig) -> SourceStatus {
        SourceStatus { configured: !cfg.url.is_empty(), ..Default::default() }
    }

    pub fn is_unlocked(&self) -> bool {
        false
    }

    pub async fn unlock_interactive(&self, _app: &AppHandle, _cfg: &SourceConfig) -> Result<()> {
        bail!("Signing in to CyberArk is not built yet.")
    }

    pub async fn lock(&self, _app: &AppHandle, _cfg: &SourceConfig) {}

    pub async fn sync(&self, _app: &AppHandle, _cfg: &SourceConfig) -> Result<()> {
        Ok(())
    }

    pub fn items(&self) -> Vec<Item> {
        Vec::new()
    }

    pub async fn search(&self, _app: &AppHandle, _cfg: &SourceConfig, _query: &str) -> Result<Vec<Item>> {
        Ok(Vec::new())
    }

    pub async fn find(&self, _app: &AppHandle, _cfg: &SourceConfig, _host: &str) -> Result<Vec<Item>> {
        Ok(Vec::new())
    }

    pub async fn login(&self, _app: &AppHandle, _cfg: &SourceConfig, _item: &str) -> Result<(String, String)> {
        bail!("Signing in to CyberArk is not built yet.")
    }

    pub async fn check_in(&self, _app: &AppHandle, _cfg: &SourceConfig, _item: &str) -> Result<()> {
        Ok(())
    }

    pub fn known_passwords(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Removes what this PC kept for the source.
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

pub async fn psmp_login(_ca: &CyberArk, _app: &AppHandle, _cfg: &SourceConfig, _target_user: &str, _target_host: &str, _target_port: u16) -> Result<PsmpLogin> {
    bail!("Connecting through CyberArk PSM is not built yet.")
}

pub async fn psm_rdp(_ca: &CyberArk, _app: &AppHandle, _cfg: &SourceConfig, _account: Option<&str>, _target_user: &str, _target_host: &str) -> Result<PsmRdp> {
    bail!("Connecting through CyberArk PSM is not built yet.")
}

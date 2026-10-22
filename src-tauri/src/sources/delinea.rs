//! Delinea Secret Server, through its REST API (the "delinea" kind of credential source).
//!
//! shortcut: a stub that only shows the interface; the client itself is still to be written.

use super::{Item, SourceConfig, SourceStatus};
use anyhow::{bail, Result};
use tauri::AppHandle;

#[derive(Default)]
pub struct Delinea {}

impl Delinea {
    pub fn status(&self, _app: &AppHandle, cfg: &SourceConfig) -> SourceStatus {
        SourceStatus { configured: !cfg.url.is_empty(), ..Default::default() }
    }

    pub fn is_unlocked(&self) -> bool {
        false
    }

    pub async fn unlock_interactive(&self, _app: &AppHandle, _cfg: &SourceConfig) -> Result<()> {
        bail!("Signing in to Secret Server is not built yet.")
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
        bail!("Signing in to Secret Server is not built yet.")
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

//! Updates from the official GitHub page only.
//!
//! Three locks keep anything else out: the address the update list comes from is fixed here in the program;
//! the installer it points to must be a release file of that same GitHub repository; and every installer
//! must carry a signature made with the Cablebag signing key, whose public half is built into the program
//! (checked by the updater before anything is run). The window cannot change any of this: it can only ask
//! "is there an update" and "install it".

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Where the list of the newest version is published.
const LATEST: &str = "https://github.com/maferick/cablebag/releases/latest/download/latest.json";
/// The only place an installer may come from.
const RELEASES: &str = "https://github.com/maferick/cablebag/releases/download/";

#[derive(Default)]
pub struct Pending(pub Mutex<Option<Update>>);

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub current: String,
    /// The newer version, or empty when Cablebag is up to date.
    pub version: String,
    pub notes: String,
    pub date: String,
}

pub async fn check(app: &AppHandle) -> Result<UpdateInfo> {
    let current = app.package_info().version.to_string();
    let updater = app
        .updater_builder()
        .endpoints(vec![LATEST.parse()?])?
        .build()?;
    let found = updater.check().await.map_err(|e| anyhow!("Could not check for updates: {e}"))?;
    let Some(update) = found else {
        *app.state::<Pending>().0.lock().unwrap() = None;
        return Ok(UpdateInfo { current, version: String::new(), notes: String::new(), date: String::new() });
    };
    let url = update.download_url.as_str();
    if !url.starts_with(RELEASES) || url.contains("..") {
        bail!("The update list points somewhere other than the official Cablebag releases ({url}); it was ignored.");
    }
    let info = UpdateInfo {
        current,
        version: update.version.clone(),
        notes: update.body.clone().unwrap_or_default(),
        date: update.date.map(|d| d.to_string()).unwrap_or_default(),
    };
    *app.state::<Pending>().0.lock().unwrap() = Some(update);
    Ok(info)
}

#[derive(Serialize, Clone)]
struct Progress {
    done: u64,
    total: u64,
}

/// Downloads the update found by `check`, checks its signature and runs the installer, which closes
/// Cablebag and starts the new version.
pub async fn install(app: &AppHandle) -> Result<()> {
    let update = app.state::<Pending>().0.lock().unwrap().take().ok_or_else(|| anyhow!("Check for updates first."))?;
    // The installer replaces the Remote Desktop helper too, which it cannot while one still runs.
    app.state::<crate::AppState>().rdp.close_all();
    let mut done = 0u64;
    let progress = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                done += chunk as u64;
                let _ = progress.emit("update-progress", Progress { done, total: total.unwrap_or(0) });
            },
            || {},
        )
        .await
        .map_err(|e| anyhow!("The update could not be installed: {e}"))?;
    app.restart();
}

//! Browser sessions: a web page in a tab. The page lives in a web view of its own, placed over the tab's area
//! the way the RDP window is. It only ever shows http and https pages: never Cablebag's own page, so a site
//! can not reach Cablebag's commands, and it keeps its cookies in a profile apart from Cablebag.

use anyhow::{anyhow, Result};
use serde::Serialize;
use tauri::webview::{DownloadEvent, NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};

#[derive(Clone, Serialize)]
struct BrowserEvent {
    id: String,
    url: String,
    title: String,
    loading: bool,
}

#[derive(Clone, Serialize)]
struct NewTab {
    from: String,
    url: String,
}

#[derive(Clone, Serialize)]
struct Download {
    id: String,
    path: String,
    success: bool,
}

fn label(id: &str) -> String {
    format!("browser-{}", id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>())
}

/// Only web pages; Cablebag's own addresses (tauri.localhost, the dev server) are refused.
fn allowed(url: &Url) -> bool {
    if url.as_str() == "about:blank" {
        return true;
    }
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let host = url.host_str().unwrap_or("").to_lowercase();
    !(host == "tauri.localhost" || host == "ipc.localhost" || host.ends_with(".tauri.localhost") || ((host == "localhost" || host == "127.0.0.1") && url.port() == Some(1420)))
}

/// Adds http:// when there is no scheme; refuses anything that is not a web page.
pub fn parse(input: &str) -> Result<Url> {
    let t = input.trim();
    let with_scheme = if t.contains("://") { t.to_string() } else { format!("https://{t}") };
    let url = Url::parse(&with_scheme).map_err(|e| anyhow!("{t} is not a web address: {e}"))?;
    if !allowed(&url) {
        anyhow::bail!("Cablebag only opens http and https pages here.");
    }
    Ok(url)
}

/// New windows a page may open: this many within this many seconds.
const NEW_WINDOWS: usize = 3;
const NEW_WINDOWS_SECS: u64 = 10;

/// Whether a page may open another window now, given when it opened the last ones (kept up to date here).
fn allow_another(times: &mut std::collections::VecDeque<std::time::Instant>) -> bool {
    let now = std::time::Instant::now();
    while times.front().is_some_and(|t| now.duration_since(*t).as_secs() >= NEW_WINDOWS_SECS) {
        times.pop_front();
    }
    if times.len() >= NEW_WINDOWS {
        return false;
    }
    times.push_back(now);
    true
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect()
}

/// The folder that keeps a web tab's cookies and logins. Without a browser profile: Cablebag's shared one. A
/// browser profile has its own, so a second tenant or account can be signed in side by side. Each WireGuard
/// network gets its own again, as WebView2 sets the proxy per folder.
fn profile_dir(profile: Option<&str>, wireguard: Option<&str>) -> std::path::PathBuf {
    let data = crate::store::data_dir();
    match (profile, wireguard) {
        (None, None) => data.join("browser"),
        (None, Some(wg)) => crate::wg::web_profile_dir(wg),
        (Some(p), None) => data.join(format!("browser-p-{}", clean(p))),
        (Some(p), Some(wg)) => data.join(format!("browser-wg-{}-p-{}", clean(wg), clean(p))),
    }
}

/// Removes the cookies and logins of a browser profile (with and without WireGuard). A tab that is open keeps
/// its folder in use; that one goes the next time.
pub fn forget_profile(profile: &str) {
    let p = clean(profile);
    if p.is_empty() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(crate::store::data_dir()) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name == format!("browser-p-{p}") || (name.starts_with("browser-wg-") && name.ends_with(&format!("-p-{p}"))) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// `wireguard`: the WireGuard network the page goes through (Cablebag's own proxy for it, and a browser
/// profile of its own: WebView2 sets the proxy per profile). `profile`: the browser profile whose cookies and
/// logins it uses; `private`: nothing is kept after the tab closes.
#[allow(clippy::too_many_arguments)]
pub async fn open(app: &AppHandle, id: &str, url: &str, wireguard: Option<String>, profile: Option<String>, private: bool, x: f64, y: f64, w: f64, h: f64) -> Result<()> {
    let url = parse(url)?;
    let label = label(id);
    if let Some(wv) = app.get_webview(&label) {
        wv.navigate(url)?;
        return bounds(app, id, x, y, w, h);
    }
    let window = app.get_window("main").ok_or_else(|| anyhow!("main window missing"))?;
    let (app1, app2, app3, id1, id2, id3) = (app.clone(), app.clone(), app.clone(), id.to_string(), id.to_string(), id.to_string());
    let (app4, app5, id4, id5) = (app.clone(), app.clone(), id.to_string(), id.to_string());
    let opened_recently = std::sync::Mutex::new(std::collections::VecDeque::new());
    let mut builder = WebviewBuilder::new(&label, WebviewUrl::External(url))
        .data_directory(profile_dir(profile.as_deref(), wireguard.as_deref()))
        .incognito(private);
    if let Some(wg) = wireguard {
        let port = crate::wg::web_proxy(app, &wg).await?;
        // Arguments of its own replace wry's, so its defaults are repeated. Without <-loopback> Chromium sends
        // loopback and link-local addresses around the proxy; WebRTC would show the page this PC's address.
        builder = builder.additional_browser_args(&format!(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --proxy-server=http://127.0.0.1:{port} --proxy-bypass-list=<-loopback> --force-webrtc-ip-handling-policy=disable_non_proxied_udp"
        ));
    }
    let builder = builder
        .on_navigation(move |u| {
            let ok = allowed(u);
            if ok {
                let _ = app1.emit("browser-event", BrowserEvent { id: id1.clone(), url: u.to_string(), title: String::new(), loading: true });
            }
            ok
        })
        .on_page_load(move |_, p| {
            if matches!(p.event(), PageLoadEvent::Finished) {
                let _ = app2.emit("browser-event", BrowserEvent { id: id2.clone(), url: p.url().to_string(), title: String::new(), loading: false });
            }
        })
        .on_document_title_changed(move |wv, title| {
            let url = wv.url().map(|u| u.to_string()).unwrap_or_default();
            let _ = app3.emit("browser-event", BrowserEvent { id: id3.clone(), url, title, loading: false });
        })
        // A link that opens a new tab becomes a Cablebag tab with the same profile and network. A window the page
        // sizes itself (a sign-in pop-up) stays a pop-up of WebView2's own, in the same profile, so the page can
        // still talk to it.
        // A page can call window.open in a loop (WebView2 has no pop-up blocker): at most a few per tab at a time.
        // An empty window (about:blank) stays a pop-up too: pages open one first and send it somewhere after.
        .on_new_window(move |u, features| {
            if !allowed(&u) || !opened_recently.lock().map(|mut t| allow_another(&mut t)).unwrap_or(false) {
                return NewWindowResponse::Deny;
            }
            if features.size().is_some() || u.as_str() == "about:blank" {
                return NewWindowResponse::Allow;
            }
            let _ = app4.emit("browser-new-tab", NewTab { from: id4.clone(), url: u.to_string() });
            NewWindowResponse::Deny
        })
        .on_download(move |_, event| {
            if let DownloadEvent::Finished { path, success, .. } = event {
                let path = path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                let _ = app5.emit("browser-download", Download { id: id5.clone(), path, success });
            }
            true
        });
    let wv = window.add_child(builder, LogicalPosition::new(x, y), LogicalSize::new(w.max(1.0), h.max(1.0)))?;
    crate::webnotify::attach(app, id, &wv);
    Ok(())
}

pub fn bounds(app: &AppHandle, id: &str, x: f64, y: f64, w: f64, h: f64) -> Result<()> {
    if let Some(wv) = app.get_webview(&label(id)) {
        wv.set_position(LogicalPosition::new(x, y))?;
        wv.set_size(LogicalSize::new(w.max(1.0), h.max(1.0)))?;
    }
    Ok(())
}

pub fn visible(app: &AppHandle, id: &str, show: bool) -> Result<()> {
    if let Some(wv) = app.get_webview(&label(id)) {
        if show {
            wv.show()?;
        } else {
            wv.hide()?;
        }
    }
    Ok(())
}

/// back, forward, reload or go (to `url`).
pub fn command(app: &AppHandle, id: &str, what: &str, url: Option<String>) -> Result<()> {
    let wv = app.get_webview(&label(id)).ok_or_else(|| anyhow!("This browser tab is not open."))?;
    match what {
        "back" => wv.eval("history.back()")?,
        "forward" => wv.eval("history.forward()")?,
        "reload" => wv.reload()?,
        "go" => wv.navigate(parse(&url.unwrap_or_default())?)?,
        _ => {}
    }
    Ok(())
}

/// Closing a child web view in Tauri only forgets it: the window stays on screen. So it is hidden, the
/// browser inside is shut down and its window destroyed here, before Tauri forgets it.
pub fn close(app: &AppHandle, id: &str) {
    let Some(wv) = app.get_webview(&label(id)) else { return };
    let _ = wv.hide();
    #[cfg(windows)]
    let _ = wv.with_webview(|p| unsafe {
        let controller = p.controller();
        // The window WebView2 lives in (wry's host window); its type comes from WebView2's own bindings.
        let mut host = std::mem::zeroed();
        let found = controller.ParentWindow(&mut host).is_ok();
        let _ = controller.Close();
        if found && !host.0.is_null() {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(windows::Win32::Foundation::HWND(host.0 as _));
        }
    });
    let _ = wv.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_profile_and_network_has_its_own_cookies() {
        let name = |p: Option<&str>, wg: Option<&str>| profile_dir(p, wg).file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(name(None, None), "browser");
        assert_eq!(name(None, Some("wg1")), "browser-wg-wg1");
        assert_eq!(name(Some("a"), None), "browser-p-a");
        assert_eq!(name(Some("a"), Some("wg1")), "browser-wg-wg1-p-a");
        assert_ne!(name(Some("a"), None), name(Some("b"), None));
        // An id cannot reach outside the data folder.
        assert_eq!(name(Some("../../x"), None), "browser-p-x");
    }

    #[test]
    fn a_page_cannot_open_windows_without_end() {
        let mut times = std::collections::VecDeque::new();
        let allowed: Vec<bool> = (0..6).map(|_| allow_another(&mut times)).collect();
        assert_eq!(allowed, [true, true, true, false, false, false]);
        // Once they are old enough, new ones may open again.
        times.iter_mut().for_each(|t| *t -= std::time::Duration::from_secs(NEW_WINDOWS_SECS));
        assert!(allow_another(&mut times));
    }

    #[test]
    fn only_web_pages() {
        assert!(parse("example.com").is_ok());
        assert_eq!(parse("example.com").unwrap().as_str(), "https://example.com/");
        assert!(parse("http://10.20.30.53:8006").is_ok());
        assert!(parse("http://tauri.localhost/").is_err());
        assert!(parse("https://tauri.localhost/index.html").is_err());
        assert!(parse("http://localhost:1420/").is_err());
        assert!(parse("file:///C:/Windows/win.ini").is_err());
        assert!(parse("javascript:alert(1)").is_err());
        assert!(parse("http://localhost:8080/").is_ok());
    }
}

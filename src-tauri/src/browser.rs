//! Browser sessions: a web page in a tab. The page lives in a web view of its own, placed over the tab's area
//! the way the RDP window is. It only ever shows http and https pages: never Cablebag's own page, so a site
//! can not reach Cablebag's commands, and it keeps its cookies in a profile apart from Cablebag.

use anyhow::{anyhow, Result};
use serde::Serialize;
use tauri::webview::{PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};

#[derive(Clone, Serialize)]
struct BrowserEvent {
    id: String,
    url: String,
    title: String,
    loading: bool,
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

pub fn open(app: &AppHandle, id: &str, url: &str, x: f64, y: f64, w: f64, h: f64) -> Result<()> {
    let url = parse(url)?;
    let label = label(id);
    if let Some(wv) = app.get_webview(&label) {
        wv.navigate(url)?;
        return bounds(app, id, x, y, w, h);
    }
    let window = app.get_window("main").ok_or_else(|| anyhow!("main window missing"))?;
    let (app1, app2, app3, id1, id2, id3) = (app.clone(), app.clone(), app.clone(), id.to_string(), id.to_string(), id.to_string());
    let profile = crate::store::data_dir().join("browser");
    let builder = WebviewBuilder::new(&label, WebviewUrl::External(url))
        .data_directory(profile)
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
        });
    window.add_child(builder, LogicalPosition::new(x, y), LogicalSize::new(w.max(1.0), h.max(1.0)))?;
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

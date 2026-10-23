//! Notifications of web tabs (Teams, chat, mail). WebView2 hands them to Cablebag instead of showing them itself;
//! the window decides whether to show one (do not disturb, a muted browser profile) and with which name, and it
//! shows as a Windows notification. Clicking it opens the tab and tells the page, as a browser would.
#![cfg_attr(not(windows), allow(dead_code, unused_imports))]

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager, Webview};

#[derive(Clone, Serialize)]
struct WebNotification {
    id: String,
    key: u64,
    title: String,
    body: String,
    /// The site that sent it, as WebView2 knows it (not what the page says), shown on the notification.
    origin: String,
}

#[derive(Clone, Serialize)]
struct Clicked {
    id: String,
}

static NEXT: AtomicU64 = AtomicU64::new(1);

#[cfg(windows)]
thread_local! {
    /// The pages' notifications by key, kept on the main thread: WebView2's objects stay on the thread that made them.
    static SHOWN: std::cell::RefCell<std::collections::HashMap<u64, webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Notification>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Has the web tab `id` give its notifications to Cablebag. Older WebView2 runtimes keep showing them themselves.
pub fn attach(app: &AppHandle, id: &str, wv: &Webview) {
    #[cfg(windows)]
    {
        let (app, id) = (app.clone(), id.to_string());
        let _ = wv.with_webview(move |p| unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_24;
            use webview2_com::NotificationReceivedEventHandler;
            use windows::core::{Interface, PWSTR};
            let Ok(core) = p.controller().CoreWebView2() else { return };
            let Ok(core24) = core.cast::<ICoreWebView2_24>() else { return };
            let handler = NotificationReceivedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                args.SetHandled(true)?;
                let n = args.Notification()?;
                let (mut title, mut body) = (PWSTR::null(), PWSTR::null());
                let title = if n.Title(&mut title).is_ok() { webview2_com::take_pwstr(title) } else { String::new() };
                let body = if n.Body(&mut body).is_ok() { webview2_com::take_pwstr(body) } else { String::new() };
                let mut origin = PWSTR::null();
                let origin = if args.SenderOrigin(&mut origin).is_ok() { webview2_com::take_pwstr(origin) } else { String::new() };
                let origin = origin.split_once("://").map(|(_, rest)| rest.trim_end_matches('/').to_string()).unwrap_or(origin);
                let _ = n.ReportShown();
                let key = NEXT.fetch_add(1, Ordering::Relaxed);
                SHOWN.with(|m| {
                    let mut m = m.borrow_mut();
                    // Pages forget their notifications too; a few hundred old ones need not be kept.
                    if m.len() > 200 {
                        m.clear();
                    }
                    m.insert(key, n);
                });
                let _ = app.emit("browser-notification", WebNotification { id: id.clone(), key, title, body, origin });
                Ok(())
            }));
            let mut token = 0i64;
            let _ = core24.add_NotificationReceived(&handler, &mut token);
            // WebView2 asks nobody about notifications and refuses them. They only reach the user through
            // Cablebag, which has its own switch and a mute per browser profile, names the sending site on each
            // and limits how many a tab may show, so pages may send them. Not saved in the profile, so this can
            // change later. Camera, microphone and the rest keep WebView2's own question.
            use webview2_com::Microsoft::Web::WebView2::Win32::{COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS, COREWEBVIEW2_PERMISSION_STATE_ALLOW};
            let permissions = webview2_com::PermissionRequestedEventHandler::create(Box::new(|_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                args.PermissionKind(&mut kind)?;
                if kind == COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS {
                    args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                }
                Ok(())
            }));
            let mut token2 = 0i64;
            let _ = core.add_PermissionRequested(&permissions, &mut token2);
        });
    }
    #[cfg(not(windows))]
    let _ = (app, id, wv);
}

/// Shows a web tab's notification (`key`) as a Windows notification, with the site that sent it on a line of its
/// own; clicking it opens tab `id`.
pub fn show(app: &AppHandle, id: &str, key: u64, title: &str, body: &str, origin: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use tauri_winrt_notification::Toast;
        // The installer gives Cablebag's shortcut this id; a development build has no shortcut, so it borrows
        // PowerShell's, as Tauri's own notifications do.
        let app_id = if cfg!(debug_assertions) { Toast::POWERSHELL_APP_ID.to_string() } else { app.config().identifier.clone() };
        let (app2, id2) = (app.clone(), id.to_string());
        let mut toast = Toast::new(&app_id).title(title).text1(body);
        if !origin.is_empty() {
            toast = toast.text2(&format!("From {origin}"));
        }
        toast
            .on_activated(move |_| {
                let app3 = app2.clone();
                let _ = app2.run_on_main_thread(move || {
                    SHOWN.with(|m| {
                        if let Some(n) = m.borrow_mut().remove(&key) {
                            let _ = unsafe { n.ReportClicked() };
                        }
                    });
                    if let Some(w) = app3.get_window("main") {
                        let _ = w.unminimize();
                        let _ = w.set_focus();
                    }
                });
                let _ = app2.emit("browser-notification-click", Clicked { id: id2.clone() });
                Ok(())
            })
            .show()
            .map_err(|e| anyhow::anyhow!("Could not show the notification: {e}"))?;
    }
    #[cfg(not(windows))]
    let _ = (app, id, key, title, body, origin);
    Ok(())
}

//! The icon of a web tab's site, for its pin on the strip. WebView2 says when a page's icon changes and hands it
//! over as a PNG; it goes to the window as a data URL. Older WebView2 runtimes send nothing, and the pin keeps its
//! letter.
#![cfg_attr(not(windows), allow(dead_code, unused_imports))]

use serde::Serialize;
use tauri::{AppHandle, Emitter, Webview};

#[derive(Clone, Serialize)]
struct SiteIcon {
    id: String,
    /// data:image/png;base64,...
    data: String,
}

/// Icons bigger than this are left out (a site's icon is a few kilobytes).
const MAX_BYTES: usize = 64 * 1024;

/// Has web tab `id` send its page's icon (event browser-icon) each time it changes.
pub fn attach(app: &AppHandle, id: &str, wv: &Webview) {
    #[cfg(windows)]
    {
        let (app, id) = (app.clone(), id.to_string());
        let _ = wv.with_webview(move |p| unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_15, COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG};
            use webview2_com::{FaviconChangedEventHandler, GetFaviconCompletedHandler};
            use windows::core::Interface;
            let Ok(core) = p.controller().CoreWebView2() else { return };
            let Ok(core15) = core.cast::<ICoreWebView2_15>() else { return };
            let handler = FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                let Some(sender) = sender else { return Ok(()) };
                let core15 = sender.cast::<ICoreWebView2_15>()?;
                let (app, id) = (app.clone(), id.clone());
                let done = GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
                    result?;
                    let Some(stream) = stream else { return Ok(()) };
                    let png = read_all(&stream);
                    if !png.is_empty() && png.len() <= MAX_BYTES {
                        use base64::Engine;
                        let data = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png));
                        let _ = app.emit("browser-icon", SiteIcon { id, data });
                    }
                    Ok(())
                }));
                core15.GetFavicon(COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, &done)
            }));
            let mut token = 0i64;
            let _ = core15.add_FaviconChanged(&handler, &mut token);
        });
    }
    #[cfg(not(windows))]
    let _ = (app, id, wv);
}

/// Everything in the stream WebView2 hands the icon over in (stops at MAX_BYTES and a bit).
#[cfg(windows)]
unsafe fn read_all(stream: &windows::Win32::System::Com::IStream) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let mut read = 0u32;
        let hr = unsafe { stream.Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut read)) };
        if hr.is_err() || read == 0 {
            break;
        }
        out.extend_from_slice(&buf[..read as usize]);
        if out.len() > MAX_BYTES {
            break;
        }
    }
    out
}

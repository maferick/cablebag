//! Single sign-on in a Cablebag window: the person signs in themselves, on their identity provider's own
//! pages, and Cablebag takes only the one thing the source needs at the end (a SAML response on its way
//! to the service, a redirect with an OAuth code, or a session cookie). Any credential source can use it.
//!
//! The window is a top-level window of its own, with a browser profile of its own (under the data folder,
//! "sso"), so its cookies stay apart from Cablebag's page and from browser tabs. It only ever shows http
//! and https pages, never Cablebag's own, and its label ("sso-...") is in no capability, so the pages it
//! shows can not reach Cablebag's commands. Popups open in the same window; downloads are refused.
//! Closing the window cancels the sign-in. Nothing it captures is logged.

use anyhow::{anyhow, bail, Result};
use std::sync::Arc;
use std::time::Duration;
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tokio::sync::mpsc;

/// The address the page script sends a caught SAML response to. It never resolves (".invalid" is
/// reserved) and the window cancels the navigation before anything leaves; the data is in the fragment.
const SENTINEL: &str = "https://cablebag-sso.invalid/saml";

/// How long the person has to sign in.
const PATIENCE: Duration = Duration::from_secs(15 * 60);

/// What to take from the sign-in. (Not every source uses every kind.)
#[allow(dead_code)]
pub enum Capture {
    /// The SAML response an identity provider posts to a service's assertion consumer address, caught in
    /// the page before it is sent: a form with a SAMLResponse field going to `acs_host` (and to an address
    /// holding `acs_path`, when not empty). Responses to other services (an identity provider that itself
    /// federates to another one) pass through.
    SamlPost { acs_host: String, acs_path: String },
    /// A redirect to an address that starts with `prefix` (an OAuth or OIDC redirect address). The
    /// navigation is stopped and the whole address, with its code and state, comes back.
    Redirect { prefix: String },
    /// A cookie called `name` that a sign-in sets, on one of `hosts` (exact host names, or ".suffix").
    /// `ignore` is a value already found not to work. The cookie is removed from the window's profile once
    /// read, so the session it holds is not kept on disk.
    Cookie { name: String, hosts: Vec<String>, ignore: Option<String> },
}

#[allow(dead_code)]
pub enum Captured {
    Saml { response: String, relay_state: String },
    Redirect(Url),
    Cookie { value: String, host: String },
}

enum Event {
    Got(Captured),
    Loaded,
    Closed,
}

/// Only web pages; Cablebag's own addresses are refused (the same rule as browser tabs).
fn allowed(url: &Url) -> bool {
    url.as_str() == "about:blank" || crate::browser::parse(url.as_str()).is_ok()
}

/// The script that catches a SAML response in the page: form.submit(), form.requestSubmit() and the
/// submit event, for a form with a SAMLResponse field going to the service.
fn saml_script(acs_host: &str, acs_path: &str) -> String {
    let host = serde_json::to_string(&acs_host.to_lowercase()).unwrap_or_else(|_| "\"\"".into());
    let path = serde_json::to_string(&acs_path.to_lowercase()).unwrap_or_else(|_| "\"\"".into());
    let sentinel = serde_json::to_string(SENTINEL).unwrap_or_default();
    format!(
        r##"(function () {{
  var HOST = {host}, PATH = {path}, SENT = {sentinel};
  function grab(form) {{
    try {{
      if (!form || !form.elements) return false;
      var r = form.elements.namedItem("SAMLResponse");
      if (!r || !r.value) return false;
      var a = new URL(form.getAttribute("action") || location.href, location.href);
      if (a.hostname.toLowerCase() !== HOST) return false;
      if (PATH && a.pathname.toLowerCase().indexOf(PATH) < 0) return false;
      var s = form.elements.namedItem("RelayState");
      var q = new URLSearchParams();
      q.set("action", a.href);
      q.set("SAMLResponse", r.value);
      q.set("RelayState", s && s.value ? s.value : "");
      location.replace(SENT + "#" + q.toString());
      return true;
    }} catch (e) {{ return false; }}
  }}
  var submit = HTMLFormElement.prototype.submit;
  HTMLFormElement.prototype.submit = function () {{ if (!grab(this)) return submit.apply(this, arguments); }};
  var request = HTMLFormElement.prototype.requestSubmit;
  if (request) HTMLFormElement.prototype.requestSubmit = function () {{ if (!grab(this)) return request.apply(this, arguments); }};
  document.addEventListener("submit", function (e) {{ if (grab(e.target)) {{ e.preventDefault(); e.stopImmediatePropagation(); }} }}, true);
}})();"##
    )
}

/// The SAML response in a sentinel address, when it is one and the form went where it should.
fn from_sentinel(url: &Url, acs_host: &str, acs_path: &str) -> Option<Captured> {
    if !url.as_str().starts_with(SENTINEL) {
        return None;
    }
    let data = url.fragment().or(url.query()).unwrap_or("");
    let mut response = String::new();
    let mut relay_state = String::new();
    let mut action = String::new();
    let pairs = Url::parse(&format!("http://sentinel/?{data}")).ok()?;
    for (k, v) in pairs.query_pairs() {
        match k.as_ref() {
            "SAMLResponse" => response = v.into_owned(),
            "RelayState" => relay_state = v.into_owned(),
            "action" => action = v.into_owned(),
            _ => {}
        }
    }
    let action = Url::parse(&action).ok()?;
    let host_ok = action.host_str().map(|h| h.eq_ignore_ascii_case(acs_host)).unwrap_or(false);
    let path_ok = acs_path.is_empty() || action.path().to_lowercase().contains(&acs_path.to_lowercase());
    if response.is_empty() || !host_ok || !path_ok || action.scheme() != "https" && action.scheme() != "http" {
        return None;
    }
    Some(Captured::Saml { response, relay_state })
}

/// Whether a cookie's domain is one of `hosts` (exact, or under a ".suffix").
fn host_fits(domain: &str, hosts: &[String]) -> bool {
    let d = domain.trim_start_matches('.').to_lowercase();
    hosts.iter().any(|h| {
        let h = h.to_lowercase();
        match h.strip_prefix('.') {
            Some(suffix) => d == suffix || d.ends_with(&format!(".{suffix}")),
            None => d == h,
        }
    })
}

fn close(win: &WebviewWindow) {
    let _ = win.destroy();
}

/// Looks for the cookie; reading cookies must not happen on the thread that runs the window's events.
async fn find_cookie(win: &WebviewWindow, name: &str, hosts: &[String], ignore: &Option<String>) -> Option<Captured> {
    let w = win.clone();
    let cookies = tokio::task::spawn_blocking(move || w.cookies()).await.ok()?.ok()?;
    let c = cookies.into_iter().find(|c| c.name() == name && !c.value().is_empty() && c.domain().map(|d| host_fits(d, hosts)).unwrap_or(false) && ignore.as_deref() != Some(c.value()))?;
    let found = Captured::Cookie { value: c.value().to_string(), host: c.domain().unwrap_or("").trim_start_matches('.').to_string() };
    let w = win.clone();
    let _ = tokio::task::spawn_blocking(move || w.delete_cookie(c)).await;
    Some(found)
}

/// Builds the window. Not async: the builder is not Sync, and the futures that wait for a sign-in must be.
fn open_window(app: &AppHandle, title: &str, start: Url, what: Arc<Capture>, tx: mpsc::UnboundedSender<Event>, private: bool) -> Result<WebviewWindow> {
    let label = format!("sso-{}", uuid::Uuid::new_v4().simple());
    let (tx_nav, what_nav) = (tx.clone(), what.clone());
    let (tx_load, tx_close) = (tx.clone(), tx.clone());
    let (app_new, label_new) = (app.clone(), label.clone());
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(start))
        .title(title)
        .inner_size(520.0, 720.0)
        .center()
        .focused(true)
        .data_directory(crate::store::data_dir().join("sso"))
        .incognito(private)
        .on_navigation(move |u| {
            match &*what_nav {
                Capture::SamlPost { acs_host, acs_path } => {
                    if u.as_str().starts_with(SENTINEL) {
                        if let Some(c) = from_sentinel(u, acs_host, acs_path) {
                            let _ = tx_nav.send(Event::Got(c));
                        }
                        return false;
                    }
                }
                Capture::Redirect { prefix } if u.as_str().starts_with(prefix.as_str()) => {
                    let _ = tx_nav.send(Event::Got(Captured::Redirect(u.clone())));
                    return false;
                }
                _ => {}
            }
            allowed(u)
        })
        .on_page_load(move |_, p| {
            if matches!(p.event(), PageLoadEvent::Finished) {
                let _ = tx_load.send(Event::Loaded);
            }
        })
        .on_new_window(move |u, _| {
            // A popup opens here instead, through the same checks.
            if allowed(&u)
                && let Some(w) = app_new.get_webview_window(&label_new)
            {
                let _ = w.navigate(u);
            }
            NewWindowResponse::Deny
        })
        .on_download(|_, _| false);
    if let Capture::SamlPost { acs_host, acs_path } = &*what {
        builder = builder.initialization_script(saml_script(acs_host, acs_path));
    }
    let win = builder.build().map_err(|e| anyhow!("Could not open the sign-in window: {e}"))?;
    win.on_window_event(move |e| {
        if matches!(e, WindowEvent::Destroyed) {
            let _ = tx_close.send(Event::Closed);
        }
    });
    Ok(win)
}

/// Opens a sign-in window at `start_url` and waits until `what` is caught, the person closes the window
/// (an error: "Sign-in cancelled.") or they take too long. `private` keeps nothing at all (an InPrivate
/// profile): the identity provider then asks every time.
pub async fn capture(app: &AppHandle, title: &str, start_url: &str, what: Capture, private: bool) -> Result<Captured> {
    let start = crate::browser::parse(start_url)?;
    let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
    let what = Arc::new(what);
    let win = open_window(app, title, start, what.clone(), tx, private)?;

    let deadline = tokio::time::sleep(PATIENCE);
    tokio::pin!(deadline);
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Some(Event::Got(c)) => {
                    close(&win);
                    return Ok(c);
                }
                Some(Event::Loaded) => {}
                Some(Event::Closed) | None => bail!("Sign-in cancelled."),
            },
            _ = tick.tick() => {}
            _ = &mut deadline => {
                close(&win);
                bail!("The sign-in took too long. Try again.");
            }
        }
        // A page loaded, or a moment passed: the cookie may be there now.
        if let Capture::Cookie { name, hosts, ignore } = &*what
            && let Some(c) = find_cookie(&win, name, hosts, ignore).await
        {
            close(&win);
            return Ok(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sentinel(action: &str, response: &str) -> Url {
        let q = Url::parse_with_params("http://x/", &[("action", action), ("SAMLResponse", response), ("RelayState", "rs1")]).unwrap();
        Url::parse(&format!("{SENTINEL}#{}", q.query().unwrap())).unwrap()
    }

    #[test]
    fn a_saml_response_for_the_service_is_taken() {
        let u = sentinel("https://pvwa.example.com/PasswordVault/api/auth/saml/logon", "PHNhbWxwOlJlc3BvbnNlIC8+");
        match from_sentinel(&u, "pvwa.example.com", "/auth/saml") {
            Some(Captured::Saml { response, relay_state }) => {
                assert_eq!(response, "PHNhbWxwOlJlc3BvbnNlIC8+");
                assert_eq!(relay_state, "rs1");
            }
            _ => panic!("expected a SAML response"),
        }
    }

    #[test]
    fn a_saml_response_for_another_service_is_not() {
        let u = sentinel("https://idp.example.org/saml/acs", "PHNhbWw+");
        assert!(from_sentinel(&u, "pvwa.example.com", "").is_none());
        let u = sentinel("https://pvwa.example.com/other", "PHNhbWw+");
        assert!(from_sentinel(&u, "pvwa.example.com", "/auth/saml").is_none());
        let empty = sentinel("https://pvwa.example.com/PasswordVault/api/auth/saml/logon", "");
        assert!(from_sentinel(&empty, "pvwa.example.com", "").is_none());
        assert!(from_sentinel(&Url::parse("https://pvwa.example.com/").unwrap(), "pvwa.example.com", "").is_none());
    }

    #[test]
    fn the_page_script_names_the_service() {
        let s = saml_script("PVWA.example.com", "/auth/saml");
        assert!(s.contains("\"pvwa.example.com\""));
        assert!(s.contains("\"/auth/saml\""));
        assert!(s.contains(SENTINEL));
        // Values are JSON strings, so a host can not break out of the script.
        assert!(saml_script("a\"b", "").contains("\"a\\\"b\""));
    }

    #[test]
    fn only_web_pages_are_shown() {
        assert!(allowed(&Url::parse("https://login.microsoftonline.com/x").unwrap()));
        assert!(allowed(&Url::parse("about:blank").unwrap()));
        assert!(!allowed(&Url::parse("http://tauri.localhost/").unwrap()));
        assert!(!allowed(&Url::parse("file:///C:/Windows/win.ini").unwrap()));
    }

    #[test]
    fn cookie_hosts() {
        let hosts = vec![".id.cyberark.cloud".to_string(), "login.example.com".to_string()];
        assert!(host_fits(".abc1234.id.cyberark.cloud", &hosts));
        assert!(host_fits("abc1234.id.cyberark.cloud", &hosts));
        assert!(host_fits("login.example.com", &hosts));
        assert!(!host_fits("evil-id.cyberark.cloud.example.net", &hosts));
        assert!(!host_fits("sub.login.example.com", &hosts));
    }
}

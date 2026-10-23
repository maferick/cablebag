//! Filling a web tab's login form from the credential sources (Vaultwarden, CyberArk, Delinea, Proton Pass), only
//! when the person asks, with a login they pick.
//!
//! Which logins fit a page: one saved for the same site (the same host, or one inside the other; IP addresses
//! exactly, with their port), or, on a provider's own sign-in page, one saved for any site of that provider
//! (portal.azure.com sends you to login.microsoftonline.com to sign in). Only the listed sign-in hosts count: other
//! pages in the provider's domains (a SharePoint site, a storage account's web page) can be made by anyone. A login
//! saved for another site is only filled when the person confirms it for this page's site; the page must still be
//! on that site when it is filled, and the page itself checks once more, so a page that moved on gets nothing.

use crate::sources::Item;
use crate::AppState;
use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use tauri::{AppHandle, Manager, Url};

/// The sites of a provider, and its own sign-in hosts. A login for any of the sites fits the sign-in hosts.
const FAMILIES: &[(&[&str], &[&str])] = &[
    (
        &[
            "microsoftonline.com",
            "microsoft.com",
            "microsoft365.com",
            "cloud.microsoft",
            "azure.com",
            "azure.net",
            "windows.net",
            "office.com",
            "office365.com",
            "live.com",
            "sharepoint.com",
        ],
        &["login.microsoftonline.com", "device.login.microsoftonline.com", "login.microsoft.com", "login.live.com", "login.windows.net"],
    ),
    (&["google.com", "gmail.com", "youtube.com"], &["accounts.google.com"]),
];

#[derive(Serialize)]
pub struct Choice {
    /// The login's reference.
    pub id: String,
    pub name: String,
    pub username: String,
    /// The credential source's name.
    pub source: String,
    /// Saved for this site; otherwise it only has the site's name in its title, and filling asks first.
    pub fits: bool,
}

#[derive(Serialize)]
pub struct Choices {
    /// The page's host.
    pub host: String,
    /// The page is not encrypted (http).
    pub plain: bool,
    pub choices: Vec<Choice>,
}

fn webview(app: &AppHandle, tab: &str) -> Result<tauri::Webview> {
    let label = format!("browser-{}", tab.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>());
    app.get_webview(&label).ok_or_else(|| anyhow!("This web tab is not open."))
}

fn bare(host: &str) -> String {
    let h = host.trim().trim_end_matches('.').to_lowercase();
    h.strip_prefix("www.").map(str::to_string).unwrap_or(h)
}

/// A site: its host, and the port (which only counts for IP addresses).
#[derive(Debug, PartialEq)]
struct Site {
    host: String,
    port: Option<u16>,
}

fn site_of(url: &Url) -> Option<Site> {
    url.host_str().map(|h| Site { host: bare(h), port: url.port_or_known_default() })
}

/// The site of an address saved with a login ("portal.azure.com", "https://x.example.com/login", "10.0.0.5:8443").
fn saved_site(u: &str) -> Option<Site> {
    let u = u.trim();
    if u.is_empty() {
        return None;
    }
    let full = if u.contains("://") { u.to_string() } else { format!("https://{u}") };
    Url::parse(&full).ok().and_then(|x| site_of(&x))
}

fn is_ip(host: &str) -> bool {
    host.trim_matches(|c| c == '[' || c == ']').parse::<std::net::IpAddr>().is_ok()
}

fn same_site(saved: &Site, page: &Site) -> bool {
    if is_ip(&saved.host) || is_ip(&page.host) {
        return saved.host == page.host && saved.port == page.port;
    }
    // One inside the other, but never a bare word ("local", "corp") that would fit a whole network's sites.
    let inside = |a: &str, b: &str| b.contains('.') && a.ends_with(&format!(".{b}"));
    saved.host == page.host || inside(&saved.host, &page.host) || inside(&page.host, &saved.host)
}

fn in_family(host: &str, domains: &[&str]) -> bool {
    domains.iter().any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Whether a login is saved for this page's site, or the page is a provider's sign-in page and the login is saved
/// for one of that provider's sites.
fn fits_site(item: &Item, page: &Site) -> bool {
    let sign_in = FAMILIES.iter().find(|(_, hosts)| hosts.contains(&page.host.as_str())).map(|(domains, _)| *domains);
    item.uris.iter().filter_map(|u| saved_site(u)).any(|s| same_site(&s, page) || sign_in.is_some_and(|d| in_family(&s.host, d)))
}

/// Whether a login without a matching address names the site in its title ("Azure portal" for portal.azure.com).
fn named(item: &Item, page: &Site) -> bool {
    let labels: Vec<&str> = page.host.split('.').collect();
    let main = if labels.len() >= 2 { labels[labels.len() - 2] } else { labels[0] };
    !is_ip(&page.host) && main.len() >= 3 && item.name.to_lowercase().contains(main)
}

fn page_site(app: &AppHandle, tab: &str) -> Result<(Site, Url)> {
    let url = webview(app, tab)?.url()?;
    let site = site_of(&url).ok_or_else(|| anyhow!("This page has no site to fill a login for."))?;
    Ok((site, url))
}

/// The logins to offer for a web tab's page: those saved for it first, then those that only name it.
pub fn choices(app: &AppHandle, tab: &str) -> Result<Choices> {
    let (site, url) = page_site(app, tab)?;
    let state = app.state::<AppState>();
    let names: std::collections::HashMap<String, String> = crate::sources::configs(app).into_iter().map(|c| (c.id, c.name)).collect();
    let items = state.creds.items(app);
    let to_choice = |i: &Item, fits: bool| Choice { id: i.id.clone(), name: i.name.clone(), username: i.username.clone(), source: names.get(&i.source).cloned().unwrap_or_default(), fits };
    let mut out: Vec<Choice> = items.iter().filter(|i| i.has_password && fits_site(i, &site)).map(|i| to_choice(i, true)).collect();
    out.extend(items.iter().filter(|i| i.has_password && !fits_site(i, &site) && named(i, &site)).take(8).map(|i| to_choice(i, false)));
    Ok(Choices { host: site.host, plain: url.scheme() == "http", choices: out })
}

/// Fills the login `item` into the tab's page. `confirmed_host`: the person confirmed this login (saved for another
/// site, or picked from all logins) for the page's site with this host; the page must still be there.
pub async fn fill(app: &AppHandle, tab: &str, item: &str, confirmed_host: Option<String>) -> Result<()> {
    let (site, url) = page_site(app, tab)?;
    let state = app.state::<AppState>();
    match confirmed_host.as_deref().map(bare) {
        Some(h) if h != site.host => bail!("The page is no longer on {h}; nothing was filled."),
        Some(_) => {}
        None => {
            let found = state.creds.items(app).into_iter().find(|i| i.id == item).ok_or_else(|| anyhow!("That login is no longer loaded; open its source again."))?;
            if !fits_site(&found, &site) {
                bail!("{} is not saved for {}.", found.name, site.host);
            }
        }
    }
    let (user, password) = state.creds.login(app, item).await?;
    // The page may have moved on while the source was asked: it must still be on the same site.
    let (now, _) = page_site(app, tab)?;
    if now != site {
        bail!("The page went to another site while the login was fetched; nothing was filled.");
    }
    webview(app, tab)?.eval(script(url.host_str().unwrap_or(""), &user, &password))?;
    Ok(())
}

/// The script that fills the form: the password field, and the user field before it; on sign-in pages that ask for
/// the name first, only a field that is clearly for the user name. It sets the values the way typing does, so the
/// page notices.
fn script(host: &str, user: &str, password: &str) -> String {
    let q = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(() => {{
  if (location.hostname !== {host}) return;
  const shown = (e) => !!(e.offsetWidth || e.offsetHeight || e.getClientRects().length) && getComputedStyle(e).visibility !== "hidden";
  const fields = [...document.querySelectorAll("input")].filter((e) => shown(e) && !e.disabled && !e.readOnly);
  const textual = (e) => ["", "text", "email", "tel"].includes(e.type);
  const userish = (e) => e.type === "email" || /user|e-?mail|login|account|loginfmt/i.test(e.name + " " + e.id + " " + e.autocomplete);
  const pw = fields.find((e) => e.type === "password");
  const before = pw ? fields.slice(0, fields.indexOf(pw)).filter(textual) : fields.filter((e) => textual(e) && userish(e));
  const user = before.find(userish) ?? (pw ? before[before.length - 1] : undefined);
  const set = (e, v) => {{
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(e, v);
    e.dispatchEvent(new Event("input", {{ bubbles: true }}));
    e.dispatchEvent(new Event("change", {{ bubbles: true }}));
  }};
  const u = {user}, p = {password};
  if (user && u) set(user, u);
  if (pw) {{ set(pw, p); pw.focus(); }} else if (user) user.focus();
}})();"#,
        host = q(host),
        user = q(user),
        password = q(password),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, uris: &[&str]) -> Item {
        Item { id: "x".into(), source: "s".into(), name: name.into(), username: "u".into(), uris: uris.iter().map(|s| s.to_string()).collect(), folder: String::new(), organization: String::new(), has_password: true }
    }

    fn fits(item: &Item, page: &str) -> bool {
        fits_site(item, &site_of(&Url::parse(page).unwrap()).unwrap())
    }

    #[test]
    fn which_logins_fit_a_page() {
        assert!(fits(&item("x", &["https://portal.azure.com/"]), "https://portal.azure.com/"));
        assert!(fits(&item("x", &["portal.azure.com"]), "https://www.portal.azure.com/"));
        assert!(fits(&item("x", &["example.com"]), "https://login.example.com/"));
        // A Microsoft login fits Microsoft's own sign-in pages, where the portal sends you.
        assert!(fits(&item("x", &["https://portal.azure.com"]), "https://login.microsoftonline.com/x"));
        assert!(fits(&item("x", &["https://teams.microsoft.com"]), "https://login.microsoftonline.com/x"));
        assert!(fits(&item("x", &["https://mail.google.com"]), "https://accounts.google.com/"));
        // But not pages in Microsoft's or Google's domains that anyone can make.
        assert!(!fits(&item("x", &["https://portal.azure.com"]), "https://evil.sharepoint.com/"));
        assert!(!fits(&item("x", &["https://portal.azure.com"]), "https://evil.z13.web.core.windows.net/"));
        assert!(!fits(&item("x", &["https://portal.azure.com"]), "https://x.blob.core.windows.net/"));
        assert!(!fits(&item("x", &["https://mail.google.com"]), "https://sites.google.com/"));
        // Not another site, nor a look-alike, nor a bare word.
        assert!(!fits(&item("x", &["https://portal.azure.com"]), "https://accounts.google.com/"));
        assert!(!fits(&item("x", &["example.com"]), "https://example.com.evil.net/"));
        assert!(!fits(&item("x", &["example.com"]), "https://notexample.com/"));
        assert!(!fits(&item("x", &["corp"]), "https://intranet.corp/"));
        assert!(!fits(&item("x", &[]), "https://example.com/"));
        // IP addresses: exactly, with the port.
        assert!(fits(&item("x", &["https://10.0.0.5:8006"]), "https://10.0.0.5:8006/"));
        assert!(!fits(&item("x", &["https://10.0.0.5:8006"]), "https://10.0.0.5:9090/"));
        assert!(!fits(&item("x", &["10.0.0.5"]), "https://10.0.0.55/"));
    }

    #[test]
    fn named_logins() {
        let page = |u: &str| site_of(&Url::parse(u).unwrap()).unwrap();
        assert!(named(&item("Azure portal (work)", &[]), &page("https://portal.azure.com/")));
        assert!(!named(&item("Teams", &[]), &page("https://portal.azure.com/")));
        assert!(!named(&item("10", &[]), &page("https://10.0.0.5/")));
    }

    #[test]
    fn the_fill_script_quotes_what_it_fills() {
        let s = script("portal.azure.com", "me\"@x", "p'w\\\"</script>");
        assert!(s.contains(r#"const u = "me\"@x", p = "p'w\\\"</script>";"#));
        assert!(s.contains(r#"location.hostname !== "portal.azure.com""#));
    }
}

//! Which logins fit a host: shared by every credential source.

use super::Item;

/// The host part of a vault URI: "ssh://root@10.0.0.5:22/x" gives "10.0.0.5".
pub fn uri_host(uri: &str) -> String {
    let u = uri.trim().to_lowercase();
    let u = u.split_once("://").map(|(_, r)| r).unwrap_or(&u);
    let u = u.split(['/', '?', '#']).next().unwrap_or("");
    let u = u.rsplit_once('@').map(|(_, h)| h).unwrap_or(u);
    if let Some(rest) = u.strip_prefix('[') {
        return rest.split(']').next().unwrap_or("").to_string();
    }
    match u.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') && p.chars().all(|c| c.is_ascii_digit()) => h.to_string(),
        _ => u.to_string(),
    }
}

/// "DOMAIN\user" and "user@domain.tld" both compare as "user". "root@pam!token" (a Proxmox API token)
/// stays whole, so it never passes for the user root.
pub fn bare_user(u: &str) -> String {
    let u = u.trim().to_lowercase();
    let u = u.rsplit_once('\\').map(|(_, n)| n.to_string()).unwrap_or(u);
    match u.split_once('@') {
        Some((n, d)) if d.contains('.') && d.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-') => n.to_string(),
        _ => u,
    }
}

pub fn scheme(uri: &str) -> String {
    uri.trim().to_lowercase().split_once("://").map(|(s, _)| s.to_string()).unwrap_or_default()
}

/// Vault logins that belong to a host, best first: the address in the item's URIs, or the item named
/// after the host or the session. With a user name, logins for another user are left out.
pub fn rank(items: &[Item], protocol: &str, host: &str, user: &str, name: &str) -> Vec<Item> {
    rank_scored(items, protocol, host, user, name).into_iter().map(|(_, i)| i).collect()
}

/// rank, with each login's score: higher fits better.
pub fn rank_scored(items: &[Item], protocol: &str, host: &str, user: &str, name: &str) -> Vec<(i32, Item)> {
    let host = host.trim().to_lowercase();
    let name = name.trim().to_lowercase();
    let short = if host.parse::<std::net::IpAddr>().is_ok() { String::new() } else { host.split('.').next().unwrap_or("").to_string() };
    let want = bare_user(user);
    let mut scored: Vec<(i32, &Item)> = items
        .iter()
        .filter(|i| i.has_password)
        .filter_map(|i| {
            let iname = i.name.trim().to_lowercase();
            let at_host: Vec<&String> = i.uris.iter().filter(|u| !host.is_empty() && uri_host(u) == host).collect();
            let mut score = if !at_host.is_empty() {
                // A website at the same address (http://host) is most likely not the login for SSH or RDP.
                if at_host.iter().all(|u| matches!(scheme(u).as_str(), "http" | "https")) { 3 } else { 6 }
            } else if !host.is_empty() && (iname == host || iname.split([' ', '(', ')', ',', '/']).any(|w| w == host)) {
                5
            } else if !name.is_empty() && iname == name {
                4
            } else if short.len() >= 3 && (iname == short || iname.split([' ', '(', ')', ',', '/', '.']).any(|w| w == short) || i.uris.iter().any(|u| uri_host(u).split('.').next() == Some(short.as_str()))) {
                3
            } else {
                return None;
            };
            // Items made for this kind of login: "ssh: pve-main (...)" or an ssh:// address.
            if iname.starts_with(&format!("{protocol}:")) || iname.starts_with(&format!("{protocol} ")) || i.uris.iter().any(|u| scheme(u) == protocol) {
                score += 2;
            }
            if !want.is_empty() {
                let have = bare_user(&i.username);
                if have == want {
                    score += 2;
                } else if !have.is_empty() {
                    return None;
                }
            }
            Some((score, i))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase())));
    scored.into_iter().map(|(s, i)| (s, i.clone())).collect()
}

#[cfg(test)]
mod match_tests {
    use super::*;

    fn item(id: &str, name: &str, user: &str, uris: &[&str]) -> Item {
        Item { id: id.into(), source: String::new(), name: name.into(), username: user.into(), uris: uris.iter().map(|s| s.to_string()).collect(), folder: String::new(), organization: String::new(), has_password: true }
    }

    #[test]
    fn hosts_from_uris() {
        assert_eq!(uri_host("ssh://root@10.0.0.5:22/x"), "10.0.0.5");
        assert_eq!(uri_host("https://Proxmox2.example.net"), "proxmox2.example.net");
        assert_eq!(uri_host("10.20.30.53:8006"), "10.20.30.53");
        assert_eq!(uri_host("rdp://[fe80::1]:3389"), "fe80::1");
    }

    #[test]
    fn users() {
        assert_eq!(bare_user("CORP\\admin.jan"), "admin.jan");
        assert_eq!(bare_user("admin.jan@corp.example"), "admin.jan");
        assert_eq!(bare_user("root@pam!dashboard-readonly"), "root@pam!dashboard-readonly");
    }

    #[test]
    fn ranking() {
        let items = vec![
            item("a", "pve root", "root", &["https://10.20.30.53:8006"]),
            item("b", "dc01", "CORP\\admin.jan", &[]),
            item("c", "something else", "root", &["https://example.com"]),
            item("d", "docker01", "jan", &["ssh://10.20.30.54"]),
            item("e", "docker01 old", "root", &["ssh://10.20.30.54"]),
        ];
        let ids = |v: Vec<Item>| v.into_iter().map(|i| i.id).collect::<Vec<_>>();
        assert_eq!(ids(rank(&items, "ssh", "10.20.30.53", "root", "")), vec!["a"]);
        assert_eq!(ids(rank(&items, "rdp", "dc01.corp.example", "admin.jan", "")), vec!["b"]);
        assert_eq!(ids(rank(&items, "ssh", "10.20.30.54", "jan", "")), vec!["d"]);
        assert_eq!(ids(rank(&items, "ssh", "10.20.30.54", "", "")), vec!["d", "e"]);
        assert_eq!(ids(rank(&items, "ssh", "10.9.9.9", "", "docker01")), vec!["d"]);
        assert!(rank(&items, "ssh", "10.9.9.9", "root", "").is_empty());
        // The pve case from the real vault: the SSH login wins; the API token and the website do not count.
        let pve = vec![
            item("web", "http://10.20.30.200", "jan", &["http://10.20.30.200"]),
            item("tok", "proxmox-dashboard", "root@pam!dashboard-readonly", &["https://10.20.30.200:8006"]),
            item("ssh", "ssh: pve-main (10.20.30.200)", "root", &["10.20.30.200"]),
        ];
        assert_eq!(ids(rank(&pve, "ssh", "10.20.30.200", "root", "")), vec!["ssh"]);
        assert_eq!(ids(rank(&pve, "ssh", "10.20.30.200", "", ""))[0], "ssh");
        // The test case that asked needlessly: a Telnet and an FTP login for the same host and user.
        let same = vec![
            item("t", "telnet: test (10.0.0.9)", "ttest", &["telnet://10.0.0.9:2323"]),
            item("f", "ftp: test (10.0.0.9)", "ttest", &["ftp://10.0.0.9:2121"]),
        ];
        let s = rank_scored(&same, "telnet", "10.0.0.9", "ttest", "");
        assert_eq!(s[0].1.id, "t");
        assert!(s[0].0 > s[1].0, "the Telnet login fits clearly better, so no list");
    }
}

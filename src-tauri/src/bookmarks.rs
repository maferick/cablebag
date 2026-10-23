//! Bookmarks of Chrome, Edge and Firefox on this PC, to import as web sessions. Only web addresses (http and
//! https) come along, in their bookmark folders; with more than one browser profile, each profile is a folder
//! of its own. The browsers' files are only read: Firefox's database is copied first, as Firefox keeps it locked
//! while it runs.

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub struct Bookmark {
    pub folder: Vec<String>,
    pub name: String,
    pub url: String,
}

fn web(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    u.starts_with("http://") || u.starts_with("https://")
}

fn env_dir(var: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(var).unwrap_or_default())
}

pub fn chrome() -> Result<Vec<Bookmark>> {
    chromium(&env_dir("LOCALAPPDATA").join("Google").join("Chrome").join("User Data"))
}

pub fn edge() -> Result<Vec<Bookmark>> {
    chromium(&env_dir("LOCALAPPDATA").join("Microsoft").join("Edge").join("User Data"))
}

pub fn firefox() -> Result<Vec<Bookmark>> {
    firefox_in(&env_dir("APPDATA").join("Mozilla").join("Firefox").join("Profiles"))
}

/// Chrome and Edge: a folder per browser profile, each with a Bookmarks file (JSON), and since 2025 also
/// AccountBookmarks for the bookmarks of the signed-in account (often the only one); the profiles' own names are in
/// Local State.
fn chromium(user_data: &Path) -> Result<Vec<Bookmark>> {
    let names: HashMap<String, String> = std::fs::read_to_string(user_data.join("Local State"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["profile"]["info_cache"].as_object().cloned())
        .map(|o| o.into_iter().filter_map(|(dir, info)| Some((dir, info["name"].as_str()?.to_string()))).collect())
        .unwrap_or_default();
    let mut profiles: Vec<PathBuf> = std::fs::read_dir(user_data)
        .with_context(|| format!("{} was not found", user_data.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| FILES.iter().any(|f| p.join(f).is_file()))
        .collect();
    profiles.sort();
    let many = profiles.len() > 1;
    let mut out: Vec<Bookmark> = Vec::new();
    for (dir, file) in profiles.iter().flat_map(|d| FILES.iter().map(move |f| (d, *f))) {
        let Ok(text) = std::fs::read_to_string(dir.join(file)) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        let dir_name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut base = Vec::new();
        if many {
            base.push(names.get(&dir_name).cloned().unwrap_or(dir_name));
        }
        for (root, label) in [("bookmark_bar", "Bookmarks bar"), ("other", "Other bookmarks"), ("synced", "Mobile bookmarks")] {
            let mut path = base.clone();
            path.push(label.to_string());
            let mut found = Vec::new();
            walk(&v["roots"][root], &mut path, &mut found);
            // The same bookmark in both files comes once.
            for b in found {
                if !out.contains(&b) {
                    out.push(b);
                }
            }
        }
    }
    Ok(out)
}

/// The bookmark files of a Chromium profile: the local ones and those of the signed-in account.
const FILES: [&str; 2] = ["Bookmarks", "AccountBookmarks"];

fn walk(node: &Value, path: &mut Vec<String>, out: &mut Vec<Bookmark>) {
    for child in node["children"].as_array().into_iter().flatten() {
        let name = child["name"].as_str().unwrap_or("").trim().to_string();
        match child["type"].as_str() {
            Some("folder") => {
                path.push(if name.is_empty() { "Folder".into() } else { name });
                walk(child, path, out);
                path.pop();
            }
            Some("url") => {
                let url = child["url"].as_str().unwrap_or("");
                if web(url) {
                    out.push(Bookmark { folder: path.clone(), name: if name.is_empty() { url.to_string() } else { name }, url: url.to_string() });
                }
            }
            _ => {}
        }
    }
}

/// Firefox: places.sqlite in each profile folder (named like abcd1234.default-release).
fn firefox_in(profiles_dir: &Path) -> Result<Vec<Bookmark>> {
    let mut profiles: Vec<PathBuf> = std::fs::read_dir(profiles_dir)
        .with_context(|| format!("{} was not found", profiles_dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("places.sqlite").is_file())
        .collect();
    profiles.sort();
    let many = profiles.len() > 1;
    let mut out = Vec::new();
    for dir in profiles {
        let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let label = name.split_once('.').map(|(_, l)| l.to_string()).unwrap_or(name);
        let base = if many { vec![label] } else { Vec::new() };
        if let Ok(list) = places(&dir.join("places.sqlite"), &base) {
            out.extend(list);
        }
    }
    Ok(out)
}

/// A row of moz_bookmarks with its address: id, parent, type (1 bookmark, 2 folder), title, guid, url.
type Row = (i64, i64, i64, Option<String>, String, Option<String>);

/// The bookmarks in one places.sqlite, read from a copy (with its write-ahead log, which holds recent changes).
fn places(db: &Path, base: &[String]) -> Result<Vec<Bookmark>> {
    // The copy holds the whole browsing history: a folder only this user can read (on Windows the temp folder
    // already is).
    let tmp = std::env::temp_dir().join(format!("cablebag-places-{}", uuid::Uuid::new_v4()));
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut std::fs::DirBuilder::new(), 0o700).create(&tmp)?;
    #[cfg(not(unix))]
    std::fs::create_dir_all(&tmp)?;
    let copy = tmp.join("places.sqlite");
    let result = (|| -> Result<Vec<Bookmark>> {
        std::fs::copy(db, &copy)?;
        let wal = db.with_file_name("places.sqlite-wal");
        if wal.is_file() {
            std::fs::copy(&wal, tmp.join("places.sqlite-wal"))?;
        }
        let conn = rusqlite::Connection::open(&copy)?;
        // id -> (parent, type, title, guid, url); type 1 is a bookmark, 2 a folder.
        let mut stmt = conn.prepare("SELECT b.id, b.parent, b.type, b.title, b.guid, p.url FROM moz_bookmarks b LEFT JOIN moz_places p ON b.fk = p.id")?;
        let rows: Vec<Row> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?.collect::<Result<_, _>>()?;
        let by_id: HashMap<i64, &Row> = rows.iter().map(|r| (r.0, r)).collect();
        let mut out = Vec::new();
        'items: for r in rows.iter().filter(|r| r.2 == 1) {
            let url = r.5.clone().unwrap_or_default();
            if !web(&url) {
                continue;
            }
            let mut folder = Vec::new();
            let mut parent = r.1;
            for _ in 0..64 {
                let Some(p) = by_id.get(&parent) else { break };
                match p.4.as_str() {
                    "root________" => break,
                    // Tags are not folders of bookmarks.
                    "tags________" => continue 'items,
                    "toolbar_____" => folder.push("Bookmarks toolbar".to_string()),
                    "menu________" => folder.push("Bookmarks menu".to_string()),
                    "unfiled_____" => folder.push("Other bookmarks".to_string()),
                    "mobile______" => folder.push("Mobile bookmarks".to_string()),
                    _ => folder.push(p.3.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| "Folder".into())),
                }
                parent = p.1;
            }
            folder.reverse();
            let name = r.3.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| url.clone());
            out.push(Bookmark { folder: base.iter().cloned().chain(folder).collect(), name: name.trim().to_string(), url });
        }
        Ok(out)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cablebag-bm-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn chromium_profiles_and_folders() {
        let d = temp("chrome");
        std::fs::write(d.join("Local State"), r#"{"profile":{"info_cache":{"Default":{"name":"Work"},"Profile 1":{"name":"Home"}}}}"#).unwrap();
        for (dir, url) in [("Default", "https://portal.azure.com/"), ("Profile 1", "https://example.org/")] {
            std::fs::create_dir_all(d.join(dir)).unwrap();
            let json = format!(
                r#"{{"roots":{{"bookmark_bar":{{"children":[{{"type":"folder","name":"Azure","children":[{{"type":"url","name":"Portal","url":"{url}"}},{{"type":"url","name":"Settings","url":"chrome://settings"}}]}}]}},"other":{{"children":[]}},"synced":{{"children":[]}}}}}}"#
            );
            std::fs::write(d.join(dir).join("Bookmarks"), json).unwrap();
        }
        // The account's bookmarks: one the same as the local file, one more.
        std::fs::write(
            d.join("Default").join("AccountBookmarks"),
            r#"{"roots":{"bookmark_bar":{"children":[{"type":"folder","name":"Azure","children":[{"type":"url","name":"Portal","url":"https://portal.azure.com/"},{"type":"url","name":"Entra","url":"https://entra.microsoft.com/"}]}]},"other":{"children":[]},"synced":{"children":[]}}}"#,
        )
        .unwrap();
        let list = chromium(&d).unwrap();
        assert_eq!(list.len(), 3, "chrome:// is left out, the same bookmark twice comes once");
        assert!(list.iter().any(|b| b.name == "Entra" && b.folder[0] == "Work"));
        assert_eq!(list[0], Bookmark { folder: vec!["Work".into(), "Bookmarks bar".into(), "Azure".into()], name: "Portal".into(), url: "https://portal.azure.com/".into() });
        assert!(list.iter().any(|b| b.folder[0] == "Home" && b.url == "https://example.org/"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn firefox_places() {
        let d = temp("firefox");
        let prof = d.join("abcd.default-release");
        std::fs::create_dir_all(&prof).unwrap();
        let conn = rusqlite::Connection::open(prof.join("places.sqlite")).unwrap();
        conn.execute_batch(
            "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT);
             CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, title TEXT, guid TEXT);
             INSERT INTO moz_places VALUES (1, 'https://teams.microsoft.com/'), (2, 'place:sort=8'), (3, 'https://tagged.example/');
             INSERT INTO moz_bookmarks VALUES (1, 2, NULL, 0, '', 'root________'), (2, 2, NULL, 1, 'toolbar', 'toolbar_____'),
               (3, 2, NULL, 2, 'Work', 'f1'), (4, 1, 1, 3, 'Teams', 'b1'), (5, 1, 2, 2, 'Recent', 'b2'),
               (6, 2, NULL, 1, 'tags', 'tags________'), (7, 2, NULL, 6, 'x', 't1'), (8, 1, 3, 7, 'Tagged', 'b3');",
        )
        .unwrap();
        drop(conn);
        let list = firefox_in(&d).unwrap();
        assert_eq!(list, vec![Bookmark { folder: vec!["Bookmarks toolbar".into(), "Work".into()], name: "Teams".into(), url: "https://teams.microsoft.com/".into() }]);
        let _ = std::fs::remove_dir_all(&d);
    }
}

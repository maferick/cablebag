//! Remote file browser operations over SFTP.

use crate::ssh::sftp;
use anyhow::{anyhow, Result};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Serialize)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_link: bool,
    pub size: u64,
    pub modified: u64,
    pub mode: u32,
    pub permissions: String,
    pub owner: String,
    pub group: String,
}

#[derive(Debug, Serialize)]
pub struct Listing {
    pub path: String,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Serialize)]
struct Progress<'a> {
    tab: &'a str,
    name: &'a str,
    done: u64,
    total: u64,
    finished: bool,
}

pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') { format!("{dir}{name}") } else { format!("{dir}/{name}") }
}

/// The last part of a path on the server, as a file name that is safe on this PC. A server may name a file
/// ..\..\x or C:\x (on Linux those are ordinary names), and Windows reads \ and : as parts of a path, so such a
/// name would otherwise land outside the chosen folder. Characters Windows does not allow become _, as do
/// trailing dots and spaces (Windows drops them, so .. would be the folder above), and device names such as
/// CON or COM1 get a _ in front.
pub fn local_name(remote: &str) -> String {
    let last = remote.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    let mut s: String = last
        .chars()
        .map(|c| if c < ' ' || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let keep = s.trim_end_matches(['.', ' ']).len();
    if keep < s.len() {
        s = format!("{}{}", &s[..keep], "_".repeat(s.len() - keep));
    }
    if s.is_empty() {
        return "download".into();
    }
    let stem = s.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.chars().count() == 4
            && stem.chars().last().is_some_and(|c| c.is_ascii_digit() || matches!(c, '¹' | '²' | '³')));
    if device {
        s.insert(0, '_');
    }
    s
}

fn perm_string(mode: u32, is_dir: bool, is_link: bool) -> String {
    let mut s = String::with_capacity(10);
    s.push(if is_link { 'l' } else if is_dir { 'd' } else { '-' });
    for shift in [6, 3, 0] {
        let b = (mode >> shift) & 7;
        s.push(if b & 4 != 0 { 'r' } else { '-' });
        s.push(if b & 2 != 0 { 'w' } else { '-' });
        s.push(if b & 1 != 0 { 'x' } else { '-' });
    }
    s
}

pub async fn home(app: &AppHandle, tab: &str) -> Result<String> {
    Ok(sftp(app, tab).await?.canonicalize(".").await?)
}

pub async fn list(app: &AppHandle, tab: &str, path: &str) -> Result<Listing> {
    let s = sftp(app, tab).await?;
    let path = s.canonicalize(if path.is_empty() { "." } else { path }).await?;
    let mut entries = Vec::new();
    for e in s.read_dir(&path).await? {
        let name = e.file_name();
        if name == "." || name == ".." {
            continue;
        }
        let meta = e.metadata();
        let is_link = meta.is_symlink();
        let full = join(&path, &name);
        let mut is_dir = meta.is_dir();
        if is_link {
            // Follow the link so folders behind symlinks can be opened.
            if let Ok(m) = s.metadata(&full).await {
                is_dir = m.is_dir();
            }
        }
        let mode = meta.permissions.unwrap_or(0) & 0o7777;
        entries.push(Entry {
            permissions: perm_string(mode, is_dir, is_link),
            name,
            path: full,
            is_dir,
            is_link,
            size: meta.size.unwrap_or(0),
            modified: meta.mtime.unwrap_or(0) as u64,
            mode,
            owner: meta.user.clone().unwrap_or_else(|| meta.uid.map(|u| u.to_string()).unwrap_or_default()),
            group: meta.group.clone().unwrap_or_else(|| meta.gid.map(|g| g.to_string()).unwrap_or_default()),
        });
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(Listing { path, entries })
}

pub async fn mkdir(app: &AppHandle, tab: &str, path: &str) -> Result<()> {
    Ok(sftp(app, tab).await?.create_dir(path).await?)
}

pub async fn new_file(app: &AppHandle, tab: &str, path: &str) -> Result<()> {
    let f = sftp(app, tab).await?.create(path).await?;
    drop(f);
    Ok(())
}

pub async fn rename(app: &AppHandle, tab: &str, from: &str, to: &str) -> Result<()> {
    Ok(sftp(app, tab).await?.rename(from, to).await?)
}

pub async fn chmod(app: &AppHandle, tab: &str, path: &str, mode: u32, recursive: bool) -> Result<()> {
    let s = sftp(app, tab).await?;
    chmod_inner(&s, path, mode, recursive).await
}

async fn chmod_inner(s: &SftpSession, path: &str, mode: u32, recursive: bool) -> Result<()> {
    let mut meta = s.metadata(path).await?;
    let is_dir = meta.is_dir();
    let kind = meta.permissions.unwrap_or(0) & !0o7777;
    meta.permissions = Some(kind | (mode & 0o7777));
    s.set_metadata(path, meta).await?;
    if recursive && is_dir {
        for e in s.read_dir(path).await? {
            let n = e.file_name();
            if n == "." || n == ".." || e.metadata().is_symlink() {
                continue;
            }
            Box::pin(chmod_inner(s, &join(path, &n), mode, true)).await?;
        }
    }
    Ok(())
}

pub async fn remove(app: &AppHandle, tab: &str, path: &str) -> Result<()> {
    let s = sftp(app, tab).await?;
    remove_inner(&s, path).await
}

async fn remove_inner(s: &SftpSession, path: &str) -> Result<()> {
    let meta = s.symlink_metadata(path).await?;
    if meta.is_dir() {
        for e in s.read_dir(path).await? {
            let n = e.file_name();
            if n == "." || n == ".." {
                continue;
            }
            Box::pin(remove_inner(s, &join(path, &n))).await?;
        }
        s.remove_dir(path).await?;
    } else {
        s.remove_file(path).await?;
    }
    Ok(())
}

async fn download_file(app: &AppHandle, tab: &str, s: &SftpSession, remote: &str, local: &Path) -> Result<()> {
    let name = remote.rsplit('/').next().unwrap_or(remote).to_string();
    let total = s.metadata(remote).await.ok().and_then(|m| m.size).unwrap_or(0);
    let mut src = s.open(remote).await?;
    let mut dst = tokio::fs::File::create(local).await?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last = std::time::Instant::now();
    loop {
        let n = src.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n]).await?;
        done += n as u64;
        if last.elapsed().as_millis() > 200 {
            last = std::time::Instant::now();
            let _ = app.emit("transfer", Progress { tab, name: &name, done, total, finished: false });
        }
    }
    dst.flush().await?;
    let _ = app.emit("transfer", Progress { tab, name: &name, done, total, finished: true });
    Ok(())
}

async fn download_inner(app: &AppHandle, tab: &str, s: &SftpSession, remote: &str, local_dir: &Path) -> Result<()> {
    let target = local_dir.join(local_name(remote));
    if s.metadata(remote).await?.is_dir() {
        tokio::fs::create_dir_all(&target).await?;
        for e in s.read_dir(remote).await? {
            let n = e.file_name();
            if n == "." || n == ".." {
                continue;
            }
            Box::pin(download_inner(app, tab, s, &join(remote, &n), &target)).await?;
        }
        Ok(())
    } else {
        download_file(app, tab, s, remote, &target).await
    }
}

pub async fn download(app: &AppHandle, tab: &str, remotes: &[String], local_dir: &str) -> Result<()> {
    let s = sftp(app, tab).await?;
    for r in remotes {
        download_inner(app, tab, &s, r, Path::new(local_dir)).await?;
    }
    Ok(())
}

async fn upload_file(app: &AppHandle, tab: &str, s: &SftpSession, local: &Path, remote: &str) -> Result<()> {
    let name = local.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let total = tokio::fs::metadata(local).await.map(|m| m.len()).unwrap_or(0);
    let mut src = tokio::fs::File::open(local).await?;
    let mut dst = s
        .open_with_flags(remote, OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE)
        .await?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last = std::time::Instant::now();
    loop {
        let n = src.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n]).await?;
        done += n as u64;
        if last.elapsed().as_millis() > 200 {
            last = std::time::Instant::now();
            let _ = app.emit("transfer", Progress { tab, name: &name, done, total, finished: false });
        }
    }
    dst.shutdown().await?;
    let _ = app.emit("transfer", Progress { tab, name: &name, done, total, finished: true });
    Ok(())
}

async fn upload_inner(app: &AppHandle, tab: &str, s: &SftpSession, local: &Path, remote_dir: &str) -> Result<()> {
    let name = local.file_name().ok_or_else(|| anyhow!("invalid local path"))?.to_string_lossy().to_string();
    let target = join(remote_dir, &name);
    if local.is_dir() {
        if !s.try_exists(&target).await.unwrap_or(false) {
            s.create_dir(&target).await?;
        }
        let mut rd = tokio::fs::read_dir(local).await?;
        while let Some(e) = rd.next_entry().await? {
            Box::pin(upload_inner(app, tab, s, &e.path(), &target)).await?;
        }
        Ok(())
    } else {
        upload_file(app, tab, s, local, &target).await
    }
}

pub async fn upload(app: &AppHandle, tab: &str, locals: &[String], remote_dir: &str) -> Result<()> {
    let s = sftp(app, tab).await?;
    for l in locals {
        upload_inner(app, tab, &s, Path::new(l), remote_dir).await?;
    }
    Ok(())
}

/// A remote text file opened in Cablebag's editor.
#[derive(Serialize)]
pub struct TextFile {
    pub text: String,
    /// A fingerprint of the content when it was read, to notice a change on the server before saving. The
    /// content itself, not the modification time: that counts in whole seconds, so two quick changes look alike.
    pub version: String,
    pub size: u64,
}

fn fingerprint(content: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(content))
}

/// Larger files are opened with a program on this PC instead.
const EDIT_MAX: u64 = 2 * 1024 * 1024;

pub async fn read_text(app: &AppHandle, tab: &str, path: &str) -> Result<TextFile> {
    let s = sftp(app, tab).await?;
    let meta = s.metadata(path).await?;
    let size = meta.size.unwrap_or(0);
    if size > EDIT_MAX {
        return Err(anyhow!("It is larger than 2 MB; open it with a program on this PC instead."));
    }
    let mut f = s.open(path).await?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).await?;
    if buf.contains(&0) {
        return Err(anyhow!("It is not a text file; open it with a program on this PC instead."));
    }
    let version = fingerprint(&buf);
    let text = String::from_utf8(buf).map_err(|_| anyhow!("It is not UTF-8 text; open it with a program on this PC instead."))?;
    Ok(TextFile { text, version, size })
}

/// Saves the editor's text. Refuses when the file changed on the server since it was read (its content, by
/// the fingerprint `read_version`); with `backup`, the version on the server is kept first as name.bak. The
/// file keeps its owner and permissions (it is written in place). Returns the new fingerprint.
pub async fn write_text(app: &AppHandle, tab: &str, path: &str, text: &str, read_version: &str, backup: bool) -> Result<String> {
    let s = sftp(app, tab).await?;
    if let Ok(meta) = s.metadata(path).await {
        let changed = anyhow!("The file changed on the server since you opened it. Open it again to see that version.");
        if meta.size.unwrap_or(0) > EDIT_MAX {
            return Err(changed);
        }
        let mut old = Vec::new();
        s.open(path).await?.read_to_end(&mut old).await?;
        if fingerprint(&old) != read_version {
            return Err(changed);
        }
        if backup {
            let bak_path = format!("{path}.bak");
            let mut bak = s.open_with_flags(&bak_path, OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE).await?;
            bak.write_all(&old).await?;
            bak.shutdown().await?;
            // The copy is as private as the file itself (a key or password file must not become readable).
            if let Some(mode) = meta.permissions {
                if let Ok(mut m) = s.metadata(&bak_path).await {
                    m.permissions = Some(mode);
                    let _ = s.set_metadata(&bak_path, m).await;
                }
            }
        }
    }
    let mut f = s.open_with_flags(path, OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE).await?;
    f.write_all(text.as_bytes()).await?;
    f.shutdown().await?;
    Ok(fingerprint(text.as_bytes()))
}

/// Downloads one file into a private temp folder so it can be opened locally.
pub async fn fetch_temp(app: &AppHandle, tab: &str, remote: &str) -> Result<PathBuf> {
    let s = sftp(app, tab).await?;
    let dir = std::env::temp_dir().join("Cablebag").join(uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&dir).await?;
    let local = dir.join(local_name(remote));
    download_file(app, tab, &s, remote, &local).await?;
    Ok(local)
}

/// Uploads a locally edited temp copy back to where it came from.
pub async fn push_back(app: &AppHandle, tab: &str, local: &str, remote: &str) -> Result<()> {
    let s = sftp(app, tab).await?;
    upload_file(app, tab, &s, Path::new(local), remote).await
}

#[cfg(test)]
mod tests {
    use super::local_name;
    use std::path::{Component, Path};

    #[test]
    fn names_from_the_server_stay_in_the_folder() {
        let dir = Path::new(r"C:\Users\someone\Downloads");
        for remote in [r"/srv/..\..\AppData\Roaming\x.ini", r"/srv/C:\Windows\x.dll", "/srv/..", "/srv/.", r"/srv/\server\share\x", "/srv/C:x", "/srv/a/../"] {
            let name = local_name(remote);
            let path = dir.join(&name);
            assert_eq!(path.parent(), Some(dir), "{remote} became {name}");
            assert!(Path::new(&name).components().all(|c| matches!(c, Component::Normal(_))), "{remote} became {name}");
        }
    }

    #[test]
    fn names_are_kept_where_they_are_safe() {
        assert_eq!(local_name("/etc/nginx/nginx.conf"), "nginx.conf");
        assert_eq!(local_name("/var/log/"), "log");
        assert_eq!(local_name("/"), "download");
        assert_eq!(local_name("/srv/a:b?.txt"), "a_b_.txt");
        assert_eq!(local_name("/srv/.."), "__");
        assert_eq!(local_name("/srv/name. "), "name__");
        assert_eq!(local_name("/srv/con.txt"), "_con.txt");
        assert_eq!(local_name("/srv/COM1"), "_COM1");
        assert_eq!(local_name("/srv/LPT¹"), "_LPT¹");
        assert_eq!(local_name("/srv/console.log"), "console.log");
        assert_eq!(local_name("/srv/.bashrc"), ".bashrc");
    }
}

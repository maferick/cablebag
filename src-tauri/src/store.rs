//! Local database: folders, sessions, settings, known host keys and encrypted secrets.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    // CABLEBAG_DATA_DIR runs Cablebag on another data folder (demo data for screenshots, tests).
    let dir = std::env::var_os("CABLEBAG_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("Cablebag"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// The app was called TraxTerm before. On the first start under the new name its data folder is copied
/// over (not moved, so an old install keeps working) and the database gets the new file name.
fn adopt_old_data() {
    if std::env::var_os("CABLEBAG_DATA_DIR").is_some() {
        return;
    }
    let Some(base) = dirs::config_dir() else { return };
    let (old, new) = (base.join("TraxTerm"), base.join("Cablebag"));
    if !old.join("traxterm.db").exists() || new.join("cablebag.db").exists() {
        return;
    }
    fn copy(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            let target = to.join(e.file_name());
            // The browser tabs' profile is only a cache, and often locked while the old app runs.
            if e.file_name() == "browser" {
                continue;
            }
            if e.file_type()?.is_dir() {
                copy(&e.path(), &target)?;
            } else {
                std::fs::copy(e.path(), &target)?;
            }
        }
        Ok(())
    }
    if let Err(e) = copy(&old, &new) {
        eprintln!("could not copy the TraxTerm data folder: {e}");
        return;
    }
    for suffix in ["", "-wal", "-shm"] {
        let from = new.join(format!("traxterm.db{suffix}"));
        if from.exists() {
            let _ = std::fs::rename(&from, new.join(format!("cablebag.db{suffix}")));
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub folder_id: Option<String>,
    pub name: String,
    /// ssh, telnet, rsh, xdmcp, rdp, vnc, ftp, sftp, serial, file, shell, browser, mosh, s3, wsl
    pub kind: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    /// Settings that depend on the session type, as JSON.
    pub data: Value,
    pub comment: String,
    pub sort: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownHost {
    pub host: String,
    pub port: i64,
    pub algo: String,
    pub key: String,
    pub fingerprint: String,
    pub added: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretInfo {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub protocol: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub updated: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiLogEntry {
    pub at: i64,
    pub host: String,
    pub model: String,
    pub about: String,
    pub direction: String,
    pub text: String,
}

pub struct Store {
    conn: Connection,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Store {
    pub fn open() -> Result<Self> {
        adopt_old_data();
        let path = data_dir().join("cablebag.db");
        let conn = Connection::open(&path).with_context(|| format!("opening {}", path.display()))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS folders(
                id TEXT PRIMARY KEY, parent_id TEXT REFERENCES folders(id) ON DELETE CASCADE,
                name TEXT NOT NULL, sort INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS sessions(
                id TEXT PRIMARY KEY, folder_id TEXT REFERENCES folders(id) ON DELETE CASCADE,
                name TEXT NOT NULL, kind TEXT NOT NULL, host TEXT NOT NULL DEFAULT '',
                port INTEGER NOT NULL DEFAULT 0, username TEXT NOT NULL DEFAULT '',
                data TEXT NOT NULL DEFAULT '{}', comment TEXT NOT NULL DEFAULT '',
                sort INTEGER NOT NULL DEFAULT 0, last_used INTEGER NOT NULL DEFAULT 0,
                created INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS known_hosts(
                host TEXT NOT NULL, port INTEGER NOT NULL, algo TEXT NOT NULL, key TEXT NOT NULL,
                fingerprint TEXT NOT NULL, added INTEGER NOT NULL, PRIMARY KEY(host, port, algo));
             CREATE TABLE IF NOT EXISTS secrets(
                id TEXT PRIMARY KEY, kind TEXT NOT NULL, label TEXT NOT NULL DEFAULT '',
                protocol TEXT NOT NULL DEFAULT '', host TEXT NOT NULL DEFAULT '',
                port INTEGER NOT NULL DEFAULT 0, username TEXT NOT NULL DEFAULT '',
                nonce BLOB NOT NULL, ciphertext BLOB NOT NULL, updated INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS vault(k TEXT PRIMARY KEY, v BLOB NOT NULL);
             CREATE TABLE IF NOT EXISTS ai_log(
                at INTEGER NOT NULL, host TEXT NOT NULL, model TEXT NOT NULL, about TEXT NOT NULL,
                direction TEXT NOT NULL, text TEXT NOT NULL);",
        )?;
        Ok(Self { conn })
    }

    // ---------- folders ----------

    pub fn folders(&self) -> Result<Vec<Folder>> {
        let mut st = self.conn.prepare("SELECT id, parent_id, name, sort FROM folders ORDER BY sort, name")?;
        let rows = st.query_map([], |r| {
            Ok(Folder { id: r.get(0)?, parent_id: r.get(1)?, name: r.get(2)?, sort: r.get(3)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn save_folder(&self, f: &Folder) -> Result<()> {
        self.conn.execute(
            "INSERT INTO folders(id, parent_id, name, sort) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET parent_id=?2, name=?3, sort=?4",
            params![f.id, f.parent_id, f.name, f.sort],
        )?;
        Ok(())
    }

    pub fn delete_folder(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM folders WHERE id=?1", params![id])?;
        Ok(())
    }

    /// Finds a folder by name under a parent, creating it when missing.
    pub fn ensure_folder(&self, parent_id: Option<&str>, name: &str) -> Result<String> {
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM folders WHERE name=?1 AND parent_id IS ?2",
                params![name, parent_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.save_folder(&Folder { id: id.clone(), parent_id: parent_id.map(str::to_string), name: name.into(), sort: 0 })?;
        Ok(id)
    }

    // ---------- sessions ----------

    fn row_to_session(r: &rusqlite::Row) -> rusqlite::Result<Session> {
        let data: String = r.get(7)?;
        Ok(Session {
            id: r.get(0)?,
            folder_id: r.get(1)?,
            name: r.get(2)?,
            kind: r.get(3)?,
            host: r.get(4)?,
            port: r.get(5)?,
            username: r.get(6)?,
            data: serde_json::from_str(&data).unwrap_or(Value::Object(Default::default())),
            comment: r.get(8)?,
            sort: r.get(9)?,
            last_used: r.get(10)?,
        })
    }

    const SESSION_COLS: &'static str =
        "id, folder_id, name, kind, host, port, username, data, comment, sort, last_used";

    pub fn sessions(&self) -> Result<Vec<Session>> {
        let sql = format!("SELECT {} FROM sessions ORDER BY sort, name COLLATE NOCASE", Self::SESSION_COLS);
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map([], Self::row_to_session)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn session(&self, id: &str) -> Result<Option<Session>> {
        let sql = format!("SELECT {} FROM sessions WHERE id=?1", Self::SESSION_COLS);
        Ok(self.conn.query_row(&sql, params![id], Self::row_to_session).optional()?)
    }

    pub fn find_session(&self, folder_id: Option<&str>, name: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM sessions WHERE name=?1 AND folder_id IS ?2",
                params![name, folder_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn save_session(&self, s: &Session) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions(id, folder_id, name, kind, host, port, username, data, comment, sort, last_used, created)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET folder_id=?2, name=?3, kind=?4, host=?5, port=?6, username=?7,
                data=?8, comment=?9, sort=?10",
            params![
                s.id, s.folder_id, s.name, s.kind, s.host, s.port, s.username,
                serde_json::to_string(&s.data)?, s.comment, s.sort, s.last_used, now()
            ],
        )?;
        Ok(())
    }

    pub fn delete_session(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM sessions WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn touch_session(&self, id: &str) -> Result<()> {
        self.conn.execute("UPDATE sessions SET last_used=?2 WHERE id=?1", params![id, now()])?;
        Ok(())
    }

    // ---------- settings ----------

    pub fn settings(&self) -> Result<serde_json::Map<String, Value>> {
        let mut st = self.conn.prepare("SELECT key, value FROM settings")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut map = serde_json::Map::new();
        for row in rows {
            let (k, v) = row?;
            map.insert(k, serde_json::from_str(&v).unwrap_or(Value::Null));
        }
        Ok(map)
    }

    pub fn set_setting(&self, key: &str, value: &Value) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Result<Option<Value>> {
        let v: Option<String> =
            self.conn.query_row("SELECT value FROM settings WHERE key=?1", params![key], |r| r.get(0)).optional()?;
        Ok(v.and_then(|s| serde_json::from_str(&s).ok()))
    }

    // ---------- known hosts ----------

    pub fn known_hosts_for(&self, host: &str, port: i64) -> Result<Vec<KnownHost>> {
        let mut st = self.conn.prepare(
            "SELECT host, port, algo, key, fingerprint, added FROM known_hosts WHERE host=?1 AND port=?2",
        )?;
        let rows = st.query_map(params![host, port], |r| {
            Ok(KnownHost {
                host: r.get(0)?,
                port: r.get(1)?,
                algo: r.get(2)?,
                key: r.get(3)?,
                fingerprint: r.get(4)?,
                added: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn all_known_hosts(&self) -> Result<Vec<KnownHost>> {
        let mut st = self
            .conn
            .prepare("SELECT host, port, algo, key, fingerprint, added FROM known_hosts ORDER BY host, port")?;
        let rows = st.query_map([], |r| {
            Ok(KnownHost {
                host: r.get(0)?,
                port: r.get(1)?,
                algo: r.get(2)?,
                key: r.get(3)?,
                fingerprint: r.get(4)?,
                added: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn save_known_host(&self, k: &KnownHost) -> Result<()> {
        self.conn.execute(
            "INSERT INTO known_hosts(host, port, algo, key, fingerprint, added) VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(host, port, algo) DO UPDATE SET key=?4, fingerprint=?5, added=?6",
            params![k.host, k.port, k.algo, k.key, k.fingerprint, k.added],
        )?;
        Ok(())
    }

    pub fn delete_known_host(&self, host: &str, port: i64, algo: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM known_hosts WHERE host=?1 AND port=?2 AND algo=?3",
            params![host, port, algo],
        )?;
        Ok(())
    }

    // ---------- the assistant's log: what was sent to an AI host and what came back ----------

    pub fn ai_log_add(&self, host: &str, model: &str, about: &str, direction: &str, text: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO ai_log(at, host, model, about, direction, text) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![now(), host, model, about, direction, text],
        )?;
        // Keep the newest 2000 entries.
        self.conn.execute("DELETE FROM ai_log WHERE rowid NOT IN (SELECT rowid FROM ai_log ORDER BY rowid DESC LIMIT 2000)", [])?;
        Ok(())
    }

    pub fn ai_log(&self, limit: usize) -> Result<Vec<AiLogEntry>> {
        let mut st = self.conn.prepare("SELECT at, host, model, about, direction, text FROM ai_log ORDER BY rowid DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit as i64], |r| {
            Ok(AiLogEntry { at: r.get(0)?, host: r.get(1)?, model: r.get(2)?, about: r.get(3)?, direction: r.get(4)?, text: r.get(5)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // ---------- secrets (values are encrypted by the vault before they get here) ----------

    #[allow(clippy::too_many_arguments)]
    pub fn put_secret(
        &self,
        kind: &str,
        label: &str,
        protocol: &str,
        host: &str,
        port: i64,
        username: &str,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<String> {
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM secrets WHERE kind=?1 AND label=?2 AND protocol=?3 AND host=?4 AND port=?5 AND username=?6",
                params![kind, label, protocol, host, port, username],
                |r| r.get(0),
            )
            .optional()?;
        let id = existing.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        self.conn.execute(
            "INSERT INTO secrets(id, kind, label, protocol, host, port, username, nonce, ciphertext, updated)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET nonce=?8, ciphertext=?9, updated=?10",
            params![id, kind, label, protocol, host, port, username, nonce, ciphertext, now()],
        )?;
        Ok(id)
    }

    pub fn find_secret(
        &self,
        kind: &str,
        label: &str,
        protocol: &str,
        host: &str,
        port: i64,
        username: &str,
    ) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT nonce, ciphertext FROM secrets
                 WHERE kind=?1 AND label=?2 AND protocol=?3 AND host=?4 AND port=?5 AND username=?6",
                params![kind, label, protocol, host, port, username],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn secret_by_id(&self, id: &str) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        Ok(self
            .conn
            .query_row("SELECT nonce, ciphertext FROM secrets WHERE id=?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?)
    }

    pub fn list_secrets(&self) -> Result<Vec<SecretInfo>> {
        let mut st = self.conn.prepare(
            "SELECT id, kind, label, protocol, host, port, username, updated FROM secrets ORDER BY kind, host, username",
        )?;
        let rows = st.query_map([], |r| {
            Ok(SecretInfo {
                id: r.get(0)?,
                kind: r.get(1)?,
                label: r.get(2)?,
                protocol: r.get(3)?,
                host: r.get(4)?,
                port: r.get(5)?,
                username: r.get(6)?,
                updated: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn delete_secret(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM secrets WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn vault_get(&self, k: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.conn.query_row("SELECT v FROM vault WHERE k=?1", params![k], |r| r.get(0)).optional()?)
    }

    pub fn vault_put(&self, k: &str, v: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT INTO vault(k, v) VALUES(?1, ?2) ON CONFLICT(k) DO UPDATE SET v=?2",
            params![k, v],
        )?;
        Ok(())
    }
}

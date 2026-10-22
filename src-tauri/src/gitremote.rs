//! Where synced files live: a repository on Gitea or Forgejo, GitHub or GitLab (through their APIs,
//! with a token), or any other git repository through the git program on this PC (with its own sign-in).
//! Each reads a file with a version marker and writes several files in one commit.

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

/// "Somebody else wrote in between": the sync tries once more.
pub const RACE: &str = "The repository changed while syncing";

const DESCRIPTION: &str = "Cablebag: sessions, folders, macros, tunnels and Proxmox servers (no passwords)";

pub enum Remote {
    Gitea(Api),
    GitHub(Api),
    GitLab(Api),
    Git(Cli),
}

pub struct Api {
    http: reqwest::Client,
    base: String,
    repo: String,
    token: String,
    branch: String,
}

pub struct Cli {
    url: String,
    dir: PathBuf,
    branch: String,
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent("Cablebag").timeout(Duration::from_secs(25)).build()?)
}

/// Which kind of server an address is, when not chosen: the big hosted ones by name, others by asking.
pub async fn detect(url: &str) -> Result<&'static str> {
    let host = url.split("://").nth(1).unwrap_or(url).split(['/', ':']).next().unwrap_or("").to_lowercase();
    if host == "github.com" || host.ends_with(".github.com") {
        return Ok("github");
    }
    if host == "gitlab.com" {
        return Ok("gitlab");
    }
    if url.starts_with("git@") || url.starts_with("ssh://") || url.ends_with(".git") {
        return Ok("git");
    }
    let http = client()?;
    let base = url.trim_end_matches('/');
    if let Ok(r) = http.get(format!("{base}/api/v1/version")).send().await {
        if r.status().is_success() && r.json::<Value>().await.ok().and_then(|v| v.get("version").cloned()).is_some() {
            return Ok("gitea");
        }
    }
    if let Ok(r) = http.get(format!("{base}/api/v4/version")).send().await {
        // GitLab answers 401 without a token, but it answers.
        if r.status().as_u16() == 401 || r.status().is_success() {
            return Ok("gitlab");
        }
    }
    bail!("Could not tell which git server {url} is. Choose it under Settings, Sync.")
}

impl Remote {
    pub fn new(kind: &str, url: &str, repo: &str, token: String, workdir: PathBuf) -> Result<Remote> {
        let url = url.trim().trim_end_matches('/').to_string();
        let repo = repo.trim().trim_matches('/').to_string();
        if kind != "git" && !repo.contains('/') {
            bail!("The repository looks like owner/name.");
        }
        let api = |base: String| -> Result<Api> { Ok(Api { http: client()?, base, repo: repo.clone(), token: token.clone(), branch: "main".into() }) };
        Ok(match kind {
            "gitea" => Remote::Gitea(api(format!("{url}/api/v1"))?),
            "github" => {
                let host = url.split("://").nth(1).unwrap_or(&url).split('/').next().unwrap_or("").to_lowercase();
                Remote::GitHub(api(if host == "github.com" || host.is_empty() { "https://api.github.com".into() } else { format!("{url}/api/v3") })?)
            }
            "gitlab" => Remote::GitLab(api(format!("{url}/api/v4"))?),
            "git" => Remote::Git(Cli { url, dir: workdir, branch: String::new() }),
            other => bail!("Unknown git server kind {other}."),
        })
    }

    /// Makes sure the repository exists (created private on the API kinds) and learns its branch.
    pub async fn open(&mut self) -> Result<()> {
        match self {
            Remote::Gitea(a) => a.gitea_open().await,
            Remote::GitHub(a) => a.github_open().await,
            Remote::GitLab(a) => a.gitlab_open().await,
            Remote::Git(c) => c.open().await,
        }
    }

    /// A file's text and its version, or None when it is not there yet.
    pub async fn read(&self, file: &str) -> Result<Option<(String, String)>> {
        match self {
            Remote::Gitea(a) => a.gitea_read(file).await,
            Remote::GitHub(a) => a.github_read(file).await,
            Remote::GitLab(a) => a.gitlab_read(file).await,
            Remote::Git(c) => c.read(file),
        }
    }

    /// Writes files in one commit: (path, text, version read before or None for a new file).
    pub async fn write(&self, files: Vec<(String, String, Option<String>)>, message: &str) -> Result<()> {
        match self {
            Remote::Gitea(a) => a.gitea_write(files, message).await,
            Remote::GitHub(a) => a.github_write(files, message).await,
            Remote::GitLab(a) => a.gitlab_write(files, message).await,
            Remote::Git(c) => c.write(files, message).await,
        }
    }
}

fn decode_b64(v: &Value) -> Result<String> {
    let raw = v.as_str().unwrap_or("").replace(['\n', '\r'], "");
    Ok(String::from_utf8(B64.decode(raw)?)?)
}

async fn fail(res: reqwest::Response, what: &str) -> anyhow::Error {
    let status = res.status();
    let body: String = res.text().await.unwrap_or_default().chars().take(200).collect();
    match status.as_u16() {
        401 => anyhow!("The git server refused the token ({what})."),
        403 => anyhow!("The token may not do this ({what}): {body}"),
        409 | 422 => anyhow!("{RACE} ({what})."),
        _ => anyhow!("The git server answered {status} ({what}): {body}"),
    }
}

impl Api {
    fn req(&self, method: reqwest::Method, path: &str, auth: &str) -> reqwest::RequestBuilder {
        let r = self.http.request(method, format!("{}{path}", self.base));
        match auth {
            "token" => r.header("Authorization", format!("token {}", self.token)),
            "bearer" => r.header("Authorization", format!("Bearer {}", self.token)).header("Accept", "application/vnd.github+json"),
            _ => r.header("PRIVATE-TOKEN", &self.token),
        }
    }

    fn owner_name(&self) -> (String, String) {
        let (o, n) = self.repo.split_once('/').unwrap_or(("", &self.repo));
        (o.to_string(), n.to_string())
    }

    // ---------- Gitea and Forgejo ----------

    async fn gitea_open(&mut self) -> Result<()> {
        let res = self.req(reqwest::Method::GET, &format!("/repos/{}", self.repo), "token").send().await.map_err(|e| anyhow!("Could not reach the git server: {e}"))?;
        if res.status().is_success() {
            let v: Value = res.json().await?;
            self.branch = v["default_branch"].as_str().unwrap_or("main").to_string();
            return Ok(());
        }
        if res.status().as_u16() != 404 {
            return Err(fail(res, "open the repository").await);
        }
        let (owner, name) = self.owner_name();
        let me: Value = self.req(reqwest::Method::GET, "/user", "token").send().await?.error_for_status()?.json().await?;
        let path = if me["login"].as_str().is_some_and(|l| l.eq_ignore_ascii_case(&owner)) { "/user/repos".to_string() } else { format!("/orgs/{owner}/repos") };
        let body = json!({ "name": name, "private": true, "auto_init": true, "default_branch": "main", "description": DESCRIPTION });
        let res = self.req(reqwest::Method::POST, &path, "token").json(&body).send().await?;
        if !res.status().is_success() {
            return Err(fail(res, "create the repository").await);
        }
        self.branch = "main".into();
        Ok(())
    }

    async fn gitea_read(&self, file: &str) -> Result<Option<(String, String)>> {
        let res = self.req(reqwest::Method::GET, &format!("/repos/{}/contents/{file}?ref={}", self.repo, self.branch), "token").send().await?;
        if res.status().as_u16() == 404 {
            return Ok(None);
        }
        if !res.status().is_success() {
            return Err(fail(res, "read").await);
        }
        let v: Value = res.json().await?;
        Ok(Some((decode_b64(&v["content"])?, v["sha"].as_str().unwrap_or("").to_string())))
    }

    async fn gitea_write(&self, files: Vec<(String, String, Option<String>)>, message: &str) -> Result<()> {
        let files: Vec<Value> = files
            .into_iter()
            .map(|(path, text, sha)| match sha {
                Some(sha) => json!({ "operation": "update", "path": path, "content": B64.encode(text), "sha": sha }),
                None => json!({ "operation": "create", "path": path, "content": B64.encode(text) }),
            })
            .collect();
        let res = self
            .req(reqwest::Method::POST, &format!("/repos/{}/contents", self.repo), "token")
            .json(&json!({ "branch": self.branch, "message": message, "files": files }))
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(fail(res, "write").await);
        }
        Ok(())
    }

    // ---------- GitHub ----------

    async fn github_open(&mut self) -> Result<()> {
        let res = self.req(reqwest::Method::GET, &format!("/repos/{}", self.repo), "bearer").send().await.map_err(|e| anyhow!("Could not reach GitHub: {e}"))?;
        if res.status().is_success() {
            let v: Value = res.json().await?;
            self.branch = v["default_branch"].as_str().unwrap_or("main").to_string();
            return Ok(());
        }
        if res.status().as_u16() != 404 {
            return Err(fail(res, "open the repository").await);
        }
        let (owner, name) = self.owner_name();
        let me: Value = self.req(reqwest::Method::GET, "/user", "bearer").send().await?.error_for_status()?.json().await?;
        let path = if me["login"].as_str().is_some_and(|l| l.eq_ignore_ascii_case(&owner)) { "/user/repos".to_string() } else { format!("/orgs/{owner}/repos") };
        let res = self.req(reqwest::Method::POST, &path, "bearer").json(&json!({ "name": name, "private": true, "auto_init": true, "description": DESCRIPTION })).send().await?;
        if !res.status().is_success() {
            return Err(fail(res, "create the repository").await);
        }
        let v: Value = res.json().await?;
        self.branch = v["default_branch"].as_str().unwrap_or("main").to_string();
        Ok(())
    }

    async fn github_read(&self, file: &str) -> Result<Option<(String, String)>> {
        let res = self.req(reqwest::Method::GET, &format!("/repos/{}/contents/{file}?ref={}", self.repo, self.branch), "bearer").send().await?;
        if res.status().as_u16() == 404 {
            return Ok(None);
        }
        if !res.status().is_success() {
            return Err(fail(res, "read").await);
        }
        let v: Value = res.json().await?;
        Ok(Some((decode_b64(&v["content"])?, v["sha"].as_str().unwrap_or("").to_string())))
    }

    /// One commit for all files through the Git Data API: tree on top of the branch, commit, move the branch.
    async fn github_write(&self, files: Vec<(String, String, Option<String>)>, message: &str) -> Result<()> {
        let r = &self.repo;
        let head: Value = self.req(reqwest::Method::GET, &format!("/repos/{r}/git/ref/heads/{}", self.branch), "bearer").send().await?.error_for_status()?.json().await?;
        let parent = head["object"]["sha"].as_str().ok_or_else(|| anyhow!("GitHub sent no branch head"))?.to_string();
        let commit: Value = self.req(reqwest::Method::GET, &format!("/repos/{r}/git/commits/{parent}"), "bearer").send().await?.error_for_status()?.json().await?;
        let tree: Vec<Value> = files.iter().map(|(path, text, _)| json!({ "path": path, "mode": "100644", "type": "blob", "content": text })).collect();
        let res = self.req(reqwest::Method::POST, &format!("/repos/{r}/git/trees"), "bearer").json(&json!({ "base_tree": commit["tree"]["sha"], "tree": tree })).send().await?;
        if !res.status().is_success() {
            return Err(fail(res, "write").await);
        }
        let new_tree: Value = res.json().await?;
        let res = self.req(reqwest::Method::POST, &format!("/repos/{r}/git/commits"), "bearer").json(&json!({ "message": message, "tree": new_tree["sha"], "parents": [parent] })).send().await?;
        if !res.status().is_success() {
            return Err(fail(res, "write").await);
        }
        let new_commit: Value = res.json().await?;
        let res = self.req(reqwest::Method::PATCH, &format!("/repos/{r}/git/refs/heads/{}", self.branch), "bearer").json(&json!({ "sha": new_commit["sha"], "force": false })).send().await?;
        if !res.status().is_success() {
            return Err(fail(res, "write").await);
        }
        Ok(())
    }

    // ---------- GitLab ----------

    fn project(&self) -> String {
        self.repo.replace('/', "%2F")
    }

    async fn gitlab_open(&mut self) -> Result<()> {
        let res = self.req(reqwest::Method::GET, &format!("/projects/{}", self.project()), "private").send().await.map_err(|e| anyhow!("Could not reach GitLab: {e}"))?;
        if res.status().is_success() {
            let v: Value = res.json().await?;
            self.branch = v["default_branch"].as_str().unwrap_or("main").to_string();
            return Ok(());
        }
        if res.status().as_u16() != 404 {
            return Err(fail(res, "open the repository").await);
        }
        let (owner, name) = self.owner_name();
        let me: Value = self.req(reqwest::Method::GET, "/user", "private").send().await?.error_for_status()?.json().await?;
        let mut body = json!({ "name": name, "path": name, "visibility": "private", "initialize_with_readme": true, "default_branch": "main", "description": DESCRIPTION });
        if !me["username"].as_str().is_some_and(|u| u.eq_ignore_ascii_case(&owner)) {
            let ns: Value = self.req(reqwest::Method::GET, &format!("/namespaces/{}", owner.replace('/', "%2F")), "private").send().await?.error_for_status()?.json().await?;
            body["namespace_id"] = ns["id"].clone();
        }
        let res = self.req(reqwest::Method::POST, "/projects", "private").json(&body).send().await?;
        if !res.status().is_success() {
            return Err(fail(res, "create the repository").await);
        }
        self.branch = "main".into();
        Ok(())
    }

    async fn gitlab_read(&self, file: &str) -> Result<Option<(String, String)>> {
        let res = self.req(reqwest::Method::GET, &format!("/projects/{}/repository/files/{}?ref={}", self.project(), file.replace('/', "%2F"), self.branch), "private").send().await?;
        if res.status().as_u16() == 404 {
            return Ok(None);
        }
        if !res.status().is_success() {
            return Err(fail(res, "read").await);
        }
        let v: Value = res.json().await?;
        Ok(Some((decode_b64(&v["content"])?, v["last_commit_id"].as_str().unwrap_or("").to_string())))
    }

    async fn gitlab_write(&self, files: Vec<(String, String, Option<String>)>, message: &str) -> Result<()> {
        let actions: Vec<Value> = files
            .into_iter()
            .map(|(path, text, version)| {
                let mut a = json!({ "action": if version.is_some() { "update" } else { "create" }, "file_path": path, "content": B64.encode(text), "encoding": "base64" });
                if let Some(v) = version.filter(|v| !v.is_empty()) {
                    a["last_commit_id"] = Value::String(v);
                }
                a
            })
            .collect();
        let res = self
            .req(reqwest::Method::POST, &format!("/projects/{}/repository/commits", self.project()), "private")
            .json(&json!({ "branch": self.branch, "commit_message": message, "actions": actions }))
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(fail(res, "write").await);
        }
        Ok(())
    }
}

// ---------- any git repository, through the git program ----------

fn git(dir: Option<&PathBuf>, args: &[&str]) -> Result<String> {
    let mut cmd = std::process::Command::new("git");
    if let Some(d) = dir {
        cmd.arg("-C").arg(d);
    }
    cmd.args(args).env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console window
    }
    let out = cmd.output().map_err(|e| anyhow!("The git program could not be started ({e}). Install Git for Windows, or choose another kind of server."))?;
    if !out.status.success() {
        bail!("git {}: {}", args.first().unwrap_or(&""), String::from_utf8_lossy(&out.stderr).trim().lines().last().unwrap_or(""));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

impl Cli {
    /// A working copy of Cablebag's own, brought level with the server.
    async fn open(&mut self) -> Result<()> {
        let (url, dir) = (self.url.clone(), self.dir.clone());
        self.branch = tokio::task::spawn_blocking(move || -> Result<String> {
            if !dir.join(".git").exists() {
                if let Some(parent) = dir.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let _ = std::fs::remove_dir_all(&dir);
                git(None, &["clone", "--quiet", &url, &dir.to_string_lossy()]).context("Could not get the repository; it has to exist on the server first")?;
            }
            git(Some(&dir), &["fetch", "--quiet", "origin"])?;
            let branch = git(Some(&dir), &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
                .ok()
                .and_then(|b| b.strip_prefix("origin/").map(String::from))
                .unwrap_or_else(|| "main".into());
            // An empty repository has no branch yet: the first write makes it.
            if git(Some(&dir), &["rev-parse", "--verify", "--quiet", &format!("origin/{branch}")]).is_ok() {
                git(Some(&dir), &["checkout", "--quiet", "-B", &branch, &format!("origin/{branch}")])?;
                git(Some(&dir), &["reset", "--quiet", "--hard", &format!("origin/{branch}")])?;
            } else {
                git(Some(&dir), &["checkout", "--quiet", "--orphan", &branch]).ok();
            }
            Ok(branch)
        })
        .await??;
        Ok(())
    }

    fn read(&self, file: &str) -> Result<Option<(String, String)>> {
        match std::fs::read_to_string(self.dir.join(file)) {
            Ok(text) => Ok(Some((text, String::new()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn write(&self, files: Vec<(String, String, Option<String>)>, message: &str) -> Result<()> {
        let (dir, branch, message) = (self.dir.clone(), self.branch.clone(), message.to_string());
        tokio::task::spawn_blocking(move || -> Result<()> {
            for (path, text, _) in &files {
                std::fs::write(dir.join(path), text)?;
            }
            let paths: Vec<&str> = files.iter().map(|(p, _, _)| p.as_str()).collect();
            let mut add = vec!["add", "--"];
            add.extend(paths);
            git(Some(&dir), &add)?;
            let pc = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "pc".into()).to_lowercase();
            git(Some(&dir), &["-c", "user.name=Cablebag", "-c", &format!("user.email=cablebag@{pc}"), "commit", "--quiet", "-m", &message])?;
            match git(Some(&dir), &["push", "--quiet", "origin", &format!("HEAD:{branch}")]) {
                Ok(_) => Ok(()),
                Err(e) if e.to_string().contains("rejected") || e.to_string().contains("fetch first") => Err(anyhow!("{RACE} ({e}).")),
                Err(e) => Err(e),
            }
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The git program path, read only: GIT_SYNC_URL=https://host/owner/repo.git cargo test --lib gitremote -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn live_git_program() {
        let url = std::env::var("GIT_SYNC_URL").unwrap();
        let dir = std::env::temp_dir().join("cablebag-sync-test");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(detect(&url).await.unwrap(), "git");
        let mut r = Remote::new("git", &url, "", String::new(), dir.clone()).unwrap();
        r.open().await.unwrap();
        if let Remote::Git(c) = &r {
            println!("branch {}", c.branch);
        }
        for f in ["folders.json", "sessions.json", "macros.json", "tunnels.json", "proxmox.json"] {
            let (text, _) = r.read(f).await.unwrap().expect("file there");
            let v: Value = serde_json::from_str(&text).unwrap();
            println!("{f}: {} with {} items", v["format"].as_str().unwrap_or("readable"), v["items"].as_array().map(|a| a.len()).unwrap_or(0));
        }
        // Opening again fetches and lines up with the server instead of cloning.
        r.open().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn detects_hosted() {
        assert_eq!(detect("https://github.com").await.unwrap(), "github");
        assert_eq!(detect("https://gitlab.com").await.unwrap(), "gitlab");
        assert_eq!(detect("git@bitbucket.org:me/x.git").await.unwrap(), "git");
    }
}

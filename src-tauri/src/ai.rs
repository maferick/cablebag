//! The assistant's side in Rust: AI hosts (any OpenAI-compatible API), their keys, the model list, streaming
//! chat, hiding secrets and the log.
//!
//! Everything that goes to a model passes `redact` here, right before it leaves this PC: whatever the window
//! puts in a message, passwords Cablebag knows (saved ones and, when the vault is open, the vault's) and
//! secret-looking text (private keys, tokens, "password=" values) are replaced first. Keys for the hosts never
//! reach the window.

use crate::AppState;
use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tokio::sync::watch;

const HOSTS_KEY: &str = "ai.hosts";

/// An OpenAI-compatible host: LiteLLM, vLLM, Ollama, a cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Host {
    pub id: String,
    pub name: String,
    /// The API base, ending in /v1 for most hosts.
    pub base_url: String,
    /// A Vaultwarden item whose password is the API key; empty to keep the key on this PC (or use none).
    #[serde(default)]
    pub vault_item: String,
    /// The model for questions and commands.
    #[serde(default)]
    pub model: String,
    /// The model for screenshots (RDP, VNC); may be the same.
    #[serde(default)]
    pub vision_model: String,
    /// May receive screens of sessions in protected folders.
    #[serde(default)]
    pub protected_ok: bool,
}

pub fn hosts(app: &AppHandle) -> Vec<Host> {
    let state = app.state::<AppState>();
    let v = state.store.lock().setting(HOSTS_KEY).ok().flatten().unwrap_or(Value::Array(vec![]));
    serde_json::from_value(v).unwrap_or_default()
}

fn host(app: &AppHandle, id: &str) -> Result<Host> {
    hosts(app).into_iter().find(|h| h.id == id).ok_or_else(|| anyhow!("This AI host is no longer in the settings."))
}

/// Saves a host (new when its id is empty) and, when given, its key on this PC.
pub fn save_host(app: &AppHandle, mut h: Host, key: Option<String>) -> Result<Host> {
    h.base_url = h.base_url.trim().trim_end_matches('/').to_string();
    if !h.base_url.starts_with("http://") && !h.base_url.starts_with("https://") {
        bail!("The address must start with https:// (or http:// for a host on your own network).");
    }
    let mut list = hosts(app);
    if h.id.is_empty() {
        h.id = uuid::Uuid::new_v4().to_string();
    }
    match list.iter_mut().find(|x| x.id == h.id) {
        Some(old) => *old = h.clone(),
        None => list.push(h.clone()),
    }
    let state = app.state::<AppState>();
    state.store.lock().set_setting(HOSTS_KEY, &serde_json::to_value(&list)?)?;
    if let Some(k) = key.filter(|k| !k.trim().is_empty()) {
        state.vault.save_password(&state.store.lock(), "ai", &h.base_url, 0, &h.id, k.trim())?;
    }
    Ok(h)
}

pub fn delete_host(app: &AppHandle, id: &str) -> Result<()> {
    let list: Vec<Host> = hosts(app).into_iter().filter(|h| h.id != id).collect();
    let state = app.state::<AppState>();
    let store = state.store.lock();
    store.set_setting(HOSTS_KEY, &serde_json::to_value(&list)?)?;
    for s in store.list_secrets()? {
        if s.protocol == "ai" && s.username == id {
            store.delete_secret(&s.id)?;
        }
    }
    Ok(())
}

pub fn has_key(app: &AppHandle, h: &Host) -> bool {
    let state = app.state::<AppState>();
    let store = state.store.lock();
    matches!(store.find_secret("password", "", "ai", &h.base_url, 0, &h.id), Ok(Some(_)))
}

async fn key_for(app: &AppHandle, h: &Host) -> Result<String> {
    let state = app.state::<AppState>();
    if !h.vault_item.is_empty() {
        let (_, pw) = state.vw.login(app, &h.vault_item).await?;
        return Ok(pw);
    }
    Ok(state.vault.password(&state.store.lock(), "ai", &h.base_url, 0, &h.id)?.unwrap_or_default())
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent("Cablebag").connect_timeout(std::time::Duration::from_secs(10)).build()?)
}

fn authed(rb: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
    if key.is_empty() {
        rb
    } else {
        rb.bearer_auth(key)
    }
}

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    /// What the host says (LiteLLM /model/info, Ollama /api/show); None when it does not say.
    pub tools: Option<bool>,
    pub vision: Option<bool>,
    pub context: Option<u64>,
}

/// The models of a host, with what each can do when the host tells.
pub async fn models(app: &AppHandle, id: &str) -> Result<Vec<ModelInfo>> {
    let h = host(app, id)?;
    let key = key_for(app, &h).await?;
    let http = client()?;
    let r = authed(http.get(format!("{}/models", h.base_url)), &key).timeout(std::time::Duration::from_secs(20)).send().await?;
    if !r.status().is_success() {
        bail!("{} answered {} for its model list.", h.name, r.status());
    }
    let v: Value = r.json().await?;
    let mut list: Vec<ModelInfo> = v["data"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["id"].as_str()).map(|id| ModelInfo { id: id.to_string(), tools: None, vision: None, context: None }).collect())
        .unwrap_or_default();
    // LiteLLM tells per model what it supports.
    let root = h.base_url.trim_end_matches("/v1");
    if let Ok(r) = authed(http.get(format!("{root}/model/info")), &key).timeout(std::time::Duration::from_secs(20)).send().await {
        if let Ok(info) = r.json::<Value>().await {
            for m in info["data"].as_array().into_iter().flatten() {
                if let Some(e) = list.iter_mut().find(|x| Some(x.id.as_str()) == m["model_name"].as_str()) {
                    let i = &m["model_info"];
                    e.tools = i["supports_function_calling"].as_bool();
                    e.vision = i["supports_vision"].as_bool();
                    e.context = i["max_input_tokens"].as_u64();
                }
            }
        }
    }
    list.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(list)
}

// ---- Hiding secrets ----

static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----", "[private key hidden]"),
        (r"(?i)\b(authorization:\s*(?:bearer|basic|token))\s+\S+", "$1 [hidden]"),
        (r#"(?i)\b(password|passwd|pwd|secret|token|api[_-]?key|access[_-]?key|client[_-]?secret)(\s*[:=]\s*)("[^"]*"|'[^']*'|[^\s"';,]+)"#, "$1$2[hidden]"),
        (r"\bAKIA[0-9A-Z]{16}\b", "[access key hidden]"),
        (r"\bgh[pousr]_[A-Za-z0-9]{30,}\b", "[token hidden]"),
        (r"\bglpat-[A-Za-z0-9_-]{20,}\b", "[token hidden]"),
        (r"\bsk-[A-Za-z0-9_-]{20,}\b", "[key hidden]"),
        (r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b", "[token hidden]"),
        (r"\$(?:1|2[aby]|5|6|y)\$[^\s:]{20,}", "[password hash hidden]"),
    ]
    .into_iter()
    .map(|(p, r)| (Regex::new(p).expect("pattern"), r))
    .collect()
});

/// Every password Cablebag can see right now: its own saved ones and, when the vault is open, the vault's.
fn known_secrets(app: &AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    let mut out = Vec::new();
    {
        let store = state.store.lock();
        for s in store.list_secrets().unwrap_or_default() {
            if let Ok(Some((n, c))) = store.secret_by_id(&s.id) {
                if let Ok(p) = state.vault.decrypt(&store, &n, &c) {
                    out.push(p);
                }
            }
        }
    }
    out.extend(state.vw.known_passwords());
    out.retain(|p| p.chars().count() >= 4);
    out.sort_by_key(|p| std::cmp::Reverse(p.len()));
    out.dedup();
    out
}

pub fn redact_with(text: &str, secrets: &[String]) -> String {
    // The rules first, while the text is still whole.
    let mut s = text.to_string();
    for (re, with) in PATTERNS.iter() {
        s = re.replace_all(&s, *with).into_owned();
    }
    // Then every known password, in one pass over the text: find all places first (longest password first,
    // no overlaps), then rebuild, so a replacement is never searched again.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for secret in secrets {
        // A short password is only hidden where it stands alone, so "pass" does not eat "password".
        let whole = secret.chars().count() < 8;
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        for (at, _) in s.match_indices(secret.as_str()) {
            let end = at + secret.len();
            if whole && (word(s[..at].chars().next_back()) || word(s[end..].chars().next())) {
                continue;
            }
            if !spans.iter().any(|&(a, b)| at < b && end > a) {
                spans.push((at, end));
            }
        }
    }
    if spans.is_empty() {
        return s;
    }
    spans.sort();
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for (a, b) in spans {
        out.push_str(&s[last..a]);
        out.push_str("[password hidden]");
        last = b;
    }
    out.push_str(&s[last..]);
    out
}

/// Hides secrets in every string of a JSON value (message texts, tool results).
fn redact_value(v: &mut Value, secrets: &[String]) {
    match v {
        Value::String(s) => {
            // Images are data URLs; leave them as they are.
            if !s.starts_with("data:image/") {
                *s = redact_with(s, secrets);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| redact_value(x, secrets)),
        Value::Object(o) => o.iter_mut().for_each(|(_, x)| redact_value(x, secrets)),
        _ => {}
    }
}

/// Hides secrets in the messages, except Cablebag's own instructions (the system message).
fn redact_messages(messages: &mut Value, secrets: &[String]) {
    for m in messages.as_array_mut().into_iter().flatten() {
        if m["role"] != "system" {
            redact_value(m, secrets);
        }
    }
}

// ---- Chat, streamed ----

#[derive(Default)]
pub struct Chats {
    stop: parking_lot::Mutex<HashMap<String, watch::Sender<bool>>>,
}

#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEvent {
    /// More answer text.
    Text { text: String },
    /// The model's thinking, when it shows it.
    Reasoning { text: String },
    /// The model asks for a tool; arguments arrive complete.
    ToolCall { id: String, name: String, arguments: String },
    Done { finish: String },
    Error { message: String },
}

#[derive(Deserialize)]
pub struct ChatRequest {
    pub request_id: String,
    pub host_id: String,
    pub model: String,
    pub messages: Value,
    #[serde(default)]
    pub tools: Option<Value>,
    /// For the log: which tab and host the conversation is about.
    #[serde(default)]
    pub about: String,
}

pub fn stop(app: &AppHandle, request_id: &str) {
    if let Some(tx) = app.state::<Arc<Chats>>().stop.lock().remove(request_id) {
        let _ = tx.send(true);
    }
}

pub async fn chat(app: AppHandle, req: ChatRequest, out: Channel<ChatEvent>) -> Result<()> {
    let h = host(&app, &req.host_id)?;
    let key = key_for(&app, &h).await?;
    let mut messages = req.messages;
    let secrets = known_secrets(&app);
    redact_messages(&mut messages, &secrets);
    let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
    if let Some(t) = req.tools.filter(|t| t.as_array().is_some_and(|a| !a.is_empty())) {
        body["tools"] = t;
    }
    log(&app, &h, &req.model, &req.about, "sent", &body["messages"].to_string());

    let (tx, mut stopped) = watch::channel(false);
    app.state::<Arc<Chats>>().stop.lock().insert(req.request_id.clone(), tx);
    let result = stream(&h, &key, &body, &out, &mut stopped).await;
    app.state::<Arc<Chats>>().stop.lock().remove(&req.request_id);
    match result {
        Ok((text, finish)) => {
            log(&app, &h, &req.model, &req.about, "received", &text);
            let _ = out.send(ChatEvent::Done { finish });
        }
        Err(e) => {
            log(&app, &h, &req.model, &req.about, "error", &e.to_string());
            let _ = out.send(ChatEvent::Error { message: e.to_string() });
        }
    }
    Ok(())
}

/// Reads the server-sent events of a streamed answer. Tool calls arrive in pieces and are put together here.
async fn stream(h: &Host, key: &str, body: &Value, out: &Channel<ChatEvent>, stopped: &mut watch::Receiver<bool>) -> Result<(String, String)> {
    let http = client()?;
    let mut r = authed(http.post(format!("{}/chat/completions", h.base_url)), key).json(body).send().await.with_context(|| format!("Could not reach {}", h.name))?;
    if !r.status().is_success() {
        let status = r.status();
        let text = r.text().await.unwrap_or_default();
        bail!("{} answered {status}: {}", h.name, text.chars().take(300).collect::<String>());
    }
    let mut buf = String::new();
    let mut all = String::new();
    let mut finish = String::new();
    // Tool calls by index: id, name, arguments so far.
    let mut calls: Vec<(String, String, String)> = Vec::new();
    loop {
        let chunk = tokio::select! {
            c = r.chunk() => c?,
            _ = stopped.changed() => { finish = "stopped".into(); break; }
        };
        let Some(chunk) = chunk else { break };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf.drain(..=pos);
            let Some(data) = line.strip_prefix("data:").map(str::trim) else { continue };
            if data == "[DONE]" {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
            let choice = &v["choices"][0];
            let d = &choice["delta"];
            if let Some(t) = d["content"].as_str().filter(|t| !t.is_empty()) {
                all.push_str(t);
                let _ = out.send(ChatEvent::Text { text: t.to_string() });
            }
            if let Some(t) = d["reasoning_content"].as_str().or(d["reasoning"].as_str()).filter(|t| !t.is_empty()) {
                let _ = out.send(ChatEvent::Reasoning { text: t.to_string() });
            }
            for tc in d["tool_calls"].as_array().into_iter().flatten() {
                let i = tc["index"].as_u64().unwrap_or(calls.len() as u64) as usize;
                while calls.len() <= i {
                    calls.push(Default::default());
                }
                if let Some(id) = tc["id"].as_str() {
                    calls[i].0 = id.to_string();
                }
                if let Some(n) = tc["function"]["name"].as_str() {
                    calls[i].1.push_str(n);
                }
                if let Some(a) = tc["function"]["arguments"].as_str() {
                    calls[i].2.push_str(a);
                }
            }
            if let Some(f) = choice["finish_reason"].as_str() {
                finish = f.to_string();
            }
        }
    }
    for (i, (id, name, args)) in calls.into_iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        all.push_str(&format!("\n[tool {name} {args}]"));
        let id = if id.is_empty() { format!("call_{i}") } else { id };
        let _ = out.send(ChatEvent::ToolCall { id, name, arguments: if args.trim().is_empty() { "{}".into() } else { args } });
    }
    Ok((all, finish))
}

// ---- The log ----

fn log(app: &AppHandle, h: &Host, model: &str, about: &str, direction: &str, text: &str) {
    let state = app.state::<AppState>();
    let _ = state.store.lock().ai_log_add(&h.name, model, about, direction, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_secrets() {
        // "pass" is a real-world weak password: it must not break the other rules by eating "password".
        let known = vec!["Sup3r-Geheim!".to_string(), "pass".to_string()];
        let t = "login ok\nmysql -u root -pSup3r-Geheim!\nPASSWORD=hunter22 ./run\necho \"db password=hunter23\"\nAuthorization: Bearer abc.def.ghi\n\
                 export AWS_ACCESS_KEY_ID=AKIAABCDEFGHIJKLMNOP\ntoken: ghp_0123456789abcdefghijklmnopqrstuvwxyz12\n\
                 -----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAA\n-----END OPENSSH PRIVATE KEY-----\n\
                 root:$6$saltsalt$abcdefghijklmnopqrstuvwxyz0123456789:19000:0:99999:7:::\nuptime 5 days";
        let r = redact_with(t, &known);
        for leak in ["Sup3r-Geheim!", "hunter22", "hunter23", "abc.def.ghi", "AKIAABCDEFGHIJKLMNOP", "ghp_0123", "b3BlbnNzaC1rZXktdjEAAAA", "$6$saltsalt"] {
            assert!(!r.contains(leak), "{leak} leaked in:\n{r}");
        }
        assert!(r.contains("uptime 5 days") && r.contains("login ok"));
        // The quote after a hidden value stays, so the line still reads right.
        assert!(r.contains("db password=[hidden]\""), "{r}");
        assert!(redact_with("user pass logged in", &known).contains("user [password hidden] logged in"));
    }
}

//! Questions from the backend to the person at the screen (host key, password, passphrase...).
//! The backend emits a "prompt" event and waits until the window answers with `prompt_answer`.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize, Default)]
pub struct Field {
    pub name: String,
    pub label: String,
    /// text, password, checkbox
    pub kind: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct Button {
    pub id: String,
    pub label: String,
    /// primary, danger or normal
    pub style: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptRequest {
    pub id: String,
    pub title: String,
    pub message: String,
    /// Optional label/value rows shown under the message, e.g. key fingerprints.
    pub details: Vec<(String, String)>,
    pub fields: Vec<Field>,
    pub buttons: Vec<Button>,
    /// When true the dialog is drawn as a warning.
    pub warning: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PromptAnswer {
    pub button: String,
    #[serde(default)]
    pub values: HashMap<String, Value>,
}

impl PromptAnswer {
    pub fn text(&self, name: &str) -> String {
        self.values.get(name).and_then(|v| v.as_str()).unwrap_or("").to_string()
    }
    pub fn flag(&self, name: &str) -> bool {
        self.values.get(name).and_then(|v| v.as_bool()).unwrap_or(false)
    }
}

#[derive(Default)]
pub struct Prompts {
    pending: Mutex<HashMap<String, oneshot::Sender<PromptAnswer>>>,
}

pub fn button(id: &str, label: &str, style: &str) -> Button {
    Button { id: id.into(), label: label.into(), style: style.into() }
}

pub fn field(name: &str, label: &str, kind: &str, value: Value) -> Field {
    Field { name: name.into(), label: label.into(), kind: kind.into(), value }
}

impl Prompts {
    /// Shows the request and waits for the answer. A closed dialog counts as "cancel".
    pub async fn ask(&self, app: &AppHandle, mut req: PromptRequest) -> PromptAnswer {
        req.id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(req.id.clone(), tx);
        let _ = app.emit("prompt", &req);
        rx.await.unwrap_or_else(|_| PromptAnswer { button: "cancel".into(), values: HashMap::new() })
    }

    pub fn answer(&self, id: &str, answer: PromptAnswer) {
        if let Some(tx) = self.pending.lock().remove(id) {
            let _ = tx.send(answer);
        }
    }

    /// Asks for one secret value. Returns None when cancelled.
    pub async fn ask_secret(
        &self,
        app: &AppHandle,
        title: &str,
        message: &str,
        label: &str,
        offer_save: bool,
    ) -> Option<(String, bool)> {
        let mut fields = vec![field("secret", label, "password", Value::String(String::new()))];
        if offer_save {
            fields.push(field("save", "Remember in the password vault", "checkbox", Value::Bool(false)));
        }
        let ans = self
            .ask(
                app,
                PromptRequest {
                    id: String::new(),
                    title: title.into(),
                    message: message.into(),
                    details: vec![],
                    fields,
                    buttons: vec![button("ok", "Connect", "primary"), button("cancel", "Cancel", "normal")],
                    warning: false,
                },
            )
            .await;
        if ans.button != "ok" {
            return None;
        }
        Some((ans.text("secret"), ans.flag("save")))
    }
}

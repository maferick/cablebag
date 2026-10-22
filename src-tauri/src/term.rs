//! Registry of open terminals. Each tab owns one terminal; the frontend sends input and
//! size changes through it, output flows back over a binary Tauri channel.

use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::UnboundedSender;

pub enum TermCmd {
    Input(Vec<u8>),
    Resize(u16, u16),
    Close,
}

#[derive(Default)]
pub struct Terms {
    map: Mutex<HashMap<String, UnboundedSender<TermCmd>>>,
}

impl Terms {
    pub fn insert(&self, id: &str, tx: UnboundedSender<TermCmd>) {
        if let Some(old) = self.map.lock().insert(id.to_string(), tx) {
            let _ = old.send(TermCmd::Close);
        }
    }

    pub fn send(&self, id: &str, cmd: TermCmd) -> bool {
        match self.map.lock().get(id) {
            Some(tx) => tx.send(cmd).is_ok(),
            None => false,
        }
    }

    pub fn remove(&self, id: &str) {
        if let Some(tx) = self.map.lock().remove(id) {
            let _ = tx.send(TermCmd::Close);
        }
    }
}

/// Where a terminal writes its output.
#[derive(Clone)]
pub struct TermOut {
    pub id: String,
    pub channel: Channel<InvokeResponseBody>,
    pub app: AppHandle,
    pub log: Option<crate::termlog::SharedLog>,
}

#[derive(Clone, Serialize)]
struct StatusEvent<'a> {
    id: &'a str,
    /// connecting, connected, closed, failed
    state: &'a str,
    message: &'a str,
}

/// Anything that can show progress lines of a connection.
pub trait Sink: Sync {
    fn info(&self, text: &str);
    fn error(&self, text: &str);
}

impl Sink for TermOut {
    fn info(&self, text: &str) {
        TermOut::info(self, text)
    }
    fn error(&self, text: &str) {
        TermOut::error(self, text)
    }
}

impl TermOut {
    pub fn data(&self, bytes: &[u8]) {
        if let Some(l) = &self.log {
            l.lock().write(bytes);
        }
        let _ = self.channel.send(InvokeResponseBody::Raw(bytes.to_vec()));
    }

    /// Writes a dimmed informational line into the terminal.
    pub fn info(&self, text: &str) {
        self.data(format!("\x1b[2m{}\x1b[0m\r\n", text.replace('\n', "\r\n")).as_bytes());
    }

    pub fn error(&self, text: &str) {
        self.data(format!("\r\n\x1b[31m{}\x1b[0m\r\n", text.replace('\n', "\r\n")).as_bytes());
    }

    pub fn status(&self, state: &str, message: &str) {
        let _ = self.app.emit("term-status", StatusEvent { id: &self.id, state, message });
    }
}

//! Writes terminal output to a log file: raw bytes, printable text only, or text with timestamps.

use parking_lot::Mutex;
use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize)]
pub struct LogSpec {
    pub path: String,
    /// "text", "raw" or "timestamps"
    pub kind: String,
}

#[derive(Default)]
enum Esc {
    #[default]
    None,
    Start,
    Csi,
    Osc,
    OscEsc,
}

pub struct Logger {
    file: File,
    kind: String,
    esc: Esc,
    params: String,
    /// The line being drawn; written out when the terminal moves to the next line.
    line: Vec<char>,
    cursor: usize,
    pending: Vec<u8>,
}

pub type SharedLog = Arc<Mutex<Logger>>;

pub fn open(spec: &LogSpec) -> std::io::Result<SharedLog> {
    if let Some(dir) = Path::new(&spec.path).parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(&spec.path)?;
    Ok(Arc::new(Mutex::new(Logger {
        file,
        kind: spec.kind.clone(),
        esc: Esc::None,
        params: String::new(),
        line: Vec::new(),
        cursor: 0,
        pending: Vec::new(),
    })))
}

fn timestamp() -> String {
    let secs = crate::store::now();
    let local = secs + local_offset_secs();
    let days = local.div_euclid(86400);
    let t = local.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("[{y}-{m:02}-{d:02} {:02}:{:02}:{:02}] ", t / 3600, (t / 60) % 60, t % 60)
}

#[cfg(windows)]
fn local_offset_secs() -> i64 {
    use windows::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
    let mut tz = TIME_ZONE_INFORMATION::default();
    let r = unsafe { GetTimeZoneInformation(&mut tz) };
    // 2 = daylight saving time is in effect
    let bias = tz.Bias + if r == 2 { tz.DaylightBias } else { tz.StandardBias };
    -(bias as i64) * 60
}

#[cfg(not(windows))]
fn local_offset_secs() -> i64 {
    0
}

impl Logger {
    pub fn write(&mut self, bytes: &[u8]) {
        if self.kind == "raw" {
            let _ = self.file.write_all(bytes);
            return;
        }
        // Decode UTF-8, keeping an incomplete sequence for the next chunk.
        self.pending.extend_from_slice(bytes);
        let valid = match std::str::from_utf8(&self.pending) {
            Ok(s) => s.len(),
            Err(e) => e.valid_up_to(),
        };
        let text: String = String::from_utf8_lossy(&self.pending[..valid]).into_owned();
        self.pending.drain(..valid);
        if self.pending.len() > 8 {
            self.pending.clear();
        }
        for ch in text.chars() {
            self.feed(ch);
        }
    }

    fn feed(&mut self, ch: char) {
        match self.esc {
            Esc::None => match ch {
                '\x1b' => self.esc = Esc::Start,
                '\r' => self.cursor = 0,
                '\n' => self.flush_line(),
                '\x08' => self.cursor = self.cursor.saturating_sub(1),
                '\t' => {
                    let next = (self.cursor / 8 + 1) * 8;
                    while self.cursor < next {
                        self.put(' ');
                    }
                }
                c if (c as u32) < 0x20 || c == '\x7f' => {}
                c => self.put(c),
            },
            Esc::Start => {
                self.esc = match ch {
                    '[' => {
                        self.params.clear();
                        Esc::Csi
                    }
                    ']' => Esc::Osc,
                    _ => Esc::None,
                }
            }
            Esc::Csi => {
                if ('\x40'..='\x7e').contains(&ch) {
                    self.csi(ch);
                    self.esc = Esc::None;
                } else {
                    self.params.push(ch);
                }
            }
            Esc::Osc => match ch {
                '\x07' => self.esc = Esc::None,
                '\x1b' => self.esc = Esc::OscEsc,
                _ => {}
            },
            Esc::OscEsc => self.esc = Esc::None,
        }
    }

    /// Writes a character at the cursor, overwriting what is there.
    fn put(&mut self, c: char) {
        if self.cursor < self.line.len() {
            self.line[self.cursor] = c;
        } else {
            while self.line.len() < self.cursor {
                self.line.push(' ');
            }
            self.line.push(c);
        }
        self.cursor += 1;
    }

    /// The cursor and erase sequences programs use to redraw a line (PowerShell, bash readline).
    fn csi(&mut self, fin: char) {
        if self.params.starts_with('?') {
            return;
        }
        let nums: Vec<usize> = self.params.split(';').map(|p| p.parse().unwrap_or(0)).collect();
        let n = nums.first().copied().filter(|&v| v > 0).unwrap_or(1);
        match fin {
            'D' => self.cursor = self.cursor.saturating_sub(n),
            'C' => self.cursor += n,
            'G' => self.cursor = n - 1,
            'H' | 'f' => self.cursor = nums.get(1).copied().filter(|&v| v > 0).unwrap_or(1) - 1,
            'K' => match nums.first().copied().unwrap_or(0) {
                0 => self.line.truncate(self.cursor),
                1 => {
                    for i in 0..self.cursor.min(self.line.len()) {
                        self.line[i] = ' ';
                    }
                }
                _ => self.line.clear(),
            },
            'P' => {
                let end = (self.cursor + n).min(self.line.len());
                if self.cursor < end {
                    self.line.drain(self.cursor..end);
                }
            }
            'X' => {
                for i in self.cursor..(self.cursor + n).min(self.line.len()) {
                    self.line[i] = ' ';
                }
            }
            '@' => {
                let at = self.cursor.min(self.line.len());
                for _ in 0..n {
                    self.line.insert(at, ' ');
                }
            }
            _ => {}
        }
    }

    fn flush_line(&mut self) {
        let text: String = self.line.iter().collect();
        let mut out = String::new();
        if self.kind == "timestamps" {
            out.push_str(&timestamp());
        }
        out.push_str(text.trim_end());
        out.push_str("\r\n");
        let _ = self.file.write_all(out.as_bytes());
        self.line.clear();
        self.cursor = 0;
    }
}

impl Drop for Logger {
    fn drop(&mut self) {
        if self.kind != "raw" && !self.line.is_empty() {
            self.flush_line();
        }
    }
}

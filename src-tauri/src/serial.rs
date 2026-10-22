//! Serial sessions: a COM port as a terminal, for switches, routers and other devices with a console port.

use crate::store::Session;
use crate::term::{TermCmd, TermOut};
use serde::Serialize;
use serde_json::Value;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Serialize)]
pub struct Port {
    pub name: String,
    /// What Windows says is behind it, e.g. "USB Serial Port (FTDI)".
    pub description: String,
}

/// The COM ports on this PC.
pub fn ports() -> Vec<Port> {
    let mut list: Vec<Port> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let description = match p.port_type {
                serialport::SerialPortType::UsbPort(u) => [u.product.unwrap_or_default(), u.manufacturer.unwrap_or_default()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(", "),
                serialport::SerialPortType::BluetoothPort => "Bluetooth".into(),
                serialport::SerialPortType::PciPort => "Built in".into(),
                serialport::SerialPortType::Unknown => String::new(),
            };
            Port { name: p.port_name, description }
        })
        .collect();
    list.sort_by_key(|p| p.name.trim_start_matches("COM").parse::<u32>().unwrap_or(u32::MAX));
    list
}

fn num(v: &Value, k: &str, default: u64) -> u64 {
    v.get(k).and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(default)
}

fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

pub async fn run(session: Session, out: TermOut, mut rx: mpsc::UnboundedReceiver<TermCmd>) {
    let d = &session.data;
    let name = match text(d, "serial_port") {
        p if !p.is_empty() => p,
        _ => session.host.trim().to_string(),
    };
    let baud = num(d, "baud", 9600) as u32;
    out.status("connecting", "");
    if name.is_empty() {
        out.error("Choose a COM port in the session.");
        out.status("failed", "No COM port.");
        return;
    }
    let builder = serialport::new(&name, baud)
        .data_bits(match num(d, "data_bits", 8) {
            5 => serialport::DataBits::Five,
            6 => serialport::DataBits::Six,
            7 => serialport::DataBits::Seven,
            _ => serialport::DataBits::Eight,
        })
        .parity(match text(d, "parity").as_str() {
            "odd" => serialport::Parity::Odd,
            "even" => serialport::Parity::Even,
            _ => serialport::Parity::None,
        })
        .stop_bits(if num(d, "stop_bits", 1) == 2 { serialport::StopBits::Two } else { serialport::StopBits::One })
        .flow_control(match text(d, "flow").as_str() {
            "hardware" => serialport::FlowControl::Hardware,
            "software" => serialport::FlowControl::Software,
            _ => serialport::FlowControl::None,
        })
        .timeout(Duration::from_millis(100));
    let mut port = match builder.open() {
        Ok(p) => p,
        Err(e) => {
            out.error(&format!("Could not open {name}: {e}"));
            out.status("failed", &e.to_string());
            return;
        }
    };
    let mut reader = match port.try_clone() {
        Ok(r) => r,
        Err(e) => {
            out.status("failed", &e.to_string());
            return;
        }
    };
    out.info(&format!("Opened {name} at {baud} baud."));
    out.status("connected", "");

    // Reading blocks, so it gets a thread of its own.
    let stop = Arc::new(AtomicBool::new(false));
    let (stop2, out2) = (stop.clone(), out.clone());
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while !stop2.load(Ordering::Relaxed) {
            match reader.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => out2.data(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => {
                    out2.error(&format!("The port stopped: {e}"));
                    out2.status("closed", &e.to_string());
                    stop2.store(true, Ordering::Relaxed);
                }
            }
        }
    });

    while let Some(cmd) = rx.recv().await {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match cmd {
            TermCmd::Input(data) => {
                if let Err(e) = port.write_all(&data).and_then(|_| port.flush()) {
                    out.error(&format!("Could not write to {name}: {e}"));
                }
            }
            TermCmd::Resize(..) => {}
            TermCmd::Close => break,
        }
    }
    stop.store(true, Ordering::Relaxed);
    out.status("closed", "");
}

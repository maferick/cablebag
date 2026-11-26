//! Why an SSH connection failed while the encryption was being set up. Connects once more only to read what the
//! server offers: its first message (the version line) and its first packet, the list of methods it allows,
//! which every SSH server sends unencrypted before any login. Nothing is sent but Cablebag's own version line,
//! and no host key is looked at. The result is put next to what Cablebag offers, part by part, so a report shows
//! which part has nothing in common (as with servers that only allow methods Cablebag lacks).

use anyhow::{bail, Result};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// What a server offers, as its first packet (SSH_MSG_KEXINIT) lists it.
#[derive(Debug, Default, PartialEq)]
pub struct Offer {
    pub version: String,
    pub kex: Vec<String>,
    pub host_key: Vec<String>,
    pub cipher: Vec<String>,
    pub mac: Vec<String>,
}

/// Reads the server's version line and method lists. Gives up after a few seconds.
pub async fn read_offer(host: &str, port: u16) -> Result<Offer> {
    let work = async {
        let stream = tokio::net::TcpStream::connect((host, port)).await?;
        let mut r = BufReader::new(stream);
        // Servers may send other lines before the version line.
        let mut version = String::new();
        for _ in 0..20 {
            let mut line = String::new();
            if r.read_line(&mut line).await? == 0 {
                bail!("the server closed the connection before saying which version it speaks");
            }
            if line.starts_with("SSH-") {
                version = line.trim().to_string();
                break;
            }
        }
        if version.is_empty() {
            bail!("the server did not send an SSH version line");
        }
        r.get_mut().write_all(b"SSH-2.0-Cablebag_check\r\n").await?;
        let len = r.read_u32().await? as usize;
        if !(6..=256 * 1024).contains(&len) {
            bail!("the server's first packet has an unlikely length");
        }
        let mut packet = vec![0u8; len];
        r.read_exact(&mut packet).await?;
        let mut offer = parse_kexinit(&packet)?;
        offer.version = version;
        Ok(offer)
    };
    tokio::time::timeout(Duration::from_secs(8), work).await.unwrap_or_else(|_| bail!("no answer within 8 seconds"))
}

/// The method lists from an unencrypted packet: padding length, then the payload (message 20, a 16-byte cookie
/// and the name-lists).
fn parse_kexinit(packet: &[u8]) -> Result<Offer> {
    let pad = *packet.first().unwrap_or(&0) as usize;
    let payload = packet.get(1..packet.len().saturating_sub(pad)).unwrap_or_default();
    if payload.first() != Some(&20) {
        bail!("the server's first packet is not its list of methods");
    }
    let mut at = 17;
    let mut lists = Vec::new();
    for _ in 0..6 {
        let Some(len) = payload.get(at..at + 4) else { bail!("the list of methods ends early") };
        let len = u32::from_be_bytes([len[0], len[1], len[2], len[3]]) as usize;
        let Some(text) = payload.get(at + 4..at + 4 + len) else { bail!("the list of methods ends early") };
        lists.push(String::from_utf8_lossy(text).split(',').filter(|s| !s.is_empty()).map(str::to_string).collect::<Vec<_>>());
        at += 4 + len;
    }
    let mut lists = lists.into_iter();
    let mut next = || lists.next().unwrap_or_default();
    let kex = next();
    let host_key = next();
    let cipher = next();
    let _cipher_back = next();
    let mac = next();
    Ok(Offer { version: String::new(), kex, host_key, cipher, mac })
}

/// One part of the comparison: what both offer, or both lists when nothing is shared.
fn part(name: &str, server: &[String], ours: &[String], skip_markers: bool) -> (String, bool) {
    // ext-info-* and kex-strict-* are markers, not ways to exchange keys.
    let real = |s: &&String| !skip_markers || !(s.starts_with("ext-info-") || s.starts_with("kex-strict-"));
    let server: Vec<&String> = server.iter().filter(real).collect();
    let ours: Vec<&String> = ours.iter().filter(real).collect();
    let common: Vec<&str> = ours.iter().filter(|o| server.contains(o)).map(|s| s.as_str()).collect();
    if let Some(first) = common.first() {
        return (format!("{name}: fine, both can use {first}."), true);
    }
    let list = |v: &[&String]| v.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
    (format!("{name}: nothing in common. The server allows {}; Cablebag offers {}.", list(&server), list(&ours)), false)
}

/// Lines for the tab: each part, and what it means.
pub fn report(offer: &Offer, ours: &Offer) -> Vec<String> {
    let mut lines = vec![format!("What the server offers ({}), next to what Cablebag can use:", offer.version)];
    let (kex, kex_ok) = part("Key exchange", &offer.kex, &ours.kex, true);
    let (key, key_ok) = part("Host key", &offer.host_key, &ours.host_key, false);
    let (cipher, cipher_ok) = part("Encryption", &offer.cipher, &ours.cipher, false);
    // GCM and ChaCha20 ciphers check integrity themselves; the separate list then does not matter.
    let aead = offer.cipher.iter().any(|c| ours.cipher.contains(c) && (c.contains("gcm") || c.contains("chacha20")));
    let (mac, mac_ok) = part("Integrity check", &offer.mac, &ours.mac, false);
    lines.extend([kex, key, cipher]);
    if !aead {
        lines.push(mac);
    }
    if kex_ok && key_ok && cipher_ok && (aead || mac_ok) {
        lines.push(
            "Every part has a method in common. So one of the methods named here does not work with this server, or the server (or something on the way, such as a firewall) ended the connection for another reason."
                .into(),
        );
    }
    lines
}

/// What Cablebag offers, as the names the server would see.
pub fn ours(kex: &[russh::kex::Name]) -> Offer {
    let p = russh::Preferred::DEFAULT;
    Offer {
        version: String::new(),
        kex: kex.iter().map(|n| n.as_ref().to_string()).collect(),
        host_key: p.key.iter().map(|a| a.as_str().to_string()).collect(),
        cipher: p.cipher.iter().map(|n| n.as_ref().to_string()).collect(),
        mac: p.mac.iter().map(|n| n.as_ref().to_string()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kexinit(lists: [&str; 10]) -> Vec<u8> {
        let mut payload = vec![20u8];
        payload.extend([7u8; 16]);
        for l in lists {
            payload.extend((l.len() as u32).to_be_bytes());
            payload.extend(l.as_bytes());
        }
        payload.extend([0u8; 5]);
        let mut packet = vec![4u8];
        packet.extend(payload);
        packet.extend([0u8; 4]);
        packet
    }

    #[test]
    fn reads_the_lists_and_names_the_part_with_nothing_in_common() {
        let packet = kexinit([
            "ecdh-sha2-nistp256,diffie-hellman-group14-sha256,kex-strict-s-v00@openssh.com",
            "rsa-sha2-512,ecdsa-sha2-nistp256",
            "aes256-ctr",
            "aes256-ctr",
            "hmac-sha1",
            "hmac-sha1",
            "none",
            "none",
            "",
            "",
        ]);
        let mut offer = parse_kexinit(&packet).unwrap();
        assert_eq!(offer.kex.len(), 3);
        assert_eq!(offer.cipher, ["aes256-ctr"]);
        offer.version = "SSH-2.0-OpenSSH_9.9".into();
        let ours = ours(&[russh::kex::CURVE25519, russh::kex::ECDH_SHA2_NISTP256]);
        let lines = report(&offer, &ours);
        assert!(lines[1].starts_with("Key exchange: fine, both can use ecdh-sha2-nistp256"), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("Integrity check: nothing in common. The server allows hmac-sha1;")), "{lines:?}");
        assert!(!lines.iter().any(|l| l.starts_with("Every part")), "{lines:?}");
    }

    #[test]
    fn says_when_every_part_matches() {
        let offer = Offer {
            version: "SSH-2.0-x".into(),
            kex: vec!["curve25519-sha256".into()],
            host_key: vec!["ssh-ed25519".into()],
            cipher: vec!["aes256-gcm@openssh.com".into()],
            mac: vec!["hmac-sha1".into()],
        };
        let lines = report(&offer, &ours(&[russh::kex::CURVE25519]));
        assert!(lines.last().unwrap().starts_with("Every part has a method in common"), "{lines:?}");
    }
}

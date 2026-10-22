//! Local port forwarding over an SSH connection: listen on 127.0.0.1 and carry each accepted
//! connection through the SSH server to a destination host and port.

use crate::ssh::SshConn;
use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub struct LocalForward {
    pub port: u16,
    task: JoinHandle<()>,
    _conn: Arc<SshConn>,
}

impl Drop for LocalForward {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Starts forwarding `bind_port` (0 picks a free port) to `dest_host:dest_port` as seen from the SSH server.
pub async fn local_forward(conn: Arc<SshConn>, bind_port: u16, dest_host: String, dest_port: u16) -> Result<LocalForward> {
    let listener = TcpListener::bind(("127.0.0.1", bind_port)).await?;
    let port = listener.local_addr()?.port();
    let c = conn.clone();
    let task = tokio::spawn(async move {
        while let Ok((mut sock, peer)) = listener.accept().await {
            let c = c.clone();
            let host = dest_host.clone();
            tokio::spawn(async move {
                let ch = match c
                    .handle
                    .channel_open_direct_tcpip(host, dest_port as u32, peer.ip().to_string(), peer.port() as u32)
                    .await
                {
                    Ok(ch) => ch,
                    Err(_) => return,
                };
                let mut stream = ch.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut sock, &mut stream).await;
            });
        }
    });
    Ok(LocalForward { port, task, _conn: conn })
}

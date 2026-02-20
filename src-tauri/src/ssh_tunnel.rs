//! SSH local port forwarding (Phase 8: SSH feature enhancements).
//! Listens on a local port and forwards each connection through the SSH session
//! to a remote host:port via direct-tcpip channels.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::io;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tauri::AppHandle;
use russh::client::Handle;
use russh::Disconnect;
use log::{error, info};

use crate::ssh::Client;
use crate::ssh_auth;
use crate::validation;

/// Active tunnel: local bind and remote target.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TunnelInfo {
    pub id: String,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Clone)]
pub struct TunnelState {
    /// tunnel_id -> (cancel_tx to stop the tunnel task, info)
    tunnels: Arc<Mutex<HashMap<String, (mpsc::Sender<()>, TunnelInfo)>>>,
}

impl TunnelState {
    pub fn new() -> Self {
        Self {
            tunnels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn list(&self) -> Vec<TunnelInfo> {
        let guard = self.tunnels.lock().unwrap_or_else(|e| e.into_inner());
        guard.values().map(|(_, info)| info.clone()).collect()
    }

    pub fn remove(&self, id: &str) -> bool {
        let mut guard = self.tunnels.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((tx, _)) = guard.remove(id) {
            let _ = tx.try_send(());
            true
        } else {
            false
        }
    }
}

/// Copy data both ways between a TCP stream and an SSH channel stream.
async fn copy_bidirectional<A, B>(mut a: A, mut b: B) -> std::io::Result<()>
where
    A: io::AsyncRead + io::AsyncWrite + Unpin,
    B: io::AsyncRead + io::AsyncWrite + Unpin,
{
    tokio::io::copy_bidirectional(&mut a, &mut b).await?;
    Ok(())
}

/// Run the tunnel loop: accept local connections and forward each via SSH direct-tcpip.
async fn run_tunnel_loop(
    tunnel_id: String,
    mut cancel_rx: mpsc::Receiver<()>,
    listener: TcpListener,
    handle: Handle<Client>,
    remote_host: String,
    remote_port: u16,
) {
    loop {
        tokio::select! {
            _ = cancel_rx.recv() => {
                info!("Tunnel {} stopped by user", tunnel_id);
                break;
            }
            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(x) => x,
                    Err(e) => {
                        error!("Tunnel {} accept error: {}", tunnel_id, e);
                        continue;
                    }
                };
                let originator_addr = peer.ip().to_string();
                let originator_port = peer.port() as u32;

                let channel = match handle
                    .channel_open_direct_tcpip(
                        remote_host.clone(),
                        remote_port as u32,
                        originator_addr,
                        originator_port,
                    )
                    .await
                {
                    Ok(ch) => ch,
                    Err(e) => {
                        error!("Tunnel {} channel_open_direct_tcpip error: {}", tunnel_id, e);
                        drop(stream);
                        continue;
                    }
                };

                let ssh_stream = channel.into_stream();
                tokio::spawn(async move {
                    if let Err(e) = copy_bidirectional(stream, ssh_stream).await {
                        error!("Tunnel copy_bidirectional error: {}", e);
                    }
                });
            }
        }
    }

    // Explicitly disconnect SSH session when tunnel loop exits (user stop or error)
    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// Start a local port forward: bind to local_port and forward to remote_host:remote_port via SSH.
pub async fn start_tunnel(
    app_handle: AppHandle,
    tunnel_state: &TunnelState,
    tunnel_id: String,
    ssh_host: String,
    ssh_port: u16,
    ssh_user: String,
    password: Option<String>,
    key_path: Option<String>,
    private_key: Option<String>,
    key_passphrase: Option<String>,
    local_port: u16,
    remote_host: String,
    remote_port: u16,
) -> Result<TunnelInfo, String> {
    validation::validate_port(ssh_port)?;
    validation::validate_port(local_port)?;
    validation::validate_port(remote_port)?;
    validation::validate_username(&ssh_user)?;
    if validation::validate_ip(&ssh_host).is_err() && validation::validate_hostname(&ssh_host).is_err() {
        return Err(format!("Invalid SSH host: {}", ssh_host));
    }

    let mut config = russh::client::Config::default();
    config.window_size = 4 * 1024 * 1024;
    config.maximum_packet_size = 32 * 1024;
    let config = Arc::new(config);

    let client = Client {
        app_handle: app_handle.clone(),
        host: ssh_host.clone(),
        port: ssh_port,
    };

    let addr = format!("{}:{}", ssh_host, ssh_port);
    let mut handle = match tokio::time::timeout(
        Duration::from_secs(10),
        russh::client::connect(config, addr, client),
    )
    .await
    {
        Ok(Ok(h)) => h,
        Ok(Err(e)) => return Err(e.to_string()),
        Err(_) => return Err("SSH connection timed out".to_string()),
    };

    ssh_auth::authenticate(
        &mut handle,
        &ssh_user,
        password.as_deref(),
        key_path.as_deref(),
        private_key.as_deref(),
        key_passphrase.as_deref(),
    )
    .await?;

    let bind_addr = format!("127.0.0.1:{}", local_port);
    let listener = match TcpListener::bind(&bind_addr).await {
        Ok(l) => l,
        Err(e) => {
            let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
            return Err(format!("Failed to bind {}: {}", bind_addr, e));
        }
    };

    let (cancel_tx, cancel_rx) = mpsc::channel(1);
    let info = TunnelInfo {
        id: tunnel_id.clone(),
        ssh_host: ssh_host.clone(),
        ssh_port,
        local_port,
        remote_host: remote_host.clone(),
        remote_port,
    };

    {
        let mut guard = tunnel_state.tunnels.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(tunnel_id.clone(), (cancel_tx, info.clone()));
    }

    let state_for_cleanup = (*tunnel_state).clone();
    tokio::spawn(async move {
        run_tunnel_loop(
            tunnel_id.clone(),
            cancel_rx,
            listener,
            handle,
            remote_host,
            remote_port,
        )
        .await;
        // Remove from state when loop exits (e.g. connection dropped)
        let mut guard = state_for_cleanup.tunnels.lock().unwrap_or_else(|e| e.into_inner());
        guard.remove(&tunnel_id);
    });

    Ok(info)
}

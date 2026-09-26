//! SSH local port forwarding (Phase 8: SSH feature enhancements).
//! Listens on a local port and forwards each connection through the SSH session
//! to a remote host:port via direct-tcpip channels.

use log::{error, info};
use russh::client::Handle;
use russh::Disconnect;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tauri::AppHandle;
use tokio::io;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

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

    fn in_use(&self, id: &str) -> bool {
        self.tunnels.lock().unwrap_or_else(|e| e.into_inner()).contains_key(id)
    }

    /// Registers a tunnel under `id` unless the id is taken. Replacing an entry dropped the
    /// old tunnel's cancel sender, which ended its loop, whose cleanup then removed the new
    /// tunnel's entry by the same id (P7-4 review).
    fn register(&self, id: &str, cancel_tx: mpsc::Sender<()>, info: TunnelInfo) -> Result<(), String> {
        let mut guard = self.tunnels.lock().unwrap_or_else(|e| e.into_inner());
        if guard.contains_key(id) {
            return Err(format!("Tunnel id {} is already in use", id));
        }
        guard.insert(id.to_string(), (cancel_tx, info));
        Ok(())
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

/// How often an idle tunnel checks whether its SSH session is still alive.
const SESSION_CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// SSH keepalives, so a dead network path is noticed even when no data flows
/// (russh closes the session after `keepalive_max` unanswered keepalives).
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const KEEPALIVE_MAX: usize = 3;

/// Run the tunnel loop: accept local connections and forward each via SSH direct-tcpip.
/// Ends when the user stops the tunnel or the SSH session dies; either way the local
/// port is released and the tunnel leaves the active list. It used to run forever on a
/// dead session, still listed as active, failing every connection (audit RUST-008).
async fn run_tunnel_loop<H: russh::client::Handler>(
    tunnel_id: String,
    mut cancel_rx: mpsc::Receiver<()>,
    listener: TcpListener,
    handle: Handle<H>,
    remote_host: String,
    remote_port: u16,
) {
    let mut session_check = tokio::time::interval(SESSION_CHECK_INTERVAL);
    loop {
        tokio::select! {
            _ = cancel_rx.recv() => {
                info!("Tunnel {} stopped by user", tunnel_id);
                break;
            }
            _ = session_check.tick() => {
                if handle.is_closed() {
                    error!("Tunnel {} closed: SSH session ended", tunnel_id);
                    break;
                }
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
                        if handle.is_closed() {
                            error!("Tunnel {} closed: SSH session ended", tunnel_id);
                            break;
                        }
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
    if tunnel_state.in_use(&tunnel_id) {
        return Err(format!("Tunnel id {} is already in use", tunnel_id));
    }
    validation::validate_port(ssh_port)?;
    validation::validate_port(local_port)?;
    validation::validate_port(remote_port)?;
    validation::validate_username(&ssh_user)?;
    if validation::validate_ip(&ssh_host).is_err()
        && validation::validate_hostname(&ssh_host).is_err()
    {
        return Err(format!("Invalid SSH host: {}", ssh_host));
    }

    let config = Arc::new(russh::client::Config {
        window_size: 4 * 1024 * 1024,
        maximum_packet_size: 32 * 1024,
        keepalive_interval: Some(KEEPALIVE_INTERVAL),
        keepalive_max: KEEPALIVE_MAX,
        ..Default::default()
    });

    let client = Client {
        app_handle: app_handle.clone(),
        host: ssh_host.clone(),
        port: ssh_port,
    };

    let mut handle = crate::ssh_connect::connect_with_timeouts(
        config,
        &ssh_host,
        ssh_port,
        client,
        Duration::from_secs(10),
        crate::ssh::INTERACTIVE_HANDSHAKE_TIMEOUT,
    )
    .await?;

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

    if let Err(e) = tunnel_state.register(&tunnel_id, cancel_tx, info.clone()) {
        let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
        return Err(e);
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
        let mut guard = state_for_cleanup
            .tunnels
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.remove(&tunnel_id);
    });

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AcceptAnyKey;

    impl russh::client::Handler for AcceptAnyKey {
        type Error = russh::Error;
        async fn check_server_key(
            &mut self,
            _key: &russh::keys::PublicKeyOrCertificate,
        ) -> Result<bool, Self::Error> {
            Ok(true)
        }
    }

    #[test]
    fn a_tunnel_id_can_not_be_reused_while_active() {
        let state = TunnelState::new();
        let info = |id: &str| TunnelInfo {
            id: id.to_string(),
            ssh_host: "h".into(),
            ssh_port: 22,
            local_port: 1080,
            remote_host: "r".into(),
            remote_port: 80,
        };
        let (tx1, _rx1) = mpsc::channel(1);
        state.register("t1", tx1, info("t1")).unwrap();
        let (tx2, mut rx2) = mpsc::channel(1);
        assert!(state.register("t1", tx2, info("t1")).is_err());
        // The rejected tunnel's sender was dropped, not stored under the old id.
        assert!(rx2.try_recv().is_err());
        assert!(state.in_use("t1"));
        assert!(state.remove("t1"));
        assert!(!state.in_use("t1"));
    }

    fn tunnel_info(id: &str, local_port: u16) -> TunnelInfo {
        TunnelInfo {
            id: id.to_string(),
            ssh_host: "h".into(),
            ssh_port: 22,
            local_port,
            remote_host: "r".into(),
            remote_port: 80,
        }
    }

    #[test]
    fn removing_a_tunnel_signals_its_loop_to_stop() {
        let state = TunnelState::new();
        let (tx, mut rx) = mpsc::channel(1);
        state.register("t1", tx, tunnel_info("t1", 1080)).unwrap();
        assert!(state.remove("t1"));
        assert!(rx.try_recv().is_ok(), "the tunnel loop must get a stop signal");
        // Removing it again, or an id that never existed, reports false.
        assert!(!state.remove("t1"));
        assert!(!state.remove("nope"));
    }

    #[test]
    fn list_reports_every_active_tunnel() {
        let state = TunnelState::new();
        assert!(state.list().is_empty());
        let (tx1, _rx1) = mpsc::channel(1);
        let (tx2, _rx2) = mpsc::channel(1);
        state.register("a", tx1, tunnel_info("a", 1080)).unwrap();
        state.register("b", tx2, tunnel_info("b", 1081)).unwrap();
        let mut ports: Vec<u16> = state.list().iter().map(|t| t.local_port).collect();
        ports.sort_unstable();
        assert_eq!(ports, vec![1080, 1081]);
        state.remove("a");
        let listed = state.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "b");
    }

    #[test]
    fn clones_share_the_same_tunnel_table() {
        let state = TunnelState::new();
        let clone = state.clone();
        let (tx, _rx) = mpsc::channel(1);
        state.register("t", tx, tunnel_info("t", 2000)).unwrap();
        assert!(clone.in_use("t"));
        assert!(clone.remove("t"));
        assert!(!state.in_use("t"));
    }

    #[test]
    fn remove_does_not_block_when_the_stop_signal_is_already_queued() {
        let state = TunnelState::new();
        let (tx, _rx) = mpsc::channel(1);
        // Fill the channel so try_send has nowhere to put the signal.
        tx.try_send(()).unwrap();
        state.register("t", tx, tunnel_info("t", 3000)).unwrap();
        assert!(state.remove("t"));
        assert!(!state.in_use("t"));
    }

    #[tokio::test]
    async fn copy_bidirectional_forwards_both_ways_until_both_sides_close() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (local, mut local_peer) = tokio::io::duplex(64);
        let (remote, mut remote_peer) = tokio::io::duplex(64);
        let pump = tokio::spawn(copy_bidirectional(local, remote));

        local_peer.write_all(b"ping").await.unwrap();
        let mut buf = [0u8; 4];
        remote_peer.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"ping");

        remote_peer.write_all(b"pong").await.unwrap();
        local_peer.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"pong");

        drop(local_peer);
        drop(remote_peer);
        tokio::time::timeout(Duration::from_secs(5), pump)
            .await
            .expect("the copy must end once both sides close")
            .unwrap()
            .unwrap();
    }

    /// RUST-008: when the SSH session dies, the loop ends and frees the local port.
    #[tokio::test]
    async fn tunnel_loop_ends_when_the_session_dies() {
        let policy = crate::ssh_test_server::Policy { accept_none: true, ..Default::default() };
        let (port, _, killer) = crate::ssh_test_server::spawn_killable(policy).await;
        let mut handle = crate::ssh_connect::connect_with_diagnostics(
            Arc::new(russh::client::Config::default()),
            "127.0.0.1",
            port,
            AcceptAnyKey,
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        ssh_auth::authenticate(&mut handle, "u", None, None, None, None).await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local = listener.local_addr().unwrap();
        let (_cancel_tx, cancel_rx) = mpsc::channel(1);
        let tunnel = tokio::spawn(run_tunnel_loop(
            "t".to_string(),
            cancel_rx,
            listener,
            handle,
            "127.0.0.1".to_string(),
            9,
        ));

        killer.kill_connections().await;
        tokio::time::timeout(SESSION_CHECK_INTERVAL * 3, tunnel)
            .await
            .expect("tunnel loop must end after the session dies")
            .unwrap();
        // The listener was dropped with the loop, so the port can be bound again.
        assert!(TcpListener::bind(local).await.is_ok());
    }
}

use crate::crypto;
use crate::validation;
use crate::vault::SshKeyManager;
use log::error;
use russh::keys::PublicKeyBase64;
use russh::*;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// The key a handshake actually presented, kept with its pending approval so that trusting
/// it persists *these* bytes, not whatever the caller supplies (IPC-002).
pub struct PresentedHostKey {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub key_type: String,
    pub key_bytes: Vec<u8>,
    /// Set when the host already had a different stored key (a possible MITM).
    pub old_fingerprint: Option<String>,
}

struct PendingApproval {
    sender: tokio::sync::oneshot::Sender<bool>,
    key: PresentedHostKey,
}

/// How long an interactive connection waits for the user to answer the host-key prompt.
pub const HOST_KEY_APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);

/// Handshake budget for interactive connections: the prompt plus time for the key exchange
/// itself (RUST-001).
pub const INTERACTIVE_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(135);

pub struct HostKeyApprovalState {
    pending: Mutex<HashMap<String, PendingApproval>>,
}

impl HostKeyApprovalState {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }

    fn register(
        &self,
        key: PresentedHostKey,
    ) -> Result<(String, tokio::sync::oneshot::Receiver<bool>), String> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| "Host key approval state is unavailable".to_string())?
            .insert(request_id.clone(), PendingApproval { sender, key });
        Ok((request_id, receiver))
    }

    /// For a pending *changed* key, the text of the native confirmation to show before
    /// accepting it (once or permanently). `None` for a first-seen key.
    pub fn changed_key_warning(&self, request_id: &str) -> Result<Option<String>, String> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| "Host key approval state is unavailable".to_string())?;
        let approval = pending
            .get(request_id)
            .ok_or_else(|| "Host key approval request is no longer pending".to_string())?;
        let key = &approval.key;
        Ok(key.old_fingerprint.as_ref().map(|old| {
            format!(
                "The host key for {}:{} has CHANGED.\n\nStored: {}\nPresented: {}\n\nThis can mean someone is intercepting the connection. Only continue if you have verified the new fingerprint with the server's administrator.",
                key.host, key.port, old, key.fingerprint
            )
        }))
    }

    fn take(&self, request_id: &str) -> Result<PendingApproval, String> {
        self.pending
            .lock()
            .map_err(|_| "Host key approval state is unavailable".to_string())?
            .remove(request_id)
            .ok_or_else(|| "Host key approval request is no longer pending".to_string())
    }

    /// Answers a pending prompt without persisting anything (one-time trust or reject).
    pub fn resolve(&self, request_id: &str, accepted: bool) -> Result<(), String> {
        self.take(request_id)?
            .sender
            .send(accepted)
            .map_err(|_| "SSH connection stopped waiting for host key approval".to_string())
    }

    /// Accepts a pending prompt and hands back the key that handshake presented, for the
    /// caller to persist. The connection is released only after `persist` succeeds.
    pub async fn accept_and_persist<F, Fut>(&self, request_id: &str, persist: F) -> Result<(), String>
    where
        F: FnOnce(PresentedHostKey) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        let PendingApproval { sender, key } = self.take(request_id)?;
        match persist(key).await {
            Ok(()) => sender
                .send(true)
                .map_err(|_| "SSH connection stopped waiting for host key approval".to_string()),
            Err(e) => {
                let _ = sender.send(false);
                Err(e)
            }
        }
    }

    fn cancel(&self, request_id: &str) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(request_id);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum InteractiveDecision {
    Accept,
    Refuse,
    Prompt,
}

/// What an interactive connection does with a verification result. A key the user
/// rejected is refused without a prompt: a webview prompt could be answered by a
/// compromised webview, and un-rejecting goes through `update_ssh_host_trust`'s native
/// confirmation (P7-4 review).
fn interactive_decision(result: &crate::vault::ssh_keys::HostKeyVerificationResult) -> InteractiveDecision {
    if result.allowed {
        InteractiveDecision::Accept
    } else if matches!(result.status, crate::vault::TrustStatus::Rejected) {
        InteractiveDecision::Refuse
    } else {
        InteractiveDecision::Prompt
    }
}

#[derive(Clone)]
pub struct Client {
    pub app_handle: AppHandle,
    pub host: String,
    pub port: u16,
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        let ssh_key_manager = self.app_handle.state::<SshKeyManager>();

        let key_bytes = server_public_key.public_key_bytes();
        let fingerprint = crypto::ssh_host_key_fingerprint(&key_bytes);
        let key_type = "ssh-key";

        match ssh_key_manager
            .verify_host_key_by_fingerprint(&self.host, self.port, &fingerprint, key_type)
            .await
        {
            Ok(result) => {
                let decision = interactive_decision(&result);
                if decision == InteractiveDecision::Accept {
                    Ok(true)
                } else if decision == InteractiveDecision::Refuse {
                    error!("Refusing rejected host key for {}:{}", self.host, self.port);
                    Ok(false)
                } else {
                    let approval_state = self.app_handle.state::<HostKeyApprovalState>();
                    let (request_id, receiver) = approval_state
                        .register(PresentedHostKey {
                            host: self.host.clone(),
                            port: self.port,
                            fingerprint: fingerprint.clone(),
                            key_type: key_type.to_string(),
                            key_bytes: key_bytes.clone(),
                            old_fingerprint: result.old_fingerprint.clone(),
                        })
                        .map_err(|_| russh::Error::Disconnect)?;

                    // Keep this handshake pending while the frontend displays the
                    // trust prompt. The response is correlated by requestId so an
                    // approval cannot accidentally release another connection.
                    if self
                        .app_handle
                        .emit(
                            "ssh-host-key-verification",
                            serde_json::json!({
                                "requestId": request_id,
                                "host": self.host,
                                "port": self.port,
                                "fingerprint": fingerprint,
                                "keyType": key_type,
                                "keyBytes": key_bytes,
                                "status": format!("{:?}", result.status),
                                "message": result.message,
                                "oldFingerprint": result.old_fingerprint,
                            }),
                        )
                        .is_err()
                    {
                        approval_state.cancel(&request_id);
                        return Err(russh::Error::Disconnect);
                    }

                    let approved =
                        match tokio::time::timeout(HOST_KEY_APPROVAL_TIMEOUT, receiver).await {
                            Ok(Ok(approved)) => approved,
                            _ => false,
                        };
                    approval_state.cancel(&request_id);
                    Ok(approved)
                }
            }
            Err(e) => {
                error!("Host key verification error: {}", e);
                Err(russh::Error::Disconnect)
            }
        }
    }

    // NOTE: We intentionally do NOT override data() here. All channel data is
    // consumed via Channel::wait() in the I/O task, which properly drains the
    // internal buffer and lets russh manage SSH window adjustments. Overriding
    // data() without draining wait() causes the internal buffer to fill, the
    // connection driver to block, and the terminal to hang.
}

pub struct SshConnection {
    pub disconnect_tx: tokio::sync::mpsc::Sender<()>,
    #[allow(dead_code)]
    pub bytes_received: Arc<AtomicU64>,
    pub last_activity: Arc<Mutex<Instant>>,
    pub stats_cancel_tx: tokio::sync::mpsc::Sender<()>,
    pub write_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    pub resize_tx: tokio::sync::mpsc::UnboundedSender<(u32, u32)>,
}

pub struct SshState {
    pub sessions: Arc<Mutex<HashMap<String, SshConnection>>>,
    pending_connections: Arc<Mutex<HashMap<String, PendingConnectionState>>>,
}

#[derive(Default)]
struct PendingConnectionState {
    cancelled: bool,
}

impl SshState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn begin_pending_connection(&self, id: &str) -> Result<(), String> {
        if self
            .sessions
            .lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?
            .contains_key(id)
        {
            return Err("Session already exists".to_string());
        }

        let mut pending = self
            .pending_connections
            .lock()
            .map_err(|e| format!("Failed to acquire pending-session lock: {}", e))?;
        if let Some(state) = pending.get(id) {
            if state.cancelled {
                pending.remove(id);
                return Err("Session connection was cancelled".to_string());
            }
            return Err("Session is already connecting".to_string());
        }
        pending.insert(id.to_string(), PendingConnectionState::default());
        Ok(())
    }

    fn mark_pending_cancelled(&self, id: &str) -> Result<bool, String> {
        let mut pending = self
            .pending_connections
            .lock()
            .map_err(|e| format!("Failed to acquire pending-session lock: {}", e))?;
        if let Some(state) = pending.get_mut(id) {
            state.cancelled = true;
        } else {
            pending.insert(id.to_string(), PendingConnectionState { cancelled: true });
        }
        Ok(true)
    }

    fn is_pending_cancelled(&self, id: &str) -> Result<bool, String> {
        Ok(self
            .pending_connections
            .lock()
            .map_err(|e| format!("Failed to acquire pending-session lock: {}", e))?
            .get(id)
            .map(|state| state.cancelled)
            .unwrap_or(false))
    }

    fn finish_pending_connection(&self, id: &str) {
        if let Ok(mut pending) = self.pending_connections.lock() {
            pending.remove(id);
        }
    }
}

/// Connect and authenticate SSH session. Use password and/or key (key_path or private_key PEM).
/// When enable_agent_forwarding is true, requests auth-agent@openssh.com forwarding on the session channel.
pub async fn connect_ssh(
    state: tauri::State<'_, SshState>,
    app_handle: AppHandle,
    id: String,
    host: String,
    user: String,
    port: u16,
    password: Option<String>,
    key_path: Option<String>,
    private_key: Option<String>,
    key_passphrase: Option<String>,
    enable_agent_forwarding: bool,
) -> Result<(), String> {
    state.begin_pending_connection(&id)?;

    let result = async {
    validation::validate_port(port)?;
    validation::validate_username(&user)?;

    if validation::validate_ip(&host).is_err() && validation::validate_hostname(&host).is_err() {
        return Err(format!("Invalid host format: {}", host));
    }

    let config = Arc::new(russh::client::Config {
        window_size: 4 * 1024 * 1024,
        maximum_packet_size: 32 * 1024,
        ..Default::default()
    });

    let sh = Client {
        app_handle: app_handle.clone(),
        host: host.clone(),
        port,
    };

    let mut session = crate::ssh_connect::connect_with_timeouts(
        config,
        &host,
        port,
        sh,
        Duration::from_secs(10),
        INTERACTIVE_HANDSHAKE_TIMEOUT,
    )
    .await?;

    crate::ssh_auth::authenticate(
        &mut session,
        &user,
        password.as_deref(),
        key_path.as_deref(),
        private_key.as_deref(),
        key_passphrase.as_deref(),
    )
    .await?;
    if state.is_pending_cancelled(&id)? {
        let _ = session
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        return Err("Session connection was cancelled".to_string());
    }

    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| e.to_string())?;
    if enable_agent_forwarding {
        let _ = channel.agent_forward(true).await; // best-effort; server may not support it
    }
    channel
        .request_pty(false, "xterm", 80, 24, 0, 0, &[])
        .await
        .map_err(|e| e.to_string())?;
    channel
        .request_shell(true)
        .await
        .map_err(|e| e.to_string())?;

    let bytes_received = Arc::new(AtomicU64::new(0));

    let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, mut resize_rx) = tokio::sync::mpsc::unbounded_channel::<(u32, u32)>();
    let (data_tx, mut data_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    let (disconnect_tx, mut disconnect_rx) = tokio::sync::mpsc::channel(1);
    let (stats_cancel_tx, mut stats_cancel_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Unified I/O task: owns the channel, reads via Channel::wait(), writes via
    // channel.data(), and handles resizes. Using Channel::wait() is critical —
    // it drains the channel's internal buffer so russh can send
    // SSH_MSG_CHANNEL_WINDOW_ADJUST back to the server, preventing stalls.
    let bytes_for_io = bytes_received.clone();
    tokio::spawn(async move {
        let mut pending_writes: VecDeque<Vec<u8>> = VecDeque::new();
        let mut pending_resizes: VecDeque<(u32, u32)> = VecDeque::new();

        loop {
            // Drain pending writes before blocking on wait() (FIFO so order is preserved)
            while let Some(data) = pending_writes.pop_front() {
                if channel.data(data.as_slice()).await.is_err() {
                    return;
                }
            }
            // Drain any freshly-queued writes (non-blocking)
            while let Ok(data) = write_rx.try_recv() {
                if channel.data(data.as_slice()).await.is_err() {
                    return;
                }
            }
            // Drain pending + freshly-queued resizes (FIFO so order is preserved)
            while let Some((cols, rows)) = pending_resizes.pop_front() {
                let _ = channel.window_change(cols, rows, 0, 0).await;
            }
            while let Ok((cols, rows)) = resize_rx.try_recv() {
                let _ = channel.window_change(cols, rows, 0, 0).await;
            }

            tokio::select! {
                msg = channel.wait() => {
                    match msg {
                        Some(ChannelMsg::Data { ref data }) => {
                            bytes_for_io.fetch_add(data.len() as u64, Ordering::Relaxed);
                            let _ = data_tx.send(data.to_vec());
                        }
                        Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                            bytes_for_io.fetch_add(data.len() as u64, Ordering::Relaxed);
                            let _ = data_tx.send(data.to_vec());
                        }
                        None => break,
                        _ => {}
                    }
                }
                Some(data) = write_rx.recv() => {
                    pending_writes.push_back(data);
                }
                Some(pair) = resize_rx.recv() => {
                    pending_resizes.push_back(pair);
                }
            }
        }
    });

    // Data batching task: buffers small chunks from the I/O task and flushes to
    // the frontend on newline/CR, when buffer exceeds 4 KB, or every 4 ms.
    let app_handle_for_batcher = app_handle.clone();
    let id_for_batcher = id.clone();
    tokio::spawn(async move {
        let mut buf: Vec<u8> = Vec::with_capacity(8192);
        let mut flush_interval = tokio::time::interval(Duration::from_millis(4));
        flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let event_name = format!("ssh_data_{}", id_for_batcher);

        loop {
            tokio::select! {
                biased;

                chunk = data_rx.recv() => {
                    match chunk {
                        Some(data) => {
                            buf.extend_from_slice(&data);
                            let should_flush_size = buf.len() >= 4096;
                            let should_flush_line = data.contains(&b'\r') || data.contains(&b'\n');
                            if should_flush_size || should_flush_line {
                                let s = String::from_utf8_lossy(&buf).to_string();
                                let _ = app_handle_for_batcher.emit(&event_name, s);
                                buf.clear();
                            }
                        }
                        None => {
                            if !buf.is_empty() {
                                let s = String::from_utf8_lossy(&buf).to_string();
                                let _ = app_handle_for_batcher.emit(&event_name, s);
                            }
                            break;
                        }
                    }
                }

                _ = flush_interval.tick() => {
                    if !buf.is_empty() {
                        let s = String::from_utf8_lossy(&buf).to_string();
                        let _ = app_handle_for_batcher.emit(&event_name, s);
                        buf.clear();
                    }
                }
            }
        }
    });

    let conn = SshConnection {
        disconnect_tx,
        bytes_received: bytes_received.clone(),
        last_activity: Arc::new(Mutex::new(Instant::now())),
        stats_cancel_tx,
        write_tx,
        resize_tx,
    };

    state
        .sessions
        .lock()
        .map_err(|e| format!("Failed to acquire session lock: {}", e))?
        .insert(id.clone(), conn);

    // Session driver: keeps the connection alive and emits close event.
    let id_for_driver = id.clone();
    let app_handle_for_driver = app_handle.clone();
    let sessions_for_driver = state.sessions.clone();

    tokio::spawn(async move {
        tokio::select! {
            _ = session => {
                let closed_session = {
                    let mut sessions = match sessions_for_driver.lock() {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                    sessions.remove(&id_for_driver)
                };
                if let Some(conn) = closed_session {
                    let _ = conn.stats_cancel_tx.send(()).await;
                }
                let _ = app_handle_for_driver.emit(&format!("ssh_closed_{}", id_for_driver), ());
            }
            _ = disconnect_rx.recv() => {
                let _ = app_handle_for_driver.emit(&format!("ssh_closed_{}", id_for_driver), ());
            }
        }
    });
    if state.is_pending_cancelled(&id)? {
        let cancelled_conn = state
            .sessions
            .lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?
            .remove(&id);
        if let Some(conn) = cancelled_conn {
            let _ = conn.stats_cancel_tx.send(()).await;
            let _ = conn.disconnect_tx.send(()).await;
        }
        return Err("Session connection was cancelled".to_string());
    }

    // Stats reporter: emits bandwidth info every second.
    let app_handle_for_stats = app_handle.clone();
    let id_for_stats = id.clone();
    let bytes_for_stats = bytes_received.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut last_check = Instant::now();
        let stats_event = format!("ssh_stats_{}", id_for_stats);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let exists = {
                        let state = app_handle_for_stats.state::<SshState>();
                        let has_key = match state.sessions.lock() {
                            Ok(s) => s.contains_key(&id_for_stats),
                            Err(_) => false,
                        };
                        has_key
                    };
                    if !exists {
                        break;
                    }

                    let bytes = bytes_for_stats.swap(0, Ordering::Relaxed);
                    let now = Instant::now();
                    let elapsed = now.duration_since(last_check).as_secs_f64();
                    last_check = now;

                    let kbps = if elapsed > 0.0 {
                        (bytes as f64 * 8.0) / (elapsed * 1024.0)
                    } else {
                        0.0
                    };

                    let bandwidth_str = if kbps > 1024.0 {
                        format!("{:.1} Mbps", kbps / 1024.0)
                    } else {
                        format!("{:.1} Kbps", kbps)
                    };

                    let _ = app_handle_for_stats.emit(&stats_event, serde_json::json!({
                        "bandwidth": bandwidth_str,
                        "latency": 0
                    }));
                }
                _ = stats_cancel_rx.recv() => {
                    break;
                }
            }
        }
    });

    Ok(())
    }
    .await;
    state.finish_pending_connection(&id);
    result
}

#[tauri::command]
pub async fn write_ssh(
    state: tauri::State<'_, SshState>,
    id: String,
    data: String,
) -> Result<(), String> {
    let (write_tx, last_activity) = {
        let sessions = state
            .sessions
            .lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions
            .get(&id)
            .map(|c| (c.write_tx.clone(), c.last_activity.clone()))
            .ok_or("Session not found")?
    };

    if let Ok(mut activity) = last_activity.lock() {
        *activity = Instant::now();
    }

    write_tx
        .send(data.into_bytes())
        .map_err(|_| "Session write channel closed".to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn resize_ssh(
    state: tauri::State<'_, SshState>,
    id: String,
    rows: u32,
    cols: u32,
) -> Result<(), String> {
    let resize_tx = {
        let sessions = state
            .sessions
            .lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions
            .get(&id)
            .map(|c| c.resize_tx.clone())
            .ok_or("Session not found")?
    };

    resize_tx
        .send((cols, rows))
        .map_err(|_| "Resize channel closed".to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn disconnect_ssh(state: tauri::State<'_, SshState>, id: String) -> Result<(), String> {
    let mut conn = {
        let mut sessions = state
            .sessions
            .lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions.remove(&id)
    };

    if let Some(conn) = conn {
        let _ = conn.disconnect_tx.send(()).await;
        let _ = conn.stats_cancel_tx.send(()).await;
        return Ok(());
    }

    let pending_cancelled = state.mark_pending_cancelled(&id)?;
    if conn.is_none() {
        conn = state
            .sessions
            .lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?
            .remove(&id);
        if let Some(conn) = conn {
            let _ = conn.disconnect_tx.send(()).await;
            let _ = conn.stats_cancel_tx.send(()).await;
            return Ok(());
        }
    }

    if pending_cancelled {
        return Ok(());
    }

    Ok(())
}

#[cfg(test)]
mod host_key_approval_tests {
    use super::{HostKeyApprovalState, PresentedHostKey, SshState};

    fn presented(host: &str) -> PresentedHostKey {
        PresentedHostKey {
            host: host.to_string(),
            port: 22,
            fingerprint: format!("SHA256:{}", host),
            key_type: "ssh-key".to_string(),
            key_bytes: vec![1, 2, 3],
            old_fingerprint: None,
        }
    }

    #[test]
    fn rejected_keys_are_refused_without_a_prompt() {
        use super::{interactive_decision, InteractiveDecision};
        use crate::vault::{ssh_keys::HostKeyVerificationResult, TrustStatus};
        let result = |allowed, status| HostKeyVerificationResult {
            allowed,
            status,
            fingerprint: "fp".into(),
            message: String::new(),
            old_fingerprint: None,
        };
        assert_eq!(interactive_decision(&result(true, TrustStatus::Trusted)), InteractiveDecision::Accept);
        assert_eq!(interactive_decision(&result(false, TrustStatus::Rejected)), InteractiveDecision::Refuse);
        assert_eq!(interactive_decision(&result(false, TrustStatus::Unknown)), InteractiveDecision::Prompt);
        assert_eq!(interactive_decision(&result(false, TrustStatus::Changed)), InteractiveDecision::Prompt);
    }

    /// Accepting a changed key needs a native confirmation; a first-seen key doesn't.
    #[test]
    fn only_changed_keys_carry_a_native_warning() {
        let state = HostKeyApprovalState::new();
        let (fresh, _rx1) = state.register(presented("a")).unwrap();
        assert_eq!(state.changed_key_warning(&fresh).unwrap(), None);
        let mut changed = presented("b");
        changed.old_fingerprint = Some("SHA256:old".to_string());
        let (id, _rx2) = state.register(changed).unwrap();
        let warning = state.changed_key_warning(&id).unwrap().unwrap();
        assert!(warning.contains("SHA256:old") && warning.contains("SHA256:b"));
        assert!(state.changed_key_warning("missing").is_err());
    }

    #[tokio::test]
    async fn resolves_the_matching_pending_approval() {
        let state = HostKeyApprovalState::new();
        let (request_id, receiver) = state.register(presented("a")).unwrap();

        state.resolve(&request_id, true).unwrap();

        assert!(receiver.await.unwrap());
        assert!(state.resolve(&request_id, true).is_err());
    }

    /// IPC-002: trusting persists the key the handshake presented (not caller data), and
    /// the connection is only released once that succeeded.
    #[tokio::test]
    async fn accept_persists_the_presented_key_then_releases_the_handshake() {
        let state = HostKeyApprovalState::new();
        let (request_id, receiver) = state.register(presented("real.host")).unwrap();
        let mut persisted = None;
        state
            .accept_and_persist(&request_id, |key| {
                persisted = Some((key.host.clone(), key.fingerprint.clone(), key.key_bytes.clone()));
                async { Ok(()) }
            })
            .await
            .unwrap();
        assert_eq!(
            persisted,
            Some(("real.host".to_string(), "SHA256:real.host".to_string(), vec![1, 2, 3]))
        );
        assert!(receiver.await.unwrap());
        assert!(state.accept_and_persist(&request_id, |_| async { Ok(()) }).await.is_err());
    }

    #[tokio::test]
    async fn failed_persist_rejects_the_handshake() {
        let state = HostKeyApprovalState::new();
        let (request_id, receiver) = state.register(presented("h")).unwrap();
        let err = state
            .accept_and_persist(&request_id, |_| async { Err("disk full".to_string()) })
            .await
            .unwrap_err();
        assert_eq!(err, "disk full");
        assert!(!receiver.await.unwrap());
    }

    #[test]
    fn pending_connection_cancellation_is_tracked() {
        let state = SshState::new();
        state.begin_pending_connection("session-1").unwrap();
        assert!(state.mark_pending_cancelled("session-1").unwrap());
        assert!(state.is_pending_cancelled("session-1").unwrap());
        state.finish_pending_connection("session-1");
        assert!(state.mark_pending_cancelled("session-1").unwrap());
    }

    #[test]
    fn cancelled_tombstone_blocks_a_late_connection_start() {
        let state = SshState::new();

        assert!(state.mark_pending_cancelled("session-2").unwrap());
        assert_eq!(
            state.begin_pending_connection("session-2").unwrap_err(),
            "Session connection was cancelled"
        );
        state.begin_pending_connection("session-2").unwrap();
    }
}

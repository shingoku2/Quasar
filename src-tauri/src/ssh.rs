use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use russh::*;
use russh::keys::PublicKeyBase64;
use tokio::sync::Mutex as TokioMutex;
use log::error;
use crate::crypto;
use crate::vault::SshKeyManager;
use crate::validation;

#[derive(Clone)]
pub struct Client {
    app_handle: AppHandle,
    host: String,
    port: u16,
    data_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Get SSH key manager from app state
        let ssh_key_manager = self.app_handle.state::<SshKeyManager>();
        
        // Compute SHA256 fingerprint using shared utility (RFC 4253 §6.6)
        let key_bytes = server_public_key.public_key_bytes();
        let fingerprint = crypto::ssh_host_key_fingerprint(&key_bytes);
        let key_type = "ssh-key"; // Generic type since russh doesn't expose key type easily
        
        // Verify host key
        match ssh_key_manager.verify_host_key_by_fingerprint(
            &self.host,
            self.port,
            &fingerprint,
            key_type,
        ).await {
            Ok(result) => {
                if result.allowed {
                    Ok(true)
                } else {
                    // Emit event to frontend for user decision
                    let _ = self.app_handle.emit("ssh-host-key-verification", serde_json::json!({
                        "host": self.host,
                        "port": self.port,
                        "fingerprint": fingerprint,
                        "keyType": key_type,
                        "keyBytes": key_bytes,
                        "status": format!("{:?}", result.status),
                        "message": result.message,
                    }));
                    
                    // Reject connection - user must explicitly trust via frontend
                    Err(russh::Error::Disconnect)
                }
            }
            Err(e) => {
                error!("Host key verification error: {}", e);
                Err(russh::Error::Disconnect)
            }
        }
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // Lock-free: send data to the batching task via unbounded channel.
        // No mutex acquisition here — the session driver is never blocked.
        let _ = self.data_tx.send(data.to_vec());
        Ok(())
    }
}

pub struct SshConnection {
    pub channel: Arc<TokioMutex<russh::Channel<russh::client::Msg>>>,
    pub disconnect_tx: tokio::sync::mpsc::Sender<()>,
    #[allow(dead_code)] // Accessed via Arc clones in batching/stats tasks
    pub bytes_received: Arc<AtomicU64>,
    pub last_activity: Arc<Mutex<Instant>>,
    pub stats_cancel_tx: tokio::sync::mpsc::Sender<()>,
}

pub struct SshState {
    pub sessions: Arc<Mutex<HashMap<String, SshConnection>>>,
}

impl SshState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Connect and authenticate SSH session. Use password and/or key (key_path or private_key PEM).
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
) -> Result<(), String> {
    // Input validation
    validation::validate_port(port)?;
    validation::validate_username(&user)?;
    
    // Validate host (can be IP or hostname)
    if validation::validate_ip(&host).is_err() && validation::validate_hostname(&host).is_err() {
        return Err(format!("Invalid host format: {}", host));
    }
    
    // Increase window sizes to prevent flow-control stalls on high-output commands
    let mut config = russh::client::Config::default();
    config.window_size = 4 * 1024 * 1024;     // 4 MB (default 2 MB)
    config.maximum_packet_size = 32 * 1024;    // 32 KB — must not exceed TCP max (65535)
    let config = Arc::new(config);

    // Create the unbounded channel for lock-free data forwarding from the
    // Handler callback to the batching task.
    let (data_tx, mut data_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    let sh = Client {
        app_handle: app_handle.clone(),
        host: host.clone(),
        port,
        data_tx,
    };

    let addr = format!("{}:{}", host, port);
    
    let mut session = match tokio::time::timeout(
        Duration::from_secs(5),
        russh::client::connect(config, addr, sh)
    ).await {
        Ok(res) => res.map_err(|e| e.to_string())?,
        Err(_) => return Err("Connection timed out".to_string()),
    };

    crate::ssh_auth::authenticate(
        &mut session,
        &user,
        password.as_deref(),
        key_path.as_deref(),
        private_key.as_deref(),
        key_passphrase.as_deref(),
    ).await?;

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_pty(false, "xterm", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    channel.request_shell(true).await.map_err(|e| e.to_string())?;

    let channel_handle = Arc::new(TokioMutex::new(channel));
    
    let bytes_received = Arc::new(AtomicU64::new(0));

    let (disconnect_tx, mut disconnect_rx) = tokio::sync::mpsc::channel(1);
    let (stats_cancel_tx, mut stats_cancel_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Spawn session driver
    let id_for_driver = id.clone();
    let app_handle_for_driver = app_handle.clone();
    
    tokio::spawn(async move {
        tokio::select! {
            _ = session => {
                let _ = app_handle_for_driver.emit(&format!("ssh_closed_{}", id_for_driver), ());
            }
            _ = disconnect_rx.recv() => {
                // Dropping 'session' here will close the connection
                let _ = app_handle_for_driver.emit(&format!("ssh_closed_{}", id_for_driver), ());
            }
        }
    });

    // Spawn data batching task: reads from the unbounded channel, buffers data,
    // and flushes to the frontend either when the buffer exceeds 4 KB or every
    // 8 ms — whichever comes first. This reduces IPC events by 10-100x for
    // high-throughput commands compared to emitting every SSH chunk individually.
    let bytes_for_batcher = bytes_received.clone();
    let app_handle_for_batcher = app_handle.clone();
    let id_for_batcher = id.clone();
    tokio::spawn(async move {
        let mut buf: Vec<u8> = Vec::with_capacity(8192);
        let mut flush_interval = tokio::time::interval(Duration::from_millis(8));
        flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let event_name = format!("ssh_data_{}", id_for_batcher);

        loop {
            tokio::select! {
                biased;

                chunk = data_rx.recv() => {
                    match chunk {
                        Some(data) => {
                            bytes_for_batcher.fetch_add(data.len() as u64, Ordering::Relaxed);
                            buf.extend_from_slice(&data);
                            // Flush immediately when the buffer is large enough
                            let should_flush_size = buf.len() >= 4096;
                            // Flush on carriage return so progress lines (e.g. apt "Reading package lists... 0%\r") appear without delay
                            let should_flush_cr = data.contains(&b'\r');
                            if should_flush_size || should_flush_cr {
                                let s = String::from_utf8_lossy(&buf).to_string();
                                let _ = app_handle_for_batcher.emit(&event_name, s);
                                buf.clear();
                            }
                        }
                        None => {
                            // Sender dropped — session ended. Flush remaining data.
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
        channel: channel_handle,
        disconnect_tx,
        bytes_received: bytes_received.clone(),
        last_activity: Arc::new(Mutex::new(Instant::now())),
        stats_cancel_tx,
    };

    state.sessions.lock()
        .map_err(|e| format!("Failed to acquire session lock: {}", e))?
        .insert(id.clone(), conn);

    // Spawn stats reporter with cancellation support.
    // Uses atomic swap to read and reset bytes — no mutex contention.
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
                    // Check if session still exists
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
                    // Explicitly cancelled - clean exit
                    break;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn write_ssh(
    state: tauri::State<'_, SshState>,
    id: String,
    data: String,
) -> Result<(), String> {
    let (channel_arc, last_activity) = {
        let sessions = state.sessions.lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions.get(&id).map(|c| (c.channel.clone(), c.last_activity.clone())).ok_or("Session not found")?
    };

    // Update last activity timestamp
    if let Ok(mut activity) = last_activity.lock() {
        *activity = Instant::now();
    }

    let channel = channel_arc.lock().await;
    channel.data(data.as_bytes())
        .await
        .map_err(|_| "Failed to send data".to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn resize_ssh(
    state: tauri::State<'_, SshState>,
    id: String,
    rows: u32,
    cols: u32,
) -> Result<(), String> {
    let conn = {
        let sessions = state.sessions.lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions.get(&id).map(|c| c.channel.clone())
    };

    if let Some(channel_arc) = conn {
        let channel = channel_arc.lock().await;
        channel.window_change(cols, rows, 0, 0)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Session not found".to_string())
    }
}

#[tauri::command]
pub async fn disconnect_ssh(
    state: tauri::State<'_, SshState>,
    id: String,
) -> Result<(), String> {
    let conn = {
        let mut sessions = state.sessions.lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions.remove(&id)
    };

    if let Some(conn) = conn {
        // Send disconnect signal to background driver
        let _ = conn.disconnect_tx.send(()).await;
        // Cancel stats reporter task
        let _ = conn.stats_cancel_tx.send(()).await;
        Ok(())
    } else {
        Err("Session not found".to_string())
    }
}

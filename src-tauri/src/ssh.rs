use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use russh::*;
use russh::keys::PublicKeyBase64;
use log::error;
use crate::crypto;
use crate::vault::SshKeyManager;
use crate::validation;

#[derive(Clone)]
pub struct Client {
    app_handle: AppHandle,
    host: String,
    port: u16,
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
                    let _ = self.app_handle.emit("ssh-host-key-verification", serde_json::json!({
                        "host": self.host,
                        "port": self.port,
                        "fingerprint": fingerprint,
                        "keyType": key_type,
                        "keyBytes": key_bytes,
                        "status": format!("{:?}", result.status),
                        "message": result.message,
                    }));
                    Err(russh::Error::Disconnect)
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
    validation::validate_port(port)?;
    validation::validate_username(&user)?;

    if validation::validate_ip(&host).is_err() && validation::validate_hostname(&host).is_err() {
        return Err(format!("Invalid host format: {}", host));
    }

    let mut config = russh::client::Config::default();
    config.window_size = 4 * 1024 * 1024;
    config.maximum_packet_size = 32 * 1024;
    let config = Arc::new(config);

    let sh = Client {
        app_handle: app_handle.clone(),
        host: host.clone(),
        port,
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

    let mut channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_pty(false, "xterm", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    channel.request_shell(true).await.map_err(|e| e.to_string())?;

    let bytes_received = Arc::new(AtomicU64::new(0));

    let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, mut resize_rx) = tokio::sync::mpsc::unbounded_channel::<(u32, u32)>();
    let (data_tx, mut data_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    let (disconnect_tx, mut disconnect_rx) = tokio::sync::mpsc::channel(1);
    let (stats_cancel_tx, mut stats_cancel_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Session driver: keeps the connection alive and emits close event.
    let id_for_driver = id.clone();
    let app_handle_for_driver = app_handle.clone();

    tokio::spawn(async move {
        tokio::select! {
            _ = session => {
                let _ = app_handle_for_driver.emit(&format!("ssh_closed_{}", id_for_driver), ());
            }
            _ = disconnect_rx.recv() => {
                let _ = app_handle_for_driver.emit(&format!("ssh_closed_{}", id_for_driver), ());
            }
        }
    });

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
                if channel.data(data.as_slice()).await.is_err() { return; }
            }
            // Drain any freshly-queued writes (non-blocking)
            while let Ok(data) = write_rx.try_recv() {
                if channel.data(data.as_slice()).await.is_err() { return; }
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

    state.sessions.lock()
        .map_err(|e| format!("Failed to acquire session lock: {}", e))?
        .insert(id.clone(), conn);

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

#[tauri::command]
pub async fn write_ssh(
    state: tauri::State<'_, SshState>,
    id: String,
    data: String,
) -> Result<(), String> {
    let (write_tx, last_activity) = {
        let sessions = state.sessions.lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions.get(&id)
            .map(|c| (c.write_tx.clone(), c.last_activity.clone()))
            .ok_or("Session not found")?
    };

    if let Ok(mut activity) = last_activity.lock() {
        *activity = Instant::now();
    }

    write_tx.send(data.into_bytes()).map_err(|_| "Session write channel closed".to_string())?;
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
        let sessions = state.sessions.lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions.get(&id)
            .map(|c| c.resize_tx.clone())
            .ok_or("Session not found")?
    };

    resize_tx.send((cols, rows)).map_err(|_| "Resize channel closed".to_string())?;
    Ok(())
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
        let _ = conn.disconnect_tx.send(()).await;
        let _ = conn.stats_cancel_tx.send(()).await;
        Ok(())
    } else {
        Err("Session not found".to_string())
    }
}

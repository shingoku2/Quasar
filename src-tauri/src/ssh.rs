use std::collections::HashMap;
use std::sync::{Arc, Mutex};
<<<<<<< HEAD
use std::sync::atomic::{AtomicU64, Ordering};
=======
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use russh::*;
use russh::keys::PublicKeyBase64;
<<<<<<< HEAD
=======
use tokio::sync::Mutex as TokioMutex;
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
use log::error;
use crate::crypto;
use crate::vault::SshKeyManager;
use crate::validation;

#[derive(Clone)]
pub struct Client {
<<<<<<< HEAD
=======
    id: String,
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
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
<<<<<<< HEAD
        let ssh_key_manager = self.app_handle.state::<SshKeyManager>();

        let key_bytes = server_public_key.public_key_bytes();
        let fingerprint = crypto::ssh_host_key_fingerprint(&key_bytes);
        let key_type = "ssh-key";

=======
        // Get SSH key manager from app state
        let ssh_key_manager = self.app_handle.state::<SshKeyManager>();
        
        // Compute SHA256 fingerprint using shared utility (RFC 4253 §6.6)
        let key_bytes = server_public_key.public_key_bytes();
        let fingerprint = crypto::ssh_host_key_fingerprint(&key_bytes);
        let key_type = "ssh-key"; // Generic type since russh doesn't expose key type easily
        
        // Verify host key
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
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
<<<<<<< HEAD
=======
                    // Emit event to frontend for user decision
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
                    let _ = self.app_handle.emit("ssh-host-key-verification", serde_json::json!({
                        "host": self.host,
                        "port": self.port,
                        "fingerprint": fingerprint,
                        "keyType": key_type,
                        "keyBytes": key_bytes,
                        "status": format!("{:?}", result.status),
                        "message": result.message,
                    }));
<<<<<<< HEAD
=======
                    
                    // Reject connection - user must explicitly trust via frontend
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
                    Err(russh::Error::Disconnect)
                }
            }
            Err(e) => {
                error!("Host key verification error: {}", e);
                Err(russh::Error::Disconnect)
            }
        }
    }

<<<<<<< HEAD
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
=======
    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // Update stats
        let state = self.app_handle.state::<SshState>();
        if let Ok(sessions) = state.sessions.lock() {
            if let Some(conn) = sessions.get(&self.id) {
                if let Ok(mut bytes) = conn.bytes_received.lock() {
                    *bytes += data.len() as u64;
                }
            }
        }

        let data_str = String::from_utf8_lossy(data).to_string();
        let _ = self.app_handle.emit(&format!("ssh_data_{}", self.id), data_str);
        Ok(())
    }
}

pub struct SshConnection {
    pub channel: Arc<TokioMutex<russh::Channel<russh::client::Msg>>>,
    pub disconnect_tx: tokio::sync::mpsc::Sender<()>,
    pub bytes_received: Arc<Mutex<u64>>,
    pub last_stats_check: Arc<Mutex<Instant>>,
    pub last_activity: Arc<Mutex<Instant>>,
    pub stats_cancel_tx: tokio::sync::mpsc::Sender<()>,
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
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
<<<<<<< HEAD
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
=======
    // Input validation
    validation::validate_port(port)?;
    validation::validate_username(&user)?;
    
    // Validate host (can be IP or hostname)
    if validation::validate_ip(&host).is_err() && validation::validate_hostname(&host).is_err() {
        return Err(format!("Invalid host format: {}", host));
    }
    
    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = Client {
        id: id.clone(),
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
        app_handle: app_handle.clone(),
        host: host.clone(),
        port,
    };

    let addr = format!("{}:{}", host, port);
<<<<<<< HEAD

=======
    
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
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

<<<<<<< HEAD
    let mut channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_pty(false, "xterm", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    channel.request_shell(true).await.map_err(|e| e.to_string())?;

    let bytes_received = Arc::new(AtomicU64::new(0));

    let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, mut resize_rx) = tokio::sync::mpsc::unbounded_channel::<(u32, u32)>();
    let (data_tx, mut data_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
=======
    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_pty(false, "xterm", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    channel.request_shell(true).await.map_err(|e| e.to_string())?;

    let channel_handle = Arc::new(TokioMutex::new(channel));
    
    let bytes_received = Arc::new(Mutex::new(0));
    let last_stats_check = Arc::new(Mutex::new(Instant::now()));
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7

    let (disconnect_tx, mut disconnect_rx) = tokio::sync::mpsc::channel(1);
    let (stats_cancel_tx, mut stats_cancel_rx) = tokio::sync::mpsc::channel::<()>(1);

<<<<<<< HEAD
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
        let mut pending_writes: Vec<Vec<u8>> = Vec::new();
        let mut pending_resizes: Vec<(u32, u32)> = Vec::new();

        loop {
            // Drain pending writes before blocking on wait()
            while let Some(data) = pending_writes.pop() {
                if channel.data(data.as_slice()).await.is_err() { return; }
            }
            // Drain any freshly-queued writes (non-blocking)
            while let Ok(data) = write_rx.try_recv() {
                if channel.data(data.as_slice()).await.is_err() { return; }
            }
            // Drain pending + freshly-queued resizes
            while let Some((cols, rows)) = pending_resizes.pop() {
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
                    pending_writes.push(data);
                }
                Some(pair) = resize_rx.recv() => {
                    pending_resizes.push(pair);
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
=======
    // Spawn session driver
    let id_for_closure = id.clone();
    let app_handle_for_closure = app_handle.clone();
    
    tokio::spawn(async move {
        tokio::select! {
            _ = session => {
                let _ = app_handle_for_closure.emit(&format!("ssh_closed_{}", id_for_closure), ());
            }
            _ = disconnect_rx.recv() => {
                // Dropping 'session' here will close the connection
                let _ = app_handle_for_closure.emit(&format!("ssh_closed_{}", id_for_closure), ());
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
            }
        }
    });

    let conn = SshConnection {
<<<<<<< HEAD
        disconnect_tx,
        bytes_received: bytes_received.clone(),
        last_activity: Arc::new(Mutex::new(Instant::now())),
        stats_cancel_tx,
        write_tx,
        resize_tx,
=======
        channel: channel_handle,
        disconnect_tx,
        bytes_received: bytes_received.clone(),
        last_stats_check: last_stats_check.clone(),
        last_activity: Arc::new(Mutex::new(Instant::now())),
        stats_cancel_tx,
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
    };

    state.sessions.lock()
        .map_err(|e| format!("Failed to acquire session lock: {}", e))?
        .insert(id.clone(), conn);

<<<<<<< HEAD
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
=======
    // Spawn stats reporter with cancellation support
    let app_handle_clone = app_handle.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let stats_result = {
                        let state = app_handle_clone.state::<SshState>();
                        let sessions = match state.sessions.lock() {
                            Ok(s) => s,
                            Err(_) => break,
                        };
                        if let Some(c) = sessions.get(&id_clone) {
                            // Hold both locks for the entire read-modify-write operation
                            let mut bytes_guard = match c.bytes_received.lock() {
                                Ok(g) => g,
                                Err(_) => continue,
                            };
                            let mut time_guard = match c.last_stats_check.lock() {
                                Ok(g) => g,
                                Err(_) => continue,
                            };
                            
                            let b = *bytes_guard;
                            let t = *time_guard;
                            
                            *bytes_guard = 0;
                            *time_guard = Instant::now();
                            
                            Some((b, t))
                        } else {
                            None
                        }
                    };

                    if let Some((bytes, last_time)) = stats_result {
                        let elapsed = last_time.elapsed().as_secs_f64();
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

                        let _ = app_handle_clone.emit(&format!("ssh_stats_{}", id_clone), serde_json::json!({
                            "bandwidth": bandwidth_str,
                            "latency": 0
                        }));
                    } else {
                        break;
                    }
                }
                _ = stats_cancel_rx.recv() => {
                    // Explicitly cancelled - clean exit
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
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
<<<<<<< HEAD
    let (write_tx, last_activity) = {
        let sessions = state.sessions.lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions.get(&id)
            .map(|c| (c.write_tx.clone(), c.last_activity.clone()))
            .ok_or("Session not found")?
    };

=======
    let (channel_arc, last_activity) = {
        let sessions = state.sessions.lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions.get(&id).map(|c| (c.channel.clone(), c.last_activity.clone())).ok_or("Session not found")?
    };

    // Update last activity timestamp
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
    if let Ok(mut activity) = last_activity.lock() {
        *activity = Instant::now();
    }

<<<<<<< HEAD
    write_tx.send(data.into_bytes()).map_err(|_| "Session write channel closed".to_string())?;
=======
    let channel = channel_arc.lock().await;
    channel.data(data.as_bytes())
        .await
        .map_err(|_| "Failed to send data".to_string())?;
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
    Ok(())
}

#[tauri::command]
pub async fn resize_ssh(
    state: tauri::State<'_, SshState>,
    id: String,
    rows: u32,
    cols: u32,
) -> Result<(), String> {
<<<<<<< HEAD
    let resize_tx = {
        let sessions = state.sessions.lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        sessions.get(&id)
            .map(|c| c.resize_tx.clone())
            .ok_or("Session not found")?
    };

    resize_tx.send((cols, rows)).map_err(|_| "Resize channel closed".to_string())?;
    Ok(())
=======
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
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
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
<<<<<<< HEAD
        let _ = conn.disconnect_tx.send(()).await;
=======
        // Send disconnect signal to background driver
        let _ = conn.disconnect_tx.send(()).await;
        // Cancel stats reporter task
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
        let _ = conn.stats_cancel_tx.send(()).await;
        Ok(())
    } else {
        Err("Session not found".to_string())
    }
<<<<<<< HEAD
}
=======
}
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7

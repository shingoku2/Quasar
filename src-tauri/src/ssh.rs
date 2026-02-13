use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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
    id: String,
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

#[tauri::command]
pub async fn connect_ssh(
    state: tauri::State<'_, SshState>,
    app_handle: AppHandle,
    id: String,
    host: String,
    user: String,
    port: u16,
    password: Option<String>,
) -> Result<(), String> {
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

    if let Some(pass) = password {
        let auth_res = session.authenticate_password(&user, pass).await.map_err(|e| e.to_string())?;
        let is_success = match auth_res {
             russh::client::AuthResult::Success => true,
             _ => false,
        };
        if !is_success {
             return Err("Authentication failed".to_string());
        }
    } else {
        return Err("No password provided".to_string());
    };

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_pty(false, "xterm", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    channel.request_shell(true).await.map_err(|e| e.to_string())?;

    let channel_handle = Arc::new(TokioMutex::new(channel));
    
    let bytes_received = Arc::new(Mutex::new(0));
    let last_stats_check = Arc::new(Mutex::new(Instant::now()));

    let (disconnect_tx, mut disconnect_rx) = tokio::sync::mpsc::channel(1);
    let (stats_cancel_tx, mut stats_cancel_rx) = tokio::sync::mpsc::channel::<()>(1);

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
            }
        }
    });

    let conn = SshConnection {
        channel: channel_handle,
        disconnect_tx,
        bytes_received: bytes_received.clone(),
        last_stats_check: last_stats_check.clone(),
        last_activity: Arc::new(Mutex::new(Instant::now())),
        stats_cancel_tx,
    };

    state.sessions.lock()
        .map_err(|e| format!("Failed to acquire session lock: {}", e))?
        .insert(id.clone(), conn);

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
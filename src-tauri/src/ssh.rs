use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use russh::*;
use russh::keys::*;
use russh::client::*;
use tokio::sync::Mutex as TokioMutex;

#[derive(Clone)]
pub struct Client {
    id: String,
    app_handle: AppHandle,
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
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
    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = Client {
        id: id.clone(),
        app_handle: app_handle.clone(),
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

    let mut channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_pty(false, "xterm", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    channel.request_shell(true).await.map_err(|e| e.to_string())?;

    let channel_handle = Arc::new(TokioMutex::new(channel));
    
    let bytes_received = Arc::new(Mutex::new(0));
    let last_stats_check = Arc::new(Mutex::new(Instant::now()));

    let (disconnect_tx, mut disconnect_rx) = tokio::sync::mpsc::channel(1);

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
    };

    state.sessions.lock().unwrap().insert(id.clone(), conn);

    // Spawn stats reporter
    let app_handle_clone = app_handle.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            let (bytes, last_time) = {
                let state = app_handle_clone.state::<SshState>();
                let sessions = state.sessions.lock().unwrap();
                if let Some(c) = sessions.get(&id_clone) {
                    let b = *c.bytes_received.lock().unwrap();
                    let t = *c.last_stats_check.lock().unwrap();
                    
                    *c.bytes_received.lock().unwrap() = 0;
                    *c.last_stats_check.lock().unwrap() = Instant::now();
                    
                    (b, t)
                } else {
                    break;
                }
            };

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
    let conn = {
        let sessions = state.sessions.lock().unwrap();
        sessions.get(&id).map(|c| c.channel.clone())
    };

    if let Some(channel_arc) = conn {
        let mut channel = channel_arc.lock().await;
        channel.data(data.as_bytes())
            .await
            .map_err(|_| "Failed to send data".to_string())?;
        Ok(())
    } else {
        Err("Session not found".to_string())
    }
}

#[tauri::command]
pub async fn resize_ssh(
    state: tauri::State<'_, SshState>,
    id: String,
    rows: u32,
    cols: u32,
) -> Result<(), String> {
    let conn = {
        let sessions = state.sessions.lock().unwrap();
        sessions.get(&id).map(|c| c.channel.clone())
    };

    if let Some(channel_arc) = conn {
        let mut channel = channel_arc.lock().await;
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
        let mut sessions = state.sessions.lock().unwrap();
        sessions.remove(&id)
    };

    if let Some(conn) = conn {
        // Send disconnect signal to background driver
        let _ = conn.disconnect_tx.send(()).await;
        Ok(())
    } else {
        Err("Session not found".to_string())
    }
}
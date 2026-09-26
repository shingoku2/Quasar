//! App-lifetime background loops started from `setup`, after every `app.manage(...)` and
//! the migrations. They run until the app exits.

use crate::{ssh, vault};
use log::error;
use tauri::{AppHandle, Emitter, Manager};

/// Locks the vault after its idle timeout (checked every 30 s) and tells the UI.
pub(crate) fn spawn_auto_lock(app: &AppHandle) {
    let vault_state_clone = app.state::<vault::VaultState>().inner().clone();
    let app_handle_vault = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            match vault_state_clone.check_auto_lock().await {
                Ok(locked) => {
                    if locked {
                        let _ = app_handle_vault.emit("vault-auto-locked", ());
                    }
                }
                Err(e) => {
                    error!("Auto-lock check failed: {}", e);
                }
            }
        }
    });
}

/// Disconnects interactive SSH sessions idle for 30 minutes (checked every minute) and
/// emits `ssh_timeout_<id>` for each.
pub(crate) fn spawn_ssh_idle_reaper(app: &AppHandle) {
    let ssh_state_ref = app.state::<ssh::SshState>();
    let ssh_sessions = ssh_state_ref.sessions.clone();
    let app_handle_ssh = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        let timeout_duration = std::time::Duration::from_secs(30 * 60); // 30 minutes

        loop {
            interval.tick().await;

            let sessions_to_disconnect: Vec<(
                String,
                tokio::sync::mpsc::Sender<()>,
                tokio::sync::mpsc::Sender<()>,
            )> = {
                let mut sessions = match ssh_sessions.lock() {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                let timed_out_ids: Vec<String> = sessions
                    .iter()
                    .filter_map(|(id, conn)| {
                        if let Ok(last_activity) = conn.last_activity.lock() {
                            if last_activity.elapsed() > timeout_duration {
                                return Some(id.clone());
                            }
                        }
                        None
                    })
                    .collect();

                let mut timed_out_sessions = Vec::with_capacity(timed_out_ids.len());
                for session_id in timed_out_ids {
                    if let Some(conn) = sessions.remove(&session_id) {
                        timed_out_sessions.push((
                            session_id,
                            conn.disconnect_tx,
                            conn.stats_cancel_tx,
                        ));
                    }
                }

                timed_out_sessions
            };

            for (session_id, disconnect_tx, stats_cancel_tx) in sessions_to_disconnect {
                let _ = stats_cancel_tx.send(()).await;
                let _ = disconnect_tx.send(()).await;
                let _ = app_handle_ssh.emit(&format!("ssh_timeout_{}", session_id), ());
            }
        }
    });
}

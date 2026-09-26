//! Running one task: credential resolution, SSH exec or SFTP transfer, result recording, and
//! the in-flight guard that keeps runs of one task from overlapping (invariant 16).

use chrono::{DateTime, Utc};
use log::{error, info};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};

use crate::db;
use crate::ssh_exec;
use super::*;

/// Ids of tasks with a run in progress, from the scheduler or "Run now". A task never runs
/// twice at once: a double click or a cron tick during a manual run used to start a
/// second concurrent run (audit RUST-005).
pub(super) fn in_flight() -> &'static Mutex<HashSet<String>> {
    static IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

/// "Run now" error for a task with a run in progress. Shown to the user verbatim.
pub const ALREADY_RUNNING: &str = "This task is already running or waiting to run";

/// Marks a task as running until dropped.
pub(super) struct InFlightGuard(pub(super) String);

impl InFlightGuard {
    pub(super) fn claim(task_id: &str) -> Option<Self> {
        let mut running = in_flight().lock().unwrap_or_else(|e| e.into_inner());
        running.insert(task_id.to_string()).then(|| Self(task_id.to_string()))
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        in_flight()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.0);
    }
}

/// Result of a single task run (scheduled or manual).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskRunResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}

/// Resolve credential for a task (async, does not hold DB connection).
pub(super) async fn resolve_cred_for_task(
    app: &AppHandle,
    credential_id: Option<&str>,
    target_host: &str,
    task_type: &str,
    user_initiated: bool,
) -> Result<(String, Option<String>, Option<String>, Option<String>), String> {
    let (password, key_path, private_key, key_passphrase) = if let Some(cid) = credential_id {
        let vault_state = app
            .try_state::<crate::vault::VaultState>()
            .ok_or_else(|| "Vault not available".to_string())?;
        let credential_manager = app
            .try_state::<crate::vault::CredentialManager>()
            .ok_or_else(|| "Credential manager not available".to_string())?;
        // Scheduled runs are background work: they must not reset the auto-lock timer
        // (RSEC-002). "Run now" is the user acting, so it counts as activity.
        let access = if user_initiated {
            vault_state.credential_access().await
        } else {
            vault_state.credential_access_background().await
        }
        .map_err(|_| "Vault is locked — unlock the vault for scheduled tasks to run".to_string())?;
        let cred = credential_manager
            .get_credential(access.key(), cid)
            .map_err(|e| format!("Credential error: {}", e))?;
        if !crate::vault::credentials::host_allowed(cred.host.as_deref(), target_host) {
            return Err(crate::vault::credentials::host_mismatch_error(&cred.name, cred.host.as_deref()));
        }
        // Checked again at run time: the credential's type can change after the task was saved.
        crate::vault::credentials::check_type(
            &cred.name,
            &cred.credential_type,
            crate::vault::credentials::CredentialUse::for_task_type(task_type),
        )?;
        (
            cred.password,
            cred.key_path.clone(),
            cred.private_key.clone(),
            cred.key_passphrase.clone(),
        )
    } else {
        (String::new(), None, None, None)
    };
    Ok((password, key_path, private_key, key_passphrase))
}

pub(super) async fn bounded_sftp<F>(transfer: F) -> Result<(), String>
where
    F: std::future::Future<Output = Result<(), String>>,
{
    tokio::time::timeout(SFTP_TASK_TIMEOUT, transfer)
        .await
        .unwrap_or_else(|_| Err("SFTP transfer timed out".to_string()))
}

/// Execute SSH command with pre-resolved host and credential. No DB reference (Send-safe).
pub(super) async fn run_one_task(
    app: &AppHandle,
    address: &str,
    port_u16: u16,
    username: &str,
    password: &str,
    key_path: Option<&str>,
    private_key: Option<&str>,
    key_passphrase: Option<&str>,
    task_type: &str,
    command: &str,
    local_path: Option<&str>,
    remote_path: Option<&str>,
) -> Result<TaskRunResult, String> {
    match task_type {
        "sftp_upload" => {
            let (local, remote) = match (local_path, remote_path) {
                (Some(l), Some(r)) => (l, r),
                _ => return Err("SFTP upload requires local_path and remote_path".to_string()),
            };
            match bounded_sftp(crate::sftp::upload_file(
                app.clone(),
                address,
                port_u16,
                username,
                password,
                local,
                remote,
                None,
            ))
            .await
            {
                Ok(()) => Ok(TaskRunResult {
                    success: true,
                    output: Some("Upload completed.".to_string()),
                    error: None,
                }),
                Err(e) => Ok(TaskRunResult {
                    success: false,
                    output: None,
                    error: Some(e),
                }),
            }
        }
        "sftp_download" => {
            let (remote, local) = match (remote_path, local_path) {
                (Some(r), Some(l)) => (r, l),
                _ => return Err("SFTP download requires remote_path and local_path".to_string()),
            };
            match bounded_sftp(crate::sftp::download_file(
                app.clone(),
                address,
                port_u16,
                username,
                password,
                remote,
                local,
                None,
            ))
            .await
            {
                Ok(()) => Ok(TaskRunResult {
                    success: true,
                    output: Some("Download completed.".to_string()),
                    error: None,
                }),
                Err(e) => Ok(TaskRunResult {
                    success: false,
                    output: None,
                    error: Some(e),
                }),
            }
        }
        "ssh" => {
            match ssh_exec::execute_ssh_command(
                app.clone(),
                address,
                port_u16,
                username,
                password,
                key_path,
                private_key,
                key_passphrase,
                command,
                SSH_TIMEOUT_SECS,
            )
            .await
            {
                Ok(output) => Ok(TaskRunResult {
                    success: true,
                    output: Some(output),
                    error: None,
                }),
                Err(e) => Ok(TaskRunResult {
                    success: false,
                    output: None,
                    error: Some(e),
                }),
            }
        }
        // Only the three types the commands accept. Unknown values used to fall through
        // to SSH exec (running `command`, possibly empty) instead of failing (IPC-011).
        other => Err(format!("Unknown task type: {}", other)),
    }
}

/// Run a scheduled task once by id (manual run). Updates last_run_* and returns the result.
pub async fn run_scheduled_task_now(
    app: &AppHandle,
    task_id: &str,
) -> Result<TaskRunResult, String> {
    let db_path = get_db_path(app)?;
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| "Invalid database path".to_string())?
        .to_string();
    let conn = db::open_connection(&db_path_str)?;

    let task = load_task_by_id(&conn, task_id)?.ok_or_else(|| "Task not found".to_string())?;
    let _running = InFlightGuard::claim(task_id).ok_or_else(|| ALREADY_RUNNING.to_string())?;
    let host_info = get_host_credentials(&conn, &task.host_id)?
        .ok_or_else(|| format!("Host not found: {}", task.host_id))?;

    let (address, port, username) = (host_info.0.clone(), host_info.1, host_info.2.clone());
    let port_u16 = port.clamp(1, 65535) as u16;
    let task_command = task.command.clone();
    let task_type = task.task_type.clone();
    let local_path = task.local_path.clone();
    let remote_path = task.remote_path.clone();
    let cred_id = task.credential_id.clone();
    drop(conn);

    let cred = resolve_cred_for_task(app, cred_id.as_deref(), &address, &task_type, true).await?;
    let result = run_one_task(
        app,
        &address,
        port_u16,
        &username,
        &cred.0,
        cred.1.as_deref(),
        cred.2.as_deref(),
        cred.3.as_deref(),
        &task_type,
        &task_command,
        local_path.as_deref(),
        remote_path.as_deref(),
    )
    .await?;

    let conn2 = db::open_connection(&db_path_str)?;
    let status = if result.success { "success" } else { "failure" };
    set_run_result(
        &conn2,
        task_id,
        Utc::now().timestamp(),
        status,
        result.error.as_deref(),
        result.output.as_deref(),
    )?;

    Ok(result)
}

/// Runs a single due task (credential resolution, execution, result persistence)
/// and reports the (task_id, ran_at) pair to record in `last_executed`, or `None`
/// if the task never got far enough to run (matching the pre-refactor behavior of
/// `continue`-ing before that point without recording an execution).
pub(super) async fn run_and_record_task(
    app: &AppHandle,
    db_path_str: &str,
    task: TaskRow,
    now: DateTime<Utc>,
    now_ts: i64,
) -> Option<(String, DateTime<Utc>)> {
    info!("scheduler: running task '{}'", task.name);

    let conn = match db::open_connection(db_path_str) {
        Ok(c) => c,
        Err(e) => {
            error!("scheduler: failed to open db: {}", e);
            return None;
        }
    };
    let host_info = match get_host_credentials(&conn, &task.host_id) {
        Ok(Some(h)) => h,
        Ok(None) => {
            error!(
                "scheduler: host_id {} not found for task {}",
                task.host_id, task.name
            );
            return None;
        }
        Err(e) => {
            error!("scheduler: get host failed: {}", e);
            return None;
        }
    };
    let (address, port, username) = (host_info.0.clone(), host_info.1, host_info.2.clone());
    let port_u16 = port.clamp(1, 65535) as u16;
    let task_id = task.id.clone();
    let task_name = task.name.clone();
    drop(conn);

    let result = match resolve_cred_for_task(app, task.credential_id.as_deref(), &address, &task.task_type, false).await {
        Ok(cred) => {
            run_one_task(
                app,
                &address,
                port_u16,
                &username,
                &cred.0,
                cred.1.as_deref(),
                cred.2.as_deref(),
                cred.3.as_deref(),
                &task.task_type,
                &task.command,
                task.local_path.as_deref(),
                task.remote_path.as_deref(),
            )
            .await
        }
        Err(e) => {
            error!("scheduler: task '{}' cred error: {}", task_name, e);
            Ok(TaskRunResult {
                success: false,
                output: None,
                error: Some(e),
            })
        }
    };

    let conn2 = match db::open_connection(db_path_str) {
        Ok(c) => c,
        Err(e) => {
            error!("scheduler: failed to open db for result: {}", e);
            // Record in-memory regardless of DB persistence outcome.
            return Some((task_id, now));
        }
    };
    match result {
        Ok(r) => {
            let status = if r.success { "success" } else { "failure" };
            if r.success {
                info!("scheduler: task '{}' completed", task_name);
            } else {
                error!("scheduler: task '{}' failed: {:?}", task_name, r.error);
            }
            if let Err(e) = set_run_result(
                &conn2,
                &task_id,
                now_ts,
                status,
                r.error.as_deref(),
                r.output.as_deref(),
            ) {
                error!(
                    "scheduler: failed to persist run result for task '{}': {}",
                    task_name, e
                );
            }
        }
        Err(e) => {
            error!("scheduler: task '{}' run error: {}", task_name, e);
            if let Err(pe) =
                set_run_result(&conn2, &task_id, now_ts, "failure", Some(e.as_str()), None)
            {
                error!(
                    "scheduler: failed to persist run result for task '{}': {}",
                    task_name, pe
                );
            }
        }
    }

    Some((task_id, now))
}

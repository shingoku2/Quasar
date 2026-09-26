//! Scheduled tasks: validation (types, hosts, credentials, local-path grants) and the task commands.

use crate::*;

pub(crate) fn scheduled_tasks_conn(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    app_db_connection(app)
}

#[tauri::command]
pub(crate) async fn list_scheduled_tasks(app: AppHandle) -> Result<Vec<scheduler::ScheduledTask>, String> {
    let conn = scheduled_tasks_conn(&app)?;
    scheduler::list_scheduled_tasks(&conn).map_err(|e| sanitize_error(e, "scheduled tasks"))
}

pub(crate) const MAX_TASK_NAME_LEN: usize = 200;
pub(crate) const MAX_TASK_COMMAND_LEN: usize = 16 * 1024;

/// Validates a scheduled task before it's stored (IPC-011): known task type, an existing
/// host, bounded name/command, and the paths an SFTP task needs. Returns the task type.
pub(crate) fn validate_scheduled_task<'a>(
    conn: &rusqlite::Connection,
    name: &str,
    host_id: &str,
    command: &str,
    task_type: Option<&'a str>,
    local_path: Option<&str>,
    remote_path: Option<&str>,
    credential_id: Option<&str>,
) -> Result<&'a str, String> {
    let task_type = task_type.unwrap_or("ssh");
    if !scheduler::TASK_TYPES.contains(&task_type) {
        return Err(format!("Unknown task type: {}", task_type));
    }
    if name.trim().is_empty() || name.len() > MAX_TASK_NAME_LEN {
        return Err(format!("Task name must be 1-{} characters", MAX_TASK_NAME_LEN));
    }
    if command.len() > MAX_TASK_COMMAND_LEN {
        return Err("Task command is too long".to_string());
    }
    if task_type == "ssh" && command.trim().is_empty() {
        return Err("An SSH task needs a command".to_string());
    }
    if task_type != "ssh" {
        match (local_path, remote_path) {
            (Some(lp), Some(rp)) => {
                validate_path(lp)?;
                validate_path(rp)?;
            }
            _ => return Err("SFTP tasks need both a local and a remote path".to_string()),
        }
    }
    use rusqlite::OptionalExtension;
    let protocol: Option<String> = conn
        .query_row("SELECT protocol FROM hosts WHERE id = ?1", [host_id], |r| r.get(0))
        .optional()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let Some(protocol) = protocol else {
        return Err("Host not found".to_string());
    };
    // Database, API and other hosts are inventory-only: tasks run over SSH (PR #68 review).
    if !scheduler::host_protocol_runs_tasks(&protocol) {
        return Err("Scheduled tasks run over SSH: choose an SSH host".to_string());
    }
    // Reject a host-bound credential for another host when saving, not at the first run.
    if let Some(cid) = credential_id {
        check_credential_binding(conn, host_id, cid, vault::credentials::CredentialUse::for_task_type(task_type))?;
    }
    Ok(task_type)
}

/// The local-file access a scheduled task type needs: uploads read the file, downloads write it.
pub(crate) fn task_local_intent(task_type: &str) -> Option<local_paths::Intent> {
    match task_type {
        "sftp_upload" => Some(local_paths::Intent::Read),
        "sftp_download" => Some(local_paths::Intent::Write),
        _ => None,
    }
}

/// A saved SFTP task's transfer: local path, task type, host id, remote path.
pub(crate) type StoredTransfer = (Option<String>, Option<String>, String, Option<String>);

/// Whether an edited SFTP task keeps the whole transfer its grant was given for. The local
/// file was picked for one transfer: the same file, direction, host and remote path. Changing
/// any of them needs a new pick, or a saved upload could be pointed at another destination
/// without the dialog (PR #68 review), and switching upload/download turns read access into
/// write access.
pub(crate) fn task_transfer_unchanged(
    stored: Option<StoredTransfer>,
    local_path: &str,
    task_type: &str,
    host_id: &str,
    remote_path: Option<&str>,
) -> bool {
    matches!(stored, Some((Some(p), Some(t), h, r))
        if p == local_path && t == task_type && h == host_id && r.as_deref() == remote_path)
}

#[tauri::command]
pub(crate) async fn add_scheduled_task(
    app: AppHandle,
    audit: State<'_, vault::AuditLogManager>,
    grants: State<'_, local_paths::LocalPathGrants>,
    name: String,
    cron_expression: String,
    host_id: String,
    command: String,
    credential_id: Option<String>,
    enabled: bool,
    task_type: Option<String>,
    local_path: Option<String>,
    remote_path: Option<String>,
) -> Result<String, String> {
    cron::Schedule::from_str(&cron_expression)
        .map_err(|e| format!("Invalid cron expression: {}", e))?;
    let conn = scheduled_tasks_conn(&app)?;
    let task_type = validate_scheduled_task(
        &conn,
        &name,
        &host_id,
        &command,
        task_type.as_deref(),
        local_path.as_deref(),
        remote_path.as_deref(),
        credential_id.as_deref(),
    )?;
    if let (Some(intent), Some(lp)) = (task_local_intent(task_type), local_path.as_deref()) {
        grants.take(lp, intent)?;
    }
    let id = scheduler::add_scheduled_task(
        &conn,
        &name,
        &cron_expression,
        &host_id,
        &command,
        credential_id.as_deref(),
        enabled,
        task_type,
        local_path.as_deref(),
        remote_path.as_deref(),
    )
    .map_err(|e| sanitize_error(e, "scheduled task"))?;
    audit.record("scheduled_task_create", Some(&id), "scheduled_task", "create", "success", Some(&name));
    Ok(id)
}

#[tauri::command]
pub(crate) async fn update_scheduled_task(
    app: AppHandle,
    audit: State<'_, vault::AuditLogManager>,
    grants: State<'_, local_paths::LocalPathGrants>,
    id: String,
    name: String,
    cron_expression: String,
    host_id: String,
    command: String,
    credential_id: Option<String>,
    enabled: bool,
    task_type: Option<String>,
    local_path: Option<String>,
    remote_path: Option<String>,
) -> Result<(), String> {
    cron::Schedule::from_str(&cron_expression)
        .map_err(|e| format!("Invalid cron expression: {}", e))?;
    let conn = scheduled_tasks_conn(&app)?;
    let task_type = validate_scheduled_task(
        &conn,
        &name,
        &host_id,
        &command,
        task_type.as_deref(),
        local_path.as_deref(),
        remote_path.as_deref(),
        credential_id.as_deref(),
    )?;
    if let (Some(intent), Some(lp)) = (task_local_intent(task_type), local_path.as_deref()) {
        // An unchanged transfer was granted when the task was saved; any change needs a new pick.
        use rusqlite::OptionalExtension;
        let stored: Option<StoredTransfer> = conn
            .query_row(
                "SELECT local_path, task_type, host_id, remote_path FROM scheduled_tasks WHERE id = ?1",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(|e| sanitize_error(e.to_string(), "database"))?;
        if !task_transfer_unchanged(stored, lp, task_type, &host_id, remote_path.as_deref()) {
            grants.take(lp, intent)?;
        }
    }
    scheduler::update_scheduled_task(
        &conn,
        &id,
        &name,
        &cron_expression,
        &host_id,
        &command,
        credential_id.as_deref(),
        enabled,
        task_type,
        local_path.as_deref(),
        remote_path.as_deref(),
    )
    .map_err(|e| sanitize_error(e, "scheduled task"))?;
    audit.record("scheduled_task_update", Some(&id), "scheduled_task", "update", "success", Some(&name));
    Ok(())
}

#[tauri::command]
pub(crate) async fn remove_scheduled_task(
    app: AppHandle,
    audit: State<'_, vault::AuditLogManager>,
    id: String,
) -> Result<(), String> {
    let conn = scheduled_tasks_conn(&app)?;
    scheduler::remove_scheduled_task(&conn, &id).map_err(|e| sanitize_error(e, "scheduled task"))?;
    audit.record("scheduled_task_delete", Some(&id), "scheduled_task", "delete", "success", None);
    Ok(())
}

#[tauri::command]
pub(crate) async fn run_scheduled_task_now(
    app: AppHandle,
    audit: State<'_, vault::AuditLogManager>,
    id: String,
) -> Result<scheduler::TaskRunResult, String> {
    let result = scheduler::run_scheduled_task_now(&app, &id).await;
    audit.record(
        "scheduled_task_run",
        Some(&id),
        "scheduled_task",
        "run",
        if matches!(&result, Ok(r) if r.success) { "success" } else { "failure" },
        None,
    );
    result.map_err(|e| {
        if e == scheduler::ALREADY_RUNNING {
            e
        } else {
            sanitize_error(e, "run task")
        }
    })
}

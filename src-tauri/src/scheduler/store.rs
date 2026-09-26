//! Scheduled-task rows: the columns, mappers and CRUD, plus run results and the task host.

use chrono::Utc;
use tauri::AppHandle;

use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    pub cron_expression: String,
    pub host_id: String,
    pub command: String,
    pub credential_id: Option<String>,
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    pub last_run_status: Option<String>,
    pub last_run_error: Option<String>,
    pub last_run_output: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub task_type: String,
    pub local_path: Option<String>,
    pub remote_path: Option<String>,
}

#[allow(dead_code)]
pub(super) struct TaskRow {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) cron_expression: String,
    pub(super) host_id: String,
    pub(super) command: String,
    pub(super) credential_id: Option<String>,
    pub(super) enabled: i64,
    pub(super) last_run_at: Option<i64>,
    pub(super) created_at: i64,
    pub(super) updated_at: i64,
    pub(super) task_type: String,
    pub(super) local_path: Option<String>,
    pub(super) remote_path: Option<String>,
}

pub(super) fn get_db_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    crate::db::app_db_path(app).map(std::path::PathBuf::from)
}

/// Columns `task_row_from` reads, in order.
pub(super) const TASK_ROW_COLUMNS: &str = "id, name, cron_expression, host_id, command, credential_id, enabled, last_run_at, created_at, updated_at, task_type, local_path, remote_path";

/// A row selected with `TASK_ROW_COLUMNS`. A NULL `task_type` (rows from before 012) is SSH.
pub(super) fn task_row_from(row: &rusqlite::Row) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: row.get(0)?,
        name: row.get(1)?,
        cron_expression: row.get(2)?,
        host_id: row.get(3)?,
        command: row.get(4)?,
        credential_id: row.get(5)?,
        enabled: row.get::<_, i64>(6)?,
        last_run_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        task_type: row.get::<_, Option<String>>(10)?.unwrap_or_else(|| "ssh".to_string()),
        local_path: row.get(11)?,
        remote_path: row.get(12)?,
    })
}

/// Columns `scheduled_task_from` reads, in order.
pub(super) const SCHEDULED_TASK_COLUMNS: &str = "id, name, cron_expression, host_id, command, credential_id, enabled, last_run_at, last_run_status, last_run_error, last_run_output, created_at, updated_at, task_type, local_path, remote_path";

/// A row selected with `SCHEDULED_TASK_COLUMNS`, as the UI sees it.
pub(super) fn scheduled_task_from(row: &rusqlite::Row) -> rusqlite::Result<ScheduledTask> {
    Ok(ScheduledTask {
        id: row.get(0)?,
        name: row.get(1)?,
        cron_expression: row.get(2)?,
        host_id: row.get(3)?,
        command: row.get(4)?,
        credential_id: row.get(5)?,
        enabled: row.get::<_, i64>(6).map(|n| n != 0)?,
        last_run_at: row.get(7)?,
        last_run_status: row.get(8)?,
        last_run_error: row.get(9)?,
        last_run_output: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        task_type: row.get::<_, Option<String>>(13)?.unwrap_or_else(|| "ssh".to_string()),
        local_path: row.get(14)?,
        remote_path: row.get(15)?,
    })
}

pub(super) fn load_enabled_tasks(conn: &rusqlite::Connection) -> Result<Vec<TaskRow>, String> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM scheduled_tasks WHERE enabled = 1", TASK_ROW_COLUMNS)
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], task_row_from)
        .map_err(|e| e.to_string())?;
    rows.map(|r| r.map_err(|e| e.to_string())).collect()
}

/// Whether a saved host's protocol lets scheduled tasks (SSH exec and SFTP) run against it.
/// Other protocols are inventory-only (PR #68 review).
pub fn host_protocol_runs_tasks(protocol: &str) -> bool {
    protocol.trim().eq_ignore_ascii_case("ssh")
}

/// Address, port and username of the task's host. An error if the host has since been
/// changed to a protocol tasks don't run against.
pub(super) fn get_host_credentials(
    conn: &rusqlite::Connection,
    host_id: &str,
) -> Result<Option<(String, i64, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT address, port, username, protocol FROM hosts WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query_map(rusqlite::params![host_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?
                    .unwrap_or_else(|| "root".to_string()),
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    match rows.next().transpose().map_err(|e| e.to_string())? {
        Some((_, _, _, protocol)) if !host_protocol_runs_tasks(&protocol) => {
            Err(format!("Host {} is not an SSH host; scheduled tasks only run over SSH", host_id))
        }
        Some((address, port, username, _)) => Ok(Some((address, port, username))),
        None => Ok(None),
    }
}

/// Task types a scheduled task may have.
pub const TASK_TYPES: [&str; 3] = ["ssh", "sftp_upload", "sftp_download"];

pub(super) const MAX_OUTPUT_LEN: usize = 4096;

pub(super) fn set_run_result(
    conn: &rusqlite::Connection,
    task_id: &str,
    at: i64,
    status: &str,
    error_opt: Option<&str>,
    output_opt: Option<&str>,
) -> Result<(), String> {
    let output_trunc = output_opt.map(|s| {
        if s.len() > MAX_OUTPUT_LEN {
            // Truncate on a UTF-8 char boundary: remote command output is arbitrary
            // text, and slicing mid-codepoint would panic.
            let mut end = MAX_OUTPUT_LEN;
            while end > 0 && !s.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &s[..end])
        } else {
            s.to_string()
        }
    });
    conn.execute(
        "UPDATE scheduled_tasks SET last_run_at = ?1, last_run_status = ?2, last_run_error = ?3, last_run_output = ?4, updated_at = ?1 WHERE id = ?5",
        rusqlite::params![at, status, error_opt, output_trunc.as_deref(), task_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// List all scheduled tasks (for UI).
pub fn list_scheduled_tasks(conn: &rusqlite::Connection) -> Result<Vec<ScheduledTask>, String> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM scheduled_tasks ORDER BY name ASC", SCHEDULED_TASK_COLUMNS)
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], scheduled_task_from)
        .map_err(|e| e.to_string())?;
    rows.map(|r| r.map_err(|e| e.to_string())).collect()
}

/// Fetch a single task by id.
#[cfg(test)]
pub fn get_scheduled_task(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<Option<ScheduledTask>, String> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM scheduled_tasks WHERE id = ?1", SCHEDULED_TASK_COLUMNS)
    ).map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query_map(rusqlite::params![id], scheduled_task_from)
        .map_err(|e| e.to_string())?;
    rows.next().transpose().map_err(|e| e.to_string())
}

/// Load a single task by id for execution (enabled or not).
pub(super) fn load_task_by_id(conn: &rusqlite::Connection, task_id: &str) -> Result<Option<TaskRow>, String> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM scheduled_tasks WHERE id = ?1", TASK_ROW_COLUMNS)
    ).map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query_map(rusqlite::params![task_id], task_row_from)
        .map_err(|e| e.to_string())?;
    rows.next().transpose().map_err(|e| e.to_string())
}

/// Add a new scheduled task. Returns the new task id.
pub fn add_scheduled_task(
    conn: &rusqlite::Connection,
    name: &str,
    cron_expression: &str,
    host_id: &str,
    command: &str,
    credential_id: Option<&str>,
    enabled: bool,
    task_type: &str,
    local_path: Option<&str>,
    remote_path: Option<&str>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO scheduled_tasks (id, name, cron_expression, host_id, command, credential_id, enabled, created_at, updated_at, task_type, local_path, remote_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![
            id,
            name,
            cron_expression,
            host_id,
            command,
            credential_id,
            if enabled { 1i64 } else { 0 },
            now,
            now,
            task_type,
            local_path,
            remote_path,
        ],
    ).map_err(|e| e.to_string())?;
    Ok(id)
}

/// Update an existing scheduled task.
pub fn update_scheduled_task(
    conn: &rusqlite::Connection,
    id: &str,
    name: &str,
    cron_expression: &str,
    host_id: &str,
    command: &str,
    credential_id: Option<&str>,
    enabled: bool,
    task_type: &str,
    local_path: Option<&str>,
    remote_path: Option<&str>,
) -> Result<(), String> {
    let now = Utc::now().timestamp();
    let updated = conn.execute(
        "UPDATE scheduled_tasks SET name = ?1, cron_expression = ?2, host_id = ?3, command = ?4, credential_id = ?5, enabled = ?6, updated_at = ?7, task_type = ?8, local_path = ?9, remote_path = ?10 WHERE id = ?11",
        rusqlite::params![
            name,
            cron_expression,
            host_id,
            command,
            credential_id,
            if enabled { 1i64 } else { 0 },
            now,
            task_type,
            local_path,
            remote_path,
            id,
        ],
    ).map_err(|e| e.to_string())?;
    if updated == 0 {
        return Err("Task not found".to_string());
    }
    Ok(())
}

/// Remove a scheduled task.
pub fn remove_scheduled_task(conn: &rusqlite::Connection, id: &str) -> Result<(), String> {
    let updated = conn
        .execute(
            "DELETE FROM scheduled_tasks WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
    if updated == 0 {
        return Err("Task not found".to_string());
    }
    Ok(())
}

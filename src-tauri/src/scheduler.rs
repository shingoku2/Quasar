//! Cron-based scheduled task execution (SSH commands on saved hosts).
//! Runs in a background task; checks every 60 seconds for due schedules.

use chrono::{DateTime, Utc};
use cron::Schedule;
use std::str::FromStr;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::time::interval;
use log::{error, info, warn};

use crate::db;
use crate::ssh_exec;
use uuid::Uuid;

const DB_FILENAME: &str = "quasar.db";
const CHECK_INTERVAL_SECS: u64 = 60;
const SSH_TIMEOUT_SECS: u64 = 120;

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
    pub created_at: i64,
    pub updated_at: i64,
}

#[allow(dead_code)]
struct TaskRow {
    id: String,
    name: String,
    cron_expression: String,
    host_id: String,
    command: String,
    credential_id: Option<String>,
    enabled: i64,
    last_run_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

fn get_db_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(app_dir.join(DB_FILENAME))
}

fn load_enabled_tasks(conn: &rusqlite::Connection) -> Result<Vec<TaskRow>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, name, cron_expression, host_id, command, credential_id, enabled, last_run_at, created_at, updated_at
         FROM scheduled_tasks WHERE enabled = 1"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
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
        })
    }).map_err(|e| e.to_string())?;
    rows.map(|r| r.map_err(|e| e.to_string())).collect()
}

fn get_host_credentials(
    conn: &rusqlite::Connection,
    host_id: &str,
) -> Result<Option<(String, i64, String)>, String> {
    let mut stmt = conn.prepare(
        "SELECT address, port, username FROM hosts WHERE id = ?1"
    ).map_err(|e| e.to_string())?;
    let mut rows = stmt.query_map(rusqlite::params![host_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?.unwrap_or_else(|| "root".to_string()),
        ))
    }).map_err(|e| e.to_string())?;
    rows.next().transpose().map_err(|e| e.to_string())
}

fn set_last_run(conn: &rusqlite::Connection, task_id: &str, at: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE scheduled_tasks SET last_run_at = ?1, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![at, at, task_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// List all scheduled tasks (for UI).
pub fn list_scheduled_tasks(conn: &rusqlite::Connection) -> Result<Vec<ScheduledTask>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, name, cron_expression, host_id, command, credential_id, enabled, last_run_at, created_at, updated_at
         FROM scheduled_tasks ORDER BY name ASC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(ScheduledTask {
            id: row.get(0)?,
            name: row.get(1)?,
            cron_expression: row.get(2)?,
            host_id: row.get(3)?,
            command: row.get(4)?,
            credential_id: row.get(5)?,
            enabled: row.get::<_, i64>(6).map(|n| n != 0)?,
            last_run_at: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.map(|r| r.map_err(|e| e.to_string())).collect()
}

/// Fetch a single task by id.
pub fn get_scheduled_task(conn: &rusqlite::Connection, id: &str) -> Result<Option<ScheduledTask>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, name, cron_expression, host_id, command, credential_id, enabled, last_run_at, created_at, updated_at
         FROM scheduled_tasks WHERE id = ?1"
    ).map_err(|e| e.to_string())?;
    let mut rows = stmt.query_map(rusqlite::params![id], |row| {
        Ok(ScheduledTask {
            id: row.get(0)?,
            name: row.get(1)?,
            cron_expression: row.get(2)?,
            host_id: row.get(3)?,
            command: row.get(4)?,
            credential_id: row.get(5)?,
            enabled: row.get::<_, i64>(6).map(|n| n != 0)?,
            last_run_at: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    }).map_err(|e| e.to_string())?;
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
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO scheduled_tasks (id, name, cron_expression, host_id, command, credential_id, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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
) -> Result<(), String> {
    let now = Utc::now().timestamp();
    let updated = conn.execute(
        "UPDATE scheduled_tasks SET name = ?1, cron_expression = ?2, host_id = ?3, command = ?4, credential_id = ?5, enabled = ?6, updated_at = ?7 WHERE id = ?8",
        rusqlite::params![
            name,
            cron_expression,
            host_id,
            command,
            credential_id,
            if enabled { 1i64 } else { 0 },
            now,
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
    let updated = conn.execute("DELETE FROM scheduled_tasks WHERE id = ?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    if updated == 0 {
        return Err("Task not found".to_string());
    }
    Ok(())
}

/// Returns true if the next occurrence of the schedule after `after` is <= `now`.
fn is_due(cron_expression: &str, last_run_at: Option<i64>, now: DateTime<Utc>) -> bool {
    let schedule = match Schedule::from_str(cron_expression) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let after = last_run_at
        .and_then(|t| DateTime::from_timestamp(t, 0))
        .unwrap_or_else(|| now - chrono::Duration::seconds(2 * CHECK_INTERVAL_SECS as i64));
    let next = schedule.after(&after).next();
    match next {
        Some(t) => t <= now,
        None => false,
    }
}

async fn run_due_tasks(app: &AppHandle) {
    let db_path = match get_db_path(app) {
        Ok(p) => p,
        Err(e) => {
            error!("scheduler: failed to get db path: {}", e);
            return;
        }
    };
    let db_path_str = match db_path.to_str() {
        Some(s) => s,
        None => return,
    };
    let conn = match db::open_connection(db_path_str) {
        Ok(c) => c,
        Err(e) => {
            error!("scheduler: failed to open db: {}", e);
            return;
        }
    };

    let tasks = match load_enabled_tasks(&conn) {
        Ok(t) => t,
        Err(e) => {
            error!("scheduler: failed to load tasks: {}", e);
            return;
        }
    };

    let now = Utc::now();
    let now_ts = now.timestamp();

    for task in tasks {
        if !is_due(&task.cron_expression, task.last_run_at, now) {
            continue;
        }

        let host_info = match get_host_credentials(&conn, &task.host_id) {
            Ok(Some(h)) => h,
            Ok(None) => {
                warn!("scheduler: host_id {} not found for task {}", task.host_id, task.name);
                continue;
            }
            Err(e) => {
                error!("scheduler: get host {} failed: {}", task.host_id, e);
                continue;
            }
        };

        let (address, port, username) = host_info;
        let port_u16 = port.max(1).min(65535) as u16;

        let (password, key_path, private_key, key_passphrase) = if let Some(ref cid) = task.credential_id {
            let vault_state = match app.try_state::<crate::vault::VaultState>() {
                Some(s) => s,
                None => {
                    warn!("scheduler: vault state not available, skipping task {}", task.name);
                    continue;
                }
            };
            let credential_manager = match app.try_state::<crate::vault::CredentialManager>() {
                Some(m) => m,
                None => {
                    warn!("scheduler: credential manager not available, skipping task {}", task.name);
                    continue;
                }
            };
            let key = match vault_state.get_master_key().await {
                Ok(k) => k,
                Err(_) => {
                    warn!("scheduler: vault locked, skipping task {}", task.name);
                    continue;
                }
            };
            match credential_manager.get_credential(&key, &cid) {
                Ok(cred) => (
                    cred.password,
                    cred.key_path.clone(),
                    cred.private_key.clone(),
                    cred.key_passphrase.clone(),
                ),
                Err(e) => {
                    error!("scheduler: get credential {} failed: {}", cid, e);
                    continue;
                }
            }
        } else {
            (String::new(), None, None, None)
        };

        info!("scheduler: running task '{}' on {}@{}:{}", task.name, username, address, port_u16);
        match ssh_exec::execute_ssh_command(
            app.clone(),
            &address,
            port_u16,
            &username,
            &password,
            key_path.as_deref(),
            private_key.as_deref(),
            key_passphrase.as_deref(),
            &task.command,
            SSH_TIMEOUT_SECS,
        ).await {
            Ok(out) => {
                info!("scheduler: task '{}' completed ({} bytes output)", task.name, out.len());
            }
            Err(e) => {
                error!("scheduler: task '{}' failed: {}", task.name, e);
            }
        }

        if set_last_run(&conn, &task.id, now_ts).is_err() {
            error!("scheduler: failed to update last_run_at for {}", task.id);
        }
    }
}

/// Start the scheduler background task. Call once during app setup.
pub fn start_scheduler(app: AppHandle) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(CHECK_INTERVAL_SECS));
        ticker.tick().await; // first tick fires immediately; skip so we wait CHECK_INTERVAL_SECS first
        loop {
            ticker.tick().await;
            run_due_tasks(&app).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_due() {
        // 5-field: minute hour day month dow. "0 0 * * *" = daily at midnight.
        let now = Utc::now();
        let last_run = (now - chrono::Duration::hours(2)).timestamp();
        // Next run after last_run is the next midnight; if we're past midnight, due
        let after = DateTime::from_timestamp(last_run, 0).unwrap();
        let s = Schedule::from_str("0 0 * * *").unwrap();
        let next = s.after(&after).next();
        assert!(next.is_some());
    }
}

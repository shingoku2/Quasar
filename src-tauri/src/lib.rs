mod ai;
mod crypto;
mod db;
mod discovery;
mod errors;
mod health;
mod host_tracker;
mod launcher;
mod monitoring;
mod scanner;
mod scheduler;
mod sftp;
mod ssh;
mod ssh_auth;
mod ssh_connect;
mod ssh_exec;
mod ssh_pool;
mod ssh_tunnel;
mod validation;
mod vault;

use errors::sanitize_error;
use log::error;
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use rusqlite::{OptionalExtension, Transaction};
use rusqlite_migration::{HookError, Migrations, M};
use std::str::FromStr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use validation::{
    validate_cidr, validate_credential_name, validate_hostname, validate_ip,
    validate_master_password, validate_path, validate_port, validate_username,
};

/// Migration 011 hook: add last_run_status, last_run_error, last_run_output to scheduled_tasks
/// only if missing (idempotent for DBs where 010 already created the table with these columns).
fn add_scheduled_task_run_result_columns_if_missing(tx: &Transaction) -> Result<(), HookError> {
    let names: Vec<String> = tx
        .prepare("PRAGMA table_info(scheduled_tasks)")
        .map_err(|e| HookError::Hook(e.to_string()))?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| HookError::Hook(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| HookError::Hook(e.to_string()))?;
    for (col, sql) in [
        (
            "last_run_status",
            "ALTER TABLE scheduled_tasks ADD COLUMN last_run_status TEXT",
        ),
        (
            "last_run_error",
            "ALTER TABLE scheduled_tasks ADD COLUMN last_run_error TEXT",
        ),
        (
            "last_run_output",
            "ALTER TABLE scheduled_tasks ADD COLUMN last_run_output TEXT",
        ),
    ] {
        if !names.contains(&col.to_string()) {
            tx.execute(sql, [])
                .map_err(|e| HookError::Hook(e.to_string()))?;
        }
    }
    Ok(())
}

/// Migration 012 hook: add task_type, local_path, remote_path to scheduled_tasks
/// only if missing (idempotent for DBs where 010 already created the table with these columns).
fn add_scheduled_tasks_sftp_columns_if_missing(tx: &Transaction) -> Result<(), HookError> {
    let names: Vec<String> = tx
        .prepare("PRAGMA table_info(scheduled_tasks)")
        .map_err(|e| HookError::Hook(e.to_string()))?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| HookError::Hook(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| HookError::Hook(e.to_string()))?;
    for (col, sql) in [
        (
            "task_type",
            "ALTER TABLE scheduled_tasks ADD COLUMN task_type TEXT NOT NULL DEFAULT 'ssh'",
        ),
        (
            "local_path",
            "ALTER TABLE scheduled_tasks ADD COLUMN local_path TEXT",
        ),
        (
            "remote_path",
            "ALTER TABLE scheduled_tasks ADD COLUMN remote_path TEXT",
        ),
    ] {
        if !names.contains(&col.to_string()) {
            tx.execute(sql, [])
                .map_err(|e| HookError::Hook(e.to_string()))?;
        }
    }
    Ok(())
}

// Define migrations (001 → 003 → 004 → 005 → 006 → 007 → 008 → 009 → 010 → 011 → 012 → 013 → 014)
// The rusqlite_migration crate tracks applied migrations in user_version.
static MIGRATIONS: Lazy<Migrations> = Lazy::new(|| {
    Migrations::new(vec![
        M::up(include_str!("../migrations/001_initial_schema.sql")),
        M::up(include_str!("../migrations/003_security_vault.sql")),
        M::up(include_str!("../migrations/004_monitoring.sql")),
        M::up(include_str!(
            "../migrations/005_consolidate_credentials.sql"
        )),
        M::up(include_str!("../migrations/006_discovered_hosts.sql")),
        M::up(include_str!(
            "../migrations/007_monitoring_host_credential.sql"
        )),
        M::up(include_str!("../migrations/008_ssh_key_credentials.sql")),
        M::up(include_str!("../migrations/009_nullable_password.sql")),
        M::up(include_str!("../migrations/010_scheduled_tasks.sql")),
        M::up_with_hook(
            "SELECT 1;",
            add_scheduled_task_run_result_columns_if_missing,
        ),
        M::up_with_hook("SELECT 1;", add_scheduled_tasks_sftp_columns_if_missing),
        M::up(include_str!("../migrations/013_indexes.sql")),
        M::up(include_str!(
            "../migrations/014_scheduled_tasks_cascade.sql"
        )),
    ])
});

use once_cell::sync::Lazy;

/// Database filename (renamed from titan.db for Quasar).
const DB_FILENAME: &str = "quasar.db";

/// One-time migration: rename titan.db to quasar.db for existing installs.
fn migrate_titan_db_to_quasar(app_dir: &std::path::Path) -> std::io::Result<()> {
    let old_path = app_dir.join("titan.db");
    let new_path = app_dir.join(DB_FILENAME);
    if old_path.exists() && !new_path.exists() {
        std::fs::rename(&old_path, &new_path)?;
    }
    Ok(())
}

#[tauri::command]
async fn connect_ssh(
    ssh_state: State<'_, ssh::SshState>,
    app: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    id: String,
    host: String,
    user: String,
    port: u16,
    password: Option<String>,
    credential_id: Option<String>,
) -> Result<(), String> {
    let (username, password, key_path, private_key, key_passphrase) =
        if let Some(cid) = credential_id {
            let key = vault_state
                .get_master_key()
                .await
                .map_err(|e| sanitize_error(e, "vault"))?;
            let cred = credential_manager
                .get_credential(&key, &cid)
                .map_err(|e| sanitize_error(e, "credential"))?;
            (
                cred.username,
                if cred.password.is_empty() {
                    None
                } else {
                    Some(cred.password)
                },
                cred.key_path,
                cred.private_key,
                cred.key_passphrase,
            )
        } else {
            (user, password, None, None, None)
        };
    ssh::connect_ssh(
        ssh_state,
        app,
        id,
        host,
        username,
        port,
        password,
        key_path,
        private_key,
        key_passphrase,
        false, // enable_agent_forwarding: can be added to frontend later
    )
    .await
}

#[tauri::command]
async fn start_ssh_tunnel(
    app: AppHandle,
    tunnel_state: State<'_, ssh_tunnel::TunnelState>,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    tunnel_id: String,
    ssh_host: String,
    ssh_port: u16,
    ssh_user: String,
    password: Option<String>,
    credential_id: Option<String>,
    local_port: u16,
    remote_host: String,
    remote_port: u16,
) -> Result<ssh_tunnel::TunnelInfo, String> {
    if validate_ip(&ssh_host).is_err() && validate_hostname(&ssh_host).is_err() {
        return Err("Invalid SSH host format".to_string());
    }
    validate_port(ssh_port)?;
    validate_port(local_port)?;
    if validate_ip(&remote_host).is_err() && validate_hostname(&remote_host).is_err() {
        return Err("Invalid remote host format".to_string());
    }
    validate_port(remote_port)?;
    let (username, password, key_path, private_key, key_passphrase) =
        if let Some(cid) = credential_id {
            let key = vault_state
                .get_master_key()
                .await
                .map_err(|e| sanitize_error(e, "vault"))?;
            let cred = credential_manager
                .get_credential(&key, &cid)
                .map_err(|e| sanitize_error(e, "credential"))?;
            (
                cred.username,
                if cred.password.is_empty() {
                    None
                } else {
                    Some(cred.password)
                },
                cred.key_path,
                cred.private_key,
                cred.key_passphrase,
            )
        } else {
            (ssh_user, password, None, None, None)
        };
    ssh_tunnel::start_tunnel(
        app,
        tunnel_state.inner(),
        tunnel_id,
        ssh_host,
        ssh_port,
        username,
        password,
        key_path,
        private_key,
        key_passphrase,
        local_port,
        remote_host,
        remote_port,
    )
    .await
    .map_err(|e| sanitize_error(e, "tunnel"))
}

#[tauri::command]
fn list_ssh_tunnels(
    tunnel_state: State<'_, ssh_tunnel::TunnelState>,
) -> Vec<ssh_tunnel::TunnelInfo> {
    tunnel_state.list()
}

#[tauri::command]
fn close_ssh_tunnel(
    tunnel_state: State<'_, ssh_tunnel::TunnelState>,
    tunnel_id: String,
) -> Result<(), String> {
    if tunnel_state.remove(&tunnel_id) {
        Ok(())
    } else {
        Err("Tunnel not found".to_string())
    }
}

#[tauri::command]
async fn launch_ssh_external(address: String, username: Option<String>) -> Result<(), String> {
    if validate_ip(&address).is_err() && validate_hostname(&address).is_err() {
        return Err("Invalid host address format".to_string());
    }
    if let Some(ref u) = username {
        validate_username(u)?;
    }
    launcher::launch_ssh(&address, username.as_deref()).map_err(|e| sanitize_error(e, "ssh"))
}

#[tauri::command]
async fn connect_rdp(address: String) -> Result<(), String> {
    if validate_ip(&address).is_err() && validate_hostname(&address).is_err() {
        return Err("Invalid host address format".to_string());
    }
    launcher::launch_rdp(&address).map_err(|e| sanitize_error(e, "network"))
}

#[tauri::command]
fn start_discovery(app: AppHandle, state: State<'_, discovery::DiscoveryState>) {
    discovery::start_mdns_discovery(app, state.running.clone(), state.stop_requested.clone());
}

#[tauri::command]
async fn check_ai_status() -> bool {
    ai::check_ollama_status().await
}

#[tauri::command]
async fn list_ai_models() -> Result<Vec<String>, String> {
    ai::list_models()
        .await
        .map_err(|e| sanitize_error(e, "network"))
}

// Simple struct to receive messages from frontend
#[derive(serde::Deserialize)]
struct FrontendMessage {
    role: String,
    content: String,
}

#[tauri::command]
async fn send_ai_chat(
    app: AppHandle,
    model: String,
    messages: Vec<FrontendMessage>,
) -> Result<(), String> {
    let chat_messages = messages
        .into_iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "user" => MessageRole::User,
                "assistant" => MessageRole::Assistant,
                "system" => MessageRole::System,
                _ => MessageRole::User,
            };
            ChatMessage::new(role, m.content)
        })
        .collect();

    ai::chat_request(app, model, chat_messages)
        .await
        .map_err(|e| sanitize_error(e, "network"))
}

#[tauri::command]
async fn scan_network(
    app: AppHandle,
    state: State<'_, Arc<scanner::ScannerState>>,
    tracker_state: State<'_, host_tracker::HostTracker>,
    cidr: String,
) -> Result<(), String> {
    validate_cidr(&cidr)?;
    let scanner_state = Arc::clone(state.inner());
    let app_for_progress = app.clone();
    let app_for_result = app.clone();
    let app_for_events = app.clone();

    // Clone the managed HostTracker to use in the spawned task
    let tracker = tracker_state.inner().clone();

    tokio::spawn(async move {
        let on_progress = move |progress: scanner::ScanProgress| {
            let _ = app_for_progress.emit("scan_progress", progress);
        };

        let on_result = move |result: scanner::ScanResult| {
            // Save host to database if alive
            if result.is_alive {
                if let Err(e) = tracker.save_host(&result) {
                    error!("Failed to save discovered host {}: {}", result.ip, e);
                    let _ = app_for_result
                        .emit("scan_error", sanitize_error(e.to_string(), "database"));
                }
            }
            let _ = app_for_result.emit("scan_result", result);
        };

        match scanner::scan_network(scanner_state, cidr, on_progress, on_result).await {
            Ok(()) => {
                let _ = app_for_events.emit("scan_complete", ());
            }
            Err(e) => {
                let _ = app_for_events.emit("scan_error", sanitize_error(e, "scanner"));
                let _ = app_for_events.emit("scan_complete", ());
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn stop_scan(state: State<'_, Arc<scanner::ScannerState>>) {
    scanner::stop_scan(&state);
}

#[tauri::command]
fn get_scan_progress(state: State<'_, Arc<scanner::ScannerState>>) -> scanner::ScanProgress {
    scanner::get_scan_progress(&state)
}

#[tauri::command]
fn is_scanning(state: State<'_, Arc<scanner::ScannerState>>) -> bool {
    scanner::is_scanning(&state)
}

#[tauri::command]
fn get_discovered_hosts(
    host_tracker: State<'_, host_tracker::HostTracker>,
    limit: Option<usize>,
) -> Result<Vec<host_tracker::DiscoveredHost>, String> {
    host_tracker
        .list_hosts(limit)
        .map_err(|e| sanitize_error(e, "database"))
}

#[tauri::command]
fn get_host_details(
    host_tracker: State<'_, host_tracker::HostTracker>,
    ip: String,
) -> Result<Option<host_tracker::DiscoveredHost>, String> {
    validate_ip(&ip)?;
    host_tracker
        .get_host(&ip)
        .map_err(|e| sanitize_error(e, "database"))
}

#[tauri::command]
fn search_discovered_hosts(
    host_tracker: State<'_, host_tracker::HostTracker>,
    query: String,
) -> Result<Vec<host_tracker::DiscoveredHost>, String> {
    host_tracker
        .search_hosts(&query)
        .map_err(|e| sanitize_error(e, "database"))
}

#[tauri::command]
fn delete_discovered_host(
    host_tracker: State<'_, host_tracker::HostTracker>,
    ip: String,
) -> Result<(), String> {
    validate_ip(&ip)?;
    host_tracker
        .delete_host(&ip)
        .map_err(|e| sanitize_error(e, "database"))
}

/// Saved remote host from the hosts table (used for remote monitoring).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedHost {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: i64,
    pub username: Option<String>,
    pub protocol: String,
    /// When set, monitoring will use this vault credential to fetch SSH metrics (CPU, memory, disk).
    pub credential_id: Option<String>,
}

/// Result of a reachability/latency check for a saved host; may include SSH metrics when a credential is linked.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteHostMetric {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: i64,
    pub reachable: bool,
    pub latency_ms: Option<u32>,
    pub error: Option<String>,
    /// Present when host has a monitoring credential and vault is unlocked; SSH metrics (CPU, memory, disk).
    pub metrics: Option<health::HealthMetrics>,
}

fn get_saved_hosts_from_db(app: &AppHandle) -> Result<Vec<SavedHost>, String> {
    let conn = app_db_connection(app)?;
    get_saved_hosts_from_conn(&conn)
}

fn get_saved_hosts_from_conn(conn: &rusqlite::Connection) -> Result<Vec<SavedHost>, String> {
    // Join with monitoring_host_credential so we know which credential to use for SSH metrics (if any)
    let sql = "SELECT h.id, h.name, h.address, h.port, h.username, h.protocol, m.credential_id
               FROM hosts h
               LEFT JOIN monitoring_host_credential m ON h.id = m.host_id
               ORDER BY h.name ASC";
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SavedHost {
                id: row.get(0)?,
                name: row.get(1)?,
                address: row.get(2)?,
                port: row.get::<_, i64>(3)?,
                username: row.get(4)?,
                protocol: row.get::<_, String>(5)?,
                credential_id: row.get::<_, Option<String>>(6).ok().flatten(),
            })
        })
        .map_err(|e| e.to_string())?;
    let hosts: Vec<SavedHost> = rows.filter_map(|r| r.ok()).collect();
    Ok(hosts)
}

fn get_saved_host_by_id(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<Option<SavedHost>, String> {
    conn.query_row(
        "SELECT h.id, h.name, h.address, h.port, h.username, h.protocol, m.credential_id
         FROM hosts h
         LEFT JOIN monitoring_host_credential m ON h.id = m.host_id
         WHERE h.id = ?1",
        [id],
        |row| {
            Ok(SavedHost {
                id: row.get(0)?,
                name: row.get(1)?,
                address: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                protocol: row.get(5)?,
                credential_id: row.get::<_, Option<String>>(6).ok().flatten(),
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn upsert_saved_host_in_conn(
    conn: &mut rusqlite::Connection,
    name: &str,
    address: &str,
    protocol: &str,
    port: Option<u16>,
    username: Option<&str>,
) -> Result<SavedHost, String> {
    let port = port.unwrap_or(if protocol.eq_ignore_ascii_case("rdp") {
        3389
    } else {
        22
    });
    validate_port(port)?;
    let now = chrono::Utc::now().timestamp();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let existing_id = tx
        .query_row(
            "SELECT id FROM hosts WHERE address = ?1 AND protocol = ?2 AND port = ?3 LIMIT 1",
            rusqlite::params![address, protocol, i64::from(port)],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let id = if let Some(id) = existing_id {
        tx.execute(
            "UPDATE hosts SET name = ?1, username = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![name, username.filter(|value| !value.is_empty()), now, id],
        )
        .map_err(|e| e.to_string())?;
        id
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO hosts (id, name, address, protocol, port, username, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            rusqlite::params![
                id,
                name,
                address,
                protocol,
                i64::from(port),
                username.filter(|value| !value.is_empty()),
                now,
            ],
        )
        .map_err(|e| e.to_string())?;
        id
    };
    tx.commit().map_err(|e| e.to_string())?;

    get_saved_host_by_id(conn, &id)?
        .ok_or_else(|| "Saved host was not found after upsert".to_string())
}

fn remove_saved_hosts_in_conn(
    conn: &mut rusqlite::Connection,
    ids: &[String],
) -> Result<usize, String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut removed = 0;
    for id in ids {
        removed += tx
            .execute("DELETE FROM hosts WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(removed)
}
/// Resolve hostname to an IP for ping. Returns None if resolution fails.
async fn resolve_to_ip(address: &str, port: i64) -> Option<String> {
    let port = port.clamp(1, 65535) as u16;
    tokio::net::lookup_host((address, port))
        .await
        .ok()
        .and_then(|mut addrs| addrs.next())
        .map(|sa| sa.ip().to_string())
}

#[tauri::command]
async fn get_saved_hosts(app: AppHandle) -> Result<Vec<SavedHost>, String> {
    get_saved_hosts_from_db(&app).map_err(|e| sanitize_error(e, "database"))
}

#[tauri::command]
async fn upsert_saved_host(
    app: AppHandle,
    name: String,
    address: String,
    protocol: String,
    port: Option<u16>,
    username: Option<String>,
) -> Result<SavedHost, String> {
    let mut conn = app_db_connection(&app).map_err(|e| sanitize_error(e, "database"))?;
    upsert_saved_host_in_conn(
        &mut conn,
        &name,
        &address,
        &protocol,
        port,
        username.as_deref(),
    )
    .map_err(|e| sanitize_error(e, "database"))
}

#[tauri::command]
async fn remove_saved_hosts(app: AppHandle, ids: Vec<String>) -> Result<usize, String> {
    let mut conn = app_db_connection(&app).map_err(|e| sanitize_error(e, "database"))?;
    remove_saved_hosts_in_conn(&mut conn, &ids).map_err(|e| sanitize_error(e, "database"))
}

fn app_db_connection(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let db_path = app_dir.join(DB_FILENAME);
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| "Invalid database path".to_string())?;
    db::open_connection(db_path_str).map_err(|e| sanitize_error(e, "database"))
}

fn scheduled_tasks_conn(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    app_db_connection(app)
}

#[tauri::command]
async fn list_scheduled_tasks(app: AppHandle) -> Result<Vec<scheduler::ScheduledTask>, String> {
    let conn = scheduled_tasks_conn(&app)?;
    scheduler::list_scheduled_tasks(&conn).map_err(|e| sanitize_error(e, "scheduled tasks"))
}

#[tauri::command]
async fn get_scheduled_task(
    app: AppHandle,
    id: String,
) -> Result<Option<scheduler::ScheduledTask>, String> {
    let conn = scheduled_tasks_conn(&app)?;
    scheduler::get_scheduled_task(&conn, &id).map_err(|e| sanitize_error(e, "scheduled task"))
}

#[tauri::command]
async fn add_scheduled_task(
    app: AppHandle,
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
    let task_type = task_type.as_deref().unwrap_or("ssh");
    if task_type == "sftp_upload" || task_type == "sftp_download" {
        if let Some(ref lp) = local_path {
            validate_path(lp)?;
        }
        if let Some(ref rp) = remote_path {
            validate_path(rp)?;
        }
    }
    let conn = scheduled_tasks_conn(&app)?;
    scheduler::add_scheduled_task(
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
    .map_err(|e| sanitize_error(e, "scheduled task"))
}

#[tauri::command]
async fn update_scheduled_task(
    app: AppHandle,
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
    let task_type = task_type.as_deref().unwrap_or("ssh");
    if task_type == "sftp_upload" || task_type == "sftp_download" {
        if let Some(ref lp) = local_path {
            validate_path(lp)?;
        }
        if let Some(ref rp) = remote_path {
            validate_path(rp)?;
        }
    }
    let conn = scheduled_tasks_conn(&app)?;
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
    .map_err(|e| sanitize_error(e, "scheduled task"))
}

#[tauri::command]
async fn remove_scheduled_task(app: AppHandle, id: String) -> Result<(), String> {
    let conn = scheduled_tasks_conn(&app)?;
    scheduler::remove_scheduled_task(&conn, &id).map_err(|e| sanitize_error(e, "scheduled task"))
}

#[tauri::command]
async fn run_scheduled_task_now(
    app: AppHandle,
    id: String,
) -> Result<scheduler::TaskRunResult, String> {
    scheduler::run_scheduled_task_now(&app, &id)
        .await
        .map_err(|e| sanitize_error(e, "run task"))
}

/// How many hosts are probed at once by `get_remote_hosts_health`.
///
/// Each probe is a DNS lookup, an ICMP ping and (when a credential is linked) a
/// full SSH connect, so probing serially made the command's runtime scale with
/// the size of the inventory — on a 30 second poll that overlapped badly.
const MAX_CONCURRENT_HOST_CHECKS: usize = 16;

/// Probe a single host: resolve, ping, and optionally collect SSH metrics.
async fn probe_host(
    app: AppHandle,
    host: SavedHost,
    credential: Option<vault::credentials::Credential>,
) -> RemoteHostMetric {
    let port_u16 = host.port.clamp(1, 65535) as u16;
    let ping_target = if host.address.parse::<std::net::IpAddr>().is_ok() {
        Some(host.address.clone())
    } else {
        resolve_to_ip(&host.address, host.port).await
    };

    let (reachable, latency_ms, error, metrics) = match ping_target {
        Some(ref ip) => match health::check_ping(ip).await {
            Ok(ms) => {
                let mut metrics = None;
                if let Some(cred) = credential {
                    let password = if cred.password.is_empty() {
                        None
                    } else {
                        Some(cred.password.as_str())
                    };
                    let ssh_result = health::check_ssh_health(
                        app,
                        ip,
                        port_u16,
                        &cred.username,
                        password,
                        true,
                        cred.key_path.as_deref(),
                        cred.private_key.as_deref(),
                        cred.key_passphrase.as_deref(),
                    )
                    .await;
                    metrics = ssh_result.metrics;
                }
                (true, Some(ms), None, metrics)
            }
            Err(e) => (false, None, Some(e), None),
        },
        None => (
            false,
            None,
            Some("Could not resolve hostname".to_string()),
            None,
        ),
    };

    RemoteHostMetric {
        id: host.id,
        name: host.name,
        address: host.address,
        port: host.port,
        reachable,
        latency_ms,
        error,
        metrics,
    }
}

#[tauri::command]
async fn get_remote_hosts_health(
    app: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
) -> Result<Vec<RemoteHostMetric>, String> {
    use futures::stream::StreamExt;

    let hosts = get_saved_hosts_from_db(&app)?;
    let master_key = vault_state.get_master_key().await.ok(); // None if vault locked

    // Decrypt credentials up front: these are blocking SQLite reads, so they are
    // kept out of the concurrent network phase below.
    let jobs: Vec<(SavedHost, Option<vault::credentials::Credential>)> = hosts
        .into_iter()
        .map(|h| {
            let credential = h
                .credential_id
                .as_ref()
                .zip(master_key.as_ref())
                .and_then(|(cred_id, key)| credential_manager.get_credential(key, cred_id).ok());
            (h, credential)
        })
        .collect();

    // `buffered` preserves input order, so results stay sorted by host name.
    let results = futures::stream::iter(jobs.into_iter().map(|(host, credential)| {
        let app = app.clone();
        probe_host(app, host, credential)
    }))
    .buffered(MAX_CONCURRENT_HOST_CHECKS)
    .collect::<Vec<RemoteHostMetric>>()
    .await;

    Ok(results)
}

#[tauri::command]
async fn set_host_monitoring_credential(
    app: AppHandle,
    host_id: String,
    credential_id: Option<String>,
) -> Result<(), String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let db_path = app_dir.join(DB_FILENAME);
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| "Invalid database path".to_string())?;
    let conn = db::open_connection(db_path_str)?;
    match credential_id.as_deref() {
        Some(id) if !id.is_empty() => {
            conn.execute(
                "INSERT OR REPLACE INTO monitoring_host_credential (host_id, credential_id) VALUES (?1, ?2)",
                rusqlite::params![host_id, id],
            ).map_err(|e| sanitize_error(e.to_string(), "database"))?;
        }
        _ => {
            conn.execute(
                "DELETE FROM monitoring_host_credential WHERE host_id = ?1",
                rusqlite::params![host_id],
            )
            .map_err(|e| sanitize_error(e.to_string(), "database"))?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn preflight_check(host: String) -> Result<health::HealthCheckResult, String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    Ok(health::preflight_check(&host).await)
}

#[tauri::command]
async fn check_host_health(
    app: AppHandle,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
) -> Result<health::HealthCheckResult, String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    Ok(health::check_ssh_health(
        app,
        &host,
        port,
        &username,
        password.as_deref(),
        false,
        None,
        None,
        None,
    )
    .await)
}

#[tauri::command]
fn get_system_metrics(
    collector: State<'_, Arc<std::sync::Mutex<monitoring::MetricsCollector>>>,
) -> Result<monitoring::SystemMetrics, String> {
    let mut collector = collector.lock().map_err(|e| {
        sanitize_error(
            format!("Failed to acquire metrics collector lock: {}", e),
            "monitoring",
        )
    })?;
    Ok(collector.collect())
}

#[tauri::command]
async fn get_metrics_history(
    start: u64,
    end: u64,
    host: Option<String>,
    app: AppHandle,
) -> Result<Vec<monitoring::SystemMetrics>, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "monitoring"))?;
    let db_path = app_dir.join(DB_FILENAME);
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| sanitize_error("Invalid database path".to_string(), "monitoring"))?
        .to_string();

    let store = monitoring::MetricsStore::new(db_path_str, 30)
        .map_err(|e| sanitize_error(e, "monitoring"))?;
    store
        .get_metrics_range(start, end, &host.unwrap_or_else(|| "localhost".to_string()))
        .map_err(|e| sanitize_error(e, "monitoring"))
}

#[tauri::command]
async fn get_alert_history(
    start: u64,
    end: u64,
    app: AppHandle,
) -> Result<Vec<monitoring::Alert>, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "monitoring"))?;
    let db_path = app_dir.join(DB_FILENAME);
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| sanitize_error("Invalid database path".to_string(), "monitoring"))?
        .to_string();

    let store = monitoring::MetricsStore::new(db_path_str, 30)
        .map_err(|e| sanitize_error(e, "monitoring"))?;
    store
        .get_alert_history(start, end)
        .map_err(|e| sanitize_error(e, "monitoring"))
}

#[tauri::command]
fn add_alert_rule(state: State<'_, Arc<monitoring::AlertEngine>>, rule: monitoring::AlertRule) {
    state.add_rule(rule);
}

#[tauri::command]
fn remove_alert_rule(state: State<'_, Arc<monitoring::AlertEngine>>, rule_id: String) {
    state.remove_rule(&rule_id);
}

#[tauri::command]
fn get_alert_rules(state: State<'_, Arc<monitoring::AlertEngine>>) -> Vec<monitoring::AlertRule> {
    state.get_rules()
}

// Vault commands
#[tauri::command]
async fn is_vault_initialized(state: State<'_, vault::VaultState>) -> Result<bool, String> {
    state
        .is_initialized()
        .await
        .map_err(|e| sanitize_error(e, "vault"))
}

#[tauri::command]
async fn initialize_vault(
    state: State<'_, vault::VaultState>,
    master_password: String,
) -> Result<(), String> {
    use secrecy::SecretString;
    validate_master_password(&master_password)?;
    state
        .initialize_vault(SecretString::from(master_password))
        .await
        .map_err(|e| sanitize_error(e, "vault"))
}

#[tauri::command]
async fn unlock_vault(
    state: State<'_, vault::VaultState>,
    master_password: String,
) -> Result<(), String> {
    use secrecy::SecretString;
    if master_password.is_empty() {
        return Err("Password cannot be empty".to_string());
    }
    state
        .unlock_vault(SecretString::from(master_password))
        .await
        .map_err(|e| sanitize_error(e, "vault"))
}

#[tauri::command]
async fn lock_vault(state: State<'_, vault::VaultState>) -> Result<(), String> {
    state
        .lock_vault()
        .await
        .map_err(|e| sanitize_error(e, "vault"))
}

#[tauri::command]
async fn is_vault_locked(state: State<'_, vault::VaultState>) -> Result<bool, String> {
    Ok(state.is_locked().await)
}

#[tauri::command]
async fn get_vault_settings(
    state: State<'_, vault::VaultState>,
) -> Result<vault::VaultSettings, String> {
    Ok(state.get_settings().await)
}

#[tauri::command]
async fn update_vault_settings(
    state: State<'_, vault::VaultState>,
    settings: vault::VaultSettings,
) -> Result<(), String> {
    state
        .update_settings(settings)
        .await
        .map_err(|e| sanitize_error(e, "vault"))
}

// Credential management commands
#[tauri::command]
async fn add_credential(
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    name: String,
    username: String,
    password: String,
    credential_type: String,
    host: Option<String>,
    port: Option<u16>,
    metadata: Option<String>,
    key_path: Option<String>,
    private_key: Option<String>,
    key_passphrase: Option<String>,
) -> Result<String, String> {
    validate_credential_name(&name)?;
    validate_username(&username)?;
    if let Some(ref h) = host {
        if validate_ip(h).is_err() && validate_hostname(h).is_err() {
            return Err("Invalid host format".to_string());
        }
    }
    if let Some(p) = port {
        validate_port(p)?;
    }
    let master_key = vault_state
        .get_master_key()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    credential_manager
        .add_credential(
            &master_key,
            name.clone(),
            username,
            password,
            credential_type,
            host,
            port,
            metadata,
            key_path,
            private_key,
            key_passphrase,
        )
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
async fn get_credential(
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
) -> Result<vault::CredentialFrontendView, String> {
    let master_key = vault_state
        .get_master_key()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    credential_manager
        .get_credential(&master_key, &credential_id)
        .map(vault::CredentialFrontendView::from)
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
async fn list_credentials(
    credential_manager: State<'_, vault::CredentialManager>,
) -> Result<Vec<vault::CredentialSummary>, String> {
    credential_manager
        .list_credentials()
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
async fn update_credential(
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
    name: Option<String>,
    username: Option<String>,
    password: Option<String>,
    metadata: Option<String>,
    credential_type: Option<String>,
    key_path: Option<String>,
    private_key: Option<String>,
    key_passphrase: Option<String>,
) -> Result<(), String> {
    if let Some(ref n) = name {
        validate_credential_name(n)?;
    }
    if let Some(ref u) = username {
        validate_username(u)?;
    }
    let master_key = vault_state
        .get_master_key()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    credential_manager
        .update_credential(
            &master_key,
            &credential_id,
            name,
            username,
            password,
            metadata,
            credential_type,
            key_path,
            private_key,
            key_passphrase,
        )
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
async fn delete_credential(
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
) -> Result<(), String> {
    credential_manager
        .delete_credential(&credential_id)
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
async fn search_credentials(
    credential_manager: State<'_, vault::CredentialManager>,
    query: String,
) -> Result<Vec<vault::CredentialSummary>, String> {
    credential_manager
        .search_credentials(&query)
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
async fn verify_ssh_host_key(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    host: String,
    port: u16,
    fingerprint: String,
    key_type: String,
) -> Result<vault::HostKeyVerificationResult, String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    ssh_key_manager
        .verify_host_key_by_fingerprint(&host, port, &fingerprint, &key_type)
        .await
        .map_err(|e| sanitize_error(e, "ssh"))
}

#[tauri::command]
async fn trust_ssh_host_key(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    host: String,
    port: u16,
    fingerprint: String,
    key_type: String,
    key_bytes: Vec<u8>,
    trust_status: vault::TrustStatus,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    ssh_key_manager
        .trust_host_key(
            &host,
            port,
            &fingerprint,
            &key_type,
            key_bytes,
            trust_status,
        )
        .await
        .map_err(|e| sanitize_error(e, "ssh"))
}

#[tauri::command]
async fn get_known_ssh_hosts(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
) -> Result<Vec<vault::SshHostKey>, String> {
    ssh_key_manager
        .get_known_hosts()
        .await
        .map_err(|e| sanitize_error(e, "ssh"))
}

#[tauri::command]
async fn remove_ssh_host_key(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    host: String,
    port: u16,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    ssh_key_manager
        .remove_host_key(&host, port)
        .await
        .map_err(|e| sanitize_error(e, "ssh"))
}

#[tauri::command]
async fn update_ssh_host_trust(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    host: String,
    port: u16,
    trust_status: vault::TrustStatus,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    ssh_key_manager
        .update_trust_status(&host, port, trust_status)
        .await
        .map_err(|e| sanitize_error(e, "ssh"))
}

// Change master password command
#[tauri::command]
async fn change_master_password(
    state: State<'_, vault::VaultState>,
    current_password: String,
    new_password: String,
) -> Result<(), String> {
    if current_password.is_empty() {
        return Err("Current password cannot be empty".to_string());
    }
    validate_master_password(&new_password)?;
    state
        .change_master_password(
            secrecy::SecretString::from(current_password),
            secrecy::SecretString::from(new_password),
        )
        .await
        .map_err(|e| sanitize_error(e, "vault"))
}

// Audit log commands
#[tauri::command]
async fn get_audit_logs(
    audit_manager: State<'_, vault::AuditLogManager>,
    filter: Option<vault::AuditLogFilter>,
) -> Result<Vec<vault::AuditLogEntry>, String> {
    audit_manager
        .get_audit_logs(filter)
        .map_err(|e| sanitize_error(e, "database"))
}

#[tauri::command]
async fn get_audit_log_count(
    audit_manager: State<'_, vault::AuditLogManager>,
    filter: Option<vault::AuditLogFilter>,
) -> Result<i64, String> {
    audit_manager
        .get_audit_log_count(filter)
        .map_err(|e| sanitize_error(e, "database"))
}

// SFTP commands
#[tauri::command]
async fn sftp_upload_file(
    app_handle: AppHandle,
    host: String,
    port: u16,
    username: String,
    password: String,
    local_path: String,
    remote_path: String,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    validate_path(&local_path)?;
    validate_path(&remote_path)?;
    sftp::upload_file(
        app_handle,
        &host,
        port,
        &username,
        &password,
        &local_path,
        &remote_path,
        None,
    )
    .await
    .map_err(|e| sanitize_error(e, "sftp"))
}

#[tauri::command]
async fn sftp_download_file(
    app_handle: AppHandle,
    host: String,
    port: u16,
    username: String,
    password: String,
    remote_path: String,
    local_path: String,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    validate_path(&remote_path)?;
    validate_path(&local_path)?;
    sftp::download_file(
        app_handle,
        &host,
        port,
        &username,
        &password,
        &remote_path,
        &local_path,
        None,
    )
    .await
    .map_err(|e| sanitize_error(e, "sftp"))
}

#[tauri::command]
async fn sftp_list_directory(
    app_handle: AppHandle,
    host: String,
    port: u16,
    username: String,
    password: String,
    remote_path: String,
) -> Result<Vec<sftp::RemoteFile>, String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    sftp::list_directory(app_handle, &host, port, &username, &password, &remote_path)
        .await
        .map_err(|e| sanitize_error(e, "sftp"))
}

#[tauri::command]
async fn sftp_remote_exists(
    app_handle: AppHandle,
    host: String,
    port: u16,
    username: String,
    password: String,
    remote_path: String,
) -> Result<bool, String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    sftp::remote_exists(app_handle, &host, port, &username, &password, &remote_path)
        .await
        .map_err(|e| sanitize_error(e, "sftp"))
}

/// App info for Settings (About, Data tabs).
#[derive(serde::Serialize)]
pub struct AppInfo {
    pub app_data_dir: String,
    pub db_path: String,
    pub db_size_bytes: Option<u64>,
    pub version: String,
    pub platform: String,
    pub arch: String,
}

#[tauri::command]
fn get_app_info(app: AppHandle) -> Result<AppInfo, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let app_data_dir = app_dir
        .to_str()
        .ok_or_else(|| "Invalid app data path".to_string())?
        .to_string();
    let db_path = app_dir.join(DB_FILENAME);
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| "Invalid database path".to_string())?
        .to_string();
    let db_size_bytes =
        std::fs::metadata(&db_path)
            .ok()
            .and_then(|m| if m.is_file() { Some(m.len()) } else { None });
    let version = app.package_info().version.to_string();
    let platform = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    Ok(AppInfo {
        app_data_dir,
        db_path: db_path_str,
        db_size_bytes,
        version,
        platform,
        arch,
    })
}

#[tauri::command]
fn clear_metrics_data(app: AppHandle) -> Result<(), String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "monitoring"))?;
    let db_path = app_dir.join(DB_FILENAME);
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| "Invalid database path".to_string())?;
    let conn = db::open_connection(db_path_str)?;
    conn.execute("DELETE FROM metrics_history", [])
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    conn.execute("DELETE FROM alert_history", [])
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    Ok(())
}

#[tauri::command]
fn export_database(app: AppHandle, dest_path: String) -> Result<(), String> {
    validate_path(&dest_path)?;
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let src = app_dir.join(DB_FILENAME);
    if !src.exists() {
        return Err("Database file not found".to_string());
    }
    // Use the rusqlite backup API instead of VACUUM INTO string interpolation,
    // which was vulnerable to SQL injection via a crafted dest_path.
    let src_conn =
        rusqlite::Connection::open(&src).map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let mut dst_conn = rusqlite::Connection::open(&dest_path)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let backup = rusqlite::backup::Backup::new(&src_conn, &mut dst_conn)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    backup
        .run_to_completion(100, std::time::Duration::from_millis(0), None)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    Ok(())
}

#[tauri::command]
fn import_database(app: AppHandle, source_path: String) -> Result<(), String> {
    validate_path(&source_path)?;
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let db_path = app_dir.join(DB_FILENAME);

    // Validate that the source file is a readable SQLite database.
    if !std::path::Path::new(&source_path).exists() {
        return Err("Source file not found".to_string());
    }
    rusqlite::Connection::open_with_flags(&source_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|c| {
            c.execute_batch("SELECT count(*) FROM sqlite_master;")?;
            Ok(())
        })
        .map_err(|_| "Source is not a valid SQLite database".to_string())?;

    // Both the backup and the restore go through SQLite's backup API rather than
    // touching files directly.
    //
    // The database runs in WAL mode, so committed data may live in `-wal` beside
    // the main file. Copying only `quasar.db` could capture an incomplete backup,
    // and renaming a replacement over the main file would leave the previous
    // `-wal`/`-shm` next to it — stale frames that SQLite could then apply to the
    // imported database. Going through SQLite keeps the main file, WAL and shared
    // index consistent with each other, and cooperates with the connections that
    // monitoring and the scheduler already hold open.
    let backup_path = app_dir.join(format!("{}.bak", DB_FILENAME));
    if db_path.exists() {
        // Start from a clean destination so no previous backup's WAL lingers.
        let _ = std::fs::remove_file(&backup_path);
        let src = db::open_connection(
            db_path
                .to_str()
                .ok_or_else(|| "Invalid database path".to_string())?,
        )?;
        let mut dst = rusqlite::Connection::open(&backup_path)
            .map_err(|e| sanitize_error(e.to_string(), "database"))?;
        rusqlite::backup::Backup::new(&src, &mut dst)
            .map_err(|e| sanitize_error(e.to_string(), "database"))?
            .run_to_completion(100, std::time::Duration::from_millis(0), None)
            .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    }

    // Restore into the live database file in place.
    let src = rusqlite::Connection::open_with_flags(
        &source_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let mut dst = db::open_connection(
        db_path
            .to_str()
            .ok_or_else(|| "Invalid database path".to_string())?,
    )?;
    rusqlite::backup::Backup::new(&src, &mut dst)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?
        .run_to_completion(100, std::time::Duration::from_millis(0), None)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;

    // Long-lived connections may still serve cached reads, so a restart is
    // recommended — but the file itself is now consistent either way.
    log::warn!(
        "Database imported from '{}'. Restart recommended so background tasks reload it.",
        source_path
    );
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .setup(|app| {
            // Get database path
            let app_dir = app.path().app_data_dir().map_err(|e| {
                error!("Failed to get app data dir: {}", e);
                e
            })?;

            std::fs::create_dir_all(&app_dir).map_err(|e| {
                error!("Failed to create app data dir: {}", e);
                e
            })?;
            migrate_titan_db_to_quasar(&app_dir).map_err(|e| {
                error!("Failed to migrate database file: {}", e);
                e
            })?;

            let db_path = app_dir.join(DB_FILENAME);
            let db_path_str = db_path.to_str().ok_or("Invalid database path")?;
            let db_path_str = db_path_str.to_string();

            app.manage(ssh::SshState::new());
            // Reuses authenticated sessions for one-shot commands (monitoring
            // probes, scheduled tasks) instead of re-handshaking every time.
            app.manage(ssh_pool::SshConnectionPool::new());
            app.manage(ssh_tunnel::TunnelState::new());
            app.manage(discovery::DiscoveryState::new());
            app.manage(Arc::new(scanner::ScannerState::new()));
            app.manage(Arc::new(monitoring::AlertEngine::new()));
            app.manage(vault::VaultState::new(db_path_str.clone()));
            app.manage(vault::CredentialManager::new(db_path_str.clone()));
            app.manage(vault::SshKeyManager::new(db_path_str.clone()).map_err(|e| {
                error!("Failed to create SSH key manager: {}", e);
                e
            })?);
            app.manage(vault::AuditLogManager::new(db_path_str.clone()));
            app.manage(host_tracker::HostTracker::new(db_path_str.clone()));
            app.manage(Arc::new(std::sync::Mutex::new(
                monitoring::MetricsCollector::new(),
            )));

            // Initialize database tables with foreign key constraints enabled
            let mut conn = db::open_connection(&db_path_str).map_err(|e| {
                error!("Failed to open database at {}: {}", db_path_str, e);
                e
            })?;

            // Run migrations using rusqlite_migration
            MIGRATIONS.to_latest(&mut conn).map_err(|e| {
                error!("Failed to run migrations: {}", e);
                e
            })?;

            // Start the background monitoring task using Tauri's async runtime
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                monitoring::start_monitoring_task(app_handle, 5).await;
            });

            // Remote hosts health is polled by the frontend (Monitoring tab) every 30s via get_remote_hosts_health

            // Start auto-lock checker task
            let vault_state_clone = app.state::<vault::VaultState>().inner().clone();
            let app_handle_vault = app.handle().clone();
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

            // Start SSH session timeout checker (30 minute idle timeout)
            let ssh_state_ref = app.state::<ssh::SshState>();
            let ssh_sessions = ssh_state_ref.sessions.clone();
            let app_handle_ssh = app.handle().clone();
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

            // Start cron-based scheduled task runner (SSH commands on saved hosts)
            scheduler::start_scheduler(app.handle().clone());

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    builder
        .invoke_handler(tauri::generate_handler![
            get_metrics_history,
            get_alert_history,
            launch_ssh_external,
            connect_rdp,
            start_discovery,
            check_ai_status,
            list_ai_models,
            send_ai_chat,
            connect_ssh,
            ssh::write_ssh,
            ssh::resize_ssh,
            ssh::disconnect_ssh,
            scan_network,
            stop_scan,
            get_scan_progress,
            is_scanning,
            get_discovered_hosts,
            get_host_details,
            search_discovered_hosts,
            delete_discovered_host,
            get_saved_hosts,
            upsert_saved_host,
            remove_saved_hosts,
            get_remote_hosts_health,
            set_host_monitoring_credential,
            preflight_check,
            check_host_health,
            get_system_metrics,
            add_alert_rule,
            remove_alert_rule,
            get_alert_rules,
            is_vault_initialized,
            initialize_vault,
            unlock_vault,
            lock_vault,
            is_vault_locked,
            get_vault_settings,
            update_vault_settings,
            change_master_password,
            add_credential,
            get_credential,
            list_credentials,
            update_credential,
            delete_credential,
            search_credentials,
            verify_ssh_host_key,
            trust_ssh_host_key,
            get_known_ssh_hosts,
            remove_ssh_host_key,
            update_ssh_host_trust,
            get_audit_logs,
            get_audit_log_count,
            sftp_upload_file,
            sftp_download_file,
            sftp_list_directory,
            sftp_remote_exists,
            list_scheduled_tasks,
            get_scheduled_task,
            add_scheduled_task,
            update_scheduled_task,
            remove_scheduled_task,
            run_scheduled_task_now,
            start_ssh_tunnel,
            list_ssh_tunnels,
            close_ssh_tunnel,
            get_app_info,
            clear_metrics_data,
            export_database,
            import_database
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod saved_host_tests {
    use super::*;

    fn host_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE hosts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                address TEXT NOT NULL,
                port INTEGER NOT NULL DEFAULT 22,
                username TEXT,
                protocol TEXT NOT NULL DEFAULT 'ssh',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE monitoring_host_credential (
                host_id TEXT PRIMARY KEY,
                credential_id TEXT
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn upsert_saved_host_inserts_with_default_ssh_port() {
        let mut conn = host_conn();
        let host = upsert_saved_host_in_conn(
            &mut conn,
            "Server",
            "server.example.com",
            "ssh",
            None,
            Some("root"),
        )
        .unwrap();

        assert_eq!(host.port, 22);
        assert_eq!(host.username.as_deref(), Some("root"));
        assert!(!host.id.is_empty());
    }

    #[test]
    fn upsert_saved_host_updates_matching_connection() {
        let mut conn = host_conn();
        let original =
            upsert_saved_host_in_conn(&mut conn, "Old name", "10.0.0.5", "ssh", Some(2222), None)
                .unwrap();
        let updated = upsert_saved_host_in_conn(
            &mut conn,
            "New name",
            "10.0.0.5",
            "ssh",
            Some(2222),
            Some("admin"),
        )
        .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.name, "New name");
        assert_eq!(updated.username.as_deref(), Some("admin"));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM hosts", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn upsert_saved_host_uses_default_rdp_port() {
        let mut conn = host_conn();
        let host = upsert_saved_host_in_conn(
            &mut conn,
            "Desktop",
            "desktop.example.com",
            "rdp",
            None,
            None,
        )
        .unwrap();

        assert_eq!(host.port, 3389);
    }

    #[test]
    fn remove_saved_hosts_is_transactional_and_ignores_missing_ids() {
        let mut conn = host_conn();
        let first = upsert_saved_host_in_conn(&mut conn, "One", "one", "ssh", None, None).unwrap();
        let second = upsert_saved_host_in_conn(&mut conn, "Two", "two", "ssh", None, None).unwrap();

        let removed =
            remove_saved_hosts_in_conn(&mut conn, &[first.id, "missing".to_string(), second.id])
                .unwrap();

        assert_eq!(removed, 2);
        assert!(get_saved_hosts_from_conn(&conn).unwrap().is_empty());
    }

    #[test]
    fn remove_saved_hosts_removes_one_host() {
        let mut conn = host_conn();
        let host = upsert_saved_host_in_conn(&mut conn, "One", "one", "ssh", None, None).unwrap();

        assert_eq!(
            remove_saved_hosts_in_conn(&mut conn, std::slice::from_ref(&host.id)).unwrap(),
            1
        );
        assert!(get_saved_host_by_id(&conn, &host.id).unwrap().is_none());
    }

    #[test]
    fn remove_saved_hosts_rolls_back_on_error() {
        let mut conn = host_conn();
        let first = upsert_saved_host_in_conn(&mut conn, "One", "one", "ssh", None, None).unwrap();
        let second = upsert_saved_host_in_conn(&mut conn, "Two", "two", "ssh", None, None).unwrap();
        conn.execute_batch(&format!(
            "CREATE TRIGGER reject_host_delete BEFORE DELETE ON hosts
             WHEN OLD.id = '{}' BEGIN SELECT RAISE(ABORT, 'stop'); END;",
            second.id
        ))
        .unwrap();

        assert!(remove_saved_hosts_in_conn(&mut conn, &[first.id, second.id]).is_err());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM hosts", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn saved_host_operations_return_database_errors() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        assert!(upsert_saved_host_in_conn(&mut conn, "Host", "host", "ssh", None, None).is_err());
        assert!(remove_saved_hosts_in_conn(&mut conn, &["id".to_string()]).is_err());
    }

    #[test]
    fn migrations_apply_to_fresh_and_current_databases() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        let version = conn
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap();
        assert_eq!(version, 13);

        MIGRATIONS.to_latest(&mut conn).unwrap();
        let version_after_rerun = conn
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap();
        assert_eq!(version_after_rerun, version);
    }
}

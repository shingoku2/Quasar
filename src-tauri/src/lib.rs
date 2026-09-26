mod ai;
mod background;
mod saved_hosts;
mod crypto;
mod db;
mod local_paths;
mod native_confirm;
#[cfg(test)]
mod ssh_test_server;
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
mod tailscale;
mod validation;
mod vault;

use errors::sanitize_error;
use log::error;
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use std::str::FromStr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use db::backup::{import_database_at, migrate_titan_db_to_quasar, set_quasar_application_id};
use db::migrations::MIGRATIONS;
use db::{app_db_connection, DB_FILENAME};
use saved_hosts::*;
use validation::{
    validate_cidr, validate_credential_name, validate_hostname, validate_ip,
    validate_master_password, validate_path, validate_port, validate_username,
};



/// What an interactive SSH connection or a tunnel authenticates with.
struct SshLogin {
    username: String,
    password: Option<String>,
    key_path: Option<String>,
    private_key: Option<String>,
    key_passphrase: Option<String>,
}

/// The login for `host`: from the vault credential when one is named, otherwise the typed
/// username and password. A stored credential must be allowed for `host` and be an SSH type
/// (invariant 10); vault access is dropped before returning, so no network phase holds it.
async fn resolve_ssh_login(
    vault_state: &vault::VaultState,
    credential_manager: &vault::CredentialManager,
    credential_id: Option<String>,
    host: &str,
    typed_username: String,
    typed_password: Option<String>,
) -> Result<SshLogin, String> {
    let Some(cid) = credential_id else {
        return Ok(SshLogin {
            username: typed_username,
            password: typed_password,
            key_path: None,
            private_key: None,
            key_passphrase: None,
        });
    };
    let access = vault_state
        .credential_access()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    let cred = credential_manager
        .get_credential(access.key(), &cid)
        .map_err(|e| sanitize_error(e, "credential"))?;
    drop(access);
    if !vault::credentials::host_allowed(cred.host.as_deref(), host) {
        return Err(vault::credentials::host_mismatch_error(&cred.name, cred.host.as_deref()));
    }
    vault::credentials::check_type(&cred.name, &cred.credential_type, vault::credentials::CredentialUse::Ssh)?;
    Ok(SshLogin {
        username: cred.username,
        password: (!cred.password.is_empty()).then_some(cred.password),
        key_path: cred.key_path,
        private_key: cred.private_key,
        key_passphrase: cred.key_passphrase,
    })
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
    let SshLogin { username, password, key_path, private_key, key_passphrase } =
        resolve_ssh_login(&vault_state, &credential_manager, credential_id, &host, user, password).await?;
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
    let SshLogin { username, password, key_path, private_key, key_passphrase } =
        resolve_ssh_login(&vault_state, &credential_manager, credential_id, &ssh_host, ssh_user, password).await?;
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
async fn connect_rdp(address: String) -> Result<(), String> {
    if validate_ip(&address).is_err() && validate_hostname(&address).is_err() {
        return Err("Invalid host address format".to_string());
    }
    launcher::launch_rdp(&address).map_err(|e| sanitize_error(e, "network"))
}

/// Snapshot of the local Tailscale node and its peers (via `tailscale status --json`).
/// A missing CLI is reported as `installed: false`, not as an error.
#[tauri::command]
async fn get_tailscale_status() -> Result<tailscale::TailscaleStatus, String> {
    tailscale::fetch_status()
        .await
        .map_err(|e| sanitize_error(e, "tailscale"))
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

    // Claim the scan synchronously, before spawning, so there is no window
    // in which a stop_scan() call could land between "task spawned" and
    // "task actually starts" and have its stop signal silently discarded by
    // the spawned task's own claim. See scanner::claim_scan for details.
    scanner::claim_scan(&scanner_state).map_err(|e| sanitize_error(e, "scanner"))?;

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
fn delete_discovered_host(
    host_tracker: State<'_, host_tracker::HostTracker>,
    ip: String,
) -> Result<(), String> {
    validate_ip(&ip)?;
    host_tracker
        .delete_host(&ip)
        .map_err(|e| sanitize_error(e, "database"))
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
async fn update_saved_host(
    app: AppHandle,
    host_id: String,
    name: String,
    address: String,
    protocol: String,
    port: Option<u16>,
    username: Option<String>,
) -> Result<SavedHost, String> {
    let conn = app_db_connection(&app).map_err(|e| sanitize_error(e, "database"))?;
    if let Some((from, from_port, transfers)) =
        transfers_moved_by_host_edit(&conn, &host_id, &address, &protocol, port)
            .map_err(|e| sanitize_error(e, "database"))?
    {
        native_confirm::confirm(
            &app,
            "Change host address",
            format!(
                "Move this host to a new address?\n\n{} scheduled file transfer(s) use it and will connect to the new address with the local files already chosen for them.\n\nFrom: \"{}:{}\"\nTo: \"{}\"",
                transfers,
                native_confirm::display_text(&from),
                from_port,
                native_confirm::display_text(&address),
            ),
            "Change address",
        )
        .await?;
    }
    update_saved_host_in_conn(
        &conn,
        &host_id,
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


fn scheduled_tasks_conn(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    app_db_connection(app)
}

#[tauri::command]
async fn list_scheduled_tasks(app: AppHandle) -> Result<Vec<scheduler::ScheduledTask>, String> {
    let conn = scheduled_tasks_conn(&app)?;
    scheduler::list_scheduled_tasks(&conn).map_err(|e| sanitize_error(e, "scheduled tasks"))
}


/// `set_host_monitoring_credential` used to bind any id to any host without checks; the
/// 30 s monitoring poll would then log in with it (IPC-003).
fn check_credential_binding(
    conn: &rusqlite::Connection,
    host_id: &str,
    credential_id: &str,
    usage: vault::credentials::CredentialUse,
) -> Result<(), String> {
    use rusqlite::OptionalExtension;
    let address: String = conn
        .query_row("SELECT address FROM hosts WHERE id = ?1", [host_id], |r| r.get(0))
        .optional()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?
        .ok_or_else(|| "Host not found".to_string())?;
    let (name, bound_host, credential_type): (String, Option<String>, String) = conn
        .query_row(
            "SELECT name, host, credential_type FROM credentials WHERE id = ?1",
            [credential_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?
        .ok_or_else(|| "Credential not found".to_string())?;
    if !vault::credentials::host_allowed(bound_host.as_deref(), &address) {
        return Err(vault::credentials::host_mismatch_error(&name, bound_host.as_deref()));
    }
    vault::credentials::check_type(&name, &credential_type, usage)
}

const MAX_TASK_NAME_LEN: usize = 200;
const MAX_TASK_COMMAND_LEN: usize = 16 * 1024;

/// Validates a scheduled task before it's stored (IPC-011): known task type, an existing
/// host, bounded name/command, and the paths an SFTP task needs. Returns the task type.
fn validate_scheduled_task<'a>(
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
fn task_local_intent(task_type: &str) -> Option<local_paths::Intent> {
    match task_type {
        "sftp_upload" => Some(local_paths::Intent::Read),
        "sftp_download" => Some(local_paths::Intent::Write),
        _ => None,
    }
}

/// A saved SFTP task's transfer: local path, task type, host id, remote path.
type StoredTransfer = (Option<String>, Option<String>, String, Option<String>);

/// Whether an edited SFTP task keeps the whole transfer its grant was given for. The local
/// file was picked for one transfer: the same file, direction, host and remote path. Changing
/// any of them needs a new pick, or a saved upload could be pointed at another destination
/// without the dialog (PR #68 review), and switching upload/download turns read access into
/// write access.
fn task_transfer_unchanged(
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
async fn add_scheduled_task(
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
async fn update_scheduled_task(
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
async fn remove_scheduled_task(
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
async fn run_scheduled_task_now(
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

#[tauri::command]
async fn get_remote_hosts_health(
    app: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
) -> Result<Vec<RemoteHostMetric>, String> {
    use futures::stream::StreamExt;

    let hosts = get_saved_hosts_from_db(&app)?;
    // Background poll: must not count as user activity, or the dashboard's 30 s poll keeps
    // the vault unlocked forever (RSEC-002). Skip the gate entirely when no host needs it.
    let access = if hosts.iter().any(|h| h.credential_id.is_some()) {
        vault_state.credential_access_background().await.ok() // None if vault locked
    } else {
        None
    };

    // Decrypt credentials up front: these are blocking SQLite reads, so they are
    // kept out of the concurrent network phase below.
    let jobs: Vec<(SavedHost, Option<vault::credentials::Credential>)> = hosts
        .into_iter()
        .map(|h| {
            let credential = match (h.credential_id.as_ref(), access.as_ref()) {
                (Some(cred_id), Some(access)) => credential_manager
                    .get_credential(access.key(), cred_id)
                    .ok()
                    // A credential bound to another host is never sent to this one (IPC-003).
                    .filter(|c| vault::credentials::host_allowed(c.host.as_deref(), &h.address))
                    // Only an SSH credential is ever sent to the SSH metrics probe.
                    .filter(|c| {
                        vault::credentials::type_allowed(&c.credential_type, vault::credentials::CredentialUse::Ssh)
                    }),
                _ => None,
            };
            (h, credential)
        })
        .collect();
    // Release the credential gate before the network phase, which can take seconds.
    drop(access);

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
    let conn = app_db_connection(&app)?;
    match credential_id.as_deref() {
        Some(id) if !id.is_empty() => {
            check_credential_binding(&conn, &host_id, id, vault::credentials::CredentialUse::Ssh)?;
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
fn add_alert_rule(
    state: State<'_, Arc<monitoring::AlertEngine>>,
    rule: monitoring::AlertRule,
) -> Result<(), String> {
    state.add_rule(rule)
}

#[tauri::command]
fn remove_alert_rule(
    state: State<'_, Arc<monitoring::AlertEngine>>,
    rule_id: String,
) -> Result<(), String> {
    state.remove_rule(&rule_id)
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
        .map_err(errors::user_facing_vault_error)
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
        .map_err(errors::user_facing_vault_error)
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
    let access = vault_state
        .credential_access()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    credential_manager
        .add_credential(
            access.key(),
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
    let access = vault_state
        .credential_access()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    credential_manager
        .get_credential(access.key(), &credential_id)
        .map(vault::CredentialFrontendView::from)
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
async fn reveal_credential_password(
    app: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
) -> Result<String, String> {
    let name = credential_manager
        .list_credentials()
        .map_err(|e| sanitize_error(e, "credential"))?
        .into_iter()
        .find(|c| c.id == credential_id)
        .map(|c| c.name)
        .ok_or_else(|| "Credential not found".to_string())?;
    // Don't ask the user to confirm something that will fail anyway.
    if vault_state.is_locked().await {
        return Err("Vault is locked".to_string());
    }
    // The plaintext leaves the vault only on a native confirmation, so a compromised
    // webview can't silently read every stored password (P7-3 review).
    if let Err(e) = native_confirm::confirm(
        &app,
        "Reveal password",
        format!(
            "Show the password for this credential?\n\n\"{}\"",
            native_confirm::display_text(&name)
        ),
        "Reveal",
    )
    .await
    {
        credential_manager.record_reveal_declined(&credential_id);
        return Err(e);
    }
    let access = vault_state
        .credential_access()
        .await
        .map_err(errors::user_facing_vault_error)?;
    let password = credential_manager
        .get_credential(access.key(), &credential_id)
        .map(|c| c.password)
        .map_err(|e| sanitize_error(e, "credential"))?;
    // A plaintext reveal is audited separately from ordinary (background) credential use.
    credential_manager.record_reveal(&credential_id);
    Ok(password)
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
    app: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
    name: Option<String>,
    username: Option<String>,
    password: Option<String>,
    metadata: Option<String>,
    credential_type: Option<String>,
    host: Option<String>,
    port: Option<u16>,
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
    let stored = credential_manager
        .list_credentials()
        .map_err(|e| sanitize_error(e, "credential"))?
        .into_iter()
        .find(|c| c.id == credential_id)
        .ok_or_else(|| "Credential not found".to_string())?;
    if native_confirm::host_change_needs_confirm(stored.host.as_deref(), host.as_deref()) {
        // Moving or clearing a host binding widens where the secret can be sent (IPC-003).
        let to = host.as_deref().map(str::trim).filter(|h| !h.is_empty());
        native_confirm::confirm(
            &app,
            "Change credential host",
            format!(
                "This credential is restricted to one host. {}?\n\nCredential: \"{}\"\nCurrent host: \"{}\"",
                match to {
                    Some(h) => format!("Allow it to be used with \"{}\" instead", native_confirm::display_text(h)),
                    None => "Allow it to be used with any host".to_string(),
                },
                native_confirm::display_text(&stored.name),
                native_confirm::display_text(stored.host.as_deref().unwrap_or_default()),
            ),
            "Change host",
        )
        .await?;
    }
    let access = vault_state
        .credential_access()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    credential_manager
        .update_credential(
            access.key(),
            &credential_id,
            name,
            username,
            password,
            metadata,
            credential_type,
            host,
            port,
            key_path,
            private_key,
            key_passphrase,
        )
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
async fn delete_credential(
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
) -> Result<(), String> {
    let _gate = vault_state.credential_gate().await;
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
async fn trust_ssh_host_key(
    app: AppHandle,
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    approval_state: State<'_, ssh::HostKeyApprovalState>,
    audit: State<'_, vault::AuditLogManager>,
    request_id: String,
) -> Result<(), String> {
    confirm_changed_host_key(&app, &approval_state, &request_id).await?;
    // Only a key a handshake actually presented can be trusted, and only through its
    // pending approval. The webview used to pass host/fingerprint/key bytes itself and
    // could pin any key for any host, silently enabling a MITM on the unattended SSH,
    // SFTP and monitoring paths (IPC-002).
    let manager = ssh_key_manager.inner();
    let mut trusted: Option<(String, u16, String)> = None;
    approval_state
        .accept_and_persist(&request_id, |key| {
            trusted = Some((key.host.clone(), key.port, key.fingerprint.clone()));
            async move {
                manager
                    .trust_host_key(
                        &key.host,
                        key.port,
                        &key.fingerprint,
                        &key.key_type,
                        key.key_bytes,
                        vault::TrustStatus::Trusted,
                    )
                    .await
            }
        })
        .await
        .map_err(|e| {
            if e.contains("no longer pending") || e.contains("stopped waiting") {
                e
            } else {
                sanitize_error(e, "ssh")
            }
        })?;
    if let Some((host, port, fingerprint)) = trusted {
        audit.record(
            "ssh_host_key_trust",
            None,
            "ssh_host_key",
            "trust",
            "success",
            Some(&format!("{}:{} {}", host, port, fingerprint)),
        );
    }
    Ok(())
}

#[tauri::command]
async fn respond_ssh_host_key_verification(
    app: AppHandle,
    approval_state: State<'_, ssh::HostKeyApprovalState>,
    request_id: String,
    accepted: bool,
) -> Result<(), String> {
    if accepted {
        confirm_changed_host_key(&app, &approval_state, &request_id).await?;
    }
    approval_state.resolve(&request_id, accepted)
}

/// A changed host key is accepted (once or permanently) only after a native confirmation:
/// the webview's own prompt can be skipped by a compromised webview. Declining rejects the
/// pending handshake.
async fn confirm_changed_host_key(
    app: &AppHandle,
    approval_state: &ssh::HostKeyApprovalState,
    request_id: &str,
) -> Result<(), String> {
    let Some(warning) = approval_state.changed_key_warning(request_id)? else {
        return Ok(());
    };
    if let Err(e) = native_confirm::confirm(app, "Host key changed", warning, "Trust new key").await {
        let _ = approval_state.resolve(request_id, false);
        return Err(e);
    }
    Ok(())
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
    app: AppHandle,
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    audit: State<'_, vault::AuditLogManager>,
    host: String,
    port: u16,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    // Removing a key turns the next connection into a first-seen (TOFU) prompt, which would
    // let a compromised webview replace a pinned key; so the user confirms natively.
    native_confirm::confirm(
        &app,
        "Remove host key",
        format!(
            "Remove the stored host key for {}:{}?\n\nThe next connection will ask you to trust whatever key the server presents.",
            host, port
        ),
        "Remove",
    )
    .await?;
    ssh_key_manager
        .remove_host_key(&host, port)
        .await
        .map_err(|e| sanitize_error(e, "ssh"))?;
    audit.record(
        "ssh_host_key_remove",
        None,
        "ssh_host_key",
        "delete",
        "success",
        Some(&format!("{}:{}", host, port)),
    );
    Ok(())
}

#[tauri::command]
async fn update_ssh_host_trust(
    app: AppHandle,
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    audit: State<'_, vault::AuditLogManager>,
    host: String,
    port: u16,
    trust_status: vault::TrustStatus,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    // Read the current status first: fails closed if it can't be read.
    let current = ssh_key_manager
        .trust_status(&host, port)
        .await
        .map_err(|e| sanitize_error(e, "ssh"))?;
    if native_confirm::trust_change_needs_confirm(current.as_ref(), &trust_status) {
        let message = if matches!(trust_status, vault::TrustStatus::Trusted) {
            format!(
                "Trust the stored host key for {}:{}?\n\nConnections, including scheduled tasks and monitoring, will then accept it without asking.",
                host, port
            )
        } else {
            format!(
                "Stop rejecting the stored host key for {}:{}?\n\nThe next connection will ask whether to trust it instead of refusing it.",
                host, port
            )
        };
        let (title, action) = if matches!(trust_status, vault::TrustStatus::Trusted) {
            ("Trust host key", "Trust")
        } else {
            ("Stop rejecting host key", "Stop rejecting")
        };
        native_confirm::confirm(&app, title, message, action).await?;
    }
    let details = format!("{}:{} -> {:?}", host, port, trust_status);
    ssh_key_manager
        .update_trust_status(&host, port, trust_status)
        .await
        .map_err(|e| sanitize_error(e, "ssh"))?;
    // Trust changes on stored keys are security-relevant (IPC-002).
    audit.record("ssh_host_key_trust_change", None, "ssh_host_key", "update", "success", Some(&details));
    Ok(())
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
        .map_err(errors::user_facing_vault_error)
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


// SFTP commands
/// Opens a native "open file" dialog from the backend and records the choice, so the path
/// can later be used for file I/O (IPC-001). `None` when the user cancels.
#[tauri::command]
async fn pick_local_file(
    app: AppHandle,
    grants: State<'_, local_paths::LocalPathGrants>,
    title: Option<String>,
    extensions: Option<Vec<String>>,
) -> Result<Option<String>, String> {
    let dialog_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut builder = file_dialog(&dialog_app, extensions);
        if let Some(title) = title {
            builder = builder.set_title(title);
        }
        builder.blocking_pick_file()
    })
    .await
    .map_err(|e| sanitize_error(e.to_string(), "dialog"))?;
    record_pick(&grants, picked, local_paths::Intent::Read)
}

/// Opens a native "save file" dialog from the backend and records the choice (IPC-001).
#[tauri::command]
async fn pick_save_location(
    app: AppHandle,
    grants: State<'_, local_paths::LocalPathGrants>,
    default_name: Option<String>,
    extensions: Option<Vec<String>>,
) -> Result<Option<String>, String> {
    let dialog_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut builder = file_dialog(&dialog_app, extensions);
        if let Some(name) = default_name {
            builder = builder.set_file_name(name);
        }
        builder.blocking_save_file()
    })
    .await
    .map_err(|e| sanitize_error(e.to_string(), "dialog"))?;
    record_pick(&grants, picked, local_paths::Intent::Write)
}

/// A file dialog parented to the main window, optionally filtered to `extensions`.
fn file_dialog(
    app: &AppHandle,
    extensions: Option<Vec<String>>,
) -> tauri_plugin_dialog::FileDialogBuilder<tauri::Wry> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app.dialog().file();
    if let Some(window) = app.get_webview_window("main") {
        builder = builder.set_parent(&window);
    }
    if let Some(exts) = extensions.filter(|e| !e.is_empty()) {
        let exts: Vec<&str> = exts.iter().map(String::as_str).collect();
        builder = builder.add_filter("Files", &exts);
    }
    builder
}

fn record_pick(
    grants: &local_paths::LocalPathGrants,
    picked: Option<tauri_plugin_dialog::FilePath>,
    intent: local_paths::Intent,
) -> Result<Option<String>, String> {
    let Some(picked) = picked else { return Ok(None) };
    let path = picked
        .into_path()
        .map_err(|e| sanitize_error(e.to_string(), "dialog"))?;
    let path_str = path
        .to_str()
        .ok_or_else(|| "Selected path is not valid UTF-8".to_string())?
        .to_string();
    grants.grant(path, intent);
    Ok(Some(path_str))
}

/// Resolves SFTP auth: a vault credential by id (decrypted here, never sent to the webview),
/// or a password the user typed. SFTP is password-only (see CLAUDE.md constraints).
async fn resolve_sftp_auth(
    vault_state: &vault::VaultState,
    credential_manager: &vault::CredentialManager,
    host: &str,
    username: String,
    password: Option<String>,
    credential_id: Option<String>,
) -> Result<(String, String), String> {
    match credential_id {
        Some(cid) => {
            let access = vault_state
                .credential_access()
                .await
                .map_err(errors::user_facing_vault_error)?;
            let cred = credential_manager
                .get_credential(access.key(), &cid)
                .map_err(|e| sanitize_error(e, "credential"))?;
            drop(access);
            if !vault::credentials::host_allowed(cred.host.as_deref(), host) {
                return Err(vault::credentials::host_mismatch_error(&cred.name, cred.host.as_deref()));
            }
            vault::credentials::check_type(&cred.name, &cred.credential_type, vault::credentials::CredentialUse::Sftp)?;
            if cred.password.is_empty() {
                return Err("SFTP needs a password credential".to_string());
            }
            Ok((cred.username, cred.password))
        }
        None => Ok((username, password.ok_or_else(|| "A password or a credential is required".to_string())?)),
    }
}

#[tauri::command]
async fn sftp_upload_file(
    app_handle: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    grants: State<'_, local_paths::LocalPathGrants>,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    credential_id: Option<String>,
    local_path: String,
    remote_path: String,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    let (username, password) = resolve_sftp_auth(
        &vault_state,
        &credential_manager,
        &host,
        username,
        password,
        credential_id,
    )
    .await?;
    grants.take(&local_path, local_paths::Intent::Read)?;
    validate_path(&local_path)?;
    validate_path(&remote_path)?;
    // The grant is used up even if the transfer fails: giving it back would let a
    // compromised webview replay the pick against another host (PR #68 review). The UI
    // opens the dialog again for every transfer.
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
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    grants: State<'_, local_paths::LocalPathGrants>,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    credential_id: Option<String>,
    remote_path: String,
    local_path: String,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    let (username, password) = resolve_sftp_auth(
        &vault_state,
        &credential_manager,
        &host,
        username,
        password,
        credential_id,
    )
    .await?;
    grants.take(&local_path, local_paths::Intent::Write)?;
    validate_path(&remote_path)?;
    validate_path(&local_path)?;
    // Used up even on failure, as for uploads.
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
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    credential_id: Option<String>,
    remote_path: String,
) -> Result<Vec<sftp::RemoteFile>, String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    let (username, password) = resolve_sftp_auth(
        &vault_state,
        &credential_manager,
        &host,
        username,
        password,
        credential_id,
    )
    .await?;
    sftp::list_directory(app_handle, &host, port, &username, &password, &remote_path)
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
    let conn = app_db_connection(&app)?;
    conn.execute("DELETE FROM metrics_history", [])
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    conn.execute("DELETE FROM alert_history", [])
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    Ok(())
}

#[tauri::command]
fn export_database(
    app: AppHandle,
    grants: State<'_, local_paths::LocalPathGrants>,
    dest_path: String,
) -> Result<(), String> {
    validate_path(&dest_path)?;
    grants.take(&dest_path, local_paths::Intent::Write)?;
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
    // The export carries the encrypted vault; owner-only like the live DB (RSEC-009).
    db::restrict_to_owner(std::path::Path::new(&dest_path))?;
    Ok(())
}

#[tauri::command]
async fn import_database(
    app: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    grants: State<'_, local_paths::LocalPathGrants>,
    source_path: String,
) -> Result<(), String> {
    validate_path(&source_path)?;
    grants.take(&source_path, local_paths::Intent::Read)?;
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let was_locked = vault_state.is_locked().await;
    let alert_engine = app.state::<Arc<monitoring::AlertEngine>>();
    let result = import_database_at(&app_dir, &source_path, &vault_state, Some(alert_engine.as_ref())).await;
    // The frontend handles this event by showing the unlock dialog. Emit it whenever the
    // import locked the vault, including a backup/restore failure after the lock, so the UI
    // never keeps showing an unlocked vault the backend has locked.
    if !was_locked && vault_state.is_locked().await {
        let _ = app.emit("vault-auto-locked", ());
    }
    result?;
    log::warn!(
        "Database imported from '{}'. Vault locked; restart recommended so background tasks reload it.",
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
            // Holds the vault DB and its .bak/WAL files: owner-only (RSEC-009).
            if let Err(e) = db::restrict_to_owner(&app_dir) {
                log::warn!("Could not restrict app data dir permissions: {}", e);
            }
            migrate_titan_db_to_quasar(&app_dir).map_err(|e| {
                error!("Failed to migrate database file: {}", e);
                e
            })?;

            let db_path = app_dir.join(DB_FILENAME);
            let db_path_str = db_path.to_str().ok_or("Invalid database path")?;
            let db_path_str = db_path_str.to_string();

            app.manage(ssh::SshState::new());
            app.manage(ssh::HostKeyApprovalState::new());
            app.manage(local_paths::LocalPathGrants::new());
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
            set_quasar_application_id(&conn).map_err(|e| {
                error!("Failed to set Quasar database marker: {}", e);
                e
            })?;
            // Alert rules are persisted (RUST-002); load them now that the table exists.
            match app.state::<Arc<monitoring::AlertEngine>>().attach_store(db_path_str.clone()) {
                Ok(n) => log::info!("Loaded {} alert rule(s)", n),
                Err(e) => error!("Failed to load alert rules: {}", e),
            }

            // Start the background monitoring task using Tauri's async runtime
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                monitoring::start_monitoring_task(app_handle, 5).await;
            });

            // Remote hosts health is polled by the frontend (Monitoring tab) every 30s via get_remote_hosts_health

            // Background loops that need the managed state above (keep this order).
            background::spawn_auto_lock(app.handle());
            background::spawn_ssh_idle_reaper(app.handle());

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
            connect_rdp,
            get_tailscale_status,
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
            delete_discovered_host,
            get_saved_hosts,
            upsert_saved_host,
            update_saved_host,
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
            reveal_credential_password,
            list_credentials,
            update_credential,
            delete_credential,
            search_credentials,
            trust_ssh_host_key,
            respond_ssh_host_key_verification,
            get_known_ssh_hosts,
            remove_ssh_host_key,
            update_ssh_host_trust,
            get_audit_logs,
            pick_local_file,
            pick_save_location,
            sftp_upload_file,
            sftp_download_file,
            sftp_list_directory,
            list_scheduled_tasks,
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
    use crate::db::backup::{is_recognized_quasar_database, restore_previous_database};
    use crate::db::migrations::LATEST_SCHEMA_VERSION;

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
    fn update_saved_host_changes_connection_port_in_place() {
        let mut conn = host_conn();
        let original = upsert_saved_host_in_conn(
            &mut conn,
            "VPS",
            "15.204.11.162",
            "ssh",
            Some(22),
            Some("edward"),
        )
        .unwrap();

        let updated = update_saved_host_in_conn(
            &conn,
            &original.id,
            "VPS",
            "15.204.11.162",
            "ssh",
            Some(6969),
            Some("edward"),
        )
        .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.port, 6969);
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

    fn migrated_quasar_db(path: &std::path::Path) {
        let mut conn = rusqlite::Connection::open(path).unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        set_quasar_application_id(&conn).unwrap();
    }

    fn import_test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("quasar_import_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// RSEC-004: importing replaces the vault's salt/verifier, so the vault must end up
    /// locked (its in-memory key belongs to the old database).
    #[tokio::test]
    async fn import_locks_the_vault() {
        let dir = import_test_dir("lock");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());
        vault
            .initialize_vault(secrecy::SecretString::from("LivePassword123!"))
            .await
            .unwrap();
        assert!(!vault.is_locked().await);

        let source = dir.join("backup.db");
        migrated_quasar_db(&source);
        import_database_at(&dir, source.to_str().unwrap(), &vault, None).await.unwrap();

        assert!(vault.is_locked().await, "vault must be locked after an import");
        assert!(dir.join(format!("{}.bak", DB_FILENAME)).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RUST-004: a backup from a newer schema is rejected instead of bricking startup, and
    /// the live database and vault are left alone.
    #[tokio::test]
    async fn import_rejects_newer_schema_and_leaves_vault_alone() {
        let dir = import_test_dir("newer");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());
        vault
            .initialize_vault(secrecy::SecretString::from("LivePassword123!"))
            .await
            .unwrap();

        let source = dir.join("future.db");
        migrated_quasar_db(&source);
        rusqlite::Connection::open(&source)
            .unwrap()
            .pragma_update(None, "user_version", LATEST_SCHEMA_VERSION + 1)
            .unwrap();
        let err = import_database_at(&dir, source.to_str().unwrap(), &vault, None)
            .await
            .unwrap_err();
        assert!(err.contains("newer version"), "{}", err);
        assert!(!vault.is_locked().await);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: a backup from an older schema is migrated as part of the import, so the
    /// running app finds every current table (startup migrations don't run again).
    #[tokio::test]
    async fn import_migrates_an_older_backup() {
        let dir = import_test_dir("older");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());

        let source = dir.join("old.db");
        {
            let mut conn = rusqlite::Connection::open(&source).unwrap();
            MIGRATIONS.to_version(&mut conn, (LATEST_SCHEMA_VERSION - 1) as usize).unwrap();
            set_quasar_application_id(&conn).unwrap();
            let has_rules: bool = conn
                .query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'alert_rules')", [], |r| r.get(0))
                .unwrap();
            assert!(!has_rules, "fixture: schema {} predates alert_rules", LATEST_SCHEMA_VERSION - 1);
        }
        import_database_at(&dir, source.to_str().unwrap(), &vault, None).await.unwrap();

        let conn = rusqlite::Connection::open(&live).unwrap();
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version, LATEST_SCHEMA_VERSION);
        conn.execute("INSERT INTO alert_rules (id, rule, updated_at) VALUES ('r', '{}', 0)", [])
            .expect("alert_rules exists after importing an older backup");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: the alert engine follows an imported database. Its rules were loaded
    /// once at startup, so monitoring kept evaluating the pre-import rules until restart.
    #[tokio::test]
    async fn import_reloads_alert_rules() {
        let dir = import_test_dir("alerts");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let add_rule = |path: &std::path::Path, id: &str| {
            let rule = monitoring::AlertRule {
                id: id.to_string(),
                metric: monitoring::MetricType::CpuUsage,
                operator: monitoring::ComparisonOperator::GreaterThan,
                threshold: 90.0,
                severity: monitoring::AlertSeverity::Warning,
                enabled: true,
                cooldown_seconds: 60,
            };
            rusqlite::Connection::open(path).unwrap()
                .execute(
                    "INSERT INTO alert_rules (id, rule, updated_at) VALUES (?1, ?2, 0)",
                    rusqlite::params![id, serde_json::to_string(&rule).unwrap()],
                )
                .unwrap();
        };
        add_rule(&live, "old-rule");
        let engine = monitoring::AlertEngine::new();
        assert_eq!(engine.attach_store(live.to_str().unwrap().to_string()).unwrap(), 1);

        let source = dir.join("backup.db");
        migrated_quasar_db(&source);
        add_rule(&source, "new-rule");
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());
        import_database_at(&dir, source.to_str().unwrap(), &vault, Some(&engine)).await.unwrap();

        let ids: Vec<String> = engine.get_rules().into_iter().map(|r| r.id).collect();
        assert_eq!(ids, vec!["new-rule".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: if the migration of an imported backup fails, the previous database is
    /// put back, and the error says so.
    #[tokio::test]
    async fn import_restores_the_previous_database_when_migration_fails() {
        let dir = import_test_dir("migfail");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        rusqlite::Connection::open(&live)
            .unwrap()
            .execute("INSERT INTO alert_rules (id, rule, updated_at) VALUES ('live-marker', '{}', 0)", [])
            .unwrap();
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());

        // A current schema claiming version 6: migration 008 re-adds a column that exists.
        let source = dir.join("broken.db");
        migrated_quasar_db(&source);
        rusqlite::Connection::open(&source).unwrap().pragma_update(None, "user_version", 6).unwrap();
        let err = import_database_at(&dir, source.to_str().unwrap(), &vault, None).await.unwrap_err();
        assert!(!err.contains("could not be restored"), "{}", err);

        let conn = rusqlite::Connection::open(&live).unwrap();
        let marker: i64 = conn
            .query_row("SELECT count(*) FROM alert_rules WHERE id = 'live-marker'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(marker, 1, "the previous database is live again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: the alert rules are suspended for the replacement and reloaded whatever
    /// the outcome, so a failed (and rolled back) import leaves the old rules active.
    #[tokio::test]
    async fn failed_import_keeps_the_previous_alert_rules() {
        let dir = import_test_dir("migfail_rules");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let engine = monitoring::AlertEngine::new();
        engine.attach_store(live.to_str().unwrap().to_string()).unwrap();
        engine
            .add_rule(monitoring::AlertRule {
                id: "live-rule".to_string(),
                metric: monitoring::MetricType::CpuUsage,
                operator: monitoring::ComparisonOperator::GreaterThan,
                threshold: 90.0,
                severity: monitoring::AlertSeverity::Warning,
                enabled: true,
                cooldown_seconds: 60,
            })
            .unwrap();
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());

        let source = dir.join("broken.db");
        migrated_quasar_db(&source);
        rusqlite::Connection::open(&source).unwrap().pragma_update(None, "user_version", 6).unwrap();
        import_database_at(&dir, source.to_str().unwrap(), &vault, Some(&engine)).await.unwrap_err();

        let ids: Vec<String> = engine.get_rules().into_iter().map(|r| r.id).collect();
        assert_eq!(ids, vec!["live-rule".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: the old vault's unlock lockout must not block the imported vault.
    #[tokio::test]
    async fn import_forgets_the_previous_vaults_lockout() {
        let dir = import_test_dir("lockout");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());
        vault.initialize_vault(secrecy::SecretString::from("LivePassword123!")).await.unwrap();
        vault.lock_vault().await.unwrap();
        for _ in 0..5 {
            let _ = vault.unlock_vault(secrecy::SecretString::from("wrong-password")).await;
        }
        let err = vault.unlock_vault(secrecy::SecretString::from("LivePassword123!")).await.unwrap_err();
        assert!(err.contains("locked out"), "fixture: the live vault is locked out: {}", err);

        let source = dir.join("other.db");
        migrated_quasar_db(&source);
        let other = vault::VaultState::new(source.to_str().unwrap().to_string());
        other.initialize_vault(secrecy::SecretString::from("OtherPassword123!")).await.unwrap();
        drop(other);
        import_database_at(&dir, source.to_str().unwrap(), &vault, None).await.unwrap();

        vault
            .unlock_vault(secrecy::SecretString::from("OtherPassword123!"))
            .await
            .expect("the imported vault unlocks with its own password");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: a restore that fails is reported, not swallowed.
    #[test]
    fn restore_previous_database_reports_failure() {
        let dir = import_test_dir("restorefail");
        let backup = dir.join("quasar.db.bak");
        std::fs::write(&backup, b"not a sqlite database, just some bytes long enough to have a header").unwrap();
        let mut dst = rusqlite::Connection::open(dir.join(DB_FILENAME)).unwrap();
        assert!(restore_previous_database(&backup, &mut dst).is_err());
        assert!(restore_previous_database(&dir.join("missing.bak"), &mut dst).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: when the imported alert rules can't be read, the import says so and
    /// monitoring runs with no rules, never the pre-import ones.
    #[tokio::test]
    async fn import_reports_unreadable_alert_rules_and_drops_the_old_ones() {
        let dir = import_test_dir("alertfail");
        let live = dir.join(DB_FILENAME);
        migrated_quasar_db(&live);
        let rule = monitoring::AlertRule {
            id: "old-rule".to_string(),
            metric: monitoring::MetricType::CpuUsage,
            operator: monitoring::ComparisonOperator::GreaterThan,
            threshold: 90.0,
            severity: monitoring::AlertSeverity::Warning,
            enabled: true,
            cooldown_seconds: 60,
        };
        let engine = monitoring::AlertEngine::new();
        engine.attach_store(live.to_str().unwrap().to_string()).unwrap();
        engine.add_rule(rule).unwrap();

        // A rule row whose column isn't text can't be loaded.
        let source = dir.join("backup.db");
        migrated_quasar_db(&source);
        rusqlite::Connection::open(&source)
            .unwrap()
            .execute("INSERT INTO alert_rules (id, rule, updated_at) VALUES ('bad', x'00ff', 0)", [])
            .unwrap();
        let vault = vault::VaultState::new(live.to_str().unwrap().to_string());
        let err = import_database_at(&dir, source.to_str().unwrap(), &vault, Some(&engine))
            .await
            .unwrap_err();
        assert!(err.contains("alert rules could not be loaded"), "{}", err);
        assert!(engine.get_rules().is_empty(), "pre-import rules must not stay active");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PR #68 review: a local-path grant is single-use even when the transfer fails, so only
    /// the picker commands may grant a path; nothing hands a used grant back.
    #[test]
    fn only_the_pickers_grant_local_paths() {
        let source = include_str!("lib.rs");
        let production = source.split("#[cfg(test)]\nmod saved_host_tests").next().unwrap();
        let grants: Vec<&str> = production.lines().filter(|l| l.contains(".grant(")).collect();
        assert_eq!(grants, vec!["    grants.grant(path, intent);"], "unexpected grant call sites");
    }

    fn migrated_memory_db() -> rusqlite::Connection {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        conn
    }

    /// PR #68 review: moving a host that scheduled file transfers use needs a native
    /// confirmation; renaming it, or moving a host no transfer uses, doesn't.
    #[test]
    fn moving_a_host_with_scheduled_transfers_is_detected() {
        let conn = migrated_memory_db();
        conn.execute(
            "INSERT INTO hosts (id, name, address, protocol, port, created_at, updated_at) VALUES ('h1', 'box', 'db1.example', 'ssh', 22, 0, 0)",
            [],
        )
        .unwrap();
        let moved = |address: &str, port: Option<u16>| transfers_moved_by_host_edit(&conn, "h1", address, "ssh", port).unwrap();
        assert_eq!(moved("evil.example", Some(22)), None, "no transfer uses the host yet");
        conn.execute(
            "INSERT INTO scheduled_tasks (id, name, cron_expression, host_id, command, enabled, created_at, updated_at, task_type, local_path, remote_path)
             VALUES ('t1', 'up', '0 0 * * * *', 'h1', '', 1, 0, 0, 'sftp_upload', '/home/u/report.txt', '/r')",
            [],
        )
        .unwrap();
        assert_eq!(moved("DB1.example ", None), None, "same destination (case, default port)");
        assert_eq!(moved("evil.example", Some(22)), Some(("db1.example".to_string(), 22, 1)));
        assert_eq!(moved("db1.example", Some(2222)), Some(("db1.example".to_string(), 22, 1)));
        assert_eq!(transfers_moved_by_host_edit(&conn, "missing", "x", "ssh", None).unwrap(), None);
    }

    /// IPC-011: task creation rejects unknown types (which used to run as SSH exec),
    /// missing hosts, empty SSH commands and SFTP tasks without both paths.
    /// P7-3 review: an upload task's read grant must not carry over when the task is
    /// switched to a download (write) on the same path.
    #[test]
    fn scheduled_task_path_exemption_requires_same_type() {
        use local_paths::Intent;
        assert_eq!(task_local_intent("sftp_upload"), Some(Intent::Read));
        assert_eq!(task_local_intent("sftp_download"), Some(Intent::Write));
        assert_eq!(task_local_intent("ssh"), None);
        let stored = |p: &str, t: &str| Some((Some(p.to_string()), Some(t.to_string()), "h1".to_string(), Some("/r".to_string())));
        let same = |s| task_transfer_unchanged(s, "/a", "sftp_upload", "h1", Some("/r"));
        assert!(same(stored("/a", "sftp_upload")));
        assert!(!task_transfer_unchanged(stored("/a", "sftp_upload"), "/a", "sftp_download", "h1", Some("/r")));
        assert!(!task_transfer_unchanged(stored("/a", "sftp_upload"), "/b", "sftp_upload", "h1", Some("/r")));
        assert!(!same(None));
        assert!(!same(Some((None, Some("ssh".into()), "h1".into(), None))));
        // PR #68 review: a new destination (host or remote path) needs a new pick too.
        assert!(!task_transfer_unchanged(stored("/a", "sftp_upload"), "/a", "sftp_upload", "evil", Some("/r")));
        assert!(!task_transfer_unchanged(stored("/a", "sftp_upload"), "/a", "sftp_upload", "h1", Some("/tmp/x")));
        assert!(!task_transfer_unchanged(stored("/a", "sftp_upload"), "/a", "sftp_upload", "h1", None));
    }

    #[test]
    fn scheduled_task_validation() {
        let mut conn = migrated_memory_db();
        let host = upsert_saved_host_in_conn(&mut conn, "srv", "10.0.0.5", "ssh", None, Some("root")).unwrap();
        let ok = |t: Option<&str>, cmd: &str, lp: Option<&str>, rp: Option<&str>| {
            validate_scheduled_task(&conn, "backup", &host.id, cmd, t, lp, rp, None).map(str::to_string)
        };
        assert_eq!(ok(None, "uptime", None, None).unwrap(), "ssh");
        assert!(ok(Some("SFTP_UPLOAD"), "uptime", None, None).unwrap_err().contains("Unknown task type"));
        assert!(ok(Some("ssh"), "   ", None, None).is_err());
        assert!(ok(Some("sftp_upload"), "", Some("/tmp/a"), None).is_err());
        assert_eq!(ok(Some("sftp_upload"), "", Some("/tmp/a"), Some("/srv/a")).unwrap(), "sftp_upload");
        assert!(validate_scheduled_task(&conn, "t", "no-such-host", "uptime", None, None, None, None)
            .unwrap_err()
            .contains("Host not found"));
        assert!(validate_scheduled_task(&conn, "", &host.id, "uptime", None, None, None, None).is_err());
        // PR #68 review: inventory-only hosts (database, API, other) can't run tasks.
        for protocol in ["database", "api", "other", "rdp"] {
            let other = upsert_saved_host_in_conn(&mut conn, protocol, "10.0.0.9", protocol, Some(5432), None).unwrap();
            assert!(validate_scheduled_task(&conn, "t", &other.id, "uptime", None, None, None, None)
                .unwrap_err()
                .contains("SSH host"));
        }
        // A credential bound to another host is refused when the task is saved.
        conn.execute(
            "INSERT INTO credentials (id, name, username, encrypted_password, nonce, tag, credential_type, host, created_at, updated_at)
             VALUES ('elsewhere', 'elsewhere', 'u', X'00', zeroblob(12), zeroblob(16), 'password', 'evil.example', 0, 0)",
            [],
        )
        .unwrap();
        assert!(validate_scheduled_task(&conn, "t", &host.id, "uptime", None, None, None, Some("elsewhere"))
            .unwrap_err()
            .contains("restricted"));
    }

    /// PR #68 review: only SSH-type credentials authenticate SSH, and SFTP takes SSH
    /// passwords only. An API/database/RDP/other credential's secret is never sent to an SSH
    /// server; the check runs when a task or monitoring binding is saved and again at use.
    #[test]
    fn credential_type_must_match_its_use() {
        use vault::credentials::{type_allowed, CredentialUse::{Sftp, Ssh}};
        for t in ["ssh", "password"] {
            assert!(type_allowed(t, Ssh) && type_allowed(t, Sftp), "{}", t);
        }
        assert!(type_allowed("ssh_key", Ssh) && !type_allowed("ssh_key", Sftp));
        for t in ["api", "database", "rdp", "other", ""] {
            assert!(!type_allowed(t, Ssh) && !type_allowed(t, Sftp), "{}", t);
        }

        let mut conn = migrated_memory_db();
        let host = upsert_saved_host_in_conn(&mut conn, "srv", "10.0.0.5", "ssh", None, Some("root")).unwrap();
        for (id, ty) in [("api", "api"), ("key", "ssh_key"), ("pw", "ssh")] {
            conn.execute(
                "INSERT INTO credentials (id, name, username, encrypted_password, nonce, tag, credential_type, created_at, updated_at)
                 VALUES (?1, ?1, 'u', X'00', zeroblob(12), zeroblob(16), ?2, 0, 0)",
                rusqlite::params![id, ty],
            )
            .unwrap();
        }
        let (up, down) = (Some("sftp_upload"), Some("sftp_download"));
        let paths = (Some("/tmp/a"), Some("/tmp/b"));
        // SSH command task: password or key, never an API credential.
        assert!(validate_scheduled_task(&conn, "t", &host.id, "uptime", None, None, None, Some("pw")).is_ok());
        assert!(validate_scheduled_task(&conn, "t", &host.id, "uptime", None, None, None, Some("key")).is_ok());
        assert!(validate_scheduled_task(&conn, "t", &host.id, "uptime", None, None, None, Some("api"))
            .unwrap_err()
            .contains("SSH needs"));
        // SFTP task: SSH password only.
        assert!(validate_scheduled_task(&conn, "t", &host.id, "", up, paths.0, paths.1, Some("pw")).is_ok());
        for bad in ["key", "api"] {
            assert!(validate_scheduled_task(&conn, "t", &host.id, "", down, paths.0, paths.1, Some(bad))
                .unwrap_err()
                .contains("SFTP needs"));
        }
        // Monitoring binding.
        assert!(check_credential_binding(&conn, &host.id, "key", vault::credentials::CredentialUse::Ssh).is_ok());
        assert!(check_credential_binding(&conn, &host.id, "api", vault::credentials::CredentialUse::Ssh).is_err());
    }

    /// RUST-020: the one credential-to-login path for the terminal and tunnels keeps both
    /// checks: host binding and SSH-only types (invariant 10).
    #[tokio::test]
    async fn ssh_login_resolution_enforces_host_and_type() {
        let dir = import_test_dir("sshlogin");
        let db = dir.join(DB_FILENAME);
        migrated_quasar_db(&db);
        let db = db.to_str().unwrap().to_string();
        let vault = vault::VaultState::new(db.clone());
        vault.initialize_vault(secrecy::SecretString::from("LivePassword123!")).await.unwrap();
        let creds = vault::CredentialManager::new(db);
        let add = |name: &str, ty: &str, password: &str, host: Option<&str>| {
            let vault = &vault;
            let creds = &creds;
            let (name, ty, password, host) = (name.to_string(), ty.to_string(), password.to_string(), host.map(str::to_string));
            async move {
                let access = vault.credential_access().await.unwrap();
                creds
                    .add_credential(access.key(), name, "admin".into(), password, ty, host, None, None, None, None, None)
                    .unwrap()
            }
        };
        let bound = add("bound", "ssh", "s3cret", Some("db1")).await;
        let key_only = add("key", "ssh_key", "", None).await;
        let api = add("api", "api", "token", None).await;

        let typed = resolve_ssh_login(&vault, &creds, None, "any", "me".into(), Some("pw".into())).await.unwrap();
        assert_eq!((typed.username.as_str(), typed.password.as_deref()), ("me", Some("pw")));

        let ok = resolve_ssh_login(&vault, &creds, Some(bound.clone()), "DB1", "ignored".into(), None).await.unwrap();
        assert_eq!((ok.username.as_str(), ok.password.as_deref()), ("admin", Some("s3cret")));
        assert!(resolve_ssh_login(&vault, &creds, Some(bound), "evil.example", "x".into(), None).await.is_err());

        let key = resolve_ssh_login(&vault, &creds, Some(key_only), "any", "x".into(), None).await.unwrap();
        assert_eq!(key.password, None, "an empty stored password means none");

        let err = resolve_ssh_login(&vault, &creds, Some(api), "any", "x".into(), None).await.err().unwrap();
        assert!(err.contains("SSH needs"), "{}", err);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// IPC-003: a monitoring binding needs an existing host and credential, and a credential
    /// restricted to another host can't be bound.
    #[test]
    fn monitoring_binding_respects_credential_host() {
        let mut conn = migrated_memory_db();
        let host = upsert_saved_host_in_conn(&mut conn, "srv", "10.0.0.5", "ssh", None, Some("root")).unwrap();
        let insert = |id: &str, bound: Option<&str>| {
            conn.execute(
                "INSERT INTO credentials (id, name, username, encrypted_password, nonce, tag, credential_type, host, created_at, updated_at)
                 VALUES (?1, ?1, 'u', X'00', zeroblob(12), zeroblob(16), 'password', ?2, 0, 0)",
                rusqlite::params![id, bound],
            )
            .unwrap();
        };
        insert("free", None);
        insert("bound-here", Some("10.0.0.5"));
        insert("bound-elsewhere", Some("evil.example"));
        assert!(check_credential_binding(&conn, &host.id, "free", vault::credentials::CredentialUse::Ssh).is_ok());
        assert!(check_credential_binding(&conn, &host.id, "bound-here", vault::credentials::CredentialUse::Ssh).is_ok());
        assert!(check_credential_binding(&conn, &host.id, "bound-elsewhere", vault::credentials::CredentialUse::Ssh).unwrap_err().contains("restricted"));
        assert!(check_credential_binding(&conn, &host.id, "missing", vault::credentials::CredentialUse::Ssh).is_err());
        assert!(check_credential_binding(&conn, "no-host", "free", vault::credentials::CredentialUse::Ssh).is_err());
    }

    /// IPC-007 / IPC-008: the shipped capability and CSP stay least-privilege.
    #[test]
    fn capability_and_csp_stay_least_privilege() {
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let perms: Vec<String> = cap["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().map(str::to_string).unwrap_or_else(|| p["identifier"].as_str().unwrap().to_string()))
            .collect();
        for banned in ["core:default", "core:event:default", "core:event:allow-emit", "core:event:allow-emit-to", "opener:default", "opener:allow-reveal-item-in-dir", "dialog:default"] {
            assert!(!perms.iter().any(|p| p == banned), "capability must not grant {}", banned);
        }

        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let csp = conf["app"]["security"]["csp"].as_str().unwrap();
        assert!(!csp.contains("localhost:*"), "production CSP must not allow arbitrary localhost ports: {}", csp);
        assert!(csp.contains("ipc:"), "production CSP must allow Tauri IPC: {}", csp);
    }

    #[test]
    fn migrations_apply_to_fresh_and_current_databases() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        let version = conn
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap();
        assert_eq!(version, 14);
        assert_eq!(version, LATEST_SCHEMA_VERSION, "update LATEST_SCHEMA_VERSION with MIGRATIONS");

        MIGRATIONS.to_latest(&mut conn).unwrap();
        let version_after_rerun = conn
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap();
        assert_eq!(version_after_rerun, version);
    }

    /// CLEAN-002: docs/ARCHITECTURE.md must list every registered command (CLAUDE.md once
    /// listed 21 that didn't exist). Reads `generate_handler!` from this file's source.
    #[test]
    fn architecture_doc_lists_every_command() {
        let source = include_str!("lib.rs");
        let doc = include_str!("../../docs/ARCHITECTURE.md");
        let start = source.find(concat!("generate_handler", "![")).expect("handler list") + 18;
        let list = &source[start..start + source[start..].find(']').unwrap()];
        let commands: Vec<&str> = list
            .split(',')
            .map(|c| c.trim().rsplit("::").next().unwrap_or(""))
            .filter(|c| !c.is_empty())
            .collect();
        assert!(commands.len() > 50, "parsed {} commands", commands.len());
        let missing: Vec<&str> = commands
            .into_iter()
            .filter(|c| !doc.contains(&format!("| `{}` |", c)))
            .collect();
        assert!(missing.is_empty(), "docs/ARCHITECTURE.md is missing commands: {:?}", missing);
    }

    /// RUST-018: docs/SCHEMA.md must describe every table and column the migrations create
    /// (a `### `table`` heading and a `| `column` |` row each), so it can't drift again.
    /// On failure it prints the tables as they should be documented.
    #[test]
    fn schema_doc_lists_every_table_and_column() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        let doc = include_str!("../../docs/SCHEMA.md");
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let mut missing = Vec::new();
        let mut expected = String::new();
        for table in &tables {
            let heading = format!("### `{}`", table);
            if !doc.contains(&heading) {
                missing.push(heading.clone());
            }
            expected.push_str(&format!("\n{}\n\n| Column | Type | Null | Default | Key |\n|---|---|---|---|---|\n", heading));
            let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{}\")", table)).unwrap();
            let cols = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, bool>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, i64>(5)?,
                    ))
                })
                .unwrap();
            for col in cols {
                let (name, ty, notnull, default, pk) = col.unwrap();
                let row = format!("| `{}` |", name);
                if !doc.contains(&heading) || !doc[doc.find(&heading).unwrap_or(0)..].contains(&row) {
                    missing.push(format!("{}.{}", table, name));
                }
                expected.push_str(&format!(
                    "{} {} | {} | {} | {} |\n",
                    row,
                    ty,
                    if notnull { "NOT NULL" } else { "" },
                    default.unwrap_or_default(),
                    if pk > 0 { "PK" } else { "" }
                ));
            }
        }
        assert!(missing.is_empty(), "docs/SCHEMA.md is missing {:?}. Current schema:\n{}", missing, expected);
    }

    #[test]
    fn database_recognition_accepts_quasar_application_id() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        set_quasar_application_id(&conn).unwrap();
        assert!(is_recognized_quasar_database(&conn).unwrap());
    }

    #[test]
    fn database_recognition_accepts_strict_legacy_schema() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE hosts (id TEXT, name TEXT, address TEXT, port INTEGER);
            CREATE TABLE credentials (id TEXT, name TEXT, username TEXT);
            CREATE TABLE vault_settings (key TEXT, value TEXT);
            CREATE TABLE security_audit_log (id TEXT, event_type TEXT, action TEXT);
            CREATE TABLE ssh_known_hosts (id TEXT, host TEXT, fingerprint TEXT);
            ",
        )
        .unwrap();
        assert!(is_recognized_quasar_database(&conn).unwrap());
    }

    #[test]
    fn database_recognition_rejects_unrelated_sqlite() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE random_table (id INTEGER);")
            .unwrap();
        assert!(!is_recognized_quasar_database(&conn).unwrap());
    }
}

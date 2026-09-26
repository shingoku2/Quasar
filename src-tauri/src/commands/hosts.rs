//! Saved hosts and their monitoring credentials.

use crate::*;

#[tauri::command]
pub(crate) async fn get_saved_hosts(app: AppHandle) -> Result<Vec<SavedHost>, String> {
    get_saved_hosts_from_db(&app).map_err(|e| sanitize_error(e, "database"))
}

#[tauri::command]
pub(crate) async fn upsert_saved_host(
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
pub(crate) async fn update_saved_host(
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
pub(crate) async fn remove_saved_hosts(app: AppHandle, ids: Vec<String>) -> Result<usize, String> {
    let mut conn = app_db_connection(&app).map_err(|e| sanitize_error(e, "database"))?;
    remove_saved_hosts_in_conn(&mut conn, &ids).map_err(|e| sanitize_error(e, "database"))
}

/// `set_host_monitoring_credential` used to bind any id to any host without checks; the
/// 30 s monitoring poll would then log in with it (IPC-003).
pub(crate) fn check_credential_binding(
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

#[tauri::command]
pub(crate) async fn get_remote_hosts_health(
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
pub(crate) async fn set_host_monitoring_credential(
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

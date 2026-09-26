//! Saved hosts (the `hosts` table): reading and writing them, and probing their health for
//! the Monitoring view.

use crate::db::app_db_connection;
use crate::validation::validate_port;
use crate::{health, vault};
use rusqlite::OptionalExtension;
use tauri::AppHandle;

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

pub(crate) fn get_saved_hosts_from_db(app: &AppHandle) -> Result<Vec<SavedHost>, String> {
    let conn = app_db_connection(app)?;
    get_saved_hosts_from_conn(&conn)
}

pub(crate) fn get_saved_hosts_from_conn(conn: &rusqlite::Connection) -> Result<Vec<SavedHost>, String> {
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

pub(crate) fn get_saved_host_by_id(
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

pub(crate) fn upsert_saved_host_in_conn(
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

pub(crate) fn update_saved_host_in_conn(
    conn: &rusqlite::Connection,
    host_id: &str,
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
    let updated = conn
        .execute(
            "UPDATE hosts SET name = ?1, address = ?2, protocol = ?3, port = ?4, username = ?5, updated_at = ?6 WHERE id = ?7",
            rusqlite::params![
                name,
                address,
                protocol,
                i64::from(port),
                username.filter(|value| !value.is_empty()),
                chrono::Utc::now().timestamp(),
                host_id,
            ],
        )
        .map_err(|e| e.to_string())?;

    if updated == 0 {
        return Err("Saved host was not found".to_string());
    }

    get_saved_host_by_id(conn, host_id)?
        .ok_or_else(|| "Saved host was not found after update".to_string())
}

/// A saved host whose address or port an edit would change, with the scheduled file transfers
/// that would follow it: (current address, current port, transfer count). `None` when the
/// edit keeps the destination or no transfer uses the host. A scheduled SFTP task's local
/// file was picked for one destination; moving the host moves that file access with it, so
/// the edit needs a native confirmation (PR #68 review).
pub(crate) fn transfers_moved_by_host_edit(
    conn: &rusqlite::Connection,
    host_id: &str,
    address: &str,
    protocol: &str,
    port: Option<u16>,
) -> Result<Option<(String, u16, usize)>, String> {
    let port = port.unwrap_or(if protocol.eq_ignore_ascii_case("rdp") { 3389 } else { 22 });
    let current: Option<(String, i64)> = conn
        .query_row("SELECT address, port FROM hosts WHERE id = ?1", [host_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((current_address, current_port)) = current else {
        return Ok(None);
    };
    if current_address.trim().eq_ignore_ascii_case(address.trim()) && current_port == i64::from(port) {
        return Ok(None);
    }
    let transfers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM scheduled_tasks WHERE host_id = ?1 AND task_type IN ('sftp_upload', 'sftp_download')",
            [host_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if transfers == 0 {
        return Ok(None);
    }
    Ok(Some((current_address, u16::try_from(current_port).unwrap_or(0), transfers as usize)))
}

pub(crate) fn remove_saved_hosts_in_conn(
    conn: &mut rusqlite::Connection,
    ids: &[String],
) -> Result<usize, String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut removed = 0;

    for chunk in ids.chunks(999) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!("DELETE FROM hosts WHERE id IN ({})", placeholders);
        let mut stmt = tx.prepare_cached(&sql).map_err(|e| e.to_string())?;
        removed += stmt
            .execute(rusqlite::params_from_iter(chunk.iter()))
            .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(removed)
}
/// Resolve hostname to an IP for ping. Returns None if resolution fails.
pub(crate) async fn resolve_to_ip(address: &str, port: i64) -> Option<String> {
    let port = port.clamp(1, 65535) as u16;
    tokio::net::lookup_host((address, port))
        .await
        .ok()
        .and_then(|mut addrs| addrs.next())
        .map(|sa| sa.ip().to_string())
}

/// How many hosts are probed at once by `get_remote_hosts_health`.
///
/// Each probe is a DNS lookup, an ICMP ping and (when a credential is linked) a
/// full SSH connect, so probing serially made the command's runtime scale with
/// the size of the inventory — on a 30 second poll that overlapped badly.
pub(crate) const MAX_CONCURRENT_HOST_CHECKS: usize = 16;

/// Probe a single host: resolve, ping, and optionally collect SSH metrics.
pub(crate) async fn probe_host(
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

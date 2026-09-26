//! Recognizing, importing and replacing the app database (RSEC-004, RUST-004, invariant 13).

use super::migrations::{LATEST_SCHEMA_VERSION, MIGRATIONS};
use super::DB_FILENAME;
use crate::errors::sanitize_error;
use crate::{db, monitoring, vault};

/// SQLite application_id marker for Quasar databases ("QSR1").
pub(crate) const QUASAR_APPLICATION_ID: i64 = 0x5153_5231;

fn has_required_columns(
    conn: &rusqlite::Connection,
    table: &str,
    required_columns: &[&str],
) -> Result<bool, String> {
    let sql = format!("PRAGMA table_info({})", table);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to inspect table '{}': {}", table, e))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| format!("Failed to read table info for '{}': {}", table, e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect columns for '{}': {}", table, e))?;
    Ok(required_columns.iter().all(|c| columns.iter().any(|name| name == c)))
}

pub(crate) fn has_quasar_legacy_signature(conn: &rusqlite::Connection) -> Result<bool, String> {
    let has_hosts = has_required_columns(conn, "hosts", &["id", "name", "address", "port"])?;
    let has_credentials = has_required_columns(conn, "credentials", &["id", "name", "username"])?
        || has_required_columns(conn, "credentials_new", &["id", "name", "username"])?;
    let has_vault_settings = has_required_columns(conn, "vault_settings", &["key", "value"])?;
    if !(has_hosts && has_credentials && has_vault_settings) {
        return Ok(false);
    }

    let optional_markers = [
        ("security_audit_log", &["id", "event_type", "action"] as &[&str]),
        ("ssh_known_hosts", &["id", "host", "fingerprint"]),
        ("metrics_history", &["id", "timestamp", "host"]),
        ("alert_history", &["id", "rule_id", "triggered_at"]),
        ("discovered_hosts", &["id", "ip", "last_seen"]),
        ("host_services", &["id", "host_id", "port"]),
        ("scheduled_tasks", &["id", "name", "cron_expression"]),
    ];
    let mut marker_count = 0usize;
    for (table, columns) in optional_markers {
        if has_required_columns(conn, table, columns)? {
            marker_count += 1;
        }
    }

    Ok(marker_count >= 2)
}

pub(crate) fn is_recognized_quasar_database(conn: &rusqlite::Connection) -> Result<bool, String> {
    let app_id = conn
        .pragma_query_value(None, "application_id", |row| row.get::<_, i64>(0))
        .unwrap_or(0);
    if app_id == QUASAR_APPLICATION_ID {
        return Ok(true);
    }
    has_quasar_legacy_signature(conn)
}

pub(crate) fn set_quasar_application_id(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.pragma_update(None, "application_id", QUASAR_APPLICATION_ID)
        .map_err(|e| format!("Failed to set application_id: {}", e))
}

/// One-time migration: rename titan.db to quasar.db for existing installs.
pub(crate) fn migrate_titan_db_to_quasar(app_dir: &std::path::Path) -> std::io::Result<()> {
    let old_path = app_dir.join("titan.db");
    let new_path = app_dir.join(DB_FILENAME);
    if old_path.exists() && !new_path.exists() {
        std::fs::rename(&old_path, &new_path)?;
    }
    Ok(())
}

/// Replaces the live database with `source_path` (after backing it up to `<db>.bak`).
///
/// Holds the credential gate exclusively and **locks the vault** first: the imported file
/// has its own salt/verifier, so the in-memory key belongs to the old database. Without
/// this, credentials written after an import were encrypted with the old key and lost at
/// the next unlock, and an import could interleave with a password change (RSEC-004).
/// Rejects databases from a newer schema, which used to brick startup (RUST-004).
pub(crate) async fn import_database_at(
    app_dir: &std::path::Path,
    source_path: &str,
    vault_state: &vault::VaultState,
    alert_engine: Option<&monitoring::AlertEngine>,
) -> Result<(), String> {
    let db_path = app_dir.join(DB_FILENAME);
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| "Invalid database path".to_string())?
        .to_string();

    // Validate that the source file is a readable SQLite database.
    if !std::path::Path::new(source_path).exists() {
        return Err("Source file not found".to_string());
    }
    let source_conn =
        rusqlite::Connection::open_with_flags(source_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "Source is not a valid SQLite database".to_string())?;
    source_conn
        .execute_batch("SELECT count(*) FROM sqlite_master;")
        .map_err(|_| "Source is not a valid SQLite database".to_string())?;
    if !is_recognized_quasar_database(&source_conn)? {
        return Err(
            "Source is a valid SQLite file but is not a recognized Quasar backup".to_string(),
        );
    }
    let source_version: i64 = source_conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    if source_version > LATEST_SCHEMA_VERSION {
        return Err(format!(
            "This backup is from a newer version of Quasar (schema {} > {}). Update Quasar before importing it.",
            source_version, LATEST_SCHEMA_VERSION
        ));
    }

    // In-memory state loaded from the old database must follow the new one: the alert rules
    // were read once at startup (PR #68 review). They are suspended for the replacement, so
    // the monitoring loop can't record old-rule alerts into the new file, and reloaded
    // afterwards whatever the outcome: a failed import leaves the old database (or restores
    // it), so the reload reads the right one either way.
    // Held until the reload: a tick that evaluated the old rules before the suspend can't
    // persist them into the new file in between.
    let _persistence = match alert_engine {
        Some(engine) => Some(engine.hold_persistence().await),
        None => None,
    };
    if let Some(engine) = alert_engine {
        engine.suspend();
    }
    let replaced = replace_database(app_dir, db_path, db_path_str, source_conn, vault_state).await;
    let reloaded = alert_engine.map(|engine| engine.reload());
    replaced?;
    if let Some(Err(e)) = reloaded {
        // The engine is left with no rules (never the pre-import ones); say so.
        return Err(format!(
            "The database was imported and the vault is locked (unlock it with the imported vault's master password), but its alert rules could not be loaded, so no alert rules are active: {}",
            sanitize_error(e, "database")
        ));
    }
    Ok(())
}

async fn replace_database(
    app_dir: &std::path::Path,
    db_path: std::path::PathBuf,
    db_path_str: String,
    source_conn: rusqlite::Connection,
    vault_state: &vault::VaultState,
) -> Result<(), String> {
    let exclusive = vault_state.lock_for_database_replacement().await?;
    let app_dir = app_dir.to_path_buf();
    // SQLite backup work is blocking; keep it off the async workers. The owned gate guard
    // moves into the blocking task and comes back, so the gate stays held until the restore
    // is done and the vault has forgotten the old database's lockout.
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let replaced = replace_database_blocking(&app_dir, &db_path, &db_path_str, source_conn);
        (replaced, exclusive)
    })
    .await;
    // Replaced, rolled back or failed midway: the next unlock must read the live database.
    vault_state.forget_database_state().await;
    let (replaced, _exclusive) = joined.map_err(|e| sanitize_error(e.to_string(), "database"))?;
    replaced
}

pub(crate) fn replace_database_blocking(
    app_dir: &std::path::Path,
    db_path: &std::path::Path,
    db_path_str: &str,
    source_conn: rusqlite::Connection,
) -> Result<(), String> {

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
        let src = db::open_connection(db_path_str)?;
        let mut dst = rusqlite::Connection::open(&backup_path)
            .map_err(|e| sanitize_error(e.to_string(), "database"))?;
        rusqlite::backup::Backup::new(&src, &mut dst)
            .map_err(|e| sanitize_error(e.to_string(), "database"))?
            .run_to_completion(100, std::time::Duration::from_millis(0), None)
            .map_err(|e| sanitize_error(e.to_string(), "database"))?;
        drop(dst);
        db::restrict_to_owner(&backup_path)?;
    }

    // Restore into the live database file in place.
    let src = source_conn;
    let mut dst = db::open_connection(db_path_str)?;
    rusqlite::backup::Backup::new(&src, &mut dst)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?
        .run_to_completion(100, std::time::Duration::from_millis(0), None)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    // An older backup must be brought to the current schema before the app touches it:
    // startup migrations don't run again, so a schema-14 backup would have no alert_rules
    // table until the next restart (PR #68 review). If migrating fails, put the previous
    // database back rather than leave the app on a schema it can't use.
    if let Err(e) = MIGRATIONS.to_latest(&mut dst) {
        let migrate_err = sanitize_error(format!("Failed to migrate imported database: {}", e), "database");
        if !backup_path.exists() {
            return Err(migrate_err);
        }
        // A failed restore must not read as "the previous vault is still live" (PR #68 review).
        return Err(match restore_previous_database(&backup_path, &mut dst) {
            Ok(()) => migrate_err,
            Err(restore_err) => {
                log::error!("Restoring the database after a failed import also failed: {}", restore_err);
                format!(
                    "The imported database could not be migrated, and the previous database could not be restored. It is saved as {}.bak in the app data folder: copy that file elsewhere, then import the copy to get it back.",
                    DB_FILENAME
                )
            }
        });
    }
    set_quasar_application_id(&dst).map_err(|e| sanitize_error(e, "database"))?;
    Ok(())
}

/// Copies the pre-import backup back over the live database.
pub(crate) fn restore_previous_database(
    backup_path: &std::path::Path,
    dst: &mut rusqlite::Connection,
) -> Result<(), String> {
    let prev = rusqlite::Connection::open_with_flags(backup_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    rusqlite::backup::Backup::new(&prev, dst)
        .and_then(|b| b.run_to_completion(100, std::time::Duration::from_millis(0), None))
        .map_err(|e| sanitize_error(e.to_string(), "database"))
}

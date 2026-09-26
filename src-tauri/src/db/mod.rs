//! Database connection utilities for Quasar
//!
//! This module provides centralized database connection management with
//! automatic foreign key constraint enforcement.

pub mod backup;
pub mod migrations;

use rusqlite::Connection;
use std::path::Path;
use std::time::Duration;
use tauri::Manager;

/// The app database's file name inside the app data directory. The one definition every
/// path to the database uses.
pub const DB_FILENAME: &str = "quasar.db";

/// Path of the app database inside `app_dir`, as a string (`open_connection` takes one).
pub fn db_path_in(app_dir: &Path) -> Result<String, String> {
    app_dir
        .join(DB_FILENAME)
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| "Invalid database path".to_string())
}

/// Path of the app database for a running app.
pub fn app_db_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<String, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    db_path_in(&app_dir)
}

/// How long a connection waits for a competing writer before returning SQLITE_BUSY.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens a SQLite database connection with foreign key constraints enabled.
///
/// Foreign keys are disabled by default in SQLite for backwards compatibility.
/// This function ensures they are always enabled for data integrity.
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
/// * `Ok(Connection)` - Database connection with foreign keys enabled
/// * `Err(String)` - Error message if connection or FK enablement fails
///
/// # Security
/// Enforces referential integrity per OWASP ASVS v4.0 5.1.4
///
/// # Examples
/// ```ignore
/// use quasar_lib::db;
/// let conn = db::open_connection("./quasar.db").unwrap();
/// ```
pub fn open_connection(db_path: &str) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("Failed to open database: {}", e))?;

    // Enable foreign key constraints
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("Failed to enable foreign keys: {}", e))?;

    // Write-Ahead Logging lets readers proceed while a writer holds the database,
    // which matters because monitoring, the scheduler, and the UI all hold their
    // own connections. The journal mode is persisted in the database file, so this
    // is a no-op after the first call. In-memory databases (tests) report "memory"
    // and ignore the request, which is fine — the result is intentionally unused.
    let _journal_mode: String = conn
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(|e| format!("Failed to set journal mode: {}", e))?;

    // NORMAL is the recommended durability level under WAL: it keeps the commit
    // fsync off the hot path while remaining crash-safe.
    conn.execute_batch("PRAGMA synchronous = NORMAL;")
        .map_err(|e| format!("Failed to set synchronous mode: {}", e))?;

    // Without a busy timeout, any lock contention surfaces immediately as
    // SQLITE_BUSY ("database is locked") instead of waiting for the writer.
    conn.busy_timeout(BUSY_TIMEOUT)
        .map_err(|e| format!("Failed to set busy timeout: {}", e))?;

    // Zero deleted content instead of leaving it in free space, so superseded secrets
    // (old vault verifiers, deleted credentials' ciphertext) don't linger in the file.
    // Per-connection, so it has to be set on every connection.
    conn.execute_batch("PRAGMA secure_delete = ON;")
        .map_err(|e| format!("Failed to enable secure_delete: {}", e))?;

    // The database holds the encrypted vault; don't leave it world-readable under a
    // permissive umask (audit RSEC-009). SQLite gives the -wal/-shm files the main file's
    // mode, but tighten them too in case they predate this.
    if db_path != ":memory:" && !db_path.starts_with("file:") {
        for path in [db_path.to_string(), format!("{}-wal", db_path), format!("{}-shm", db_path)] {
            let path = std::path::Path::new(&path);
            if path.exists() {
                if let Err(e) = restrict_to_owner(path) {
                    log::warn!("Could not restrict permissions on {}: {}", path.display(), e);
                }
            }
        }
    }

    Ok(conn)
}

/// Makes `path` accessible to the current user only: 0600 for files, 0700 for directories.
/// A no-op on non-Unix platforms, where the per-user app-data directory's ACLs apply.
pub fn restrict_to_owner(path: &std::path::Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if path.is_dir() { 0o700 } else { 0o600 };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_path_is_the_one_file_name_in_the_app_dir() {
        let dir = std::path::Path::new("/tmp/quasar-app");
        assert_eq!(db_path_in(dir).unwrap(), dir.join(DB_FILENAME).to_str().unwrap());
        assert!(db_path_in(dir).unwrap().ends_with("quasar.db"));
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let conn = open_connection(":memory:").unwrap();

        let fk_enabled: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();

        assert_eq!(fk_enabled, 1, "Foreign keys should be enabled");
    }

    #[test]
    fn test_wal_enabled_for_file_databases() {
        let dir = std::env::temp_dir().join(format!("quasar_wal_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("wal_test.db");
        let path_str = db_path.to_str().unwrap();

        let conn = open_connection(path_str).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal", "file databases should use WAL");

        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_secure_delete_enabled() {
        let conn = open_connection(":memory:").unwrap();
        let secure_delete: i32 = conn
            .query_row("PRAGMA secure_delete", [], |row| row.get(0))
            .unwrap();
        assert_eq!(secure_delete, 1, "secure_delete must be on for every connection");
    }

    /// RSEC-009 regression: the DB (encrypted vault) must not be readable by other users,
    /// even when it already exists with a permissive mode.
    #[cfg(unix)]
    #[test]
    fn test_database_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("quasar_perm_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("perm_test.db");
        std::fs::write(&db_path, b"").unwrap();
        std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let conn = open_connection(db_path.to_str().unwrap()).unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER);").unwrap();
        drop(conn);

        let mode = std::fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        restrict_to_owner(&dir).unwrap();
        let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_in_memory_database_still_opens() {
        // WAL is unsupported for :memory:; opening must still succeed.
        let conn = open_connection(":memory:").unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER);").unwrap();
    }
}

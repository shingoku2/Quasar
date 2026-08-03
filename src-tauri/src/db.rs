//! Database connection utilities for Quasar
//!
//! This module provides centralized database connection management with
//! automatic foreign key constraint enforcement.

use rusqlite::Connection;
use std::time::Duration;

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

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_in_memory_database_still_opens() {
        // WAL is unsupported for :memory:; opening must still succeed.
        let conn = open_connection(":memory:").unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER);").unwrap();
    }
}

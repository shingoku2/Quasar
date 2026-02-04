//! Database connection utilities for ProjectTitan
//! 
//! This module provides centralized database connection management with
//! automatic foreign key constraint enforcement.

use rusqlite::Connection;

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
/// ```no_run
/// use tauri_app_lib::db;
/// let conn = db::open_connection("./titan.db").unwrap();
/// ```
pub fn open_connection(db_path: &str) -> Result<Connection, String> {
    let conn = Connection::open(db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;
    
    // Enable foreign key constraints
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("Failed to enable foreign keys: {}", e))?;
    
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
}

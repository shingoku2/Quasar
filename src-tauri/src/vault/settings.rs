//! Vault settings (auto-lock timeout) as stored in `vault_settings`.

use super::*;

impl VaultState {
    pub async fn get_settings(&self) -> VaultSettings {
        let (db_path, in_memory) = {
            let inner = self.inner.read().await;
            (inner.db_path.clone(), inner.settings.clone())
        };
        // Load authoritative vault_initialized and auto_lock from DB so Settings page
        // shows correct state after app restart (in-memory settings default to false until init).
        let conn = match db::open_connection(&db_path) {
            Ok(c) => c,
            Err(_) => return in_memory,
        };
        let vault_initialized = conn
            .query_row(
                "SELECT value FROM vault_settings WHERE key = 'vault_initialized'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|v| v == "true")
            .unwrap_or(false);
        VaultSettings {
            vault_initialized,
            auto_lock_timeout_minutes: Self::load_auto_lock_minutes(&conn),
        }
    }

    /// Only the auto-lock timeout is caller-settable; `vault_initialized` is derived state
    /// and is ignored here (it used to be copied from the webview's struct wholesale).
    pub async fn update_settings(&self, settings: VaultSettings) -> Result<(), String> {
        let minutes = settings.auto_lock_timeout_minutes;
        if !AUTO_LOCK_MINUTES_RANGE.contains(&minutes) {
            return Err(format!(
                "Auto-lock timeout must be between {} and {} minutes",
                AUTO_LOCK_MINUTES_RANGE.start(),
                AUTO_LOCK_MINUTES_RANGE.end()
            ));
        }
        let mut inner = self.inner.write().await;
        let conn = db::open_connection(&inner.db_path)?;

        conn.execute(
            "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES ('auto_lock_timeout', ?1, ?2)",
            rusqlite::params![minutes.to_string(), chrono::Utc::now().timestamp()],
        )
        .map_err(|e| format!("Failed to update auto-lock timeout: {}", e))?;

        inner.settings.auto_lock_timeout_minutes = minutes;
        Ok(())
    }

    /// The persisted auto-lock timeout, clamped to the valid range (default 15).
    pub(super) fn load_auto_lock_minutes(conn: &rusqlite::Connection) -> u64 {
        conn.query_row(
            "SELECT value FROM vault_settings WHERE key = 'auto_lock_timeout'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|m| AUTO_LOCK_MINUTES_RANGE.contains(m))
        .unwrap_or(DEFAULT_AUTO_LOCK_MINUTES)
    }
}

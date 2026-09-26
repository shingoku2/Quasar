//! The unlock lockout, persisted in `vault_settings` so it survives a restart (invariant 5).

use super::*;

impl VaultState {
    /// Loads any lockout state persisted in `vault_settings` and folds it into
    /// the in-memory tracking. `VaultStateInner` starts fresh on every process
    /// launch, so without this an attacker with local access to the database
    /// file could bypass the escalating lockout below entirely — just by
    /// relaunching the app every 4 guesses to reset the in-memory counter.
    /// Only ever raises `inner`'s state (the DB is strictly more up to date
    /// than a fresh process), so this is safe to call unconditionally.
    pub(super) fn load_persisted_lockout(conn: &rusqlite::Connection, inner: &mut VaultStateInner) {
        let persisted_attempts: u32 = conn
            .query_row(
                "SELECT value FROM vault_settings WHERE key = 'lockout_failed_attempts'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if persisted_attempts > inner.failed_attempts {
            inner.failed_attempts = persisted_attempts;
        }

        let persisted_until_unix: Option<i64> = conn
            .query_row(
                "SELECT value FROM vault_settings WHERE key = 'lockout_until_unix'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok());

        if let Some(until_unix) = persisted_until_unix {
            let remaining_secs = until_unix - chrono::Utc::now().timestamp();
            if remaining_secs > 0 {
                let candidate = Instant::now() + Duration::from_secs(remaining_secs as u64);
                if inner.lockout_until.map(|c| candidate > c).unwrap_or(true) {
                    inner.lockout_until = Some(candidate);
                }
            }
        }
    }

    /// Persists lockout state so it survives an app restart. `lockout_until`
    /// is converted from the process-local monotonic clock to a wall-clock
    /// unix timestamp, since `Instant` has no meaning across process runs.
    pub(super) fn persist_lockout(
        conn: &rusqlite::Connection,
        failed_attempts: u32,
        lockout_until: Option<Instant>,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["lockout_failed_attempts", failed_attempts.to_string(), now],
        )
        .map_err(|e| format!("Failed to persist lockout state: {}", e))?;

        match lockout_until {
            Some(instant) => {
                let remaining = instant.checked_duration_since(Instant::now()).unwrap_or_default();
                let until_unix = now + remaining.as_secs() as i64;
                conn.execute(
                    "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                    rusqlite::params!["lockout_until_unix", until_unix.to_string(), now],
                )
                .map_err(|e| format!("Failed to persist lockout deadline: {}", e))?;
            }
            None => {
                conn.execute("DELETE FROM vault_settings WHERE key = 'lockout_until_unix'", [])
                    .map_err(|e| format!("Failed to clear lockout deadline: {}", e))?;
            }
        }

        Ok(())
    }
}

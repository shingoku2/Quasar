//! The schema migrations, in order. Append-only: never edit a shipped migration or add a
//! `002` (see CLAUDE.md, "Database").

use once_cell::sync::Lazy;
use rusqlite::Transaction;
use rusqlite_migration::{HookError, Migrations, M};

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

// Define migrations (001 → 003 → 004 → 005 → 006 → 007 → 008 → 009 → 010 → 011 → 012 → 013 → 014 → 015)
// The rusqlite_migration crate tracks applied migrations in user_version.
pub(crate) static MIGRATIONS: Lazy<Migrations> = Lazy::new(|| {
    Migrations::new(vec![
        M::up(include_str!("../../migrations/001_initial_schema.sql")),
        M::up(include_str!("../../migrations/003_security_vault.sql")),
        M::up(include_str!("../../migrations/004_monitoring.sql")),
        M::up(include_str!(
            "../../migrations/005_consolidate_credentials.sql"
        )),
        M::up(include_str!("../../migrations/006_discovered_hosts.sql")),
        M::up(include_str!(
            "../../migrations/007_monitoring_host_credential.sql"
        )),
        M::up(include_str!("../../migrations/008_ssh_key_credentials.sql")),
        M::up(include_str!("../../migrations/009_nullable_password.sql")),
        M::up(include_str!("../../migrations/010_scheduled_tasks.sql")),
        M::up_with_hook(
            "SELECT 1;",
            add_scheduled_task_run_result_columns_if_missing,
        ),
        M::up_with_hook("SELECT 1;", add_scheduled_tasks_sftp_columns_if_missing),
        M::up(include_str!("../../migrations/013_indexes.sql")),
        M::up(include_str!(
            "../../migrations/014_scheduled_tasks_cascade.sql"
        )),
        M::up(include_str!("../../migrations/015_alert_rules.sql")),
    ])
});


/// `user_version` after every migration in `MIGRATIONS` has run (one per entry). Kept in
/// step by `migrations_apply_to_fresh_and_current_databases`. `import_database` rejects
/// anything newer, which would otherwise fail `to_latest` and brick startup (RUST-004).
pub(crate) const LATEST_SCHEMA_VERSION: i64 = 14;

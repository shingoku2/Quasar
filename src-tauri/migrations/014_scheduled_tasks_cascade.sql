-- Migration 014: Add ON DELETE CASCADE to scheduled_tasks.host_id
-- Created: March 2026
--
-- SQLite does not support ALTER TABLE ... ADD/DROP CONSTRAINT.
-- We must recreate the table with the corrected FK definition.
-- Uses the standard SQLite rename-recreate-copy-drop pattern inside a
-- transaction so either the whole migration succeeds or the original table
-- is left intact.

PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE scheduled_tasks_new (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    cron_expression TEXT NOT NULL,
    host_id TEXT NOT NULL,
    command TEXT NOT NULL,
    credential_id TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_run_at INTEGER,
    last_run_status TEXT,
    last_run_error TEXT,
    last_run_output TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    task_type TEXT NOT NULL DEFAULT 'ssh',
    local_path TEXT,
    remote_path TEXT,
    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE CASCADE
);

INSERT INTO scheduled_tasks_new
    SELECT id, name, cron_expression, host_id, command, credential_id,
           enabled, last_run_at, last_run_status, last_run_error, last_run_output,
           created_at, updated_at, task_type, local_path, remote_path
    FROM scheduled_tasks;

DROP TABLE scheduled_tasks;
ALTER TABLE scheduled_tasks_new RENAME TO scheduled_tasks;

-- Restore indexes
CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_enabled  ON scheduled_tasks(enabled);
CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_host     ON scheduled_tasks(host_id);
CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_last_run ON scheduled_tasks(last_run_at);
CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_name     ON scheduled_tasks(name);

COMMIT;

PRAGMA foreign_keys = ON;

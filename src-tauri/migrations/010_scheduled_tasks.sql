-- Migration 010: Scheduled tasks (cron-based SSH command execution)
-- Created: February 18, 2026
-- Includes last_run_status, last_run_error, last_run_output so set_run_result works
-- even if migration 011 is not yet applied; 011 adds them idempotently for older DBs.

CREATE TABLE IF NOT EXISTS scheduled_tasks (
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
    FOREIGN KEY (host_id) REFERENCES hosts(id)
);

CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_enabled ON scheduled_tasks(enabled);
CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_host ON scheduled_tasks(host_id);

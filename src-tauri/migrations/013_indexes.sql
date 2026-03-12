-- Migration 013: Add missing indexes for performance
-- Created: March 2026

-- Index for querying/sorting scheduled tasks by last run time
CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_last_run ON scheduled_tasks(last_run_at);

-- Index for querying scheduled tasks by name (used in list display)
CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_name ON scheduled_tasks(name);

-- Migration 011: Last run result for scheduled tasks (success/failure, error, output)
-- Created: February 18, 2026

ALTER TABLE scheduled_tasks ADD COLUMN last_run_status TEXT;
ALTER TABLE scheduled_tasks ADD COLUMN last_run_error TEXT;
ALTER TABLE scheduled_tasks ADD COLUMN last_run_output TEXT;

-- Migration 012: Scheduled task type (SSH vs SFTP) and paths for file transfer
-- Created: February 18, 2026

ALTER TABLE scheduled_tasks ADD COLUMN task_type TEXT NOT NULL DEFAULT 'ssh';
ALTER TABLE scheduled_tasks ADD COLUMN local_path TEXT;
ALTER TABLE scheduled_tasks ADD COLUMN remote_path TEXT;

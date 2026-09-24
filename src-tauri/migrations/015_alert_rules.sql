-- Migration 015: persist alert rules (audit RUST-002)
-- Created: September 2026
--
-- Rules used to live only in memory and were lost on every restart. Each row holds one
-- rule as JSON (the serialized `monitoring::AlertRule`), so adding a field later doesn't
-- need a schema change.
CREATE TABLE IF NOT EXISTS alert_rules (
    id TEXT PRIMARY KEY,
    rule TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

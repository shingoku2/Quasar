-- Migration 007: Link saved hosts to a vault credential for SSH metrics in monitoring
-- Optional: when set, the monitoring task will use this credential to fetch CPU/memory/disk via SSH.

CREATE TABLE IF NOT EXISTS monitoring_host_credential (
    host_id TEXT PRIMARY KEY,
    credential_id TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_monitoring_host_credential_credential_id ON monitoring_host_credential(credential_id);

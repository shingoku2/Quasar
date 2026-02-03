-- Security & Credential Vault Database Schema
-- Migration 003: Add vault infrastructure tables

-- Enhanced Credentials Table
-- Note: Existing credentials table will be migrated to this schema
CREATE TABLE IF NOT EXISTS credentials_new (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    username TEXT NOT NULL,
    encrypted_password BLOB NOT NULL,
    nonce BLOB NOT NULL,
    tag BLOB NOT NULL,
    credential_type TEXT NOT NULL DEFAULT 'password',
    host TEXT,
    port INTEGER,
    metadata TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_used_at INTEGER
);

-- SSH Known Hosts Table
CREATE TABLE IF NOT EXISTS ssh_known_hosts (
    id TEXT PRIMARY KEY,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 22,
    key_type TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    public_key BLOB NOT NULL,
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    trust_status TEXT NOT NULL DEFAULT 'trusted',
    UNIQUE(host, port)
);

CREATE INDEX IF NOT EXISTS idx_known_hosts_lookup ON ssh_known_hosts(host, port);

-- Indexes for credentials table to improve search performance
CREATE INDEX IF NOT EXISTS idx_credentials_type ON credentials_new(credential_type);
CREATE INDEX IF NOT EXISTS idx_credentials_name ON credentials_new(name);

-- Security Audit Log Table
CREATE TABLE IF NOT EXISTS security_audit_log (
    id TEXT PRIMARY KEY,
    timestamp INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    resource_id TEXT,
    resource_type TEXT,
    action TEXT NOT NULL,
    result TEXT NOT NULL,
    details TEXT,
    user_context TEXT
);

CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON security_audit_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_resource ON security_audit_log(resource_type, resource_id);

-- Vault Settings Table
CREATE TABLE IF NOT EXISTS vault_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Insert default settings if not exists
INSERT OR IGNORE INTO vault_settings (key, value, updated_at) 
VALUES ('auto_lock_timeout', '15', strftime('%s', 'now'));

INSERT OR IGNORE INTO vault_settings (key, value, updated_at) 
VALUES ('require_password_on_credential_use', 'false', strftime('%s', 'now'));

INSERT OR IGNORE INTO vault_settings (key, value, updated_at) 
VALUES ('vault_initialized', 'false', strftime('%s', 'now'));

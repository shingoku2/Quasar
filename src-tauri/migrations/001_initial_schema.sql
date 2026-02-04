-- Migration 001: Initial database schema
-- Created: February 3, 2026

-- Hosts table for saved remote connections
CREATE TABLE IF NOT EXISTS hosts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    address TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 22,
    username TEXT,
    protocol TEXT NOT NULL DEFAULT 'ssh',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_hosts_name ON hosts(name);
CREATE INDEX IF NOT EXISTS idx_hosts_address ON hosts(address);

-- Credentials table (will be enhanced by later migrations)
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

CREATE INDEX IF NOT EXISTS idx_credentials_new_type ON credentials_new(credential_type);
CREATE INDEX IF NOT EXISTS idx_credentials_new_name ON credentials_new(name);

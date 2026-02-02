-- Migration 005: Consolidate credentials tables
-- Created: February 2, 2026

-- 1. Create the final credentials table if it doesn't exist
CREATE TABLE IF NOT EXISTS credentials (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    username TEXT NOT NULL,
    encrypted_password BLOB NOT NULL,
    nonce BLOB NOT NULL,
    tag BLOB NOT NULL,
    credential_type TEXT NOT NULL DEFAULT 'password',
    metadata TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_used_at INTEGER
);

-- 2. If credentials_new exists, migrate data from it to credentials
-- This handles the case where users already have data in credentials_new
INSERT OR IGNORE INTO credentials (
    id, name, username, encrypted_password, nonce, tag, 
    credential_type, metadata, created_at, updated_at, last_used_at
)
SELECT 
    id, name, username, encrypted_password, nonce, tag, 
    credential_type, metadata, created_at, updated_at, last_used_at
FROM credentials_new;

-- 3. Drop the temporary table
DROP TABLE IF EXISTS credentials_new;

-- 4. Re-create indexes for the consolidated table
CREATE INDEX IF NOT EXISTS idx_credentials_type ON credentials(credential_type);
CREATE INDEX IF NOT EXISTS idx_credentials_name ON credentials(name);

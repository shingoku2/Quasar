-- Migration 005: Consolidate credentials tables
-- Created: February 2, 2026

-- STRATEGY:
-- The backend is moving to a unified 'credentials' table with a specific schema.
-- An old 'credentials' table (created by frontend) or 'credentials_new' (intermediate) may exist.
-- 'credentials_new' contains the correctly encrypted data we want to keep.
-- The old 'credentials' table is incompatible and can be safely discarded/replaced.

-- 1. Create a temporary table to hold the valid data
CREATE TABLE IF NOT EXISTS credentials_temp (
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

-- 2. Copy valid data from credentials_new into temp table (if source exists)
INSERT OR IGNORE INTO credentials_temp (
    id, name, username, encrypted_password, nonce, tag, 
    credential_type, host, port, metadata, created_at, updated_at, last_used_at
)
SELECT 
    id, name, username, encrypted_password, nonce, tag, 
    credential_type, host, port, metadata, created_at, updated_at, last_used_at
FROM credentials_new;

-- 3. Drop ALL existing credential tables to clear the way
DROP TABLE IF EXISTS credentials;
DROP TABLE IF EXISTS credentials_new;

-- 4. Rename our temp table to be the final 'credentials' table
ALTER TABLE credentials_temp RENAME TO credentials;

-- 5. Re-create indexes
CREATE INDEX IF NOT EXISTS idx_credentials_type ON credentials(credential_type);
CREATE INDEX IF NOT EXISTS idx_credentials_name ON credentials(name);

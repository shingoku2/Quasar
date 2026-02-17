-- Migration 009: Allow clearing password when credential type is SSH key
-- Makes encrypted_password, nonce, tag nullable so update_credential can set them to NULL
-- when switching from password-based to ssh_key (empty string = clear, like key_path/private_key).

-- SQLite does not support ALTER COLUMN DROP NOT NULL; recreate table with nullable password columns.
CREATE TABLE credentials_new (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    username TEXT NOT NULL,
    encrypted_password BLOB,
    nonce BLOB,
    tag BLOB,
    credential_type TEXT NOT NULL DEFAULT 'password',
    host TEXT,
    port INTEGER,
    metadata TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_used_at INTEGER,
    key_path TEXT,
    encrypted_private_key BLOB,
    private_key_nonce BLOB,
    private_key_tag BLOB,
    encrypted_key_passphrase BLOB,
    key_passphrase_nonce BLOB,
    key_passphrase_tag BLOB
);

INSERT INTO credentials_new SELECT * FROM credentials;
DROP TABLE credentials;
ALTER TABLE credentials_new RENAME TO credentials;

CREATE INDEX IF NOT EXISTS idx_credentials_type ON credentials(credential_type);
CREATE INDEX IF NOT EXISTS idx_credentials_name ON credentials(name);

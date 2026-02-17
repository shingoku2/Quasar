-- Migration 008: SSH key credential support (key path or encrypted key material + optional passphrase)
-- Existing credentials keep encrypted_password; new columns are NULL for them.

ALTER TABLE credentials ADD COLUMN key_path TEXT;
ALTER TABLE credentials ADD COLUMN encrypted_private_key BLOB;
ALTER TABLE credentials ADD COLUMN private_key_nonce BLOB;
ALTER TABLE credentials ADD COLUMN private_key_tag BLOB;
ALTER TABLE credentials ADD COLUMN encrypted_key_passphrase BLOB;
ALTER TABLE credentials ADD COLUMN key_passphrase_nonce BLOB;
ALTER TABLE credentials ADD COLUMN key_passphrase_tag BLOB;

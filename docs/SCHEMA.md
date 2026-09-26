# Quasar database schema

A single SQLite database, `quasar.db`, in the platform app-data directory, owned and migrated by the Rust backend. Every connection goes through `db::open_connection()` (busy timeout, WAL, foreign keys, `secure_delete`, owner-only file permissions).

The column tables below are checked by the Rust test `schema_doc_lists_every_table_and_column` (`src-tauri/src/lib.rs`): it migrates a fresh database and fails if any table or column is missing here, printing the tables as they should read. After a schema change, run it and paste its output.

## Tables

### `alert_history`

Migration 004. One row per triggered alert; `acknowledged_at` / `dismissed_at` / `resolved_at` are set as the alert moves on.

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `id` | INTEGER |  |  | PK |
| `alert_id` | TEXT | NOT NULL |  |  |
| `rule_id` | TEXT | NOT NULL |  |  |
| `host` | TEXT | NOT NULL | 'localhost' |  |
| `message` | TEXT | NOT NULL |  |  |
| `severity` | TEXT | NOT NULL |  |  |
| `triggered_at` | INTEGER | NOT NULL |  |  |
| `acknowledged_at` | INTEGER |  |  |  |
| `dismissed_at` | INTEGER |  |  |  |
| `resolved_at` | INTEGER |  |  |  |

### `alert_rules`

Migration 015. `rule` is the JSON of `monitoring::AlertRule`. Loaded into `AlertEngine` at startup and written on every add/update/remove (validated by `monitoring::validate_rule`).

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `id` | TEXT |  |  | PK |
| `rule` | TEXT | NOT NULL |  |  |
| `updated_at` | INTEGER | NOT NULL |  |  |

### `credentials`

Consolidated by migrations 005 and 009 (from the legacy `credentials_new`), SSH-key columns from 008. Each secret is an AES-256-GCM ciphertext + 12-byte nonce + 16-byte tag triple; a triple is all NULL (absent) or all set, and a partial triple is an error, never "absent". `host`, when set, binds the credential to that host (`vault::credentials::host_allowed`). `key_path` is cleartext.

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `id` | TEXT |  |  | PK |
| `name` | TEXT | NOT NULL |  |  |
| `username` | TEXT | NOT NULL |  |  |
| `encrypted_password` | BLOB |  |  |  |
| `nonce` | BLOB |  |  |  |
| `tag` | BLOB |  |  |  |
| `credential_type` | TEXT | NOT NULL | 'password' |  |
| `host` | TEXT |  |  |  |
| `port` | INTEGER |  |  |  |
| `metadata` | TEXT |  |  |  |
| `created_at` | INTEGER | NOT NULL |  |  |
| `updated_at` | INTEGER | NOT NULL |  |  |
| `last_used_at` | INTEGER |  |  |  |
| `key_path` | TEXT |  |  |  |
| `encrypted_private_key` | BLOB |  |  |  |
| `private_key_nonce` | BLOB |  |  |  |
| `private_key_tag` | BLOB |  |  |  |
| `encrypted_key_passphrase` | BLOB |  |  |  |
| `key_passphrase_nonce` | BLOB |  |  |  |
| `key_passphrase_tag` | BLOB |  |  |  |

### `discovered_hosts`

Migration 006. Network scan results (`host_tracker.rs`).

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `id` | TEXT |  |  | PK |
| `ip` | TEXT | NOT NULL |  |  |
| `hostname` | TEXT |  |  |  |
| `mac_address` | TEXT |  |  |  |
| `device_type` | TEXT | NOT NULL | 'unknown' |  |
| `vendor` | TEXT |  |  |  |
| `first_seen` | INTEGER | NOT NULL |  |  |
| `last_seen` | INTEGER | NOT NULL |  |  |
| `scan_count` | INTEGER | NOT NULL | 1 |  |
| `metadata` | TEXT |  |  |  |

### `host_services`

Migration 006. Open ports per discovered host. `host_id` → `discovered_hosts(id)` ON DELETE CASCADE.

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `id` | TEXT |  |  | PK |
| `host_id` | TEXT | NOT NULL |  |  |
| `port` | INTEGER | NOT NULL |  |  |
| `protocol` | TEXT | NOT NULL |  |  |
| `service` | TEXT | NOT NULL |  |  |
| `version` | TEXT |  |  |  |
| `first_detected` | INTEGER | NOT NULL |  |  |
| `last_detected` | INTEGER | NOT NULL |  |  |

### `hosts`

Migration 001. Saved hosts. `protocol` is free-form (`ssh`, `rdp`, `database`, `api`, `other`); only `ssh` and `rdp` have an in-app client.

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `id` | TEXT |  |  | PK |
| `name` | TEXT | NOT NULL |  |  |
| `address` | TEXT | NOT NULL |  |  |
| `port` | INTEGER | NOT NULL | 22 |  |
| `username` | TEXT |  |  |  |
| `protocol` | TEXT | NOT NULL | 'ssh' |  |
| `created_at` | INTEGER | NOT NULL |  |  |
| `updated_at` | INTEGER | NOT NULL |  |  |

### `metrics_history`

Migration 004. Monitoring samples (`MetricsStore`), 30-day retention.

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `id` | INTEGER |  |  | PK |
| `timestamp` | INTEGER | NOT NULL |  |  |
| `host` | TEXT | NOT NULL | 'localhost' |  |
| `cpu_usage` | REAL |  |  |  |
| `memory_usage` | REAL |  |  |  |
| `disk_usage` | REAL |  |  |  |
| `network_rx` | INTEGER |  |  |  |
| `network_tx` | INTEGER |  |  |  |
| `network_packets_rx` | INTEGER |  |  |  |
| `network_packets_tx` | INTEGER |  |  |  |
| `load_avg_1m` | REAL |  |  |  |
| `load_avg_5m` | REAL |  |  |  |
| `load_avg_15m` | REAL |  |  |  |
| `process_count` | INTEGER |  |  |  |
| `uptime` | INTEGER |  |  |  |
| `metadata` | TEXT |  |  |  |

### `monitoring_host_credential`

Migration 007. Which credential the monitoring poll uses per host (`set_host_monitoring_credential`; the credential's `host` binding is enforced).

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `host_id` | TEXT |  |  | PK |
| `credential_id` | TEXT | NOT NULL |  |  |

### `scheduled_tasks`

Migration 010; run-result columns from 011 and `task_type`/`local_path`/`remote_path` from 012 (both Rust hooks); 014 rebuilt it so `host_id` → `hosts(id)` is ON DELETE CASCADE. `task_type` is `ssh`, `sftp_upload` or `sftp_download` (validated). There is no separate run-history table: the latest run is in `last_run_*`.

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `id` | TEXT |  |  | PK |
| `name` | TEXT | NOT NULL |  |  |
| `cron_expression` | TEXT | NOT NULL |  |  |
| `host_id` | TEXT | NOT NULL |  |  |
| `command` | TEXT | NOT NULL |  |  |
| `credential_id` | TEXT |  |  |  |
| `enabled` | INTEGER | NOT NULL | 1 |  |
| `last_run_at` | INTEGER |  |  |  |
| `last_run_status` | TEXT |  |  |  |
| `last_run_error` | TEXT |  |  |  |
| `last_run_output` | TEXT |  |  |  |
| `created_at` | INTEGER | NOT NULL |  |  |
| `updated_at` | INTEGER | NOT NULL |  |  |
| `task_type` | TEXT | NOT NULL | 'ssh' |  |
| `local_path` | TEXT |  |  |  |
| `remote_path` | TEXT |  |  |  |

### `security_audit_log`

Migration 003. Vault and credential events (`vault/audit.rs`), e.g. unlock failures, password changes, reveals, the KDF migration, scheduled-task changes.

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `id` | TEXT |  |  | PK |
| `timestamp` | INTEGER | NOT NULL |  |  |
| `event_type` | TEXT | NOT NULL |  |  |
| `resource_id` | TEXT |  |  |  |
| `resource_type` | TEXT |  |  |  |
| `action` | TEXT | NOT NULL |  |  |
| `result` | TEXT | NOT NULL |  |  |
| `details` | TEXT |  |  |  |
| `user_context` | TEXT |  |  |  |

### `ssh_known_hosts`

Migration 003. Pinned host keys. `trust_status` is `trusted`, `unknown`, `changed` or `rejected` (`vault::ssh_keys::TrustStatus`; one-time trust is never stored); lookups match `host` case-insensitively.

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `id` | TEXT |  |  | PK |
| `host` | TEXT | NOT NULL |  |  |
| `port` | INTEGER | NOT NULL | 22 |  |
| `key_type` | TEXT | NOT NULL |  |  |
| `fingerprint` | TEXT | NOT NULL |  |  |
| `public_key` | BLOB | NOT NULL |  |  |
| `first_seen_at` | INTEGER | NOT NULL |  |  |
| `last_seen_at` | INTEGER | NOT NULL |  |  |
| `trust_status` | TEXT | NOT NULL | 'trusted' |  |

### `vault_settings`

Migration 003. Key/value rows (see below).

| Column | Type | Null | Default | Key |
|---|---|---|---|---|
| `key` | TEXT |  |  | PK |
| `value` | TEXT | NOT NULL |  |  |
| `updated_at` | INTEGER | NOT NULL |  |  |

### `vault_settings` keys

| Key | Meaning |
|---|---|
| `vault_initialized` | `true` once a master password is set |
| `salt` | B64 16-byte Argon2id salt |
| `master_password_verifier` | Hex HKDF verifier, compared in constant time at unlock (v2) |
| `kdf_version` | `2`. Absent on a legacy v1 vault |
| `master_password_hash` | v1 only: a PHC string that equals the key (RSEC-001). Deleted by the v1→v2 migration on the next unlock |
| `legacy_salt` | Kept by the migration only when some credentials couldn't be decrypted (orphans), so they stay recoverable |
| `kdf_scrub_pending` | Set when the post-migration VACUUM/checkpoint scrub didn't finish; retried on every unlock |
| `auto_lock_timeout` | Minutes, 1-1440 |
| `lockout_failed_attempts`, `lockout_until_unix` | Persisted unlock lockout |

See `src-tauri/src/vault/kdf.rs` and CLAUDE.md Security Notes 10 and 15.

## Migrations

- Files live in `src-tauri/migrations/` and are registered, in order, in `MIGRATIONS` in `src-tauri/src/lib.rs`. `rusqlite_migration` tracks them by **position** in that list (`user_version`), not by filename.
- There is no `002_*.sql` (an early draft was merged into 003). Never create one: it would shift every later index and corrupt existing databases.
- **The next migration is the highest existing number + 1** (currently `016_`). Append it to `MIGRATIONS` and bump `LATEST_SCHEMA_VERSION` (a test checks they match).
- Migrations are append-only: never edit one that has shipped.
- SQLite has no `ADD COLUMN IF NOT EXISTS`. To add a column that may already exist, use a Rust hook (`M::up_with_hook("SELECT 1;", hook)`) that checks `PRAGMA table_info` first, as 011 and 012 do (`add_scheduled_task_run_result_columns_if_missing`, `add_scheduled_tasks_sftp_columns_if_missing`). To change a constraint, rebuild the table (see 014).
- `import_database` refuses a backup whose `user_version` is newer than `LATEST_SCHEMA_VERSION`, and runs the migrations on an older one before returning (the startup migrations don't run again).

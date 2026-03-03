# Quasar database schema

The application uses a single SQLite database (`quasar.db`) owned and migrated by the Rust backend. The schema is **consolidated**: one source of truth per entity.

## Canonical tables

| Table | Purpose |
|-------|--------|
| `hosts` | Saved remote hosts (address, port, username, protocol). |
| `credentials` | Encrypted credentials (password or SSH key). Consolidated from legacy `credentials_new`; single table post-migration 005/009. |
| `vault_settings` | Vault master password hash, salt, auto-lock config. |
| `security_audit_log` | Audit trail for vault and credential operations. |
| `ssh_known_hosts` | SSH host key verification (fingerprints, trust status). |
| `discovered_hosts` | Network scanner results (IP, hostname, device type, etc.). |
| `host_services` | Services/ports per discovered host. |
| `scheduled_tasks` | Cron-scheduled tasks (SSH command or SFTP upload/download). |
| Monitoring/alert tables | As defined in migrations 004 and 007. |

## Consolidation notes

- **Credentials**: Migrations 005 and 009 ensure a single `credentials` table with encryption and nullable password columns. The backend uses only `credentials`; there is no runtime use of `credentials_new`.
- **Hosts**: Single `hosts` table (created in 001); frontend and backend both use it for saved hosts.
- Migrations run in order via `rusqlite_migration`; see `src-tauri/src/lib.rs` for the migration list.
- **Migrations 011 and 012** are applied by Rust hooks (not raw SQL) so they are idempotent: they add columns to `scheduled_tasks` only if missing (010 may already create the table with those columns on fresh installs).

## Migration numbering gap

Migration file `002_*.sql` does not exist — the sequence jumps from `001_initial_schema.sql` to `003_security_vault.sql`. This is intentional: `rusqlite_migration` tracks applied migrations by their **position** in the registered vector, not by filename. The filename numbering is for developer reference only. The gap exists because an early draft of migration 002 was merged into 003. **Do not create a new file named `002_*.sql`**: doing so would shift every subsequent migration index and corrupt existing databases. The next migration to add must be numbered `013_`.

## Error handling

- **Backend**: Tauri commands return `Result<T, String>`. Internal errors are passed through `errors::sanitize_error()` so the frontend receives safe, generic messages while full details are logged server-side.
- **Critical paths**: No `.unwrap()` or `.expect()` in production code paths; tests use them for assertions. Mutex/poison and main entry-point panics are the only remaining acceptable panics.

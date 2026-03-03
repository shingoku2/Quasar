# Security Reviewer Agent Memory
# See patterns.md for detailed findings per module.

## Confirmed Safe Patterns (as of 2026-03-02 audit)
- Argon2id params: 47104 KiB memory, 2 iterations, 1 parallelism, 32-byte output — CORRECT in crypto.rs, vault.rs (initialize, unlock, change_password).
- AES-256-GCM nonces: generated via `rand::rng().fill_bytes()` (CSPRNG) in crypto.rs — CORRECT, unique per call.
- MasterKey struct implements Drop+Zeroize in vault.rs — CORRECT.
- `secrecy::Secret<String>` used for master_password in initialize_vault/unlock_vault — CORRECT.
- All user-facing Tauri commands pass errors through `sanitize_error()` for vault, credential, ssh, sftp, scanner, database, monitoring contexts.
- Host key verification: `check_server_key` in both ssh.rs and sftp.rs returns `Err(Disconnect)` for untrusted/unknown keys — NOT bypassed.
- SFTP host key verification is active (not skipped).
- SQL queries use parameterized rusqlite::params! — no string interpolation in credential/vault queries.
- Argon2 verify_password uses constant-time comparison internally (argon2 crate).

## Known Vulnerabilities Found (2026-03-02 Broad Audit)
See patterns.md for full details with file:line citations.

HIGH:
1. Credential struct serialized with private_key+key_passphrase fields — decrypted secrets sent to frontend over IPC.
2. `rand::rng()` used instead of OsRng in crypto::encrypt — non-CSPRNG nonce generation risk (requires verification).
3. Fingerprint comparison in ssh_keys.rs uses `==` not constant-time — timing oracle on host key DB.
4. `export_database` path injection — single-quote escape is insufficient (backslash bypass on some platforms).
5. `scan_error` events emit raw internal error strings to frontend (lib.rs ~329).
6. `change_master_password` passes current_password as plain String (not Secret) into spawn_blocking — appears in memory uninstrumented.

MEDIUM:
1. Several Tauri commands (set_host_monitoring_credential, export_database, import_database, clear_metrics_data, get_app_info) return raw .to_string() / format! errors without sanitize_error().
2. Decrypted Credential (with private_key/key_passphrase) returned to frontend by get_credential Tauri command.
3. Intermediate [u8;32] master_key buffers on stack in initialize_vault/unlock_vault not explicitly zeroized before drop.
4. ssh_auth.rs parse_private_key error leaks "Invalid SSH key: {e}" — may contain PEM parse detail.

LOW:
1. `verify_password` in crypto.rs wraps argon2 verify returning bool — fine for non-secret comparison but the function is used for non-master-password hashing (distinct from vault unlock path).

## Recurring Issue Pattern
- Raw `.map_err(|e| e.to_string())` used in several Tauri commands instead of sanitize_error() — check new commands carefully.
- Credential struct has sensitive fields and is Serialize — always verify it is not returned directly to frontend unless intentional.

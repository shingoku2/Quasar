# Security Reviewer Agent Memory
# See patterns.md for detailed findings per module (2026-03-02 audit — several items below
# are now fixed; each is annotated where confirmed). See vault_rs_patterns.md for the
# vault.rs locking/concurrency model (2026-08-22 review of the change_master_password
# lock-holding fix).

## Confirmed Safe Patterns (as of 2026-03-02 audit; re-confirmed 2026-08-22 where noted)
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
1. ~~Credential struct serialized with private_key+key_passphrase fields — decrypted secrets sent to frontend over IPC.~~ **FIXED, confirmed 2026-08-22**: `get_credential`'s Tauri command now returns `CredentialFrontendView` (`vault/credentials.rs`), which omits `private_key`/`key_passphrase` entirely and exposes only `has_private_key`/`has_key_passphrase` booleans. This is now the established correct pattern — see the checklist item in `.claude/agents/security-reviewer.md` and `CLAUDE.md`. Do not flag a frontend consumer reading those booleans as a vuln.
2. `rand::rng()` used instead of OsRng in crypto::encrypt — non-CSPRNG nonce generation risk (requires verification — NOT re-checked 2026-08-22, this session didn't touch crypto.rs).
3. ~~Fingerprint comparison in ssh_keys.rs uses `==` not constant-time — timing oracle on host key DB.~~ **FIXED, confirmed 2026-08-22**: `ssh_keys.rs` now imports and uses `subtle::ConstantTimeEq`.
4. `export_database` path injection — single-quote escape is insufficient (backslash bypass on some platforms). NOT re-checked 2026-08-22.
5. `scan_error` events emit raw internal error strings to frontend (lib.rs ~329). NOT re-checked 2026-08-22.
6. `change_master_password` passes current_password as plain String (not Secret) into spawn_blocking — appears in memory uninstrumented. NOT re-checked 2026-08-22 (this session's vault.rs review was scoped to the lock-holding refactor, not a fresh full pass — see vault_rs_patterns.md).

MEDIUM:
1. Several Tauri commands (set_host_monitoring_credential, export_database, import_database, clear_metrics_data, get_app_info) return raw .to_string() / format! errors without sanitize_error(). NOT re-checked 2026-08-22.
2. ~~Decrypted Credential (with private_key/key_passphrase) returned to frontend by get_credential Tauri command.~~ Same as HIGH #1 above — FIXED, confirmed 2026-08-22.
3. Intermediate [u8;32] master_key buffers on stack in initialize_vault/unlock_vault not explicitly zeroized before drop. NOT re-checked 2026-08-22 — vault_rs_patterns.md notes the same class of issue (plain `[u8;32]` vs `Zeroizing<[u8;32]>`) still present in change_master_password as of that review.
4. ssh_auth.rs parse_private_key error leaks "Invalid SSH key: {e}" — may contain PEM parse detail. NOT re-checked 2026-08-22.

**Note (2026-08-22):** two of the six HIGH findings above turned out to be already fixed by the
time this note was written — this list was stale relative to the actual codebase. Before citing
any *unconfirmed* item here as a live finding in a new review, re-verify it against current code
rather than trusting this list at face value; it was last given a full pass on 2026-03-02.

LOW:
1. `verify_password` in crypto.rs wraps argon2 verify returning bool — fine for non-secret comparison but the function is used for non-master-password hashing (distinct from vault unlock path).

## Recurring Issue Pattern
- Raw `.map_err(|e| e.to_string())` used in several Tauri commands instead of sanitize_error() — check new commands carefully.
- Credential struct has sensitive fields and is Serialize — always verify it is not returned directly to frontend unless intentional.

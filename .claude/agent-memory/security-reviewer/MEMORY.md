# Security Reviewer Agent Memory

Updated 2026-09-25 after the full audit (`AUDIT.md`, Phase 7 / EDW-26). Topic files:
- `vault_rs_patterns.md`: vault locking/concurrency model and the Argon2 baseline.
- `patterns.md`: the `claim_scan()` TOCTOU lesson (top section: claim synchronously before
  `tokio::spawn`, never inside the spawned task) and the historical 2026-03-02 findings list.
- `CLAUDE.md` Security Notes 1-19 are the authoritative list of invariants and their
  regression tests. If this file disagrees with them, they win; fix this file.

## Established safe patterns (verified 2026-09-25)
- Vault KDF v2 (`vault/kdf.rs`): one Argon2id pass (47104 KiB, t=2, p=1) → HKDF-SHA256 into
  the AES key (memory only) and a verifier (constant-time compare). No password hash stored.
- AES-256-GCM nonces come from `getrandom::fill` (OS CSPRNG), fresh per encryption.
- Credential access goes through `credential_access()` / `credential_access_background()`
  (shared `credential_gate`); rotation, v1 migration and DB import hold it exclusively.
- `get_credential` returns `CredentialFrontendView`: `has_*` flags only, no secrets. Plaintext
  passwords cross IPC only via `reveal_credential_password`, behind a native confirmation.
- Host keys: interactive trust is by pending request id only; non-interactive paths (exec, pool,
  SFTP) use `SshKeyManager::check_non_interactive` (trusted key for that host+port or refuse).
  Fingerprints come from `vault::ssh_keys::presented_key_bytes` (russh 0.63 certificates pin
  the certified key). Fingerprint comparison is constant-time.
- Local file paths for SFTP/export/import come only from backend dialogs (`LocalPathGrants`,
  single-use, intent-bound).
- DB export uses the rusqlite backup API (not `VACUUM INTO` string interpolation).
- `scan_error` events and SFTP/pool/scheduled SSH errors go through `sanitize_error()`. The
  interactive terminal connect path returns raw diagnostics by design (no credential material).
- Master/current/new passwords are `SecretString` through `change_master_password`.

## Fixed since the 2026-03-02 list (don't re-report)
Decrypted secrets in `get_credential`; non-constant-time fingerprint compare; `rand::rng()`
nonces; `export_database` path injection; raw `scan_error` strings; plain `String` passwords in
`change_master_password`; the credential-write/rekey race (EDW-15).

## Still worth checking in new code
- New Tauri commands returning `.map_err(|e| e.to_string())` without `sanitize_error()` or
  `errors::user_facing_vault_error`.
- Plain `[u8; 32]` key temporaries instead of `Zeroizing<...>`.
- Any new "drop the lock, do slow work, reacquire and apply" code: re-check state on reacquire.
- Any webview `confirm()` placed in front of a security decision (use `native_confirm`).

# Detailed Security Findings — 2026-03-02 Broad Audit

## crypto.rs
- Line 45: `rand::rng()` instead of OsRng. rand::rng() returns a thread-local CSPRNG seeded from OsRng on first use. This is likely fine but is not as explicit/auditable as using OsRng directly.
- Argon2id params confirmed correct (47104, 2, 1, Some(32)).
- AES-256-GCM tag split from ciphertext_with_tag at pos len-16 — correct.
- verify_password uses argon2 constant-time internally — OK.

## vault.rs
- MasterKey has Drop+Zeroize — GOOD.
- Secret<String> used for master_password in unlock/initialize — GOOD.
- change_master_password: current_password passed as &str, then cloned to String at line 346 for spawn_blocking. Not wrapped in Secret, not zeroized. LOW-MEDIUM risk.
- Intermediate master_key [u8;32] on stack in initialize_vault (line 155) and unlock_vault (line 236) not zeroized after stored into MasterKey struct. These temporary stack arrays persist until overwritten by compiler.
- new_master_key [u8;32] at line 384 sent back through Ok() return of spawn_blocking — exists in memory as plain bytes.
- Re-encryption transaction is atomic — GOOD.
- Validation before commit (line 450 decrypt test) — GOOD.
- changing_password flag reset on all paths — GOOD.

## vault/credentials.rs
- Credential struct derives Serialize — private_key, key_passphrase, password fields are all included in serialized output.
- get_credential Tauri command returns full Credential including decrypted private_key — intentional but worth noting.
- SQL uses parameterized queries — GOOD.
- decrypt_optional_blob validates nonce/tag lengths before copy_from_slice — GOOD.
- Decrypted bytes not zeroized after String conversion — MEDIUM.

## vault/ssh_keys.rs
- fingerprint comparison at line 123: `known.fingerprint == fingerprint` — uses Eq trait, which is NOT constant-time for String. Timing oracle on known_hosts lookup.
- Trust/reject logic correct — unknown hosts return allowed:false.
- No bypass of verification found.

## vault/audit.rs
- Dynamic query construction with user-supplied filter values uses parameterized ? placeholders — GOOD, not SQL injection.
- No secret data in audit log fields.

## ssh_auth.rs
- parse_private_key error: `format!("Invalid SSH key: {}", e)` at line 31 — may expose PEM structure in error.
- No key material logged.
- authenticate() falls through from key to password on key failure — correct fallback behavior.

## ssh.rs
- check_server_key returns Err(Disconnect) for untrusted — GOOD.
- Raw russh error may leak via e.to_string() in connect_ssh lines 130,141 — but these return to the Tauri command layer which calls sanitize_error in lib.rs.
- connect_ssh validation: validates port, username, host format — GOOD.
- Session ID is caller-supplied String — if attacker controls IDs could overwrite sessions; Tauri IPC trust model makes this lower risk.

## sftp.rs
- Host key verification active — GOOD.
- SFTP only supports password auth (require_password_auth enforces) — documented limitation.
- remote_path is passed directly to sftp.create/sftp.read_dir — no client-side path traversal validation. Server enforces chroot.
- local_path in download_file: fs::File::create(local_path) — path traversal to local filesystem possible if attacker controls local_path.
- upload_file checks Path::new(local_path).exists() but does not canonicalize — symlink attacks possible.
- No shell injection (russh-sftp uses SFTP protocol, not shell).

## scanner.rs
- CIDR validation before use — GOOD.
- No shell commands constructed from CIDR/IP.
- IPs used only in TcpStream::connect and surge_ping — safe.
- scan_error event in lib.rs line 329 emits raw error string e to frontend.
- scan_error at line 318 emits format!("Failed to save discovered host {}: {}", result.ip, e) — includes internal DB error.

## validation.rs
- IP regex: `^((25[0-5]|(2[0-4]|1\d|[1-9]|)\d)\.?\b){4}$` — anchored, correct.
- CIDR regex: anchored — correct.
- Hostname regex: anchored — correct.
- Username regex: anchored, allows a-zA-Z0-9_- — correct.
- validate_password: only checks length ≥ 8 — no complexity requirement. Acceptable for credential storage.
- validate_master_password: only checks length ≥ 12 — no complexity. Acceptable given Argon2id hardening.

## errors.rs
- sanitize_error logs full error server-side, returns generic message — GOOD.
- All known security-critical Tauri commands use it.
- Missing from: set_host_monitoring_credential, export_database, import_database, clear_metrics_data, get_app_info.

## lib.rs
- export_database VACUUM INTO path: uses single-quote escape only (replace ' with ''). On some SQLite builds or platforms with unusual filenames this may not fully prevent injection. Safer: use rusqlite's backup API.
- get_credential returns full decrypted Credential struct to frontend — intentional design but exposes private_key over IPC.

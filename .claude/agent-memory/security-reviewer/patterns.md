# scanner.rs — claim_scan() / TOCTOU pattern (2026-09-17 review)

Reviewed a fix for a real TOCTOU race in `scan_network`: the `lib.rs` command does
`tokio::spawn(async move { ...scanner::scan_network(...).await... })` and returns
immediately. If the atomic "check `is_scanning`, set it, reset `stop_signal`" claim
happens *inside* the spawned task (as it originally did, and as a bot-authored PR's
"fix" — which only reordered two locks already in the same critical section — left it),
a `stop_scan()` call landing in the window between `tokio::spawn(...)` and the task
actually starting sets `stop_signal = true`, only for the still-starting task to
unconditionally reset it back to `false` when it reaches its own claim. Silent loss of
the user's stop request.

**Fix pattern, confirmed correct**: extract the claim into a standalone
`scanner::claim_scan(state) -> Result<(), String>` and call it *synchronously in the
command handler, before `tokio::spawn`* — not from within the spawned async fn. The
spawned function must then assume the claim already happened, and must install its
`Drop`-based release guard (`ScanRunningGuard`) as its very first statement, before any
fallible work (e.g. `validate_cidr`), so an early `?` return still releases the claim.

**Verified, not just assumed**:
- Single call site (`lib.rs:scan_network` command) via full-crate grep — the module is
  private (`mod scanner;`, not `pub mod`), so nothing else can reach `scan_network`
  without going through the command that now calls `claim_scan` first.
- No deadlock risk from the lock-acquisition order inside `claim_scan` (`stop_signal`
  then `is_scanning`) — grepped for other sites locking both mutexes together; this is
  the only one, so ordering is irrelevant across call sites.
- Frontend (`NetworkScanner.tsx::handleStartScan`) already does
  `await invoke('scan_network', ...)` in try/catch and only sets `isScanning(true)` on
  success — so `claim_scan`'s `Err` (e.g. "Scan already in progress") now surfacing as
  a direct command rejection, instead of only via a delayed `scan_error` event as
  before, is handled correctly with no frontend changes needed. This is worth checking
  again if the command's error-return behavior changes further.

**One real gap found and fixed**: the new `claim_scan()` error path in the `lib.rs`
command initially bypassed `sanitize_error()`, while every other error path in the same
function used it (the two `Err` branches inside the spawned task, three lines later).
Low actual risk (the only error strings are "Scan already in progress" or a generic
lock-poison message — no paths, SQL, or internal state), but inconsistent with the
project's own convention. Fixed: `scanner::claim_scan(&scanner_state).map_err(|e|
sanitize_error(e, "scanner"))?;`.

**One residual noted, not fixed (very low priority)**: if the spawned future is somehow
never polled (realistic trigger: Tokio runtime shutdown before first poll, i.e. app
exit — essentially moot), `is_scanning` would stay `true` forever, since the only thing
that resets it (`ScanRunningGuard`) lives inside `scan_network`, which never got
entered. Previously this same scenario was harmless (the claim lived inside the
never-polled task, so was simply never taken). Not worth guarding against unless this
function gets a second caller.

**Unrelated but relevant if this file changes again**: `scan_network` also unconditionally
opened a raw ICMP `Client` (`surge_ping`) before its per-host loop's stop-signal check.
On a sandboxed CI runner without `CAP_NET_RAW`/`ping_group_range`, that fails outright
with "Permission denied creating ICMP socket" — this actually broke a regression test in
CI (locally the sandbox had raw-socket capability, masking it). Added a stop-signal check
before the `Client::new(...)` call, not just inside the loop, so a scan that's already
been asked to stop never attempts the privileged socket open at all.

---

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

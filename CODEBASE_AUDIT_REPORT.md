# Quasar Codebase Audit Report

**Date:** February 18, 2026  
**Scope:** Full codebase (src/, src-tauri/, migrations/, config, tests)  
**Fixes applied:** February 18, 2026 — all High, Medium, and applicable Low items addressed (see "Fixes applied" section at end).

Findings are ordered by severity (Critical → High → Medium → Low) and category. Each entry includes location, description, and recommended fix.

---

## Critical

*No critical bugs identified.* The codebase uses parameterized SQL, validates inputs before SSH/launcher, and sanitizes errors for the frontend. SSH host key verification is implemented (ssh.rs, ssh_exec.rs, sftp.rs use SshKeyManager).

---

## High

### H-1. Monitoring: Mutex poison can panic in production

**Location:** `src-tauri/src/monitoring.rs` (lines 396, 402, 407, 411, 414–416)  
**Issue:** `self.rules.lock().unwrap()`, `self.alert_counter.lock().unwrap()`, etc. If any thread panics while holding the lock, the mutex becomes poisoned and subsequent `.unwrap()` will panic in the monitoring task.  
**Fix:** Use `lock().unwrap_or_else(|e| e.into_inner())` to recover from poison, or handle `Err` and log/restart the monitoring loop instead of unwrapping.

### H-2. Discovery: Event listener leak if unmount during async setup

**Location:** `src/components/Discovery.tsx` (useEffect with `startScan()`)  
**Issue:** `startScan()` is not awaited. Cleanup runs when the component unmounts; if unmount happens before `startScan()` completes the first `await listen(...)`, `unlistenMdns` / `unlistenScanComplete` may still be undefined, so listeners are not removed.  
**Fix:** Use a ref or flag set in cleanup and, inside `startScan()`, check it after each await; if set, call unlisten immediately and return. Alternatively, await a “setup complete” signal and store unlisten functions in a ref so cleanup can always call them.

### H-3. MonitoringView: system-metrics unlisten may not run if promise not resolved

**Location:** `src/components/MonitoringView.tsx` (lines 104–144)  
**Issue:** `unlisten` is assigned the Promise returned by `listen(...)`. Cleanup does `unlisten.then(fn => fn())`. If the component unmounts before the promise resolves, the listener may remain until the promise resolves (minor leak / late cleanup).  
**Fix:** Store the Promise in a ref and in cleanup use a flag; when the promise resolves, check the flag and only call the unlisten function if the component is still mounted (or always call to be safe).

---

## Medium

### M-1. Scanner state: Mutex unwrap in tests and production paths

**Location:** `src-tauri/src/scanner.rs` (e.g. 476, 483, 489, 508, 522, 657, 660)  
**Issue:** `state.is_scanning.lock().unwrap()`, `state.results.lock().unwrap()`, etc. Poisoning can cause panic.  
**Fix:** Prefer `lock().unwrap_or_else(|e| e.into_inner())` or handle the error and log/abort the operation.

### M-2. AGENTS.md: Outdated “SSH Server Key Validation” and “terminal resize observer” notes

**Location:** `AGENTS.md` (e.g. ~881 “Potential memory leak in terminal resize observer”, ~907 “SSH connections accept any server key”)  
**Issue:** TerminalComponent cleanup calls `resizeObserver.disconnect()`. SSH uses SshKeyManager and host key verification in ssh.rs, ssh_exec.rs, sftp.rs. Docs still describe old behavior.  
**Fix:** Update AGENTS.md: remove or reword the terminal observer leak note; update SSH section to state that host key verification is implemented via SshKeyManager.

### M-3. MVP_COMPLETION_ROADMAP.md: Stale path to automation engine

**Location:** `MVP_COMPLETION_ROADMAP.md` (line 66)  
**Issue:** References `src-tauri/src/automation/engine.rs` for FileTransfer; that path does not exist. Execution is in scheduler + sftp/ssh_exec.  
**Fix:** Replace with a reference to scheduler.rs + sftp.rs (and ssh_exec.rs) for scheduled SFTP/SSH execution.

### M-4. Main entry point .expect

**Location:** `src-tauri/src/lib.rs` (line 1243)  
**Issue:** `.expect("error while running tauri application")` on `run()`. Standard for Tauri apps; if run fails, process exits.  
**Fix:** Optional: log before panic or use a custom panic hook. Low priority.

### M-5. Validation.rs: Regex::new().unwrap()

**Location:** `src-tauri/src/validation.rs` (lines 30, 52, 81, 94)  
**Issue:** Compile-time regexes; invalid regex would panic at startup.  
**Fix:** Acceptable as-is. If desired, use `lazy_static` or `once_cell` with `Regex::new(...).expect("valid regex")` and a comment.

### M-6. Health.rs test: serde_json::to_string().unwrap()

**Location:** `src-tauri/src/health.rs` (line 147, inside test)  
**Issue:** In test only; acceptable.  
**Fix:** None required.

---

## Low

### L-1. Launcher: Command target string not sanitized for shell

**Location:** `src-tauri/src/launcher.rs` (e.g. `format!("{}@{}", user, address)`)  
**Issue:** `launch_ssh_external` in lib.rs validates address and username (IP/hostname, username). Launcher passes the target to `cmd /C start ssh <target>` (Windows) or `x-terminal-emulator -e ssh <target>` (Linux). If validation were bypassed, metacharacters could be risky; currently validation is in place.  
**Fix:** Document that address/username must be validated by the caller (already done in lib.rs). Optional: sanitize or reject characters that are unsafe in the target string.

### L-2. Dead code / allow(dead_code)

**Location:** `scheduler.rs` (TaskRow.last_run_*), `ssh.rs` (bytes_received), `monitoring.rs`, `scanner.rs`, `validation.rs`  
**Issue:** Several `#[allow(dead_code)]` or unused fields for future use.  
**Fix:** Leave as-is or remove when features are implemented; no functional bug.

### L-3. Frontend: Some invoke errors only logged to console

**Location:** e.g. `SystemHealthWidget.tsx` (`.catch(console.error)`), `SettingsView.tsx` (get_app_info `.catch(() => setAppInfo(null))`)  
**Issue:** User may not see a message for transient failures (e.g. get_system_metrics, get_app_info).  
**Fix:** Optional: set a non-blocking error state or toast for important calls so users see failures when relevant.

### L-4. CredentialManager: Error cast to string

**Location:** `src/components/vault/CredentialManager.tsx` (e.g. `setError(err as string || '...')`)  
**Issue:** Casting `err` to string can miss `Error` objects with a `message` property.  
**Fix:** Use a small helper (e.g. `err instanceof Error ? err.message : String(err)`) for consistent error messages; same pattern already used in ScheduledTasksView.

### L-5. Database migrations: 011 and 012 applied via hooks only

**Location:** `src-tauri/src/lib.rs` (migration 011 and 012 use `up_with_hook` with no-op SQL)  
**Issue:** Raw SQL files 011 and 012 are commented out; migration logic lives in Rust. Anyone reading only SQL files might think the columns are never added.  
**Fix:** Already documented in the SQL files. No code change; consider a short note in docs/SCHEMA.md that 011/012 are applied by Rust hooks for idempotency.

---

## Positive findings

- **SQL:** Queries use parameterized statements (`rusqlite::params!`), reducing injection risk.
- **Input validation:** Host, port, username, CIDR validated before SSH, SFTP, scanner, and launcher (lib.rs, ssh.rs, scanner.rs).
- **Error sanitization:** `sanitize_error()` used for Tauri command errors; internal details not exposed to frontend.
- **Vault:** list_credentials does not require vault unlock (metadata only); get_credential/update_credential require master key.
- **Event cleanup:** TerminalComponent, VaultProvider, NetworkScanner, HostList, RemoteManager clean up listeners in useEffect return; TerminalComponent disconnects ResizeObserver.
- **Timeouts:** SSH execution and SFTP use configurable or fixed timeouts (e.g. ssh_exec, sftp 10s connection).
- **SSH host keys:** ssh.rs, ssh_exec.rs, sftp.rs use SshKeyManager for verification; unknown/changed keys emit events and block connection until user trusts.

---

## Summary

| Severity | Count |
|----------|--------|
| Critical | 0 |
| High     | 3 |
| Medium   | 6 |
| Low      | 5 |

**Recommended order of work:** Address H-1 (monitoring Mutex), then H-2 and H-3 (listener cleanup). Then update docs (M-2, M-3) and optionally harden scanner Mutex (M-1) and frontend error display (L-3, L-4).

---

## Fixes applied (February 18, 2026)

| ID | Fix |
|----|-----|
| **H-1** | `monitoring.rs`: All `AlertEngine` and `MetricsStore` mutex locks use `.unwrap_or_else(\|e\| e.into_inner())` for poison recovery. |
| **H-2** | `Discovery.tsx`: Added `cancelledRef` and `unlistenRef`; `startScan` checks cancelled after each await and stores unlisten callbacks in ref; cleanup sets cancelled and calls ref callbacks so listeners are always removed. |
| **H-3** | `MonitoringView.tsx`: Store `listen(...)` promise in `unlistenPromiseRef` immediately; cleanup calls `unlistenPromiseRef.current?.then(fn => fn())` so unlisten runs when promise resolves (or is already set). |
| **M-1** | `scanner.rs`: All `state.*.lock().unwrap()` in tests and production paths replaced with `.unwrap_or_else(\|e\| e.into_inner())`. |
| **M-2** | `AGENTS.md`: Terminal observer note updated (cleanup calls disconnect); SSH Server Key section updated to state verification is implemented via SshKeyManager. |
| **M-3** | `MVP_COMPLETION_ROADMAP.md`: Replaced `automation/engine.rs` reference with scheduler.rs + sftp.rs + ssh_exec.rs. |
| **L-1** | `launcher.rs`: Added doc comment that caller must validate address/username (lib.rs does so). |
| **L-4** | `CredentialManager.tsx`: Added `getErrorMessage(err, fallback)` helper; all `setError(err as string \|\| '...')` replaced with `getErrorMessage(err, '...')`. |
| **L-5** | `docs/SCHEMA.md`: Added note that migrations 011 and 012 are applied by Rust hooks for idempotency. |

**Not changed (per report):** M-4 (main .expect — optional), M-5/M-6 (acceptable as-is), L-2 (dead code — leave as-is), L-3 (optional frontend error state — not implemented).

---

## Second audit round (February 18, 2026) — AUD-01 through AUD-07

A second, comprehensive audit identified 7 additional issues (data safety, concurrency, frontend-backend contract, UX). All were fixed the same day.

| ID | Severity | Summary | Fix |
|----|----------|---------|-----|
| **AUD-01** | Critical | DB export/import used raw file copy while writers active | Export: `VACUUM INTO` for safe snapshot. Import: validate source, atomic rename, restart warning. |
| **AUD-02** | High | Discovery spawned unbounded threads per `start_discovery` call | `DiscoveryState` with `AtomicBool`; start-if-not-running; `DropGuard` clears flag on thread exit. |
| **AUD-03** | High | SFTP UI allowed SSH-key credentials but backend is password-only | `CredentialSelector` `allowedTypes` for SFTP; ScheduledTasksView filters; `sftp.rs` `require_password_auth()` guard. |
| **AUD-04** | High | Scheduler ignored `set_run_result` errors → duplicate runs | Log errors; in-memory `last_executed` map to skip within 60s even if DB write fails. |
| **AUD-05** | Medium | Monitoring network throughput double-divided by 1024 | Use backend MB values directly in MonitoringView (no extra `/1024`). |
| **AUD-06** | Medium | Credential `id` typed as `number` in frontend, string in backend | `CredentialSummary`/`Credential` `id: string`; handlers updated. |
| **AUD-07** | Medium | Monitoring mutex `.unwrap()` propagated poison panics | All production locks use `.unwrap_or_else(\|e\| e.into_inner())`. |

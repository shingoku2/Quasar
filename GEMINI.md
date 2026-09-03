# Quasar: Gemini Context

This file provides persistent context for the Gemini CLI agent to ensure a smooth transition between sessions.

## Current Project Status
- **Framework:** Tauri v2 + React + TypeScript + Tailwind CSS v4.
- **Status:** **Deferred audit concurrency/persistence remediation planned (Sep 2, 2026); implementation not started.**
- **Last Action:** Inspected the five deferred findings and finalized their implementation contract. Credential operations will wait behind master-password rotation; SSH connection attempts will gain pending/active cancellation-aware lifecycle ownership; host-key prompts will be FIFO; imports will be staged and checked using a Quasar `application_id` with strict legacy-schema fallback; and remotely closed sessions will remove themselves immediately. See `docs/DEFERRED_AUDIT_FIX_PLAN.md`. No implementation files changed and no validation suite was run in this planning session.
- **Previous status:** SSH connection diagnostics, credential save fix, and host protocol parity complete (Aug 21, 2026) — see below.

## Next Session Start

Implement `docs/DEFERRED_AUDIT_FIX_PLAN.md`, beginning with the vault credential-access
gate and deterministic rekey/write race tests. Keep all five findings open until focused
tests, full Rust/frontend validation, and a security review pass.

## Full Codebase Bug Audit (2026-08-22)

### Summary
- **Mechanical checks first**: `tsc --noEmit`, `npm test` (257/257), `cargo clippy -- -D warnings`, `cargo test` (113/113) all clean going in.
- **Then two parallel `code-auditor` subagents** (Rust backend, React frontend) for logic/race/security bugs — found real, verified issues in both.
- **Frontend fixes**: unimported `getErrorMessage` in `CredentialManager.tsx` (would have broken the build); `RemoteManager.tsx` wiping open session tabs whenever a host was added; `NetworkScanner.tsx`/`DashboardView.tsx` dropping scan events via listener churn from an unstable callback; `AlertRules.tsx` toggle able to silently delete a rule on a partial remove-then-add failure.
- **Backend fixes**: `HostTracker`/`MetricsStore` bypassing the shared SQLite busy-timeout (silent data loss under contention); a TOCTOU in `initialize_vault`; `change_master_password` holding the vault lock across multi-second Argon2id + re-encryption work (fixed to drop it, then a `security-reviewer` subagent caught a real regression in that fix — `lock_vault()` mid-change being silently undone — fixed and regression-tested); `scheduler.rs` running due tasks strictly sequentially so one dead host delayed everything else (now bounded-concurrency, matching `scanner.rs`'s pattern); `update_credential` missing validation `add_credential` had.
- **Cleanup**: completed an in-progress `String(err)` → `getErrorMessage()` refactor across ~20 call sites in 12 frontend files (the unfinished version was the root cause of the unimported-helper bug above); fixed a flaky test-DB-filename collision pattern in 5 Rust test modules; deleted the scratch script and ~300 stray gitignored test-DB files.

### Verification
- `npx tsc --noEmit` — clean
- `npm test` — 257 passing / 38 files
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo test` — 114 passing (110 lib + 4 integration), re-run twice to confirm the flaky-fixture fix held
- Security review of the vault.rs lock-holding change (via `security-reviewer` subagent) — found and fixed one regression before it shipped; see `.claude/agent-memory/security-reviewer/vault_rs_patterns.md`

## SSH Connection Diagnostics, Credential Save Fix & Host Protocol Parity (2026-08-21)

### Summary
- **`ssh_connect.rs`** (new) — shared connect helper used by all 6 SSH/SFTP call sites. Separately times DNS resolution, TCP connect per resolved address, and the SSH handshake, so errors name the failing phase and address instead of one generic timeout. Fixes the dual-stack trap where a dead IPv6 record starves a working IPv4 address under a shared timer.
- **Credential save bug (1 of 2)**: `CredentialManager.tsx` sent snake_case `invoke()` keys (`credential_type`, `key_path`, etc.); Tauri matches against camelCase and silently no-ops missing `Option<T>` keys rather than erroring. Renamed to camelCase; added a regression test asserting no payload key contains `_`.
- **Credential save bug (2 of 2)**: found in live re-test *after* the fix above shipped. Editing an SSH-key credential whose key was originally pasted as PEM (no `key_path`) was still blocked — `get_credential` never returns decrypted key material by design, so the form's blank `private_key` field was wrongly read as "no key stored" by the submit validation. Fixed by gating on `has_private_key`/`key_path` instead of the form field. Separate commit (`cc761a23`).
- **Host protocols**: `AddHostDialog` now offers SSH/RDP/Database/API/Other (previously SSH/RDP only), matching the credential vault. Port required when the protocol has no backend default; Connect button only for SSH/RDP.
- **Dependency updates**: russh 0.62.5 → 0.62.7, russh-sftp → 2.4.0, plus ~70 other Rust crates and 8 npm packages, all re-verified (clippy, cargo test, npm test, tsc).

### Verification
- `cargo clippy --tests -- -D warnings` — clean
- `cargo test` — 113 passing
- `npm test` — 257 passing / 38 files (final count, after both credential-bug fixes)
- `npx tsc --noEmit` — clean
- Live-verified against two real hosts: an Ubuntu VPS (old code timed out silently; new diagnostics reported the exact port mismatch, which the user confirmed fixed the connection) and a home-LAN Ubuntu laptop (separately diagnosed a stale SSH known_hosts entry + wrong `IdentityFile` in the user's own `~/.ssh/config`, unrelated to the app). Both credential-save bugs were confirmed fixed by successfully editing/using real SSH-key credentials for these hosts, with both terminal sessions connected side by side in the app.

## Credential Edit & Type-Switch Fixes (2026-02-16)

### Summary
- **update_credential** now supports SSH key fields (`key_path`, `private_key`, `key_passphrase`); frontend sends them when editing SSH key creds; empty string clears.
- **credential_type** is sent on edit and written to DB so stored type matches UI.
- **Clearing:** Frontend sends raw form values (empty string) for key fields so backend clears; when switching to password-based, key fields sent as `''`; when switching to SSH key, `password: ''` sent.
- **Backend clear password:** Migration 009 makes `encrypted_password`, `nonce`, `tag` nullable; `update_credential` sets them to NULL when `password` is empty (no more storing encrypted empty); `get_credential` returns empty password when NULL.

### Verification
- `cargo test vault::credentials::tests::` — all 7 tests passing.

## Latest Audit Remediation (2026-02-15)

### Fixed Issues
1. **Scanner stop semantics / detached task leak**
   - Added `drain_scan_futures(...)` and guaranteed final drain before scan return.
   - Regression test: `scanner::tests::test_drain_scan_futures_drains_pending_tasks`.

2. **SSH idle timeout stale session cleanup**
   - Timeout task now removes timed-out sessions from `SshState.sessions` and cancels stats tasks before disconnect.

3. **Credential malformed nonce/tag panic hardening**
   - Added explicit nonce/tag length checks before `copy_from_slice`.
   - Regression tests:
     - `test_get_credential_invalid_nonce_length_returns_error`
     - `test_get_credential_invalid_tag_length_returns_error`

4. **Vault initialization error rendering**
   - Added robust `getErrorMessage(error: unknown)` normalizer.
   - Kept `isInitialized = null` on init failure so explicit error screen renders.
   - Regression test added in `VaultProvider.test.tsx` for Error-object message rendering.

### Verification
- `cargo test scanner::tests::`
- `cargo test vault::credentials::tests::test_get_credential_invalid_`
- `cargo test`
- `npm test -- src/components/vault/VaultProvider.test.tsx`
- `npm test`

## Progress Summary (2026-01-30)

### Completed Tracks

1. **Module B: Remote Connection Manager** - COMPLETE
   - Integrated `xterm.js` with Rust-based `russh` backend
   - Built `SessionContainer` for dynamic tab management and split-view
   - Implemented `SessionToolbar` for real-time latency/bandwidth monitoring
   - Added `CredentialPrompt` for secure password entry

2. **Asset Discovery & Monitoring Track** - COMPLETE
   - **Phase 1:** LAN Scanner Backend (Rust)
     - ICMP ping scanner with concurrent execution (50 parallel)
     - TCP port scanner for common services (22, 80, 443, 445, 3389)
     - Tauri commands: `scan_network`, `stop_scan`, `get_scan_progress`
     - Real-time progress and result emitters
   - **Phase 2:** Discovery UI & Integration
     - `NetworkScanner` component with start/stop controls
     - Integrated discovery with mDNS and network scan tabs
     - Host suggestion dialog for newly discovered devices
   - **Phase 3:** Pre-flight Health Checks
     - `HealthCheckBadge` component for real-time host status
     - `PreflightDialog` for pre-connection health metrics
     - SSH-based health check infrastructure
   - **Phase 4:** Dashboard Integration
     - `DiscoveryWidget` for recent discoveries
     - `NetworkMapWidget` for visual network topology
     - Auto-refresh mechanism for health indicators

3. **Monitoring & Alerts Track** - COMPLETE
   - **Phase 1:** System Metrics Collection (Rust backend with sysinfo)
   - **Phase 2:** Alert Engine with threshold evaluation
   - **Phase 3:** AlertFeed component with real-time updates
   - **Phase 4:** AlertRules UI for managing alert configurations
   - Live metrics in DashboardView (CPU, Memory, Disk I/O, Network)

### Active Track

4. **Automation Canvas Track** - IN PROGRESS
   - Visual workflow builder with drag-and-drop nodes
   - Support for triggers (scheduled, webhook, manual)
   - Action nodes for SSH command execution, file transfer, notifications
   - Canvas.tsx component for building workflows
   - Workflow engine in Rust to execute automation flows
- **Rust/Cargo:** **Functional**. (minimum v1.95; current stable in CI).
  - `russh` 0.62.7 - SSH client library
  - `surge-ping` 0.9 - ICMP ping for network scanning
  - `cidr-utils` 0.7 - CIDR notation parsing
  - `tokio` 1.53 - Async runtime with full features
- **Node.js:** 24.15+ with npm 12.0.2+.
- **Dependencies:** `xterm.js`, `lucide-react`, `recharts`, `@tauri-apps/api`

## Next Steps / Pending Tasks
- **Module D: Automation Workflow Editor:** Implement the React Flow canvas for visual playbooks.
- **Native RDP:** Currently using external launcher; integrate `ironrdp` for embedded RDP sessions.
- **Monitoring & Alerting:** Real-time system monitoring with configurable thresholds and notifications.

## Resume Instructions
1. **Launch App:** `npm run tauri dev` to see the dashboard with discovery widgets.
2. **New Track:** Run `/conductor:setup` or `/conductor:implement` to define and start the next module (e.g., Automation Canvas).

## Key Commands
- `cargo test --lib` - Run Rust unit tests
- `npm test` - Run React component tests
- `npm run tauri dev` - Start development server
- `cargo tauri build` - Build production release

## Project Structure
- `src-tauri/src/` - Rust backend (scanner, health, ssh, discovery modules)
- `src/components/` - React components (dashboard, discovery, health, session management)
- `conductor/tracks/` - Track documentation and implementation plans

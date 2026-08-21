# Quasar: Gemini Context

This file provides persistent context for the Gemini CLI agent to ensure a smooth transition between sessions.

## Current Project Status
- **Framework:** Tauri v2 + React + TypeScript + Tailwind CSS v4.
- **Status:** **SSH connection diagnostics, credential save fix, and host protocol parity complete (Aug 21, 2026)**.
- **Last Action:** Root-caused a user-reported SSH timeout to a saved host's port not matching the server's actual sshd port; replaced the single opaque "Connection timed out" with phase-aware DNS/TCP/handshake diagnostics (`ssh_connect.rs`). Along the way found and fixed a silent credential-save bug (Tauri `invoke()` requires camelCase argument keys — snake_case keys are dropped, not errored) and brought `AddHostDialog`'s protocol list up to parity with the credential vault's (added Database/API/Other). See `AGENTS.md` for full detail.

## SSH Connection Diagnostics, Credential Save Fix & Host Protocol Parity (2026-08-21)

### Summary
- **`ssh_connect.rs`** (new) — shared connect helper used by all 6 SSH/SFTP call sites. Separately times DNS resolution, TCP connect per resolved address, and the SSH handshake, so errors name the failing phase and address instead of one generic timeout. Fixes the dual-stack trap where a dead IPv6 record starves a working IPv4 address under a shared timer.
- **Credential save bug**: `CredentialManager.tsx` sent snake_case `invoke()` keys (`credential_type`, `key_path`, etc.); Tauri matches against camelCase and silently no-ops missing `Option<T>` keys rather than erroring. Renamed to camelCase; added a regression test asserting no payload key contains `_`.
- **Host protocols**: `AddHostDialog` now offers SSH/RDP/Database/API/Other (previously SSH/RDP only), matching the credential vault. Port required when the protocol has no backend default; Connect button only for SSH/RDP.
- **Dependency updates**: russh 0.62.5 → 0.62.7, russh-sftp → 2.4.0, plus ~70 other Rust crates and 8 npm packages, all re-verified (clippy, cargo test, npm test, tsc).

### Verification
- `cargo clippy --tests -- -D warnings` — clean
- `cargo test` — 113 passing
- `npm test` — 256 passing / 38 files
- `npx tsc --noEmit` — clean
- Live-verified against a real Ubuntu VPS: old code timed out silently; new diagnostics reported the exact port mismatch, which the user then confirmed fixed the connection.

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

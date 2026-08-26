# Quasar - Agent Context & Bug Fixes

## Overview
Quasar is a Tauri-based remote infrastructure management application with React frontend and Rust backend. This document tracks major bug fixes, architectural decisions, and context for AI agents working on this codebase.

---

## Recent Implementations

### Dependabot Alert Triage: RUSTSEC-2023-0071 - Complete (August 26, 2026)

#### Overview
Third follow-up from the production-readiness assessment: checking the "1 moderate
vulnerability" Dependabot flagged on the default branch (surfaced on every `git push`).
No GitHub tool in this environment exposes Dependabot alerts or the `gh` CLI directly, so
identified it by extracting `master`'s exact `Cargo.lock`/`package-lock.json` into an
isolated temp dir and running `npm audit` and `cargo audit` against each independently.
`npm audit` was clean; `cargo audit` (after installing it — not preinstalled) found exactly
one real vulnerability (17 other "unmaintained"/"unsound" advisories are warnings that
don't fail the build): **RUSTSEC-2023-0071**, a timing side-channel ("Marvin Attack") in
the `rsa` crate, severity 5.9/medium — matches GitHub's "moderate" label.

`cargo tree -i rsa` traced it to `russh` (both directly and via `ssh-key` → `russh`) — i.e.
Quasar's own SSH implementation, not an optional or droppable dependency. RustSec lists "no
fixed upgrade is available" for this advisory (open since Nov 2023).

While tracing this, checked whether CI had ever actually caught it: `list_workflow_runs`
for the `CI` workflow returned **zero runs, ever**, on `master`. Root cause was in
`.github/workflows/ci.yml` itself — its `on: push`/`pull_request` triggers were scoped to
`branches: [main, develop]`, but this repository's only branch has always been `master`.
Every "CI: clean" claim throughout this file's history was from running `tsc`/`vitest`/
`clippy`/`cargo test` locally in-session — accurate for what was actually run, but the
GitHub Actions gate itself had never once executed on a real push or PR. Fixed by changing
both triggers to `branches: [master]`. (`release.yml` is unaffected — it triggers on `v*`
tags, and zero runs there is expected since no tag has been pushed yet.)

Risk assessment before deciding how to handle it: the Marvin Attack targets an RSA
*decryption* oracle (attacker times many PKCS#1v1.5 decryptions against the same key to
recover key bits — the same family as the classic Bleichenbacher/TLS attack). Quasar only
exercises `rsa` through russh's SSH client authentication path, which *signs* a challenge
with the user's private key rather than decrypting attacker-supplied ciphertext — the
specific oracle this advisory describes isn't reachable through that flow. Presented this
analysis and three options to the user (accept-and-document, accept-and-open-a-tracking-
issue, or leave `cargo audit` red until upstream fixes it); they chose accept-and-document.

#### Fix
- **`src-tauri/.cargo/audit.toml`** (new) — `cargo audit`'s ignore config, `ignore =
  ["RUSTSEC-2023-0071"]` with the full rationale above as an inline comment. Verified
  locally: `cargo audit` in `src-tauri/` went from exit 1 ("error: 1 vulnerability found!")
  to exit 0 (only the pre-existing 17 allowed warnings). This is also what makes the CI
  `cargo audit` step in `ci.yml` pass — unclear whether it was actually red before this,
  since GitHub Actions shows zero recorded runs of the CI workflow on `master` for this
  repo (a separate, pre-existing gap worth someone's attention — the workflow files are
  correct and pass locally, but nothing indicates they've ever actually executed on GitHub).
- **`SECURITY.md`** — new "Known Dependency Advisories" section documenting the same
  accepted risk where a reporter would actually look for it.

#### Files modified
- `src-tauri/.cargo/audit.toml` (new)
- `SECURITY.md` (Known Dependency Advisories section)
- `.github/workflows/ci.yml` (branch triggers `[main, develop]` → `[master]`)
- `CLAUDE.md` (CI/CD section — actual branch, audit steps)
- `AGENTS.md` (this entry)

---

### Vault Lockout Persistence & Security Policy - Complete (August 26, 2026)

#### Overview
Second follow-up from the production-readiness assessment. The assessment's first pass had
flagged "no brute-force throttling on vault unlock" as a gap — that turned out to be wrong:
`unlock_vault` already has an escalating lockout (5 failed attempts → 5 min, 10 → 15 min,
15 → 60 min, reset on success), just not surfaced anywhere in `CLAUDE.md`. Investigating it
turned up a real, narrower gap instead: the lockout state lives only in `VaultStateInner`,
which is rebuilt fresh on every process launch. An attacker with local access to the
(encrypted) database file could brute-force the master password in batches of 4 guesses by
relaunching the app before each lockout tier, since Argon2id's per-guess cost (~50-100ms at
the app's params) is the only thing slowing them down between relaunches.

#### 1) Persist lockout state across restarts (`src-tauri/src/vault.rs`)
- Two new private `VaultState` methods: `load_persisted_lockout()` (reads
  `lockout_failed_attempts` / `lockout_until_unix` from `vault_settings`, folds them into
  `inner` — only ever raising its state, since the DB is always at least as strict as a
  fresh process) and `persist_lockout()` (writes them back, converting `Instant` — which has
  no meaning across process runs — to a wall-clock unix timestamp via `chrono::Utc::now()`).
- `unlock_vault()` now opens its DB connection and calls `load_persisted_lockout()` before
  the existing in-memory lockout check (previously the connection was opened after), and
  calls `persist_lockout()` on every failed attempt (with the newly-escalated tier, if any),
  when an expired lockout is cleared, and on success (clearing both counters).
- The escalation policy itself (5/10/15 attempts → 5/15/60 min) is unchanged — this only
  makes it survive a restart.
- New tests: `vault::tests::test_lockout_persists_across_vault_state_restart` (5 failed
  attempts, then a brand-new `VaultState` on the same DB is still locked out even with the
  *correct* password) and `test_successful_unlock_clears_persisted_lockout` (a successful
  unlock clears the persisted counters too, not just the in-memory ones).

#### 2) `SECURITY.md` (new)
Vulnerability disclosure policy: directs reports to GitHub's private Security Advisories
(Security tab → "Report a vulnerability") rather than public issues, states response-time
expectations, defines scope (backend/frontend/release pipeline in; already-unlocked-vault
or already-compromised-machine scenarios out), and documents two things that look like
vulnerabilities but are intentional design (`connect_ssh`'s unsanitized diagnostic errors,
`list_credentials` working without an unlocked vault) so reports about them can go straight
to "is the existing handling actually safe" instead of being re-litigated from scratch.

#### Verification
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test` — 112 passed (up from 110; the two new lockout tests), including the full
  `vault::tests::` module (7 tests, all passing).

#### Files modified
- `src-tauri/src/vault.rs` (lockout persistence + 2 new tests)
- `SECURITY.md` (new)
- `CLAUDE.md` (Security Notes item 10, Key Files table)

---

### Code Signing & Auto-Updater - Complete (August 26, 2026)

#### Overview
Follow-up to a production-readiness assessment that flagged unsigned release builds (Windows
SmartScreen / macOS Gatekeeper warnings) and no update mechanism as blockers for a public
release. Added Tauri's updater plugin and wired code signing into the release pipeline.

#### 1) Auto-updater (`tauri-plugin-updater` + `@tauri-apps/plugin-updater`)
- **Backend**: `src-tauri/Cargo.toml` — `tauri-plugin-updater` and `tauri-plugin-process`
  added under `[target.'cfg(any(target_os = "macos", windows, target_os = "linux"))'.dependencies]`
  (desktop-only; this app has no mobile target). Registered in `lib.rs`'s builder chain behind
  the same `cfg` guard — `Builder::default()` is now bound to a `let builder` so the two
  plugin registrations can be conditionally chained before `.invoke_handler(...)`.
- **Capabilities**: `src-tauri/capabilities/default.json` grants `updater:default` and
  `process:allow-restart` (the latter is what lets the app call `relaunch()` after installing).
- **Config**: `tauri.conf.json` sets `bundle.createUpdaterArtifacts: true` (produces signed
  `.sig` files per platform) and `plugins.updater` with the embedded pubkey and a GitHub
  Releases `latest.json` endpoint (`https://github.com/shingoku2/quasar/releases/latest/download/latest.json`).
- **Frontend**: new `src/hooks/useUpdater.ts` wraps `check()` / `Update.downloadAndInstall()` /
  `relaunch()` behind a small status machine (`idle → checking → available|upToDate|error →
  downloading`), explicitly `.close()`s the `Update` resource on re-check or unmount per the
  plugin's resource-management contract. Two consumers:
  - `src/components/UpdateBanner.tsx` (new) — auto-checks on mount, rendered in `Layout.tsx`
    just below `TopBar`; dismissible, shows an "Install & Restart" action when available.
  - `SettingsView.tsx`'s `AboutSettings` — manual "Check for Updates" button/status card,
    same hook with `autoCheck=false`.
- **Test mocks**: `src/test-setup.ts` globally mocks `@tauri-apps/plugin-updater` (`check()` →
  resolves `null`, i.e. "no update") and `@tauri-apps/plugin-process` (`relaunch()`), so every
  existing test continues to pass unmodified — `UpdateBanner` renders nothing by default and
  no component triggers a real IPC call. New regression tests: `UpdateBanner.test.tsx` (no
  update / available / install-and-relaunch / dismiss / check-failure) and a
  `SettingsView.test.tsx` case for the About-tab check-for-updates flow.

#### 2) Code signing (`.github/workflows/release.yml`)
- **Updater artifact signing**: `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
  passed to every matrix OS — required on all three, since `createUpdaterArtifacts` isn't
  platform-specific.
- **Windows**: `certificateThumbprint` isn't settable via env var (unlike everything else
  here), so a new step imports the base64 `WINDOWS_CERTIFICATE` (.pfx) into the runner's cert
  store via PowerShell and patches the resulting thumbprint into `tauri.conf.json` with a
  one-line Node script before `tauri build` runs. Skipped when the secret isn't set, so forks
  still get an (unsigned) Windows build.
- **macOS**: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` passed straight through as env vars — the
  Tauri bundler handles keychain import, signing, and notarytool submission itself; no
  `tauri.conf.json` changes needed. Unused (harmless) on the Windows/Linux runners.
- Full secret setup instructions: `docs/RELEASE_SIGNING.md` (new).

#### Verification
- `cargo check` / `cargo clippy --all-targets -- -D warnings` / `cargo test` — clean, 110
  passed (Rust toolchain in the dev sandbox needed bumping from 1.94.1 to 1.98 to satisfy the
  crate's `rust-version = "1.95"`; unrelated to this change, just a stale local toolchain).
- `npx tsc --noEmit` / `npm test` — clean, 39 files / 263 tests (up from 38/257 — the two new
  updater test files/cases).
- Did not attempt an actual signed build (no real certificates available in this environment)
  — verified the config/workflow changes parse correctly (`python3 -c "import yaml/json..."`)
  and that the plugin wiring compiles and passes existing tests. The real signing path can
  only be exercised once the secrets in `docs/RELEASE_SIGNING.md` are added and a `v*` tag is
  pushed.

#### Files modified
- `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` (new deps)
- `src-tauri/src/lib.rs` (plugin registration)
- `src-tauri/capabilities/default.json` (updater/process permissions)
- `src-tauri/tauri.conf.json` (updater plugin config, `createUpdaterArtifacts`, Windows digest/timestamp fields)
- `package.json`, `package-lock.json` (new deps)
- `src/hooks/useUpdater.ts` (new)
- `src/components/UpdateBanner.tsx`, `UpdateBanner.test.tsx` (new)
- `src/components/Layout.tsx` (renders `UpdateBanner`)
- `src/components/SettingsView.tsx`, `SettingsView.test.tsx` (About-tab update UI + test)
- `src/test-setup.ts` (global plugin mocks)
- `.github/workflows/release.yml` (signing steps/env vars)
- `docs/RELEASE_SIGNING.md` (new)
- `CLAUDE.md` (tech stack table, Key Files table, CI/CD section)

---

### Full Codebase Bug Audit - Complete (August 22, 2026)

#### Overview
User asked for a full pass over the codebase looking for errors and bugs. Ran `tsc --noEmit`, the full Vitest suite, `cargo clippy -- -D warnings`, and `cargo test` first (all clean going in — one exception, see #1), then dispatched parallel `code-auditor` reviews over the Rust backend and the React frontend since the mechanical checks alone don't catch logic/race/security bugs. Fixed everything found; a `security-reviewer` pass on the highest-risk fix (the vault lock change, #6 below) caught a real regression before it shipped.

#### 1) `CredentialManager.tsx` called an unimported function
- **Symptom**: `getErrorMessage()` used in 6 places, never imported — `ReferenceError` at runtime / would have failed `tsc --noEmit` on the next build. Leftover from an incomplete refactor mid-session (a scratch script, `fix-errors.js`, had moved the helper to `src/lib/utils.ts` and rewritten call sites but never got a chance to add the import).
- **Fix**: added the import. Then finished the refactor properly across the rest of the frontend (see #10).

#### 2) `RemoteManager.tsx` — adding a host disconnected every open SSH/SFTP session
- **Root cause**: the effect that (re)builds the Inventory/Tunnels tabs was keyed on `refreshTrigger` (bumped every time `AddHostDialog` succeeds) and did `setTabs([inventoryTab, tunnelsTab])` — an unconditional **replace**, not a merge. Any terminal or SFTP tab opened via `addTab()` (which appends) got wiped, unmounting `TerminalComponent` and triggering its `disconnect_ssh` cleanup. `activeTabId` was left pointing at a tab that no longer existed, so the content pane went blank.
- **Fix**: the effect now filters out any existing `inventory`/`tunnels` entries and prepends the freshly-built ones ahead of whatever dynamic (session) tabs were already open, instead of discarding the array.

#### 3) `NetworkScanner.tsx` / `DashboardView.tsx` — scan results dropped mid-scan
- **Root cause**: `DashboardView` passed a new inline `onHostFound` arrow function on every render. `NetworkScanner`'s listener-setup effect depends on `[onHostFound]`, and `onHostFound` itself triggers a `setDiscoveredHosts` that re-renders `DashboardView` — a feedback loop that unlistened/relistened all four `scan_*` event channels on (up to) every discovered host during an active `/24` scan. Any event landing in the gap between `unlisten()` and the next `listen()` resolving was lost; if `scan_complete` landed there, `isScanning` never flipped false and the UI showed "Scanning…" forever.
- **Fix**: wrapped the callback in `useCallback` in `DashboardView` (stable identity, empty deps — it only calls the `setDiscoveredHosts` functional updater).

#### 4) `AlertRules.tsx` — toggling a rule could silently delete it
- **Root cause**: `toggleRule()` did `remove_alert_rule` then `add_alert_rule` (no dedicated update command). If the add failed after the remove succeeded, the `catch` only reverted local React state — the rule was already gone from the backend, with the UI showing it unchanged.
- **Fix**: dropped the `remove_alert_rule` call. The backend's `AlertEngine::add_rule` already does `rules.retain(|r| r.id != rule.id)` then `push` — it's already an atomic upsert by id, so a single `add_alert_rule` call is both simpler and closes the failure window entirely.

#### 5) `HostTracker` / `MetricsStore` bypassed the shared DB busy-timeout
- **Location**: `src-tauri/src/host_tracker.rs`, `src-tauri/src/monitoring.rs`
- **Root cause**: both opened raw `rusqlite::Connection`s directly instead of going through `db::open_connection()` (which every other DB consumer uses), so they never got the 5s busy-timeout. Under write contention — scheduler + monitoring + a scan all writing around the same tick — SQLite returned `SQLITE_BUSY` immediately instead of waiting, and the caller only logged the failure (`lib.rs`), so scanned hosts and monitoring samples were silently dropped while the UI reported success.
- **Fix**: both now delegate to `db::open_connection()`.

#### 6) `vault.rs` — lock-holding, TOCTOU, and a self-inflicted regression
- **`initialize_vault` TOCTOU**: `is_initialized()` took and released its own read lock before `initialize_vault` separately acquired the write lock — two concurrent calls (e.g. a double-submit) could both pass the check before either wrote. Fixed by moving the check inside the same write-lock acquisition as the write.
- **`change_master_password` held the write lock across the entire operation**, including a `spawn_blocking` that does 2+ Argon2id hashes (47 MiB/2 iter each) plus a full credential re-encryption transaction — multi-second work that stalled every other vault read (`get_master_key`, `is_locked`, `check_auto_lock`) for an in-flight SSH/SFTP credential lookup. Fixed to acquire the lock only briefly at the start (snapshot `old_key`/`db_path`, check `changing_password` isn't already set, set it) and briefly at the end (install the new key).
- **Regression this introduced, caught by a `security-reviewer` pass before it shipped**: `changing_password` is only checked by `check_auto_lock()` — `lock_vault()` isn't gated by it at all. With the lock dropped for the re-encryption window, a `lock_vault()` call could land mid-change; when `change_master_password` reacquired the lock at the end it unconditionally reinstalled the new key, silently re-unlocking a vault the user had just explicitly locked. Fixed by only installing the new key if `inner.master_key.is_some()` at that point — an explicit lock wins, and the new password is already persisted so the next `unlock_vault` works correctly either way.
- Added `vault::tests::test_lock_vault_during_password_change_stays_locked` — spawns the password change, yields until it's inside the `spawn_blocking` window, calls `lock_vault()`, and asserts the vault stays locked and the new password is already usable.
- See `.claude/agent-memory/security-reviewer/vault_rs_patterns.md` for the full locking model writeup.

#### 7) `scheduler.rs` — one dead host delayed every other due task
- **Root cause**: `run_due_tasks` ran due tasks in a plain sequential `for` loop; each SSH/SFTP task can take up to `SSH_TIMEOUT_SECS` (120s) to time out, so one unreachable host in a batch delayed every other task due in the same tick behind its full timeout.
- **Fix**: due tasks now run concurrently, bounded by a `tokio::sync::Semaphore` (`MAX_CONCURRENT_TASKS = 5`), matching the bounded-concurrency pattern already used in `scanner.rs`. Per-task execution/persistence logic was extracted into `run_and_record_task()`; results are joined and folded into `last_executed` after all spawned tasks complete (kept as plain sequential code — no shared-mutable-state concerns since it runs after the join, not during).

#### 8) `update_credential` skipped validation `add_credential` enforces
- **Location**: `src-tauri/src/lib.rs`
- **Fix**: added `validate_credential_name`/`validate_username` checks for `Some(name)`/`Some(username)`, matching `add_credential`.

#### 9) Flaky test-DB fixtures (found while testing #6)
- **Root cause**: 5 test modules (`vault.rs`, `vault/credentials.rs`, `vault/ssh_keys.rs`, `host_tracker.rs`, `monitoring.rs`) named their per-test SQLite file from a nanosecond timestamp alone. Adding one more concurrently-running test (#6's regression test) was enough to trigger an actual collision (`table vault_settings already exists`) — clock resolution isn't a reliable uniqueness guarantee under real thread scheduling.
- **Fix**: appended a per-process `AtomicU64` counter to the filename in all 5. (`vault/audit.rs` has the same timestamp-only pattern but only one call site — no collision is possible there, left as-is.) Also added a `credentials` table to `vault.rs`'s `setup_test_db()`, which was missing it — the new regression test was the first test in that module to actually execute `change_master_password` to completion, and it needs the table to re-encrypt against.

#### 10) `String(err)` instead of `getErrorMessage()` across ~20 call sites / 12 files
- **Risk**: `String(err)` on a plain object (rather than an `Error` or a string) renders `[object Object]` to the user. `String(err) || 'fallback'` additionally never falls back for any non-empty string, including a raw `"Error: ..."` stringification.
- **Fix**: completed a refactor a previous session had started (see `CredentialManager.tsx`'s already-fixed case) — replaced every remaining instance with `getErrorMessage(err[, fallback])` in `PreflightDialog.tsx`, `ScheduledTasksView.tsx`, `SettingsView.tsx`, `SshFileManager.tsx`, `SshTunnelsView.tsx`, `main.tsx`, and `vault/{AuditLogViewer,CredentialSelector,KnownHostsManager,VaultInitDialog,VaultSettings,VaultUnlockDialog}.tsx`. Left `TerminalComponent.tsx`'s two `String(e).includes(...)` uses alone — they're substring checks, not user-facing display.
- Deleted `fix-errors.js` (the scratch script that started this refactor) now that it's finished by hand.

#### Verification
- `npx tsc --noEmit` — clean
- `npm test` — 257 passing / 38 files (unchanged; no new frontend tests added, all fixes verified against the existing suite)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo test` — 114 passing (110 lib + 4 integration; up from 113, +1 for the new vault regression test), re-run twice to confirm the test-isolation fix held
- Also cleaned up ~300 stray gitignored `test_*.db` files accumulated from prior local test runs (`src-tauri/.gitignore` already excludes them; disk clutter only, not a git concern)

#### Files modified
- `src/components/vault/CredentialManager.tsx` (missing import)
- `src/components/RemoteManager.tsx` (tab-merge fix)
- `src/components/dashboard/DashboardView.tsx` (`useCallback` on `onHostFound`)
- `src/components/dashboard/AlertRules.tsx` (single-call upsert)
- `src-tauri/src/host_tracker.rs`, `src-tauri/src/monitoring.rs` (route through `db::open_connection`)
- `src-tauri/src/vault.rs` (TOCTOU fix, lock-holding fix, `lock_vault` race fix, new regression test, `credentials` table in test fixture)
- `src-tauri/src/scheduler.rs` (bounded-concurrency task execution)
- `src-tauri/src/lib.rs` (`update_credential` validation)
- `src-tauri/src/vault/credentials.rs`, `src-tauri/src/vault/ssh_keys.rs` (test-DB-filename collision fix)
- `src/components/{PreflightDialog,ScheduledTasksView,SettingsView,SshFileManager,SshTunnelsView}.tsx`, `src/main.tsx`, `src/components/vault/{AuditLogViewer,CredentialSelector,KnownHostsManager,VaultInitDialog,VaultSettings,VaultUnlockDialog}.tsx` (`getErrorMessage` refactor)
- `.claude/agent-memory/security-reviewer/vault_rs_patterns.md` (new — vault.rs locking model, written by the security-reviewer subagent during this session)
- `fix-errors.js` (deleted — completed scratch script)

---

### SSH Connection Diagnostics, Credential Save Fix & Host Protocol Parity - Complete (August 21, 2026)

#### Overview
User reported SSH connections from the terminal always timing out on one server, despite `ssh`/PowerShell remoting working fine from the same machine. Root-caused through three narrowing steps and fixed along with two bugs found along the way.

#### 1) Phase-aware SSH connection errors
- **Root cause of the reported bug**: the saved host's port (22) didn't match the server's actual sshd port (6969, per the user's `~/.ssh/config`). The old code wrapped DNS + TCP + handshake in one `tokio::time::timeout(5s, russh::client::connect(...))` and collapsed every failure into `"Connection timed out"` — no way to tell wrong-port from firewalled from dead-DNS.
- **Fix**: new `src-tauri/src/ssh_connect.rs::connect_with_diagnostics()` — resolves DNS separately (5s budget), tries each resolved address for TCP individually (so a dead IPv6 AAAA record can't starve a working IPv4 address under one shared timer — the classic dual-stack trap), then runs the SSH handshake via `russh::client::connect_stream()`. Each phase's error names the phase and the specific address, e.g. `TCP connection to host:22 failed — 1.2.3.4:22: no response after 10s (host down, or port filtered by a firewall)`.
- Wired into all 6 production connect call sites: `ssh.rs` (interactive terminal — also raised its outlier 5s cap to the 10s used everywhere else), all 4 SFTP operations in `sftp.rs`, `ssh_exec.rs`, `ssh_pool.rs`, `ssh_tunnel.rs`.
- Interactive terminal errors intentionally bypass `sanitize_error()` (pre-existing behavior, unchanged) — the diagnostic detail is the point.
- Regression tests in `ssh_connect.rs`: TCP-phase failure (closed port) vs handshake-phase failure (open port, silent server) produce distinguishable error text.

#### 2) Credential save silently discarding SSH key / type changes
- **Symptom**: editing a credential in the vault appeared to succeed (no error) but type switches and SSH key material never persisted.
- **Root cause**: `CredentialManager.tsx`'s `update_credential`/`add_credential` payloads used snake_case keys (`credential_type`, `key_path`, `private_key`, `key_passphrase`). Tauri's command macro matches `invoke()` JSON keys against the **camelCased** Rust parameter name (confirmed in vendored `tauri-macros` 2.6.3 and `tauri` 2.11.5 source) — a missing key for an `Option<T>` parameter silently becomes `None` rather than erroring. So those four fields never reached the backend.
- **Fix**: renamed to `credentialType`/`keyPath`/`privateKey`/`keyPassphrase`. Every other `invoke()` call site in the frontend was already correct.
- Regression test in `CredentialManager.test.tsx` asserts the full `update_credential` payload contains no key with an underscore, to catch this class of bug generally.

#### 3) Host protocol options didn't match the credential vault's list
- **User ask**: `AddHostDialog` (Remote tab) only offered SSH/RDP; the credential dialog (Security tab) already supported Database/API/Other.
- **Fix**: `AddHostDialog` now offers all 5 (SSH, RDP, Database, API, Other). Port is required for protocols without a backend default (only SSH=22 and RDP=3389 have one in `upsert_saved_host_in_conn`). `HostList` only shows the Connect button for SSH/RDP (the only protocols with an actual client); other protocols get their own badge color and remain inventory/monitoring-only entries. `RemoteManager.handleConnect` has a defensive branch explaining "no built-in client" if a non-connectable host is triggered via another path (e.g. Quick Connect).
- New tests in `HostManagement.test.tsx`: full option list pinned, port-required toggle verified.

#### 4) Credential-edit validation still blocked saves on existing SSH keys (found in live re-test, follow-up commit)
- **Symptom**: after fix #2 shipped, the user tried editing a real SSH-key credential (one originally created by pasting a PEM, no `key_path`) and saving was still blocked with "Provide either key path or paste private key PEM."
- **Root cause**: a second, distinct bug in the same dialog. `get_credential` intentionally never returns decrypted key material to the frontend (security by design — see `CredentialFrontendView` in `vault/credentials.rs`, which sends only `has_private_key`/`has_key_passphrase` booleans). The edit form's `private_key` field is therefore always blank on load, by design. But the pre-submit validation checked only the *form fields* (`key_path`/`private_key` both empty ⇒ error), with no awareness that key material could already exist server-side — so it blocked every edit to any ssh_key credential that didn't originally use a `key_path`, regardless of what the user was actually changing (even just the name).
- **Fix**: added `key_path` to the frontend `Credential` interface (removing an ad-hoc type-assertion cast in the process) and gated the validation on `credential?.key_path || credential?.has_private_key` in addition to the form fields — i.e. skip the "provide a key" error when editing a credential that's already known to have one stored.
- Regression test in `CredentialManager.test.tsx`: edits a credential with `has_private_key: true` and no `key_path`, asserts `update_credential` is called and the validation error never renders.
- Verified live against a real SSH-key credential (OVH VPS, `vps-ed25519`) after the fix — edit saved successfully.
- Commit `cc761a23` (separate from the `c4491b8d` doc-sync commit that closed out items 1–3 above).

#### Dependency updates
- Rust: `cargo update` applied 71 available in-range bumps, notably `russh` 0.62.5 → 0.62.7 and `russh-sftp` 2.3.0 → 2.4.0. Re-verified with `cargo clippy -- -D warnings` and `cargo test` (113 tests passing).
- npm: `vite`, `vitest`, `lucide-react`, `@vitejs/plugin-react`, `vis-network`, `vis-data`, `postcss`, `@testing-library/jest-dom` updated. Re-verified with `npm test` (257 tests / 38 files) and `tsc --noEmit`.

#### Files modified
- `src-tauri/src/ssh_connect.rs` (new)
- `src-tauri/src/lib.rs` (`mod ssh_connect;`)
- `src-tauri/src/ssh.rs`, `sftp.rs`, `ssh_exec.rs`, `ssh_pool.rs`, `ssh_tunnel.rs` (use shared connect helper)
- `src/components/vault/CredentialManager.tsx` (camelCase payload keys; `has_private_key`/`key_path`-aware edit validation)
- `src/components/vault/CredentialManager.test.tsx` (regression tests for both credential bugs)
- `src/components/AddHostDialog.tsx` (protocol options, required-port logic)
- `src/components/HostList.tsx` (badge colors, Connect-button gating)
- `src/components/RemoteManager.tsx` (defensive branch for non-connectable protocols)
- `src/components/HostManagement.test.tsx` (new tests)
- `src-tauri/Cargo.lock`, `package-lock.json` (dependency updates)
- `CLAUDE.md`, `README.md`, `GEMINI.md`, `.claude/agents/security-reviewer.md` (this entry + doc sync)

### Comprehensive Test Coverage Expansion - Complete (March 6, 2026)

#### Overview
Grew the frontend test suite from 20 files / ~120 tests to **32 files / 208 tests** (all passing). Added 12 new test files covering untested components and extended 2 existing ones with missing error-path and callback scenarios.

#### New test files added

| File | Component | Highlights |
|------|-----------|------------|
| `ErrorBoundary.test.tsx` | ErrorBoundary | Class component error catching, crash UI, reload |
| `TopBar.test.tsx` | TopBar | All 7 `ViewId` → label mappings |
| `vault/VaultInitDialog.test.tsx` | VaultInitDialog | All 4 password rules (length, upper, lower, number), mismatch, strength indicator, loading state |
| `vault/VaultUnlockDialog.test.tsx` | VaultUnlockDialog | Empty password error, invoke flow, cancel button conditionality, visibility toggle |
| `vault/CredentialSelector.test.tsx` | CredentialSelector | `allowedTypes` filtering (key SFTP exclusion), `hostAddress` scoping, search, select → `get_credential`, manual entry, error |
| `vault/CredentialManager.test.tsx` | CredentialManager | Full CRUD dialogs, `search_credentials`, delete confirm, error state |
| `vault/KnownHostsManager.test.tsx` | KnownHostsManager | Search by host + fingerprint, remove confirm, trust status update, error |
| `vault/AuditLogViewer.test.tsx` | AuditLogViewer | Event type + result combobox filters (queried by index), resource info label, error |
| `vault/SshHostKeyPrompt.test.tsx` | SshHostKeyPrompt | New vs changed key UI, MITM warning, trust permanently toggle, copy to clipboard |
| `vault/VaultSettings.test.tsx` | VaultSettings | Settings load/save/error, lock vault, change password with all validation branches |
| `dashboard/AlertFeed.test.tsx` | AlertFeed | Dismiss, acknowledge, clear all, metrics summary from invoke, unacknowledged count |
| `dashboard/QuickConnectWidget.test.tsx` | QuickConnectWidget | Database mock, search, connect callback, "+N more" overflow |

#### Existing files extended

- **`HostManagement.test.tsx`**: Added onConnect/onSftp callback assertions, remove-with-confirm (using `vi.spyOn(window, 'confirm')`), duplicate detection display, empty state, filter no-match.
- **`NetworkScanner.test.tsx`**: Added CIDR validation error path, scan invocation failure error display, `initialResults` prop pre-population.

#### Key fixes discovered during implementation

- **`window.confirm` stubbing**: `vi.stubGlobal('confirm', ...)` did not reliably intercept calls in jsdom. All confirm stubs use `vi.spyOn(window, 'confirm').mockReturnValue(true/false)` with `.mockRestore()` cleanup.
- **Duplicate text matches**: Several components render the same text in both a heading and a button (e.g. "Unlock Vault", "Initialize Vault"). Switched to `getAllByText(...).length ≥ N` or role-scoped queries.
- **"Remove duplicates" button vs "Remove" button**: `getAllByRole('button', { name: /Remove/i })` matched the "Remove duplicates" header button (disabled) first. Fixed with exact-string `name: 'Remove'`.
- **Unlabelled `<select>` elements**: AuditLogViewer filter dropdowns have no `aria-label`/`id`, so `getByRole('combobox', { name: ... })` fails. Use `getAllByRole('combobox')[index]`.

#### Files modified
- `src/components/ErrorBoundary.test.tsx` (new)
- `src/components/TopBar.test.tsx` (new)
- `src/components/HostManagement.test.tsx` (extended)
- `src/components/NetworkScanner.test.tsx` (extended)
- `src/components/vault/VaultInitDialog.test.tsx` (new)
- `src/components/vault/VaultUnlockDialog.test.tsx` (new)
- `src/components/vault/CredentialSelector.test.tsx` (new)
- `src/components/vault/CredentialManager.test.tsx` (new)
- `src/components/vault/KnownHostsManager.test.tsx` (new)
- `src/components/vault/AuditLogViewer.test.tsx` (new)
- `src/components/vault/SshHostKeyPrompt.test.tsx` (new)
- `src/components/vault/VaultSettings.test.tsx` (new)
- `src/components/dashboard/AlertFeed.test.tsx` (new)
- `src/components/dashboard/QuickConnectWidget.test.tsx` (new)
- `CLAUDE.md` (test inventory table + patterns section)
- `AGENTS.md` (this entry)

---

### Light Theme App UI & Terminal Fixes - Complete (February 20, 2026)

#### Overview
Fixed app UI readability when switching to Light theme in Settings → Appearance. Reverted unnecessary terminal–app-theme sync and corrected Solarized Light terminal theme colors.

#### 1) App UI text in light mode ✅
- **Location**: `src/App.css`
- **Problem**: Selecting Light theme changed CSS variables (backgrounds) and `body` color, but components use Tailwind classes (`text-white`, `text-gray-*`) with fixed colors, so text stayed light on light background and was unreadable.
- **Fix**: Added `[data-theme="light"]` overrides for `.text-white`, `.text-gray-100` through `.text-gray-900`, `.text-black`, and `.text-accent` so app shell (sidebar, settings, headers, labels) uses dark text on light backgrounds. Scrollbar thumb hover adjusted for light theme.

#### 2) Terminal: reverted app-theme sync ✅
- **Location**: `src/components/TerminalComponent.tsx`, `src/components/SettingsView.tsx`
- **Change**: Removed needless coupling of terminal theme to app theme. Removed `app-theme-changed` event, `terminalThemeFromAppTheme()`, and `appTheme` state from TerminalComponent; removed `window.dispatchEvent('app-theme-changed')` from SettingsView. Terminal theme is again driven only by the `theme` prop (default `'default'`). Kept in-place theme/font update effect and init guard so changing appearance or future terminal theme/font settings do not disconnect SSH.

#### 3) Solarized Light terminal theme ✅
- **Location**: `src/components/TerminalComponent.tsx` (`TERMINAL_THEMES.solarizedLight`)
- **Fix**: Foreground set to `#586e75` (base01) per Solarized Light spec for body text on light background. Cursor set to `#073642` (base02) so it contrasts with both foreground and background and remains visible when over text (comment: "Cursor chosen for visibility on each background").

#### Files Modified
- `src/App.css` (light-theme text overrides)
- `src/components/TerminalComponent.tsx` (revert app-theme sync; solarizedLight foreground/cursor)
- `src/components/SettingsView.tsx` (remove app-theme-changed dispatch)

---

### Comprehensive Code Audit Fixes - Complete (February 18, 2026)

#### Overview
Addressed all 7 findings from a full codebase audit (Critical/High/Medium). See `CODEBASE_AUDIT_REPORT.md` for the original audit; fixes summarized below.

#### AUD-01 (Critical): DB import/export lifecycle safety ✅
- **Location**: `src-tauri/src/lib.rs`
- **Problem**: `export_database` and `import_database` used raw `std::fs::copy` while monitoring/scheduler held active SQLite connections, risking corrupted exports/imports.
- **Fix**: Export uses `VACUUM INTO 'dest'` on a live connection for a consistent snapshot. Import validates source as SQLite, uses atomic copy-to-temp then rename, and logs a restart-required warning.

#### AUD-02 (High): Discovery thread leak / duplicate workers ✅
- **Location**: `src-tauri/src/discovery.rs`, `src-tauri/src/lib.rs`
- **Problem**: Each `start_discovery` call spawned a new thread with no singleton guard.
- **Fix**: Added `DiscoveryState` with `Arc<AtomicBool>`; `start_mdns_discovery` checks-and-sets the flag and returns early if already running. A `DropGuard` in the thread clears the flag on exit.

#### AUD-03 (High): SFTP + SSH-key credential mismatch ✅
- **Location**: `src/components/vault/CredentialSelector.tsx`, `src/components/RemoteManager.tsx`, `src/components/ScheduledTasksView.tsx`, `src-tauri/src/sftp.rs`
- **Problem**: UI allowed `ssh_key` credentials for SFTP, but backend SFTP only supports password auth.
- **Fix**: Added `allowedTypes` prop to CredentialSelector; SFTP flows pass `['ssh']` to hide key credentials. ScheduledTasksView filters out `ssh_key` for SFTP task types. Backend `require_password_auth()` guard returns a clear error if password is empty.

#### AUD-04 (High): Scheduler silent failure on run-result persistence ✅
- **Location**: `src-tauri/src/scheduler.rs`
- **Problem**: `set_run_result` errors were ignored; failed DB writes could leave `last_run_at` stale and cause duplicate runs.
- **Fix**: Log errors on `set_run_result` failure. Added in-memory `last_executed` map; tasks that ran within the last 60s are skipped even if persistence failed.

#### AUD-05 (Medium): Monitoring network throughput unit mismatch ✅
- **Location**: `src/components/MonitoringView.tsx`
- **Problem**: Backend sends `network_rx_mb`/`network_tx_mb` in MB; frontend divided by 1024 again, underreporting.
- **Fix**: Removed the extra `/1024`; display shows correct MB/s.

#### AUD-06 (Medium): Credential ID type drift ✅
- **Location**: `src/components/vault/CredentialManager.tsx`
- **Problem**: Frontend typed credential `id` as `number`; backend uses string (UUID/integer serialized as string).
- **Fix**: `CredentialSummary` and `Credential` now use `id: string`; handlers updated and `.toString()` removed.

#### AUD-07 (Medium): Mutex poison panic propagation ✅
- **Location**: `src-tauri/src/monitoring.rs`
- **Problem**: Production `.lock().unwrap()` could cascade panics after a poisoned mutex.
- **Fix**: All 12 production lock sites use `.lock().unwrap_or_else(|poisoned| poisoned.into_inner())` for recovery.

#### Files Modified (audit round)
- `src-tauri/src/lib.rs` (export/import, discovery state)
- `src-tauri/src/discovery.rs` (DiscoveryState, singleton guard)
- `src-tauri/src/scheduler.rs` (set_run_result logging, last_executed map)
- `src-tauri/src/monitoring.rs` (poison recovery)
- `src-tauri/src/sftp.rs` (require_password_auth)
- `src/components/vault/CredentialSelector.tsx`, `RemoteManager.tsx`, `ScheduledTasksView.tsx`
- `src/components/vault/CredentialManager.tsx` (id type, getErrorMessage already present)
- `src/components/MonitoringView.tsx` (network units)

---

### Workflow Scheduling (Scheduled Tasks) - Complete (February 18, 2026)

#### Overview
Cron-based scheduled SSH command execution on saved hosts, with Automation UI, last-run status/result storage, and manual "Run now."

#### 1) Backend scheduler ✅
- **Location**: `src-tauri/src/scheduler.rs`
- **Migrations**: `010_scheduled_tasks.sql` (table `scheduled_tasks`: id, name, cron_expression, host_id, command, credential_id, enabled, last_run_at, created_at, updated_at); `011_scheduled_task_run_result.sql` (last_run_status, last_run_error, last_run_output).
- **Loop**: `start_scheduler(app)` spawns a task (Tauri async runtime) that every 60s opens DB, loads enabled tasks, for each due task (cron `is_due`) resolves host and optional credential, runs `ssh_exec::execute_ssh_command` (password or SSH key), then `set_run_result` (success/failure, error, truncated output).
- **CRUD**: `list_scheduled_tasks`, `get_scheduled_task`, `add_scheduled_task`, `update_scheduled_task`, `remove_scheduled_task` (all in scheduler.rs; Tauri commands in lib.rs open DB and call these).
- **Manual run**: `run_scheduled_task_now(app, task_id)` loads task, runs once, updates last_run_*, returns `TaskRunResult { success, output, error }`. Uses `run_one_task` (no `conn` across await) so the spawn future is `Send`.

#### 2) Automation view ✅
- **Location**: `src/components/ScheduledTasksView.tsx`
- **Features**: List tasks; add/edit form (name, cron expression, host dropdown, command, optional credential, enabled); delete; **Run now** button (play icon) with loading state; result panel after run (success/failure, output/error, dismissible).
- **Last run display**: Each task card shows last run time, status (Success / Failed: message), and optional output snippet. Data from `last_run_status`, `last_run_error`, `last_run_output`.

#### 3) Navigation and commands ✅
- **Sidebar/TopBar/Layout**: New view `automation` (Clock icon, "Automation"); `ScheduledTasksView` rendered when active.
- **Invoke**: Frontend uses camelCase for command args (Tauri maps to Rust snake_case). Commands: `list_scheduled_tasks`, `get_scheduled_task`, `add_scheduled_task`, `update_scheduled_task`, `remove_scheduled_task`, `run_scheduled_task_now`.

#### 4) Scheduler tests and cron format ✅
- **Tests** (`scheduler::tests`): In-memory DB with `hosts` + `scheduled_tasks` (010/011). Tests: `test_is_due` (6-field cron), `test_scheduler_crud_and_run_result` (add/list/get/update/set_run_result/remove), `test_load_enabled_tasks_only_returns_enabled`, `test_load_task_by_id`, `test_set_run_result_truncates_long_output`. All 5 pass.
- **Cron format**: The `cron` crate 0.12 expects **6 fields** (sec min hour day month dow). UI default and placeholder updated to `0 0 9 * * *` (9:00 daily); label "sec min hour day month dow". Existing 5-field expressions never run until edited to 6-field.

#### Files Modified/Added
- `src-tauri/migrations/010_scheduled_tasks.sql`, `011_scheduled_task_run_result.sql`
- `src-tauri/src/scheduler.rs` (new)
- `src-tauri/src/lib.rs` (migration 010/011, mod scheduler, commands, start_scheduler in setup)
- `src-tauri/src/ssh_exec.rs` (execute_ssh_command: optional key_path, private_key, key_passphrase for SSH key auth)
- `src/components/ScheduledTasksView.tsx` (new)
- `src/components/Sidebar.tsx`, `TopBar.tsx`, `Layout.tsx` (automation view)
- Test mocks: `Layout.test.tsx`, `App.test.tsx` (list_scheduled_tasks, get_saved_hosts)
- `docs/CORE_WORKFLOWS.md` (new) — user-facing core workflows (vault, SSH, scheduled tasks, SFTP, monitoring, discovery; cron quick reference)

---

### SSH Terminal Flow Control & Dashboard / UX Fixes - Complete (February 18, 2026)

#### Overview
Fixed SSH terminal hanging when running consecutive or simultaneous commands in one or more sessions, improved dashboard System Health data, Real-time Metrics disk display, and various small fixes.

#### 1) SSH terminal hang (consecutive / simultaneous commands) ✅
- **Location**: `src-tauri/src/ssh.rs`
- **Root Cause**: Data was consumed via `Handler::data()` only; russh also queues data in the Channel's internal buffer for `Channel::wait()`. Nobody drained that buffer, so it filled, the connection driver blocked, and the SSH connection stalled (especially with high-output commands like `apt upgrade` or when running commands in two terminals at once).
- **Fix**: Refactored to a **unified I/O task** that owns the channel and consumes data via **`Channel::wait()`** only (removed `Handler::data()` override). This properly drains the internal buffer so russh can send `SSH_MSG_CHANNEL_WINDOW_ADJUST` and keep data flowing. The I/O task also handles writes (`write_tx`) and resizes (`resize_tx`) via unbounded channels; `write_ssh` and `resize_ssh` return immediately.
- **SshConnection**: No longer stores `channel`; added `resize_tx`. Writer and resize are queued to the I/O task.

#### 2) SSH output batching (progress / line flush) ✅
- **Location**: `src-tauri/src/ssh.rs` (batcher task)
- **Change**: Flush to frontend when chunk contains `\r` or `\n` (in addition to 4 KB or 4 ms), and reduced flush interval from 8 ms to 4 ms, so apt progress and line output appear without delay.

#### 3) SSH packet size and launcher warning ✅
- **Location**: `src-tauri/src/ssh.rs`, `src-tauri/src/launcher.rs`
- **ssh.rs**: `maximum_packet_size` reduced from 128 KB to 32 KB (must not exceed TCP max 65535); eliminates russh "Maximum packet size should not larger than a TCP packet" errors.
- **launcher.rs**: Removed unused `use super::*` in test module to clear compiler warning.

#### 4) Password input autocomplete (console warning) ✅
- **Location**: `CredentialPrompt.tsx`, `VaultUnlockDialog.tsx`, `VaultInitDialog.tsx`, `VaultSettings.tsx`, `CredentialManager.tsx`
- **Fix**: Added `autoComplete="current-password"` or `autoComplete="new-password"` (or `off` for key passphrase) to all password inputs to satisfy Chromium and remove DOM autocomplete warnings.

#### 5) Dashboard System Health widget ✅
- **Location**: `src/components/dashboard/SystemHealthWidget.tsx`
- **Hosts Online**: Now fetches `get_remote_hosts_health` and shows count of reachable saved hosts; refreshes every 30 s.
- **Vault Auto-lock**: Shows actual timeout from `get_vault_settings` (e.g. "15 min") instead of hardcoded "15:00".
- **Three-dots menus**: Removed non-functional `MoreHorizontal` buttons from System Health and Real-time Metrics cards.

#### 6) Real-time Metrics — all attached disks ✅
- **Location**: `src-tauri/src/monitoring.rs`, `src/components/dashboard/SystemHealthWidget.tsx`
- **Backend**: Added `DiskInfo` (name, mount_point, total_gb, used_gb, free_gb, usage_percent) and `disks: Vec<DiskInfo>` to `SystemMetrics`; `get_disk_list(disks)` builds per-disk list from sysinfo `Disks`. Persisted in metrics metadata.
- **Frontend**: Real-time Metrics card shows one row per disk (mount_point or name, used/total, progress bar); falls back to single aggregate row if `disks` is empty.

#### 7) tauri-dev.js spawn on Windows ✅
- **Location**: `scripts/tauri-dev.js`
- **Problem**: Using `npx.cmd` with `shell: false` caused `spawn EINVAL` on Windows.
- **Fix**: Reverted to `shell: true` on Windows so `npm run tauri dev` runs; DEP0190 deprecation warning may reappear but command works.

#### Files Modified
- `src-tauri/src/ssh.rs` (major refactor)
- `src-tauri/src/monitoring.rs`
- `src-tauri/src/launcher.rs`
- `scripts/tauri-dev.js`
- `src/components/dashboard/SystemHealthWidget.tsx`
- `src/components/CredentialPrompt.tsx`
- `src/components/vault/VaultUnlockDialog.tsx`
- `src/components/vault/VaultInitDialog.tsx`
- `src/components/vault/VaultSettings.tsx`
- `src/components/vault/CredentialManager.tsx`

---

### Credential Edit & Type-Switch Fixes - Complete (February 16, 2026)

#### Overview
Fixed credential update flow so SSH key and password-based credentials can be edited and type-switched without leaving stale data in the database. The backend now accepts and persists all credential fields on update, updates the `credential_type` column, and explicitly clears the opposite auth path when switching types.

#### 1) update_credential SSH key support ✅
- **Location**: `src-tauri/src/vault/credentials.rs`, `src-tauri/src/lib.rs`, `src/components/vault/CredentialManager.tsx`
- **Problem**: Only password-based credentials could be updated; SSH key fields (`key_path`, `private_key`, `key_passphrase`) were not sent or persisted when editing.
- **Fix**:
  - Backend `update_credential`: added optional `key_path`, `private_key`, `key_passphrase`; when provided, encrypt and update (empty string clears columns/NULL).
  - Tauri command: added same optional params and pass-through.
  - Frontend edit payload: when `credential_type === 'ssh_key'`, sends `key_path`, `private_key`, `key_passphrase` from form (empty string = clear).

#### 2) credential_type column on update ✅
- **Location**: `src-tauri/src/vault/credentials.rs`, `src-tauri/src/lib.rs`, `src/components/vault/CredentialManager.tsx`
- **Problem**: Credential type lived only in metadata JSON; DB column was never updated, so stored type could diverge from user selection.
- **Fix**:
  - Backend: added `credential_type: Option<String>` to `update_credential`; when `Some`, run `UPDATE credentials SET credential_type = ? ...`.
  - Frontend: sends `credential_type: formData.credential_type` in edit payload.

#### 3) Clearing fields when editing (empty string vs null) ✅
- **Location**: `src/components/vault/CredentialManager.tsx`
- **Problem**: Frontend sent `formData.key_path || null` etc., so cleared fields became `null`; backend skipped updates for `None`, leaving stale key data.
- **Fix**: For SSH key type, send raw form values (`formData.key_path`, etc.) so empty string is sent; backend already treats `Some("")` as clear.

#### 4) Clearing opposite auth when switching type ✅
- **Location**: `src/components/vault/CredentialManager.tsx`, `src-tauri/src/vault/credentials.rs`
- **Problem**: Switching ssh_key → password left encrypted key/passphrase in DB; switching password → ssh_key left encrypted password in DB.
- **Fix**:
  - When switching to password-based: frontend sends `key_path: ''`, `private_key: ''`, `key_passphrase: ''` so backend clears key columns.
  - When switching to SSH key: frontend sends `password: ''`. Backend now treats empty password as clear (see 5).

#### 5) Backend: clear password columns instead of storing encrypted empty ✅
- **Location**: `src-tauri/src/vault/credentials.rs`, `src-tauri/migrations/009_nullable_password.sql`, `src-tauri/src/lib.rs`
- **Problem**: Sending `password: ''` caused backend to encrypt and store empty string instead of clearing the password columns (unlike key_path/private_key which clear to NULL).
- **Fix**:
  - **Migration 009**: Recreated `credentials` with `encrypted_password`, `nonce`, `tag` nullable (BLOB without NOT NULL).
  - **update_credential**: When `password` is `Some("")`, run `UPDATE ... SET encrypted_password = NULL, nonce = NULL, tag = NULL`; otherwise encrypt and store as before.
  - **get_credential**: Read password columns as `Option<Vec<u8>>`; if any is `None`, return `password: String::new()`; otherwise decrypt as before.

#### Files Modified
- `src-tauri/src/vault/credentials.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/migrations/009_nullable_password.sql` (new)
- `src/components/vault/CredentialManager.tsx`

#### Verification
- `cargo test vault::credentials::tests::` — all 7 credential tests passing.

---

### Host Management + AI Assistant Context Updates - Complete (February 15, 2026)

#### Overview
Completed UX and context-quality improvements for Remote host management and the AI Assistant. Focus was on reducing host list duplication friction and giving the assistant direct visibility into real app network state.

#### 1) Remote saved hosts duplicate cleanup UX ✅
- **Location**: `src/components/HostList.tsx`
- **Problem**: Duplicate saved hosts could accumulate and required one-by-one manual cleanup.
- **Fix**:
  - Added bulk **Remove duplicates** action (group key: `address + protocol + port`).
  - Kept first occurrence, removed remaining duplicates with confirmation prompt.
  - Added duplicate count indicator (`X duplicate(s) detected`) next to action button.
  - Dispatches `hostsUpdated` after cleanup so host views stay in sync.
- **Verification**:
  - Ran `npm test -- src/components/HostManagement.test.tsx` (all tests passing).

#### 2) AI assistant naming + network awareness context ✅
- **Location**: `src/components/AIAssistant.tsx`, `src/components/AIAssistant.test.tsx`
- **Problem**:
  - Assistant UI still used the previous product name (Titan).
  - Chat requests lacked app-discovered network context, reducing usefulness for troubleshooting.
- **Fix**:
  - Renamed visible AI branding to **Quasar AI Assistant** and input placeholder to **Ask Quasar AI...**.
  - Added `buildNetworkContextMessage()` to inject a system context message on each chat request.
  - Context now includes:
    - scan status (`is_scanning`)
    - scan progress (`get_scan_progress`)
    - discovered hosts snapshot (`get_discovered_hosts`)
    - saved remote hosts snapshot (SQLite `hosts` table)
    - discovered/saved overlap count by address
  - Used `Promise.allSettled(...)` so partial failures do not block context generation.
- **Verification**:
  - Ran `npm test -- src/components/AIAssistant.test.tsx` (all tests passing).

#### 3) Desktop remote-control direction decision ✅
- **Decision**: Deferred in-app VNC integration for now.
- **Rationale**:
  - SSH covers command-line management needs in Quasar.
  - Desktop support can be handled by external tooling (RustDesk) without increasing in-app complexity.
- **Future Option**:
  - Add one-click "Open in RustDesk" host action if desired.

#### Files Modified
- `src/components/HostList.tsx`
- `src/components/AIAssistant.tsx`
- `src/components/AIAssistant.test.tsx`

### Systematic Audit Fixes - Complete (February 15, 2026)

#### Overview
Completed a strict issue-by-issue remediation pass from comprehensive code review findings. Each issue was fixed and validated before proceeding to the next.

#### 1) Scanner stop semantics and detached task leak ✅
- **Location**: `src-tauri/src/scanner.rs`
- **Root Cause**: `scan_network` could break on `stop_signal` and return before awaiting already-spawned scan tasks, leaving detached background work.
- **Fix**:
  - Added shared `drain_scan_futures(...)` helper to centralize result draining/progress updates.
  - Ensured all pending futures are drained before function return, including stop path.
  - Kept concurrency batching behavior (`>= 50`) while preserving final drain guarantee.
- **Verification**:
  - Added regression test `test_drain_scan_futures_drains_pending_tasks`.
  - Ran `cargo test scanner::tests::` (all scanner tests passing).

#### 2) SSH idle timeout stale session cleanup ✅
- **Location**: `src-tauri/src/lib.rs` (background SSH timeout task)
- **Root Cause**: Timeout path sent disconnect signals but did not remove sessions from `SshState.sessions`, leaving stale state and repeated timeout handling.
- **Fix**:
  - Timeout loop now acquires mutable session map, identifies timed-out IDs, removes those sessions, and collects both `disconnect_tx` and `stats_cancel_tx`.
  - Sends stats cancellation and disconnect after removal, then emits timeout event.
- **Verification**:
  - Ran full Rust test suite: `cargo test` (all tests passing).

#### 3) Credential nonce/tag malformed data panic hardening ✅
- **Location**: `src-tauri/src/vault/credentials.rs`
- **Root Cause**: `copy_from_slice` on nonce/tag vectors could panic when persisted DB data length was invalid.
- **Fix**:
  - Added explicit nonce length check (`12`) and auth tag length check (`16`) before copy.
  - Returns structured error instead of panicking on malformed encrypted rows.
- **Verification**:
  - Added tests:
    - `test_get_credential_invalid_nonce_length_returns_error`
    - `test_get_credential_invalid_tag_length_returns_error`
  - Ran `cargo test vault::credentials::tests::test_get_credential_invalid_` (both passing).

#### 4) Vault initialization error rendering quality ✅
- **Location**: `src/components/vault/VaultProvider.tsx`, `src/components/vault/VaultProvider.test.tsx`
- **Root Cause**: Error handling cast unknown errors directly to string, causing poor user-facing messages and inconsistent error UI transitions.
- **Fix**:
  - Added `getErrorMessage(error: unknown)` normalizer for string/Error/object-with-message cases.
  - Preserved `isInitialized = null` on init-check failure so explicit error screen renders reliably.
  - Added frontend regression test for Error-object rejection message rendering.
- **Verification**:
  - Ran `npm test -- src/components/vault/VaultProvider.test.tsx` (all 7 tests passing).

#### Files Modified
- `src-tauri/src/scanner.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/vault/credentials.rs`
- `src/components/vault/VaultProvider.tsx`
- `src/components/vault/VaultProvider.test.tsx`

### UI Refactor - Complete (February 12, 2026)

#### Overview
Complete visual overhaul to match new Quasar design mockup. Shifted from gray/blue palette to dark navy/cyan aesthetic across all components. No functional changes — purely visual and layout restructuring.

#### Phase 1: Theme & Color Palette ✅
- **Location**: `src/App.css`
- **Changes**: Updated CSS custom properties to new navy/cyan palette
  - `--color-bg-root: #0f1923` (dark navy)
  - `--color-bg-card: #1a2332` (card background)
  - `--color-bg-sidebar: #0a1628` (sidebar)
  - `--color-accent: #00d4ff` (cyan accent)
  - `--color-border: #1e3a5f` (navy border)
  - Scrollbar thumb hover updated to `#2a3f5f`

#### Phase 2: Sidebar Restyle ✅
- **Location**: `src/components/Sidebar.tsx`, `src/assets/quasar-logo.svg`
- **Changes**:
  - Created custom Quasar SVG logo (orbital rings + core sphere + jet beam)
  - Replaced Lock icon branding with logo image + "QUASAR" text
  - Active nav style changed from `bg-accent/10 text-accent` to filled `bg-accent text-white` pill
  - Footer replaced: "Admin Nexus" user info → Vault status badge using `useVault` hook
  - All `border-gray-800` → `border-border`

#### Phase 3: Top Bar Restyle ✅
- **Location**: `src/components/TopBar.tsx`
- **Changes**:
  - Replaced breadcrumb navigation with bold view title (`viewLabels` map)
  - Updated search placeholder to "Search hosts, credentials..."
  - Restyled action buttons with new palette (bell notification, user avatar)
  - All borders/backgrounds updated to theme variables

#### Phase 4: Dashboard Layout Restructure ✅
- **Location**: `src/components/dashboard/DashboardView.tsx`, `src/components/dashboard/SystemHealthWidget.tsx`
- **Changes**:
  - Hero section: Network Topology card (~60% height) with inline List/Topology toggle
  - Bottom row: 4 equal cards (Real-time Metrics, Active Sessions, Recent Activity, System Health)
  - Removed `DiscoveryWidget` and `NetworkMapWidget` from dashboard rendering
  - `SystemHealthWidget` now accepts `variant` prop: `'metrics'` (CPU/Memory/Disk progress bars) or `'summary'` (hosts online, alerts, vault auto-lock status rows)
- **Location**: `src/components/dashboard/QuickConnectWidget.tsx`
  - Renamed header to "Active Sessions", updated palette
- **Location**: `src/components/dashboard/AlertFeed.tsx`
  - Renamed header to "Recent Activity", updated palette, added `max-h-56`

#### Phase 5: Topology View Restyle ✅
- **Location**: `src/components/NetworkTopologyView.tsx`
- **Changes**:
  - Device colors updated: Server→sky, Router→violet, Printer→pink, Workstation→teal, Unknown→slate
  - Edge color updated to `#1e3a5f` (navy)
  - Controls/search/legend/info panels: `bg-bg-card border-border` with `backdrop-blur-sm`
  - Container changed from fixed `h-[600px]` to `h-full` (fills parent)
  - Removed outer border (parent card provides it)

#### Phase 6: Status Bar Footer ✅
- **Location**: `src/components/Layout.tsx`
- **Changes**:
  - Added `<footer>` status bar (h-7) at bottom of main content area
  - Shows connection status indicator (green dot + "Connected")
  - Shows "Last scan: --" placeholder on right side
  - Styled with `bg-bg-sidebar border-t border-border`

#### Phase 7: Test Updates ✅
- **Files Updated**: `Sidebar.test.tsx`, `Dashboard.test.tsx`, `Layout.test.tsx`, `App.test.tsx`
- **Changes**:
  - Added `useVault` mock to Sidebar, Layout, and App tests
  - Added `NetworkTopologyView` component mock to Dashboard, Layout, and App tests (vis-network can't render in JSDOM)
  - Updated assertions: active nav class `bg-accent`, "Vault: Unlocked" branding, "Network Topology" header, "List"/"Topology" toggle labels, "Real-time Metrics" widget
- **Result**: All 69 tests passing

#### Files Modified
- `src/App.css` — Theme palette
- `src/assets/quasar-logo.svg` — New logo (created)
- `src/components/Sidebar.tsx` — Logo, nav style, vault footer
- `src/components/TopBar.tsx` — View title, search, buttons
- `src/components/Layout.tsx` — Status bar footer
- `src/components/dashboard/DashboardView.tsx` — Hero + 4-card layout
- `src/components/dashboard/SystemHealthWidget.tsx` — Variant prop, progress bars
- `src/components/dashboard/QuickConnectWidget.tsx` — Palette update
- `src/components/dashboard/AlertFeed.tsx` — Palette update
- `src/components/NetworkTopologyView.tsx` — Colors, sizing, controls
- `src/App.test.tsx` — Mocks added
- `src/components/Sidebar.test.tsx` — Mocks + assertions updated
- `src/components/dashboard/Dashboard.test.tsx` — Mocks + assertions updated
- `src/components/Layout.test.tsx` — Mocks added

---

### Critical Bug Fixes - Complete (February 3, 2026)

#### React Duplicate Key Warning Fix ✅
- **Issue**: NetworkScanner showing duplicate key warnings for hosts with same IP
- **Root Cause**: `scan_result` event listener appending results without deduplication
- **Location**: `src/components/NetworkScanner.tsx:68-81`
- **Fix**: Filter existing entries with same IP before adding new results
- **Impact**: Eliminated React console warnings, improved component stability

#### Database Migration System Fix ✅
- **Issue**: Fresh installations failing with "no such table: hosts" and "no such table: credentials"
- **Root Cause**: Missing initial schema migration (001), migration 005 incorrectly excluded
- **Fixes**:
  - Created `migrations/001_initial_schema.sql` with base schema for `hosts` and `credentials_new` tables
  - Re-enabled migration 005 in migration chain (needed for credentials consolidation)
  - Updated `src-tauri/src/lib.rs:27-35` with complete migration sequence
- **Migration Order**: 001 (initial) → 003 (vault) → 004 (monitoring) → 005 (consolidate) → 006 (discovered hosts)
- **System**: Uses `rusqlite_migration` crate for proper version tracking (migrations run once per database)
- **Impact**: Fresh installations now work correctly, all required tables created

#### Code Cleanup ✅
- **Location**: `src-tauri/src/ssh.rs:13`
- **Fix**: Removed unused `crate::errors` import
- **Impact**: Reduced compiler warnings

#### Testing
- ✅ Application compiles successfully
- ✅ Database migrations run correctly on fresh installations
- ✅ React duplicate key warnings resolved
- ✅ Only minor unused function warnings remain (future features)

---

### Network Scanner Enhancement - Complete (February 2, 2026)

#### Phase 1: Backend Enhancement ✅
- **Location**: `src-tauri/src/scanner.rs`, `src-tauri/src/host_tracker.rs`
- **Implementation**: Enhanced network discovery with comprehensive host data collection
- **Enhanced Data Structures**:
  - `ServiceInfo` - Port, protocol, service name, version detection
  - `ScanResult` - Expanded with hostname, device_type, services[], mac_address, vendor, last_seen
  - Port scanning expanded from 5 to 13 common ports (SSH, Telnet, HTTP, HTTPS, SMB, MySQL, RDP, PostgreSQL, Redis, HTTP-Alt, Printer ports)
- **Features**:
  - Device type detection (server, router, printer, workstation, unknown) based on port patterns
  - Service identification for all scanned ports
  - Hostname resolution via reverse DNS (for alive hosts)
  - Automatic host persistence to database
- **Database Migration**: `migrations/006_discovered_hosts.sql`
  - `discovered_hosts` - IP, hostname, MAC, device type, vendor, timestamps, scan count
  - `host_services` - Port, protocol, service, version per host with detection timestamps
- **HostTracker Module**: Full CRUD operations for discovered hosts
  - `save_host()` - Auto-saves/updates hosts during scans
  - `get_host()`, `list_hosts()`, `search_hosts()`, `delete_host()`
  - Service tracking with first/last detected timestamps
- **Tauri Commands**: 
  - `get_discovered_hosts`, `get_host_details`, `search_discovered_hosts`, `delete_discovered_host`

#### Phase 2: Frontend Host Detail Components ✅
- **Location**: `src/components/NetworkScanner.tsx`, `src/components/HostDetailDialog.tsx`
- **Enhanced NetworkScanner**:
  - Updated interface with all new ScanResult fields
  - Device type icons (Server, Router, Printer, Laptop, Unknown)
  - Rich host cards with device icon, hostname, service count, latency
  - Clickable hosts with hover effects
  - Limited port display (first 3 + count)
- **HostDetailDialog Component**: Comprehensive modal with 3 tabs
  - **Overview Tab**: IP, hostname, MAC, device type, vendor, status, timestamps, open ports
  - **Services Tab**: Detailed service list with port, protocol, service name, version, risk color-coding
  - **Actions Tab**: Connect, save to hosts, delete from discovered hosts, quick copy actions
  - Copy-to-clipboard functionality with visual feedback
- **Dashboard Integration**: `src/components/dashboard/DashboardView.tsx`
  - State management for selected host
  - Handlers for click, connect, save, delete actions
  - Seamless integration with existing quick connect flow

#### Phase 3: Network Topology Visualization ✅
- **Location**: `src/components/NetworkTopologyView.tsx`
- **Library**: vis-network + vis-data (MIT licensed)
- **Interactive Force-Directed Graph**:
  - Gateway node (center) in cyan
  - Host nodes color-coded by device type (Blue: Server, Purple: Router, Pink: Printer, Green: Workstation, Gray: Unknown)
  - Node size scales with service count
  - Edge thickness based on latency (thinner = faster)
- **Features**:
  - Zoom controls (in/out, fit to screen)
  - Physics toggle (freeze/unfreeze layout)
  - Search with auto-focus and highlight
  - Hover tooltips with host details
  - Click → Opens HostDetailDialog
  - Double-click → Quick connect
  - Legend showing device types
  - Info panel with host count and instructions
- **Dashboard Integration**:
  - View mode toggle buttons (List / Topology)
  - Conditional rendering between NetworkScanner and NetworkTopologyView
  - Shared state for discovered hosts
  - Consistent interactions across both views

#### Impact
- **Network Discovery**: Comprehensive host information collection with 13-port scanning
- **Persistence**: All discovered hosts stored in database with history tracking
- **Visualization**: Interactive network topology with force-directed graph layout
- **User Experience**: Toggle between list and topology views, click for details, double-click to connect
- **Data Richness**: Device type classification, service detection, hostname resolution

---

## Previous Implementations (January 31, 2026)

### Security & Credential Vault - Phase 1, 2 & 3 Complete

#### Phase 1: Vault Infrastructure ✅
- **Location**: `src-tauri/src/vault.rs` (415 lines)
- **Implementation**: Master password-based vault with Argon2id key derivation
- **Features**:
  - VaultState with thread-safe RwLock for concurrent access
  - Master password key derivation (64 MB memory, 3 iterations, 4 threads)
  - Secure memory clearing with `zeroize` crate (master key wiped on lock)
  - Rate limiting & lockout policy (5/10/15 failed attempts → 5/15/60 min lockout)
  - Auto-lock timer infrastructure (configurable timeout, default 15 min)
  - Audit logging for all vault operations
- **Database Migration**: `migrations/003_security_vault.sql`
  - `vault_settings` - Master password hash, salt, configuration
  - `security_audit_log` - All credential access tracking
  - `ssh_known_hosts` - Ready for Phase 3
  - `credentials_new` - Enhanced schema with encryption fields
- **Tauri Commands**: 
  - `is_vault_initialized`, `initialize_vault`, `unlock_vault`, `lock_vault`
  - `is_vault_locked`, `get_vault_settings`, `update_vault_settings`
- **Tests**: 3/3 passing (initialization, unlock/lock, invalid password)

#### Phase 2: Credential Management Backend ✅
- **Location**: `src-tauri/src/vault/credentials.rs` (450 lines)
- **Implementation**: Full CRUD operations with AES-256-GCM encryption
- **Features**:
  - `CredentialManager` - Handles all credential operations
  - `Credential` - Full credential with decrypted password
  - `CredentialSummary` - Metadata only (safe to list without vault unlock)
  - All passwords encrypted at rest with unique nonce/tag per credential
  - Automatic last_used_at tracking on credential access
  - Comprehensive audit logging (create, read, update, delete)
  - Search functionality with fuzzy matching
- **Tauri Commands**:
  - `add_credential` - Encrypts password, requires unlocked vault
  - `get_credential` - Decrypts password, requires unlocked vault, logs access
  - `list_credentials` - Returns summaries (no passwords)
  - `update_credential` - Re-encrypts password if changed
  - `delete_credential` - Removes with audit logging
  - `search_credentials` - Fuzzy search by name/username/type
- **Tests**: 5/5 passing (add/get, list, update, delete, search)

#### Security Model
- **Encryption**: AES-256-GCM with unique 12-byte nonce per credential
- **Key Derivation**: Argon2id (memory-hard, resistant to GPU attacks)
- **Authentication**: 16-byte tag ensures data integrity
- **Audit Trail**: All operations logged with timestamps
- **Memory Safety**: Master key zeroized on vault lock/drop
- **Access Control**: Vault must be unlocked for password operations

#### Phase 3: SSH Host Key Verification ✅
- **Location**: `src-tauri/src/vault/ssh_keys.rs` (415 lines)
- **Implementation**: Known hosts management with fingerprint verification
- **Features**:
  - `SshKeyManager` - Manages SSH host key verification
  - Fingerprint-based host key verification (prevents MITM attacks)
  - Trust status tracking (Trusted, Unknown, Changed, Rejected)
  - Automatic detection of changed host keys
  - Database storage in `ssh_known_hosts` table
  - Thread-safe with `std::sync::Mutex` (compatible with Tauri async)
- **Tauri Commands**:
  - `verify_ssh_host_key` - Verify a host's key by fingerprint
  - `trust_ssh_host_key` - Add/update a host key with trust status
  - `get_known_ssh_hosts` - List all known hosts
  - `remove_ssh_host_key` - Remove a host key
  - `update_ssh_host_trust` - Update trust status for a host
- **Tests**: 3/3 passing (manager creation, unknown host, trust and verify)

#### Phase 4: Vault UI Components ✅
- **Location**: `src/components/vault/` (5 components)
- **Implementation**: Complete React UI for vault and credential management
- **Components**:
  - `VaultProvider` - Global vault state management with React Context
  - `VaultInitDialog` - First-time vault setup with password strength validation
  - `VaultUnlockDialog` - Master password entry with show/hide toggle
  - `CredentialManager` - Full CRUD UI with search, view, edit, delete
  - `CredentialSelector` - Smart credential picker for SSH connections
- **Features**:
  - Application startup vault check (init or unlock)
  - Password strength meter with real-time validation
  - Credential cards with type badges and metadata display
  - Search and filter credentials by name/username/host
  - Copy-to-clipboard for usernames and passwords
  - Integrated with RemoteManager for seamless SSH connections
  - Fallback to manual password entry if vault locked
- **Integration**:
  - Added "Security" view to sidebar navigation
  - VaultProvider wraps entire app in `main.tsx`
  - RemoteManager checks vault status before showing credential selector
  - Automatic fallback to manual entry if vault unavailable
- **UI/UX**:
  - Consistent design with existing Quasar aesthetic
  - Modal dialogs with backdrop blur and animations
  - Responsive grid layout for credential cards
  - Color-coded credential types (SSH, RDP, Database, API, Other)

#### Phase 5: SSH Host Key UI ✅
- **Location**: `src/components/vault/` (3 new components + 1 hook)
- **Implementation**: Complete SSH host key verification UI with MITM detection
- **Components**:
  - `SshHostKeyPrompt` - Trust prompt for unknown/changed host keys
  - `KnownHostsManager` - Management interface for known SSH hosts
  - `SecurityView` - Tabbed interface combining Credentials and Known Hosts
  - `useSshHostKeyVerification` - React hook for host key verification flow
- **Features**:
  - Unknown host key prompt with fingerprint display
  - Changed host key warning (MITM detection) with visual alerts
  - Copy-to-clipboard for fingerprint verification
  - Permanent vs temporary trust options
  - Known hosts list with search and filtering
  - Trust status indicators (Trusted, Changed, Rejected, Unknown)
  - Remove host keys and update trust status
  - First seen / last seen timestamps
- **Integration**:
  - SecurityView provides tabbed interface (Credentials | Known Hosts)
  - Host key verification hook integrated into RemoteManager
  - Prompts appear before SSH connection establishment
  - Backend verification commands already implemented (Phase 3)
- **Security**:
  - Visual differentiation between unknown and changed keys
  - Strong warnings for potential MITM attacks
  - Fingerprint comparison workflow
  - User education about security risks

#### Phase 6: Security Settings & Audit Log UI ✅
- **Location**: `src/components/vault/` (2 new components)
- **Implementation**: Vault settings management and security audit log viewer
- **Components**:
  - `VaultSettings` - Vault configuration management interface
  - `AuditLogViewer` - Security event log viewer with filtering
- **Features**:
  - Auto-lock timeout configuration (1-1440 minutes)
  - Security options (require password on credential use)
  - Master password management (lock vault, change password placeholder)
  - Vault status display
  - Audit log search and filtering
  - Event type and result filtering
  - Timestamp formatting and display
  - Event icons and color-coded results
- **Integration**:
  - SecurityView expanded to 4 tabs (Credentials | Known Hosts | Settings | Audit Log)
  - Backend commands already implemented (get/update vault settings)
  - Settings persist to database
  - Audit log infrastructure ready for backend implementation
- **UI/UX**:
  - Comprehensive settings with helpful descriptions
  - Security warnings and recommendations
  - Success/error feedback for all operations
  - Consistent design with Quasar aesthetic

#### Phase 7: Security Hardening & Polish ✅
- **Location**: `src-tauri/src/vault/audit.rs`, `src-tauri/src/lib.rs`, `src/components/vault/AuditLogViewer.tsx`
- **Implementation**: Complete security hardening with audit logs, auto-lock, and password management
- **Backend Features**:
  - `AuditLogManager` - Query and filter security audit logs from database
  - `get_audit_logs` command - Retrieve audit logs with filtering (event type, result, search)
  - `get_audit_log_count` command - Get total count of audit entries
  - `change_master_password` command - Change vault master password with credential re-encryption
  - Auto-lock timer task - Background task checks vault inactivity every 30 seconds
  - Emits `vault-auto-locked` event when vault auto-locks
- **Frontend Features**:
  - AuditLogViewer connected to real backend data
  - Displays all vault operations (unlock, lock, credential access, etc.)
  - Search and filter by event type and result
  - Timestamp formatting and event icons
- **Security Enhancements**:
  - Master password change re-encrypts all stored credentials
  - Auto-lock timer actively monitors vault activity
  - Comprehensive audit trail of all security events
  - Activity tracking updates on every vault operation
- **Tauri Commands**:
  - `get_audit_logs` - Query audit log with optional filters
  - `get_audit_log_count` - Get count of audit entries
  - `change_master_password` - Change master password (requires current password)

#### Next Steps
- Phase 8: SSH Feature Enhancements (SFTP, tunneling, themes, etc.)
- Phase 9: Monitoring & Alerting Enhancements
- Phase 10: Automation Workflow Builder

---

## MVP Completion Progress (February 1, 2026)

### Phase 8A: Real SSH Command Execution ✅

#### SSH Execution Module
- **Location**: `src-tauri/src/ssh_exec.rs` (280 lines)
- **Implementation**: Non-interactive SSH command execution using russh
- **Features**:
  - `execute_ssh_command` - Single command execution with configurable timeout
  - `execute_ssh_commands_batch` - Multiple commands on same session
  - `get_system_metrics` - Collect CPU, RAM, disk, uptime, load via SSH
  - Proper channel message handling (Data, ExtendedData, Eof, ExitStatus)
  - UTF-8 output validation
  - Timeout protection for all operations
- **Integration**:
  - Integrated into `automation/engine.rs` for workflow SSH actions
  - Used by `health.rs` for system metrics collection
  - Module added to `lib.rs` module tree

#### Automation Engine Enhancement
- **Location**: `src-tauri/src/automation/engine.rs:161-184`
- **Change**: Replaced simulated SSH output with real command execution
- **Features**:
  - Executes actual SSH commands on remote hosts
  - 30-second timeout per command
  - Stores output in execution context as `last_ssh_output`
  - Error handling with descriptive messages
  - Password handling for Option<String> credentials

#### Health Check Enhancement
- **Location**: `src-tauri/src/health.rs:45-83`
- **Change**: Added real SSH metrics collection to health checks
- **Features**:
  - Pre-flight ping validation (unchanged)
  - Optional SSH metrics collection with credentials
  - Collects: CPU %, memory usage/total, disk usage/total, uptime, load average
  - Graceful degradation if SSH fails (returns reachable=true, metrics=None)
  - Supports credential-less ping-only checks

#### Impact
- **Workflows**: Can now execute real commands on remote infrastructure
- **Health Checks**: Provide actual system metrics before full connection
- **Automation**: Enables complex multi-step workflows with SSH commands
- **Monitoring**: Pre-flight checks can validate system health

### Phase 8B: SFTP File Transfer ✅

#### SFTP Module
- **Location**: `src-tauri/src/sftp.rs` (380 lines)
- **Implementation**: Complete SFTP client using russh-sftp
- **Features**:
  - `upload_file` - Upload files to remote hosts with progress callback support
  - `download_file` - Download files from remote hosts with progress tracking
  - `list_directory` - List files in remote directories
  - `remote_exists` - Check if remote file/directory exists
  - 32KB chunk size for efficient transfers
  - Configurable timeout (10 seconds for connection)
  - Proper error handling and resource cleanup
- **Dependencies**: Added `russh-sftp = "2.0.4"` to Cargo.toml

#### Automation Engine Integration
- **Location**: `src-tauri/src/automation/engine.rs:187-245`
- **Change**: Implemented FileTransfer action with real SFTP operations
- **Features**:
  - Parses host string format: `username@host:port`
  - Supports Upload and Download directions
  - Retrieves password from execution context
  - Detailed success/failure messages
  - Integrates seamlessly with workflow execution

#### Tauri Commands for Frontend
- **Location**: `src-tauri/src/lib.rs:410-455`
- **Commands Added**:
  - `sftp_upload_file` - Upload file from local to remote
  - `sftp_download_file` - Download file from remote to local
  - `sftp_list_directory` - Browse remote directories
  - `sftp_remote_exists` - Check remote file existence
- **Integration**: All commands registered in invoke handler

#### Impact
- **Workflows**: Can now transfer files as part of automation
- **File Management**: Upload/download files to/from remote hosts
- **Automation**: Complete file-based workflows (backup, deployment, etc.)
- **Frontend Ready**: Commands available for UI implementation

---

## Comprehensive Code Review Findings (February 2, 2026)

### Overview
Code review identified 8 bugs (3 critical, 3 high, 2 medium). **Issues 1–7 have been fixed** in the codebase. Remaining items (vault error context, logging) are optional polish.

### Critical Issues — FIXED ✅

#### 1. Credential Re-encryption Transaction Bug ✅
- **Location**: `src-tauri/src/vault.rs` (`change_master_password`)
- **Fix**: Uses single DB connection and a transaction; re-encryption via `delete_credential_tx` / `add_credential_tx` within the same transaction. No credential data loss on failure.

#### 2. SSH/SFTP Session Resource Leaks ✅
- **Location**: `src-tauri/src/sftp.rs`, `src-tauri/src/ssh_exec.rs`
- **Fix**: All code paths call `session.disconnect(russh::Disconnect::ByApplication, "", "en").await` before return (including error paths). No reliance on Drop alone.

#### 3. Monitoring Task Panic on Initialization Failure ✅
- **Location**: `src-tauri/src/monitoring.rs` (`start_monitoring_task`)
- **Fix**: Initialization uses `match` on `app_data_dir()`, path `to_str()`, and `MetricsStore::new()`. Failures are logged with `error!`/`warn!` and the task continues with `metrics_store = None` (persistence disabled) instead of panicking.

### High Severity Issues — FIXED ✅

#### 4. Unsafe .unwrap() in Production Code ✅
- **Location**: `src-tauri/src/vault.rs` (`is_initialized`)
- **Fix**: Replaced with `matches!(result, Ok(val) if val == "true")`.

#### 5. SSH Metrics Tuple Unpacking Bug ✅
- **Location**: `src-tauri/src/ssh_exec.rs` (`get_system_metrics`)
- **Fix**: Memory and disk parsing use `.and_then(...).map(...).unwrap_or((None, None))`; no incorrect `.unzip()` on `Option<(u64, u64)>`.

#### 6. Hardcoded Timeout Ignores Parameter ✅
- **Location**: `src-tauri/src/ssh_exec.rs` (`execute_ssh_commands_batch`)
- **Fix**: Uses `timeout_secs` for connection timeout and per-command timeout (`(timeout_secs / num_commands).max(5)`).

### Medium Severity Issues

#### 7. Potential Integer Overflow in Disk Calculation ✅
- **Location**: `src-tauri/src/monitoring.rs` (`calculate_disk_space`)
- **Fix**: Uses `saturating_add()` and `saturating_sub()` for all disk space sums.

#### 8. Missing Error Context in Vault Operations (Optional)
- **Location**: `src-tauri/src/vault.rs`
- **Issue**: Generic error messages could include more operation context for diagnostics.
- **Status**: Low priority; improve when touching vault error paths.

### Code Quality (Optional)

#### Production Code Using println! for Logging
- **Location**: `src-tauri/src/monitoring.rs`
- **Status**: Monitoring now uses `log::error!` / `log::warn!` for init failures. Any remaining `println!`/`eprintln!` in this file can be migrated to `log` when convenient.

### Additional Fix (February 2026)

#### mDNS Discovery Panic on Daemon Creation ✅
- **Location**: `src-tauri/src/discovery.rs`
- **Issue**: `ServiceDaemon::new().expect(...)` could panic the discovery thread if the daemon failed to create (e.g. no multicast support).
- **Fix**: Replaced with `match ServiceDaemon::new() { Ok(d) => d, Err(e) => { error!("..."); return; } }` so the thread exits gracefully and logs the error.

#### Test Code Quality (Acceptable)
- **Location**: Multiple test modules
- **Observation**: Test code appropriately uses `.unwrap()` and `.expect()` for assertions
- **Status**: No action required - this is acceptable practice in tests

### Security Observations

#### Positive Security Practices ✅
1. Password clearing in React components (`VaultUnlockDialog.tsx:28,33`)
2. Master key zeroization with `zeroize` crate (`vault.rs:55-59`)
3. AES-256-GCM encryption with unique nonces
4. Vault lockout policy and rate limiting

#### Security Concerns ⚠️
1. **SSH Host Key Verification Bypassed**
   - **Location**: `ssh_exec.rs:12-19`, `sftp.rs:16-23`
   - **Issue**: `check_server_key()` returns `Ok(true)` for all keys
   - **Comment**: "we assume host keys are already verified"
   - **Reality**: These modules don't actually use `vault::SshKeyManager`
   - **Risk**: MITM vulnerability despite having verification infrastructure
   - **Status**: Documented, future work to integrate actual verification

### Missing/Incorrect Documentation

#### Automation Engine Reference
- **Clarification**: The path `src-tauri/src/automation/engine.rs` does not exist in the current codebase. Automation/workflow execution is implemented via `ssh_exec.rs` and `sftp.rs` (Tauri commands). Any references to an automation engine in this repo refer to that design; a separate engine module was archived or not implemented.

### Testing Gaps Identified

1. No integration tests for credential re-encryption
2. No tests for SSH session cleanup under failure scenarios
3. No tests for monitoring task recovery from initialization failures
4. No stress tests for connection pooling/resource management

### Implementation Plan Created

Comprehensive fix plan created at: `C:\Users\User\.windsurf\plans\quasar-bug-fixes-c78dd8.md`

**Timeline**: 5-7 days across 5 phases
**Priority**: Phase 1 (Critical) must be completed first
**Status**: Ready to implement starting February 3, 2026

---

## Bug Fixes & Code Quality Improvements (February 1, 2026)

### Critical Issues Fixed

#### 1. Password Validation Alignment (FIXED)
- **Issue**: Frontend required 8 chars minimum, backend required 12 chars
- **Location**: `VaultInitDialog.tsx:18` vs `vault.rs:100`
- **Fix**: Updated frontend validation to require 12 characters to match backend
- **Impact**: Prevents user confusion and failed vault initialization attempts

#### 2. Vault Auto-Lock Event Listener (FIXED)
- **Issue**: `vault-auto-locked` event listener not set up, no cleanup on unmount
- **Location**: `VaultProvider.tsx:51-53`
- **Fix**: Added proper event listener with async setup and cleanup in useEffect
- **Impact**: Vault now properly responds to auto-lock events from backend

#### 3. SSH Trust Status Enum Casing (FIXED)
- **Issue**: Frontend sent 'Trusted' (capitalized), backend expected 'trusted' (lowercase)
- **Location**: `useSshHostKeyVerification.ts:63`
- **Fix**: Changed trustStatus value to lowercase 'trusted'
- **Impact**: SSH host key trust operations now work correctly

### High Priority Issues Fixed

#### 4. Debug Console Logs in Production (FIXED)
- **Issue**: Verbose debug logging throughout terminal initialization
- **Location**: `TerminalComponent.tsx:29-119` (multiple console.log statements)
- **Fix**: Removed all debug console.log statements, kept only error logging
- **Impact**: Cleaner production code, reduced console noise

#### 5. Password Memory Security (FIXED)
- **Issue**: Master password remained in memory after failed attempts
- **Location**: `VaultUnlockDialog.tsx:27-31`, `VaultInitDialog.tsx:51-55`
- **Fix**: Clear password state immediately in both success and error paths
- **Impact**: Improved security - passwords no longer linger in component state

#### 6. Transaction Rollback Handling (FIXED)
- **Issue**: Manual rollback attempts in error handlers could fail silently
- **Location**: `vault.rs:320-373`
- **Fix**: Use proper rusqlite Transaction API with automatic rollback on drop
- **Impact**: Database consistency guaranteed - transactions properly roll back on error

### Code Quality Improvements

#### 7. Unused Imports Removed (FIXED)
- **Location**: `credentials.rs:4` - Removed unused `Zeroizing` import
- **Location**: `VaultInitDialog.tsx:2` - Removed unused `X` icon import
- **Impact**: Cleaner code, no lint warnings for these files

### Previously Documented Issues (Status Update)

#### Database Schema Drift (DOCUMENTED)
- **Location**: `db.ts:21-32` vs `003_security_vault.sql`
- **Issue**: Historically, `credentials` (frontend) and `credentials_new` (backend) coexisted.
- **Status**: Resolved - migration 005/009 consolidated to a single `credentials` table; backend uses it exclusively. See `docs/SCHEMA.md`.

#### Automation Engine (CLARIFIED)
- **Previous Reference**: `automation/engine.rs` mentioned in older documentation
- **Current Status**: File/directory does not exist in codebase
- **Clarification**: SSH command execution is implemented in `ssh_exec.rs` (Phase 8A complete)
- **Action**: Documentation corrected in February 2, 2026 code review

#### Minor Issues (Lower Priority)
- Terminal resize observer: cleanup calls `resizeObserver.disconnect()` in TerminalComponent useEffect return; no leak.
- Missing timeout handling for vault status check
- Inconsistent error message formatting across codebase (addressed in Phase 3.2 of fix plan)

---

## Recent Bug Fixes (January 30, 2026)

### Critical Issues Resolved

#### 1. System Monitoring Interval (FIXED)
- **Issue**: Monitoring task was emitting metrics every 5000 seconds (~83 minutes) instead of 5 seconds
- **Location**: `src-tauri/src/lib.rs:244`
- **Fix**: Changed `monitoring::start_monitoring_task(app_handle, 5000)` to `monitoring::start_monitoring_task(app_handle, 5)`
- **Impact**: System metrics now update in real-time as intended

#### 2. Alert Rules Non-Functional (FIXED)
- **Issue**: Alert rule commands (`add_alert_rule`, `remove_alert_rule`, `get_alert_rules`) were no-op stubs
- **Location**: `src-tauri/src/lib.rs:212-230`
- **Fix**: 
  - Added `AlertEngine` to managed state in setup
  - Implemented proper state access in command handlers
  - Connected monitoring task to use managed `AlertEngine` instance
- **Impact**: Alert rules now persist and trigger correctly

#### 3. SSH Server Key Validation (IMPLEMENTED)
- **Status**: Host key verification is implemented via `SshKeyManager` (vault). `ssh.rs`, `ssh_exec.rs`, and `sftp.rs` use `check_server_key` to verify fingerprints; unknown/changed keys emit `ssh-host-key-verification` and block the connection until the user trusts the key.

---

### High Severity Issues Resolved

#### 4. Integer Underflow in Metrics (FIXED)
- **Issue**: Metrics calculation could underflow if system counters reset
- **Location**: `src-tauri/src/monitoring.rs:116-119`
- **Fix**: Used `saturating_sub()` instead of direct subtraction
- **Impact**: Prevents panic on counter resets or overflows

#### 5. Crypto Functions Panic on Error (FIXED)
- **Issue**: `hash_password` and `verify_password` used `.expect()` causing crashes
- **Location**: `src-tauri/src/crypto.rs:14-27`
- **Fix**: Changed return types to `Result<T, String>` with proper error handling
- **Impact**: Application no longer crashes on crypto errors

#### 6. mDNS Discovery Blocking Loop (FIXED)
- **Issue**: Discovery blocked on first service type, never browsing subsequent types
- **Location**: `src-tauri/src/discovery.rs:14-59`
- **Fix**: Changed to non-blocking poll with `recv_timeout()` across all receivers
- **Impact**: Now discovers both SSH and workstation services concurrently

#### 7. Workflow SSH Execution Stub (DOCUMENTED)
- **Issue**: Workflow SSH commands return fake success without executing
- **Location**: `src-tauri/src/automation/engine.rs:161-167`
- **Status**: Documented as TODO - requires integration with existing SSH module
- **Impact**: Workflows cannot execute real SSH commands yet

---

### Medium Severity Issues Resolved

#### 8. Topological Sort Reverse Order (FIXED)
- **Issue**: Workflow nodes returned in reverse execution order
- **Location**: `src-tauri/src/automation.rs:210-213`
- **Fix**: Added `.rev()` to reverse the sorted list
- **Impact**: Workflows now execute in correct order (trigger → action)

#### 9. React Stale Closure - RemoteManager (FIXED)
- **Issue**: `handleConnect` not in useEffect dependency array
- **Location**: `src/components/RemoteManager.tsx:108`
- **Fix**: Added `handleConnect` to dependency array
- **Impact**: Prevents stale closure bugs in host connection handling

#### 10. React Stale Closure - Canvas (FIXED)
- **Issue**: `deleteSelected` not in useEffect dependency array
- **Location**: `src/components/automation/Canvas.tsx:221`
- **Fix**: Added `deleteSelected` to dependency array
- **Impact**: Keyboard delete handlers now work correctly

#### 12. Invalid SVG Path (FIXED)
- **Issue**: Missing `M` command in SVG path
- **Location**: `src/components/AddHostDialog.tsx:48`
- **Fix**: Changed `d="6 18L18 6..."` to `d="M6 18L18 6..."`
- **Impact**: Close button icon renders correctly

---

### Low Severity Issues Resolved

#### 13. Unused Variables (FIXED)
- **Location**: `src-tauri/src/health.rs:48-52, 80`
- **Fix**: Prefixed unused parameters with `_` and removed unused `Instant` import
- **Impact**: Cleaner code, fewer compiler warnings

#### 14. Disk Usage Metric Always Skipped (DOCUMENTED)
- **Location**: `src-tauri/src/monitoring.rs:203-207`
- **Status**: Added clarifying comment - requires disk capacity data in metrics
- **Impact**: Disk usage alerts cannot trigger (by design, needs enhancement)

---

## Architecture Notes

### State Management
The application uses Tauri's managed state pattern:
- `SshState` - SSH session management
- `ScannerState` - Network scanning state
- `AutomationState` - Workflow definitions and executions
- `AlertEngine` - Alert rules and active alerts
- `VaultState` - Master password management, vault lock/unlock (Phase 1)
- `CredentialManager` - Encrypted credential CRUD operations (Phase 2)
- `SshKeyManager` - SSH host key verification and known hosts management (Phase 3)

### Event System
Backend emits events to frontend:
- `system-metrics` - Real-time system metrics (every 5 seconds)
- `alerts-triggered` - When alert rules trigger
- `scan_progress` / `scan_result` - Network scan updates
- `ssh_data_{id}` / `ssh_closed_{id}` - SSH session data
- `workflow-*` - Workflow execution events

### Database Schema
SQLite database (`quasar.db`) with tables:
- `hosts` - Remote host inventory
- `credentials` - Encrypted credentials with AES-256-GCM (nonce, tag, metadata); consolidated schema (see docs/SCHEMA.md)
- `vault_settings` - Master password hash, salt, vault configuration
- `security_audit_log` - Comprehensive audit trail for all credential operations
- `ssh_known_hosts` - SSH host key verification (ready for Phase 3)

---

## Known Issues & TODOs

### Security
1. **SSH Host Key Verification** - ✅ COMPLETE - Backend implementation done (Phase 3)
   - Note: Still needs UI integration for user prompts when connecting to unknown/changed hosts
2. **Credential Storage** - ✅ COMPLETE - Fully encrypted with AES-256-GCM (Phase 1 & 2)

### Features
1. **Workflow SSH Execution** - Needs integration with SSH module
2. **Disk Usage Monitoring** - Needs disk capacity data in metrics
3. **Alert Persistence** - Alerts currently in-memory only

### Code Quality
- 21 compiler warnings (all pre-existing, mostly unused code for future features)
- Several dead code warnings for planned functionality (automation engine)
- No new warnings introduced by vault/SSH key implementation
- All vault tests passing (11/11 total: 3 vault + 5 credentials + 3 SSH keys)

---

## Testing Notes

### Verified Working
- System metrics emit every 5 seconds ✓
- Alert rules can be added/removed ✓
- Network scanner discovers hosts ✓
- SSH connections establish ✓
- Crypto functions handle errors gracefully ✓
- Vault initialization and unlock/lock operations ✓
- Credential encryption/decryption with AES-256-GCM ✓
- Credential CRUD operations (add, get, list, update, delete, search) ✓
- Audit logging for all credential operations ✓
- SSH host key verification (unknown, trusted, changed detection) ✓
- Known hosts database storage and retrieval ✓

### Needs Testing
- Alert rule triggering under load
- Workflow execution with multiple nodes
- mDNS discovery on networks with services
- SSH session stability over time

---

## Development Commands

```bash
# Run development server
npm run tauri dev
# If you see "ProjectTitan" path errors (stale CARGO_TARGET_DIR), use:
npm run tauri:dev

# Build for production
npm run tauri build

# Run tests
npm test

# Run Rust tests
cd src-tauri && cargo test
```

---

## File Structure

```
Quasar/
├── src/                    # React frontend
│   ├── components/         # UI components
│   │   ├── dashboard/      # Dashboard widgets
│   │   └── automation/     # Workflow canvas
│   ├── db.ts              # Database initialization
│   └── main.tsx           # App entry point
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── lib.rs         # Main Tauri commands
│   │   ├── monitoring.rs  # System metrics & alerts
│   │   ├── ssh.rs         # SSH client
│   │   ├── scanner.rs     # Network scanner
│   │   ├── automation.rs  # Workflow engine
│   │   ├── crypto.rs      # Encryption utilities (Argon2id, AES-256-GCM)
│   │   ├── vault.rs       # Vault state & master password management
│   │   ├── vault/
│   │   │   └── credentials.rs  # Credential CRUD with encryption
│   │   └── ...
│   ├── migrations/   # 001–012; 011/012 via Rust hooks (idempotent)
│   │   └── (010 scheduled_tasks, 011 run-result, 012 SFTP columns, etc.)
│   └── Cargo.toml
└── conductor/             # Product documentation
```

---

## Agent Guidelines

When working on this codebase:

1. **Security First**: Be cautious with SSH, credentials, and network operations
2. **Error Handling**: Use `Result` types, avoid `.expect()` and `.unwrap()` in production code
3. **State Management**: Use Tauri's managed state for shared resources
4. **Event-Driven**: Emit events for real-time updates to frontend
5. **Testing**: Add tests for critical functionality
6. **Documentation**: Update this file when making architectural changes

---

## Recent Performance Improvements

- Monitoring interval: 5000s → 5s (1000x faster)
- Discovery: Sequential → Concurrent (2x faster)
- Metrics calculation: Protected against underflow
- Error handling: Graceful degradation instead of crashes

---

*Last Updated: February 20, 2026 (Light theme app UI text overrides, terminal app-theme sync reverted, Solarized Light theme foreground/cursor).*

---

## Cursor Cloud specific instructions

### System dependencies (pre-installed, not in update script)

Tauri v2 on Linux requires system libraries that must be installed once (these are handled by the VM snapshot, not the update script):

```
sudo apt-get install -y libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev libgtk-3-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

Rust must be at least 1.85+ (edition2024 support required by transitive dependency `ctutils`). The VM snapshot has Rust 1.93.1 via `rustup default stable`.

### Running the application

- **Dev mode**: `npm run tauri dev` (or `npm run tauri:dev`) — starts Vite on `:1420` + compiles and launches the Rust backend. First run takes ~2 min for Rust compilation; subsequent runs use incremental builds.
- The app opens a native window (requires `$DISPLAY`). On first launch it shows the "Initialize Vault" dialog; set a master password (12+ chars, mixed case, number).
- A Tauri version mismatch warning (`tauri v2.9.5 vs @tauri-apps/api v2.10.1`) appears in the console but does not affect functionality.

### Testing

- **Frontend**: `npm test` (vitest, 72 tests). Pre-existing `act(...)` warnings in stderr are expected and do not indicate failures.
- **Backend**: `cd src-tauri && cargo test` (79 tests). All pass.
- **TypeScript check**: `npx tsc --noEmit` reports pre-existing type errors (unused imports, `SettingsView.tsx` type issues). The project builds fine via Vite regardless.

### Gotchas

- The `scripts/tauri-dev.js` wrapper sets `CARGO_TARGET_DIR` to `src-tauri/target` to avoid stale path issues. Prefer `npm run tauri dev` over running `cargo tauri dev` directly.
- SQLite is bundled via `rusqlite` `bundled` feature — no external database service needed.
- Ollama (AI assistant) is optional; the app shows "Ollama Offline" gracefully if not running.

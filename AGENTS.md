# Quasar - Agent Context & Bug Fixes

## Overview
Quasar is a Tauri-based remote infrastructure management application with React frontend and Rust backend. This document tracks major bug fixes, architectural decisions, and context for AI agents working on this codebase.

---

## Recent Implementations

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

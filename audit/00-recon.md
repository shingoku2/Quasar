# Phase 0 — Recon

Date: 2026-09-24 · Base commit: `fcc58d2` (branch `claude/quasar-audit-cleanup-3srz1j`, = master `fa85cfe` + one CLAUDE.md docs commit)
Linear: EDW-17. Read-only phase — nothing outside `audit/` was modified.

> **Branch note:** the Linear issue asks for `audit/2026-09-24`. This cloud session is pinned to push only to
> `claude/quasar-audit-cleanup-3srz1j`, so all audit output lives there instead.

## 1. Toolchain

| Tool | Container (used for baseline) | Required by repo | CI | Verdict |
|---|---|---|---|---|
| node | 24.15.0 (installed to `/opt/node24`; container default was 22.22.2) | `.nvmrc` 24.15.0; `engines.node` `>=24.15.0 <25` | `setup-node` 24.15.0 | OK after install |
| npm | 12.0.2 (upgraded; node 24.15.0 bundles 11.12.1) | `engines.npm` `>=12.0.2 <13`, `packageManager` npm@12.0.2 | **npm 11.12.1** (whatever node 24.15.0 bundles; no `npm i -g npm@12`/corepack step in `ci.yml` or `release.yml`) | **MISMATCH** — CI runs an npm the `engines` field rejects. No `.npmrc` `engine-strict`, so it's only an EBADENGINE warning, but the lockfile is being produced/consumed by two npm majors. |
| rustc / cargo | 1.98.1 (container default was 1.94.1, which is **below** `rust-version = "1.95"` and cannot build the crate) | `Cargo.toml` `rust-version = "1.95"` | `dtolnay/rust-toolchain@stable` (floating) | OK after `rustup update`. CI floats on stable — no pinned toolchain file (`rust-toolchain.toml` absent). |
| clippy | 0.1.98 | — | stable | OK |
| tauri-cli | 2.11.4 (`npx tauri -V`) | `@tauri-apps/cli ^2.11.4` | same via npm | OK |
| Tauri system libs | webkit2gtk-4.1 2.52.6 (apt-installed for baseline) | — | ci.yml installs via apt | OK |

TypeScript is `^7.0.2` and Vite `^8.2.0` in `package.json`; CLAUDE.md says "TypeScript 5.8 / Vite 7" — **CLAUDE.md is stale**.

## 2. Configuration snapshot

- **tauri.conf.json**: identifier `com.quasar.app`; CSP `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: data:; connect-src 'self' ws://localhost:* http://localhost:*` (same CSP for dev and prod — Phase 2B). Updater: minisign pubkey embedded, single HTTPS endpoint `https://github.com/shingoku2/quasar/releases/latest/download/latest.json`. Windows timestamp URL is plain `http://timestamp.digicert.com` (normal for RFC3161, noted for 2B).
- **capabilities/default.json** (only capability file): `core:default`, `core:event:allow-listen`, `core:event:allow-emit`, `opener:default`, `dialog:default`, `updater:default`, `process:allow-restart` for window `main`.
- **.gitignore**: no entry for `.claude/settings.local.json` (Phase 6); `*.local` only matches files *ending* in `.local`.
- **CI**: `ci.yml` = frontend (tsc, vitest, `npm audit --audit-level=high`), backend (clippy, cargo test, `cargo install cargo-audit && cargo audit`), build matrix via `tauri-apps/tauri-action@v1`. All actions pinned by **tag**, not SHA (Phase 5).

## 3. Architecture map

### 3.1 Tauri commands (73 defined, 73 registered — no orphans either way)

70 live in `lib.rs`; `write_ssh`/`resize_ssh`/`disconnect_ssh` in `ssh.rs`. Extraction: `grep -A4 '#[tauri::command]'` vs `generate_handler![…]` → identical sets.

| Command | Defined at | Purpose | Frontend call sites |
|---|---|---|---|
| `add_alert_rule` | `src-tauri/src/lib.rs:1153` | Create alert rule | components/dashboard/AlertRules.tsx:104, components/dashboard/AlertRules.tsx:137 |
| `add_credential` | `src-tauri/src/lib.rs:1237` | Encrypt + insert credential | components/RemoteManager.tsx:393, components/vault/CredentialManager.tsx:371 |
| `add_scheduled_task` | `src-tauri/src/lib.rs:822` | Create cron task | components/ScheduledTasksView.tsx:175 |
| `change_master_password` | `src-tauri/src/lib.rs:1493` | Rekey: re-encrypt all credentials under new key | components/vault/VaultSettings.tsx:90 |
| `check_ai_status` | `src-tauri/src/lib.rs:378` | Ping local Ollama | components/AIAssistant.tsx:126 |
| `check_host_health` | `src-tauri/src/lib.rs:1066` | TCP/SSH health check | components/PreflightDialog.tsx:47 |
| `clear_metrics_data` | `src-tauri/src/lib.rs:1677` | Delete metrics history | components/SettingsView.tsx:394 |
| `close_ssh_tunnel` | `src-tauri/src/lib.rs:333` | Stop a tunnel | components/SshTunnelsView.tsx:90 |
| `connect_rdp` | `src-tauri/src/lib.rs:356` | Launch external RDP client (mstsc) | components/RemoteManager.tsx:147 |
| `connect_ssh` | `src-tauri/src/lib.rs:207` | Open interactive SSH terminal session (credential id or password); returns session id | components/TerminalComponent.tsx:154 |
| `delete_credential` | `src-tauri/src/lib.rs:1368` | Delete credential | components/vault/CredentialManager.tsx:87 |
| `delete_discovered_host` | `src-tauri/src/lib.rs:520` | Delete discovered host | components/dashboard/DashboardView.tsx:131 |
| `disconnect_ssh` | `src-tauri/src/ssh.rs:570` | Close SSH session | components/TerminalComponent.tsx:221 |
| `export_database` | `src-tauri/src/lib.rs:1695` | VACUUM INTO export | components/SettingsView.tsx:355 |
| `get_alert_history` | `src-tauri/src/lib.rs:1130` | Stored alert history | **NONE — never invoked** |
| `get_alert_rules` | `src-tauri/src/lib.rs:1163` | List alert rules | components/dashboard/AlertRules.tsx:65 |
| `get_app_info` | `src-tauri/src/lib.rs:1645` | Version/paths | components/SettingsView.tsx:332, components/SettingsView.tsx:501 |
| `get_audit_log_count` | `src-tauri/src/lib.rs:1523` | Count audit log rows | **NONE — never invoked** |
| `get_audit_logs` | `src-tauri/src/lib.rs:1513` | Query security_audit_log | components/vault/AuditLogViewer.tsx:34 |
| `get_credential` | `src-tauri/src/lib.rs:1283` | Credential metadata (no secrets, has_* flags) | components/vault/CredentialManager.tsx:105, components/vault/CredentialManager.tsx:96, components/vault/CredentialSelector.tsx:71 |
| `get_discovered_hosts` | `src-tauri/src/lib.rs:489` | List discovered_hosts | components/AIAssistant.tsx:68, components/Discovery.tsx:45, components/dashboard/DashboardView.tsx:66 |
| `get_host_details` | `src-tauri/src/lib.rs:499` | One discovered host + services | **NONE — never invoked** |
| `get_known_ssh_hosts` | `src-tauri/src/lib.rs:1449` | List known hosts | components/vault/KnownHostsManager.tsx:27 |
| `get_metrics_history` | `src-tauri/src/lib.rs:1106` | Stored metrics history | **NONE — never invoked** |
| `get_remote_hosts_health` | `src-tauri/src/lib.rs:988` | SSH-poll saved hosts for metrics (uses vault creds) | components/MonitoringView.tsx:249, components/dashboard/SystemHealthWidget.tsx:110 |
| `get_saved_hosts` | `src-tauri/src/lib.rs:736` | List saved hosts | components/AIAssistant.tsx:69, components/HostList.tsx:41, components/MonitoringView.tsx:259 (+3) |
| `get_scan_progress` | `src-tauri/src/lib.rs:479` | Scan progress | components/AIAssistant.tsx:67 |
| `get_scheduled_task` | `src-tauri/src/lib.rs:813` | Get one task | **NONE — never invoked** |
| `get_system_metrics` | `src-tauri/src/lib.rs:1093` | Local sysinfo metrics | components/MonitoringView.tsx:228, components/dashboard/AlertFeed.tsx:123, components/dashboard/SystemHealthWidget.tsx:96 |
| `get_tailscale_status` | `src-tauri/src/lib.rs:366` | Run `tailscale status --json` | hooks/useTailscaleStatus.ts:84 |
| `get_vault_settings` | `src-tauri/src/lib.rs:1218` | Read vault settings | components/dashboard/SystemHealthWidget.tsx:122, components/vault/VaultSettings.tsx:36 |
| `import_database` | `src-tauri/src/lib.rs:1720` | Validate (QSR1 app id) + replace DB | components/SettingsView.tsx:377 |
| `initialize_vault` | `src-tauri/src/lib.rs:1177` | Set master password (Argon2id) | components/vault/VaultInitDialog.tsx:52 |
| `is_scanning` | `src-tauri/src/lib.rs:484` | Scan running flag | components/AIAssistant.tsx:66 |
| `is_vault_initialized` | `src-tauri/src/lib.rs:1169` | Vault init flag | components/vault/VaultProvider.tsx:65 |
| `is_vault_locked` | `src-tauri/src/lib.rs:1213` | Lock flag | components/RemoteManager.tsx:135, components/RemoteManager.tsx:168, components/vault/VaultProvider.tsx:69 |
| `launch_ssh_external` | `src-tauri/src/lib.rs:345` | Open OS terminal running ssh | **NONE — never invoked** |
| `list_ai_models` | `src-tauri/src/lib.rs:383` | List Ollama models | components/AIAssistant.tsx:129 |
| `list_credentials` | `src-tauri/src/lib.rs:1315` | List credential summaries | components/MonitoringView.tsx:260, components/ScheduledTasksView.tsx:100, components/SshTunnelsView.tsx:50 (+2) |
| `list_scheduled_tasks` | `src-tauri/src/lib.rs:807` | List tasks | components/ScheduledTasksView.tsx:98 |
| `list_ssh_tunnels` | `src-tauri/src/lib.rs:326` | List active tunnels | components/SshTunnelsView.tsx:40 |
| `lock_vault` | `src-tauri/src/lib.rs:1205` | Drop key | components/vault/VaultProvider.tsx:130, components/vault/VaultSettings.tsx:63 |
| `preflight_check` | `src-tauri/src/lib.rs:1058` | Ping/DNS pre-flight | components/HealthCheckBadge.tsx:28 |
| `remove_alert_rule` | `src-tauri/src/lib.rs:1158` | Delete alert rule | components/dashboard/AlertRules.tsx:114 |
| `remove_saved_hosts` | `src-tauri/src/lib.rs:785` | Bulk delete saved hosts | components/HostList.tsx:120, components/HostList.tsx:78 |
| `remove_scheduled_task` | `src-tauri/src/lib.rs:904` | Delete task | components/ScheduledTasksView.tsx:188 |
| `remove_ssh_host_key` | `src-tauri/src/lib.rs:1459` | Forget host key | components/vault/KnownHostsManager.tsx:46 |
| `resize_ssh` | `src-tauri/src/ssh.rs:546` | Resize SSH PTY | components/TerminalComponent.tsx:166, components/TerminalComponent.tsx:195 |
| `respond_ssh_host_key_verification` | `src-tauri/src/lib.rs:1440` | Answer pending host-key prompt by requestId | hooks/useSshHostKeyVerification.ts:85, hooks/useSshHostKeyVerification.ts:98 |
| `reveal_credential_password` | `src-tauri/src/lib.rs:1299` | Decrypt + return password (only secret-returning cmd) | components/RemoteManager.tsx:200, components/vault/CredentialManager.tsx:601, components/vault/CredentialManager.tsx:626 |
| `run_scheduled_task_now` | `src-tauri/src/lib.rs:910` | Execute task immediately | components/ScheduledTasksView.tsx:201 |
| `scan_network` | `src-tauri/src/lib.rs:421` | Claim + spawn CIDR ping/port scan | components/NetworkScanner.tsx:154 |
| `search_credentials` | `src-tauri/src/lib.rs:1378` | Search credential summaries | components/vault/CredentialManager.tsx:74 |
| `search_discovered_hosts` | `src-tauri/src/lib.rs:510` | Search discovered hosts | **NONE — never invoked** |
| `send_ai_chat` | `src-tauri/src/lib.rs:397` | Stream chat to Ollama (emits ai-chat-response) | components/AIAssistant.tsx:185 |
| `set_host_monitoring_credential` | `src-tauri/src/lib.rs:1025` | Bind credential to host for monitoring | components/MonitoringView.tsx:265 |
| `sftp_download_file` | `src-tauri/src/lib.rs:1565` | SFTP download (password auth) | components/SshFileManager.tsx:169 |
| `sftp_list_directory` | `src-tauri/src/lib.rs:1596` | SFTP ls | components/SshFileManager.tsx:63 |
| `sftp_remote_exists` | `src-tauri/src/lib.rs:1615` | SFTP stat | **NONE — never invoked** |
| `sftp_upload_file` | `src-tauri/src/lib.rs:1534` | SFTP upload (password auth) | components/SshFileManager.tsx:131 |
| `start_discovery` | `src-tauri/src/lib.rs:373` | Start mDNS discovery (singleton) | components/Discovery.tsx:68 |
| `start_ssh_tunnel` | `src-tauri/src/lib.rs:259` | Start local port forward over SSH | components/SshTunnelsView.tsx:68 |
| `stop_scan` | `src-tauri/src/lib.rs:474` | Signal scan stop | components/NetworkScanner.tsx:164 |
| `trust_ssh_host_key` | `src-tauri/src/lib.rs:1406` | Persist trusted host key + resolve pending prompt | hooks/useSshHostKeyVerification.ts:157, hooks/useSshHostKeyVerification.ts:75 |
| `unlock_vault` | `src-tauri/src/lib.rs:1190` | Derive key, unlock (persisted lockout) | components/vault/VaultUnlockDialog.tsx:28 |
| `update_credential` | `src-tauri/src/lib.rs:1324` | Re-encrypt + update credential | components/vault/CredentialManager.tsx:368 |
| `update_saved_host` | `src-tauri/src/lib.rs:762` | Edit saved host | components/AddHostDialog.tsx:55 |
| `update_scheduled_task` | `src-tauri/src/lib.rs:862` | Edit cron task | components/ScheduledTasksView.tsx:173 |
| `update_ssh_host_trust` | `src-tauri/src/lib.rs:1475` | Change trust level | components/vault/KnownHostsManager.tsx:55 |
| `update_vault_settings` | `src-tauri/src/lib.rs:1225` | Write vault settings (auto-lock) | components/vault/VaultSettings.tsx:51 |
| `upsert_saved_host` | `src-tauri/src/lib.rs:741` | Insert/merge saved host | components/AddHostDialog.tsx:55 |
| `verify_ssh_host_key` | `src-tauri/src/lib.rs:1388` | Check key against known_hosts | hooks/useSshHostKeyVerification.ts:130 |
| `write_ssh` | `src-tauri/src/ssh.rs:519` | Send keystrokes to SSH session | components/TerminalComponent.tsx:183 |

### 3.2 invoke ↔ command diff

- Frontend `invoke()` call sites (non-test): **92**, covering **65** distinct command names. One non-literal site: `AddHostDialog.tsx:55` picks `update_saved_host` / `upsert_saved_host` by ternary (counted as calling both).
- **invoke with no matching command: none.**
- **Commands never invoked by the frontend (8):** `get_alert_history`, `get_audit_log_count` (only mocked in `App.test.tsx`/`Layout.test.tsx`), `get_host_details`, `get_metrics_history`, `get_scheduled_task`, `launch_ssh_external`, `search_discovered_hosts`, `sftp_remote_exists`. Each is dead IPC surface a compromised webview can still reach (Phase 2B) or dead code (Phase 6).
- **CLAUDE.md's "Commands" section is largely fiction.** 21 of the command names it lists don't exist: `export_vault`, `import_vault`, `remove_credential`, `add_host`, `update_host`, `remove_host`, `list_hosts`, `start_ssh_session`, `close_ssh_session`, `execute_ssh_command`, `get_ssh_known_hosts`, `remove_known_host`, `update_alert_rule`, `list_alert_rules`, `start_network_scan`, `save_discovered_host`, `remove_discovered_host`, `create_tunnel`, `close_tunnel`, `list_tunnels`, `send_ai_message`. Real names are in the table above.

### 3.3 Events

| Event | Emitted at | Listened in frontend? |
|---|---|---|
| `system-metrics` | monitoring.rs:949 | yes (3) |
| `alerts-triggered` | monitoring.rs:997 | yes (2) |
| `alerts-recovered` | monitoring.rs:992 | **no listener** |
| `host-discovered` | discovery.rs:102 | yes |
| `scan_progress` / `scan_result` / `scan_complete` / `scan_error` | lib.rs:444–465 | yes |
| `vault-auto-locked` | lib.rs:1871 | yes |
| `ssh_data_{id}` / `ssh_stats_{id}` / `ssh_closed_{id}` | ssh.rs:380–499 | yes |
| `ssh_timeout_{id}` | lib.rs:1931 | yes |
| `ssh-host-key-verification` | ssh.rs:91 | yes |
| `ai-chat-response` | ai.rs:48–66 | yes |

Every `emit` result is discarded with `let _ =` (Phase 3).

### 3.4 Managed state (`lib.rs:1816–1835`)

`ssh::SshState`, `ssh::HostKeyApprovalState`, `ssh_pool::SshConnectionPool`, `ssh_tunnel::TunnelState`, `discovery::DiscoveryState`, `Arc<scanner::ScannerState>`, `Arc<monitoring::AlertEngine>`, `vault::VaultState`, `vault::CredentialManager`, `vault::SshKeyManager`, `vault::AuditLogManager`, `host_tracker::HostTracker`, `Arc<std::sync::Mutex<monitoring::MetricsCollector>>` (a **std** mutex in async code — Phase 3).

### 3.5 Database / migrations

14 migration files, `001`, `003`–`014` (**no 002**), registered in `lib.rs:104–125`. `011` and `012` are `up_with_hook("SELECT 1;", …)` Rust hooks (column-exists checks), not the SQL in the file. CLAUDE.md claims "12 files, 001–012" — stale. Schema drift vs `docs/SCHEMA.md` → Phase 3.

### 3.6 Size hot-spots

Rust (lines): `lib.rs` 2247, `monitoring.rs` 1327, `vault/credentials.rs` 1120, `vault.rs` 1111, `scheduler.rs` 1048, `scanner.rs` 893.
Frontend: `vault/CredentialManager.tsx` 722, `SettingsView.tsx` 619, `ScheduledTasksView.tsx` 550, `MonitoringView.tsx` 440, `RemoteManager.tsx` 436. 49 test files.
Frontend views (`Sidebar.tsx:25–31`): Dashboard, Remote (hosts, terminals, SFTP, tunnels, scanner/topology, Tailscale), Monitoring, AI Assistant, Automation (scheduled tasks), Security (vault, credentials, known hosts, audit log), Settings. Hooks: `useSshHostKeyVerification`, `useTailscaleStatus`, `useUpdater`, `useViewVisibility`.

## 4. Known open issues — re-verified in code

### 4.1 Credential add/update is NOT serialized against master-password rekey — **CONFIRMED**

- `add_credential` (`lib.rs:1261`) and `update_credential` (`lib.rs:1345`) call `vault_state.get_master_key()` then encrypt/write with it. `get_master_key` (`vault.rs:449–457`) only checks `master_key.is_some()`; it never looks at `changing_password`. The only reader of that flag is `check_auto_lock` (`vault.rs:426`).
- `change_master_password` (`vault.rs:495–688`) sets `changing_password = true`, **drops the vault lock**, re-encrypts every credential inside a single `conn.transaction()` (deferred) on a `spawn_blocking` thread, commits, then reacquires the lock to swap `master_key` (`vault.rs:666–684`).
- Failure modes:
  1. **Write during the rekey transaction** (after rotation holds SQLite's write lock): the concurrent insert waits on the 5 s busy-timeout, then commits *after* the rotation commit — ciphertext under the **old** key, now unreadable under the new key. Orphaned.
  2. **Write in the post-commit / pre-swap window** (`vault.rs:661–677`, acknowledged in the code comment as a "narrow torn-state window" but only for reads): `get_master_key()` returns the old key; the write succeeds; orphaned.
  3. Write before rotation's first write but after its read snapshot: WAL snapshot upgrade fails with `SQLITE_BUSY_SNAPSHOT` → the *rotation* aborts (safe but user-visible failure).
- There is no regression test for this race (`grep -rn changing_password src-tauri/tests` → none).
- Status per CLAUDE.md ("still open") is accurate. Linear: EDW-15.

### 4.2 The four "done" September 2 findings

Spot-checked their anchors exist (deep review is Phase 2A/3): `SshState::pending_connections` (ssh.rs), requestId-correlated `HostKeyApprovalState` + FIFO in `useSshHostKeyVerification.ts`, `QUASAR_APPLICATION_ID` "QSR1" (`lib.rs:133,183,191`), driver-side session removal (ssh.rs:438–441). Present; correctness not yet verified.

### 4.3 Doc-accuracy issues found during recon (feed Phase 6)

- CLAUDE.md command list (21 non-existent names, §3.2), stack versions (TS/Vite), migration count.
- `docs/DEFERRED_AUDIT_FIX_PLAN.md` referenced but absent on master (per CLAUDE.md itself).
- Root has both `.jules/` and `.Jules/` — case collision on case-insensitive filesystems.

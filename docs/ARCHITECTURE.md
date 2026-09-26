# Quasar architecture

How the pieces fit and the patterns the code relies on. CLAUDE.md has the rules in short form; this file has the reasoning. Where they disagree with the code, the code and its tests win.

## Shape

- **Frontend**: React 19 + TypeScript in a Tauri webview (`src/`). Every view stays mounted and is hidden with CSS (`Layout.tsx`); lists other views can change reload through `useOnViewShown`, polling goes through `useVisiblePolling` (`hooks/useViewVisibility.tsx`). Each view sits in its own `ErrorBoundary`.
- **Backend**: Rust (`src-tauri/src/`), Tokio. Commands live in `commands/` (one module per area) and are all registered in `generate_handler!` in `lib.rs`. There's no HTTP server: the webview talks to Rust only through Tauri IPC (commands and events).
- **Storage**: one SQLite file, `quasar.db` (see `docs/SCHEMA.md`). Every connection comes from `db::open_connection()` (busy timeout, WAL, foreign keys, `secure_delete`, owner-only permissions). Never call `Connection::open` elsewhere: before that rule, contended writers failed fast with `SQLITE_BUSY` and scan results and metrics were silently dropped.
- **Trust boundary**: the webview is treated as potentially compromised. Anything that weakens a security decision is decided in Rust, and the riskiest ones ask the user through a native OS dialog the webview can't click (`native_confirm.rs`). See "Security decisions" below.

## IPC commands

Generated from the `#[tauri::command]` signatures. `invoke()` payload keys must be lowerCamelCase: Tauri matches keys against the camelCased Rust parameter name, and a misspelled key for an `Option<T>` parameter silently arrives as `None` (no error). `?` marks optional args. Errors are `String`s, sanitized by `errors::sanitize_error` (or passed through `errors::user_facing_vault_error` for messages the user must see). The test `architecture_doc_lists_every_command` fails if a registered command is missing here.

### Vault

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `is_vault_initialized` | — | `bool` | commands/vault.rs |
| `initialize_vault` | masterPassword | `()` | commands/vault.rs |
| `unlock_vault` | masterPassword | `()` | commands/vault.rs |
| `lock_vault` | — | `()` | commands/vault.rs |
| `is_vault_locked` | — | `bool` | commands/vault.rs |
| `get_vault_settings` | — | `vault::VaultSettings` | commands/vault.rs |
| `update_vault_settings` | settings | `()` | commands/vault.rs |
| `change_master_password` | currentPassword, newPassword | `()` | commands/vault.rs |

### Credentials

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `add_credential` | name, username, password, credentialType, host?, port?, metadata?, keyPath?, privateKey?, keyPassphrase? | `String` | commands/vault.rs |
| `get_credential` | credentialId | `vault::CredentialFrontendView` | commands/vault.rs |
| `reveal_credential_password` | credentialId | `String` | commands/vault.rs |
| `list_credentials` | — | `Vec<vault::CredentialSummary>` | commands/vault.rs |
| `search_credentials` | query | `Vec<vault::CredentialSummary>` | commands/vault.rs |
| `update_credential` | credentialId, name?, username?, password?, metadata?, credentialType?, host?, port?, keyPath?, privateKey?, keyPassphrase? | `()` | commands/vault.rs |
| `delete_credential` | credentialId | `()` | commands/vault.rs |

### SSH host keys

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `trust_ssh_host_key` | requestId | `()` | commands/vault.rs |
| `respond_ssh_host_key_verification` | requestId, accepted | `()` | commands/vault.rs |
| `get_known_ssh_hosts` | — | `Vec<vault::SshHostKey>` | commands/vault.rs |
| `remove_ssh_host_key` | host, port | `()` | commands/vault.rs |
| `update_ssh_host_trust` | host, port, trustStatus | `()` | commands/vault.rs |

### Audit

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `get_audit_logs` | filter? | `Vec<vault::AuditLogEntry>` | commands/vault.rs |

### Saved hosts

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `get_saved_hosts` | — | `Vec<SavedHost>` | commands/hosts.rs |
| `upsert_saved_host` | name, address, protocol, port?, username? | `SavedHost` | commands/hosts.rs |
| `update_saved_host` | hostId, name, address, protocol, port?, username? | `SavedHost` | commands/hosts.rs |
| `remove_saved_hosts` | ids | `usize` | commands/hosts.rs |

### SSH terminal

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `connect_ssh` | id, host, user, port, password?, credentialId? | `()` | commands/ssh.rs |
| `write_ssh` | id, data | `()` | ssh.rs |
| `resize_ssh` | id, rows, cols | `()` | ssh.rs |
| `disconnect_ssh` | id | `()` | ssh.rs |

### SFTP and local files

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `pick_local_file` | title?, extensions? | `Option<String>` | commands/files.rs |
| `pick_save_location` | defaultName?, extensions? | `Option<String>` | commands/files.rs |
| `sftp_list_directory` | host, port, username, password?, credentialId?, remotePath | `Vec<sftp::RemoteFile>` | commands/files.rs |
| `sftp_upload_file` | host, port, username, password?, credentialId?, localPath, remotePath | `()` | commands/files.rs |
| `sftp_download_file` | host, port, username, password?, credentialId?, remotePath, localPath | `()` | commands/files.rs |

### SSH tunnels

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `start_ssh_tunnel` | tunnelId, sshHost, sshPort, sshUser, password?, credentialId?, localPort, remoteHost, remotePort | `ssh_tunnel::TunnelInfo` | commands/ssh.rs |
| `list_ssh_tunnels` | — | `Vec<ssh_tunnel::TunnelInfo>` | commands/ssh.rs |
| `close_ssh_tunnel` | tunnelId | `()` | commands/ssh.rs |

### Scheduled tasks

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `list_scheduled_tasks` | — | `Vec<scheduler::ScheduledTask>` | commands/scheduler.rs |
| `add_scheduled_task` | name, cronExpression, hostId, command, credentialId?, enabled, taskType?, localPath?, remotePath? | `String` | commands/scheduler.rs |
| `update_scheduled_task` | id, name, cronExpression, hostId, command, credentialId?, enabled, taskType?, localPath?, remotePath? | `()` | commands/scheduler.rs |
| `remove_scheduled_task` | id | `()` | commands/scheduler.rs |
| `run_scheduled_task_now` | id | `scheduler::TaskRunResult` | commands/scheduler.rs |

### Monitoring and alerts

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `get_system_metrics` | — | `monitoring::SystemMetrics` | commands/monitoring.rs |
| `get_remote_hosts_health` | — | `Vec<RemoteHostMetric>` | commands/hosts.rs |
| `set_host_monitoring_credential` | hostId, credentialId? | `()` | commands/hosts.rs |
| `add_alert_rule` | rule | `()` | commands/monitoring.rs |
| `remove_alert_rule` | ruleId | `()` | commands/monitoring.rs |
| `get_alert_rules` | — | `Vec<monitoring::AlertRule>` | commands/monitoring.rs |
| `clear_metrics_data` | — | `()` | commands/app_data.rs |

### Health

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `preflight_check` | host | `health::HealthCheckResult` | commands/network.rs |
| `check_host_health` | host, port, username, password? | `health::HealthCheckResult` | commands/network.rs |

### Discovery and scanning

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `start_discovery` | — | `()` | commands/network.rs |
| `scan_network` | cidr | `()` | commands/network.rs |
| `stop_scan` | — | `()` | commands/network.rs |
| `get_scan_progress` | — | `scanner::ScanProgress` | commands/network.rs |
| `is_scanning` | — | `bool` | commands/network.rs |
| `get_discovered_hosts` | limit? | `Vec<host_tracker::DiscoveredHost>` | commands/network.rs |
| `delete_discovered_host` | ip | `()` | commands/network.rs |

### Tailscale, RDP, AI

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `get_tailscale_status` | — | `tailscale::TailscaleStatus` | commands/network.rs |
| `connect_rdp` | address | `()` | commands/ssh.rs |
| `check_ai_status` | — | `bool` | commands/ai.rs |
| `list_ai_models` | — | `Vec<String>` | commands/ai.rs |
| `send_ai_chat` | model, messages | `()` | commands/ai.rs |

### App and data

| Command | Args (camelCase) | Returns | Defined in |
|---|---|---|---|
| `get_app_info` | — | `AppInfo` | commands/app_data.rs |
| `export_database` | destPath | `()` | commands/app_data.rs |
| `import_database` | sourcePath | `()` | commands/app_data.rs |
## Events (backend → frontend)

| Event | Payload | Emitted by |
|---|---|---|
| `ssh_data_{id}` | terminal output chunk (batched) | `ssh.rs` |
| `ssh_stats_{id}` | `{ bandwidth, latency }` | `ssh.rs` |
| `ssh_closed_{id}` | — | `ssh.rs` (server closed the session) |
| `ssh_timeout_{id}` | — | `ssh.rs` (30 min idle) |
| `ssh-host-key-verification` | pending host-key request (id, host, port, fingerprint, old fingerprint for a changed key) | `ssh.rs`; shown once, app-wide, by `vault/HostKeyPromptHost` (queued; keyed by request id so confirmation state never carries over) |
| `system-metrics` | `SystemMetrics` | monitoring loop |
| `alerts-triggered` / `alerts-recovered` | alerts / recoveries | `monitoring/task.rs` |
| `vault-auto-locked` | — | auto-lock ticker, and `import_database` (the imported vault needs its own password) |
| `host-discovered` | `DiscoveredHost` | `discovery.rs` (mDNS) |
| `scan_progress`, `scan_result`, `scan_complete`, `scan_error` | progress / one host / total / sanitized error | `scan_network` |
| `ai-chat-response` | streamed token | `ai.rs` |

The webview's capability has no `core:event` emit permission: it can listen, not emit.

## Managed state

Registered with `app.manage` in `lib.rs` setup: `SshState` (interactive sessions), `HostKeyApprovalState` (pending host-key requests and the keys their handshakes presented), `LocalPathGrants`, `SshConnectionPool`, `TunnelState`, `DiscoveryState`, `ScannerState`, `AlertEngine`, `VaultState`, `CredentialManager`, `SshKeyManager`, `AuditLogManager`, `HostTracker`, `MetricsCollector`.

## Vault

- **Key derivation (v2, `vault/kdf.rs`)**: Argon2id (47104 KiB, t=2, p=1) → 32-byte `ikm` → HKDF-SHA256 → AES key (memory only) + verifier (stored, constant-time compare). No password hash is stored. v1 vaults, whose stored PHC hash was the key (RSEC-001), migrate on their next unlock inside one IMMEDIATE transaction; after that commit nothing is allowed to fail the unlock, and the old hash is scrubbed with VACUUM + `wal_checkpoint(TRUNCATE)`.
- **Concurrency**: `VaultState.inner` is a tokio `RwLock` holding `master_key: Option<MasterKey>` (the one source of truth for "unlocked"). `credential_gate` (tokio `RwLock<()>`) serializes credential work with key changes:
  - Credential code gets the key only via `credential_access()` (user-initiated: counts as activity) or `credential_access_background()` (monitoring, scheduled runs: doesn't reset auto-lock). Both hold the gate shared. Drop the returned `CredentialAccess` as soon as decryption is done, before any network phase, so a slow connect can't stall a password change.
  - `change_master_password`, the v1→v2 migration and `import_database` hold the gate exclusively. Lock order is always gate → `inner`.
  - `change_master_password` drops `inner` during the slow Argon2 + re-encryption pass, then re-takes it and installs the new key only if the vault is still unlocked, so a `lock_vault()` in that window is respected. It runs as a spawned task the caller awaits, so dropping the caller can't cancel it between the DB commit and installing the new key. `changing_password` (an `AtomicBool` set by a drop guard) only suspends auto-lock.
- **Lockout**: 5/10/15 failures → 5/15/60 minutes, persisted in `vault_settings` and folded in at the top of every unlock (only ever raised), so relaunching the app doesn't reset it.

## Security decisions

- **Host keys**: an interactive connect emits `ssh-host-key-verification` and waits (up to 120 s; the handshake budget is 135 s). The frontend answers with the request id only: `trust_ssh_host_key(requestId)` stores the key that handshake presented, so the webview can't pin arbitrary keys. Changed keys also need a native confirmation. Non-interactive paths (exec, pool, SFTP) never prompt: `SshKeyManager::check_non_interactive` accepts only a key already trusted for that host and port. Fingerprints are taken over `vault::ssh_keys::presented_key_bytes` (a host certificate pins the key it certifies).
- **Local files**: `pick_local_file` / `pick_save_location` open the dialog in Rust and record a single-use grant (read for open, write for save), used up even if the command then fails. SFTP, export, import and scheduled SFTP tasks accept only granted paths. A saved SFTP task keeps its grant only while its file, direction, host and remote path stay the same; changing any of them needs a new pick.
- **Credential binding**: a credential with a stored `host` works only against that host (`vault::credentials::host_allowed`), on every path. Unbound credentials work anywhere, by design.
- **Credential type**: only SSH-type credentials (`ssh`, `ssh_key`, and the legacy column default `password`) ever authenticate an SSH connection, and SFTP takes SSH passwords only (`vault::credentials::check_type`). An API, database, RDP or other secret is never sent to an SSH server. Checked when a scheduled task or monitoring binding is saved, and again at use (the type can change later).
- **Native confirmations** (`native_confirm.rs`): changed host key, removing a pin, marking a key trusted, any change out of `rejected`, moving/clearing a credential's host, moving a host (address or port) that scheduled file transfers use, revealing a password. One dialog at a time, a 3 s refusal window after a decline, and an answer within 800 ms counts as a decline.

## Background work

- **Scan claim**: `scan_network` returns immediately and spawns the scan. The claim (`is_scanning` check-and-set + `stop_signal` reset, `scanner::claim_scan()`) happens synchronously in the command, before `tokio::spawn`. If it happened inside the task, a `stop_scan()` landing between spawn and task start would be overwritten. The general rule: any command that spawns work and returns must claim before spawning.
- **Scheduler**: a 60 s tick starts due runs detached (a `Semaphore` that outlives ticks bounds concurrency). `InFlightGuard` is claimed before spawning, by cron runs and "Run now" alike, so a task never overlaps itself.
- **SSH connect** (`ssh_connect.rs`): DNS, TCP and handshake are separate phases with their own timeouts, and errors name the phase that failed. The interactive terminal returns these diagnostics raw by design; other paths sanitize.
- **One-shot exec** (`ssh_exec.rs`): reads until the channel closes (OpenSSH sends exit-status after EOF), then waits at most 2 s for the close; output capped at 1 MiB.
- **Pool** (`ssh_pool.rs`): never disconnects a session a lease still holds, whether it is swept for age or invalidated after a failed command; `invalidate` only drops the session the failing lease used, not a replacement another task already made. **Tunnels** (`ssh_tunnel.rs`) end when their SSH session dies (checked every 5 s, keepalives).
- **Discovery** (`discovery.rs`): a singleton mDNS thread; it drops receivers that report `Disconnected` and ends when none remain.

## Process notes

- Bot-authored PRs have reverted a fix mid-review before (the scan claim, September 2026). Re-verify the invariants above after merging bot pushes; each has a named regression test in CLAUDE.md.

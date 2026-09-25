# CLAUDE.md — Quasar Codebase Guide

Quasar is a Tauri 2.x desktop application for remote infrastructure management: SSH sessions, SFTP, system monitoring, network discovery, cron-scheduled automation, and a security vault. React + TypeScript frontend, Rust backend, SQLite database.

## Session Start (agents: do this first)

1. **Load Memco memory before your first substantive reply.** Call `start_session` on
   **Memco Personal Memory** and on **Memco Shared Memory** (call `list_domains` before your
   first shared-memory search or write). If a Memco server isn't connected in this session, say so.
   Don't skip it silently.
2. **Keep Linear and Notion in sync with any Quasar work.** This is personal memory
   `quasar_linear_notion_continuous_updates`. Do it during the same task, not at session end:
   - **Identify the issue first.** Use an explicit EDW identifier from the task, branch, PR, or
     commit. Without one, pick an issue only if exactly one open issue's scope clearly matches the
     work, and name it (with the reason) in your reply. If none or several match, don't guess:
     skip the sync entirely (no Linear or Notion edits of any kind, including Last verified),
     and report that it was skipped and why.
   - **Match the state change to the evidence** (for the identified issue). Only finished work,
     backed by a merged or pushed commit/PR and passing checks, moves it to Done or ticks a "Done
     when" box. Tick only the boxes that evidence covers. Partial work sets In Progress and gets a
     comment listing what is left. Review-only or investigation work gets a comment and no status
     change.
   - **Overlapping issues are comment-only** (comment and link, no status or box changes, even for
     partial work) unless the same evidence completes their own "Done when" criteria, in which
     case they follow the Done rule above.
   - **Linear** (team "Edward's Playground", key EDW): apply the status and boxes above, and
     comment with the outcome, evidence, and commit/PR links. Projects: "Quasar: Full Audit +
     Cleanup" (EDW-17..EDW-26), "Quasar: Maintenance & PR triage".
   - **Notion** ("Quasar" entry in App Projects → Projects): whenever an issue was identified,
     set Last verified date and add links. Change Next action or the phase table under Current priorities only to match
     tracker changes made under the rules above. Never mark a phase done that Linear doesn't
     show as Done.
   - If an update fails or is skipped, report which one and why.

## Recent Work: Linear Backlog EDW-15 / EDW-16 (September 24, 2026)

- **EDW-15: credential writes are now serialized with master-password rekeying.** This was
  the last open deferred audit finding, and all five September 2 findings are now fixed.
  `VaultState::credential_gate` (tokio `RwLock<()>`) is held shared by every credential
  operation via `credential_access()` / `credential_gate()` and exclusively by
  `change_master_password` from its first line (before it sets `changing_password`, so a
  rotation dropped while queued can't leave the flag stuck) until the new key is installed. A write that arrives mid-rotation now waits and is encrypted under the new key.
  Before, it was encrypted with the old key and orphaned. See Security Notes item 14 and
  `AGENTS.md`.
- **EDW-16: Dependabot alert #1 is almost certainly `rsa` / RUSTSEC-2023-0071**
  (GHSA-c38w-74pg-36hr, CVSS 5.9, no upstream patch). It was identified by elimination
  (`npm audit` clean, `cargo audit`'s only vulnerability is that one) because the session
  couldn't read the Dependabot API (403). It's the already-documented accepted risk in
  `SECURITY.md`, which now carries the GHSA aliases. Dismissing the alert as "tolerable risk"
  has to be done in the GitHub UI.

---

## Recent Work: PR Triage (September 24, 2026)

Nine open bot-authored PRs (#54–#62) were reviewed against their Codex/Sourcery feedback.
Three had real, verified defects that were fixed before merging:

- **#60 "Fix plaintext password exposure" didn't fix it.** It added `reveal_credential_password`
  but left `password` in `CredentialFrontendView`, so `get_credential` still put the plaintext
  into React state on every view/edit/select. Now the view carries only `has_password`; the
  edit form no longer prefills the password (blank = keep); SFTP fetches it on demand (see
  Security Notes item 13). Its type allowlist also hid password controls for
  rdp/database/api/other credentials. That's now gated on `has_password`.
- **#58 (which folded in #54–56):** the `Intl.DateTimeFormat` cache in `MonitoringView` was
  keyed on UTC offset, which misses zone switches between zones sharing an offset (New York →
  Lima in winter). The cache was removed; metrics arrive every ~5s, so it saved nothing.
  `formatMetricTime` is now per-call; the test covers the same-offset switch.
- **#62's IPC audit note recorded "None found"** while the credential-write/rekey race was
  still open. It now records that race as an exception (fixed later the same day, EDW-15).

#54/#55/#56 were closed as superseded by #58. #57 (AuditLogViewer `useMemo`), #59 (host-key
hook error-path tests, which were mutation-checked and do catch a regression) and #61
(`React.memo`'d `RemoteHostsList`) were clean. #57, #61 and #62 had never had CI run on them.

---

## Previous Work: PR Backlog Cleanup (September 17, 2026)

26 open PRs (bot-authored: perf micro-optimizations, dead-code/comment removal, and test
coverage additions) were triaged and merged. 17 were clean as-authored. 9 had real,
verified problems and were fixed before merging — mostly test-quality issues (weak
assertions that couldn't actually catch a regression, mock state leaking across tests) but
one was a genuine concurrency bug: **PR #39's own "TOCTOU fix" for `scan_network` was
itself a no-op** (it only reordered two lock acquisitions already inside the same critical
section). The real race — a `stop_scan()` call landing in the window between
`tokio::spawn(...)` and the spawned task actually starting could be silently discarded —
is fixed by `scanner::claim_scan()`, called synchronously in the `scan_network` command
*before* `tokio::spawn`, not from inside the spawned task. See Security Notes item 11
below and `AGENTS.md` for the full writeup, including an incident where an automated bot
reverted this fix mid-review and it had to be reapplied.

---

## Repository Layout

```
Quasar/
├── src/                        # React/TypeScript frontend
│   ├── components/             # 55+ React components (views, dialogs, widgets)
│   │   ├── dashboard/          # Dashboard widgets (metrics, charts, maps)
│   │   └── vault/              # Vault/credential UI
│   ├── hooks/                  # Custom React hooks
│   ├── lib/utils.ts            # cn() helper (clsx + tailwind-merge)
│   ├── db.ts                   # Tauri SQL plugin interface
│   ├── test-setup.ts           # Vitest global mocks
│   ├── App.tsx                 # Root component + routing
│   └── *.test.ts*              # 49 test files / 351 tests (co-located with sources)
├── src-tauri/                  # Rust/Tauri backend
│   ├── src/
│   │   ├── lib.rs              # App setup, migrations, ALL Tauri commands
│   │   ├── vault.rs            # Vault state + auto-lock
│   │   ├── vault/
│   │   │   ├── credentials.rs  # AES-256-GCM encryption, CRUD
│   │   │   ├── ssh_keys.rs     # SSH host key trust management
│   │   │   └── audit.rs        # Security event logging
│   │   ├── ssh.rs              # Interactive SSH sessions (Channel::wait())
│   │   ├── ssh_auth.rs         # Shared password/key authentication
│   │   ├── ssh_connect.rs      # Phase-aware connect (DNS/TCP/handshake), used by all SSH/SFTP paths
│   │   ├── ssh_exec.rs         # One-shot SSH command execution
│   │   ├── ssh_pool.rs         # Pooled/reused SSH sessions for scheduled tasks
│   │   ├── ssh_tunnel.rs       # SSH port forwarding
│   │   ├── sftp.rs             # SFTP file transfer (password-auth only)
│   │   ├── tailscale.rs        # `tailscale status --json` CLI integration (no API key)
│   │   ├── launcher.rs         # External SSH/RDP client launch (OS terminal, mstsc)
│   │   ├── scheduler.rs        # Cron task runner
│   │   ├── monitoring.rs       # System metrics + alert rules
│   │   ├── scanner.rs          # Network port scanning
│   │   ├── discovery.rs        # mDNS network discovery (singleton)
│   │   ├── host_tracker.rs     # Discovered host persistence
│   │   ├── health.rs           # Ping + SSH pre-flight checks
│   │   ├── crypto.rs           # AES-256-GCM, Argon2id utilities
│   │   ├── validation.rs       # Input validation (IP, hostname, port, CIDR)
│   │   ├── errors.rs           # Error types + frontend sanitization
│   │   ├── ai.rs               # Ollama LLM integration
│   │   ├── db.rs               # DB connection helpers
│   │   └── main.rs             # Binary entry point
│   ├── migrations/             # numbered SQL migrations (001, 003–015; 011/012 are Rust hooks)
│   ├── tests/                  # Rust integration tests
│   ├── Cargo.toml
│   └── tauri.conf.json
├── conductor/                  # Internal docs & style guides
│   └── code_styleguides/       # typescript.md, html-css.md, general.md
├── docs/
│   ├── SCHEMA.md               # Database schema reference
│   └── CORE_WORKFLOWS.md       # End-to-end user workflows
├── scripts/tauri-dev.js        # Tauri dev wrapper (sets CARGO_TARGET_DIR)
├── .github/workflows/
│   ├── ci.yml                  # PR checks (TypeScript, Vitest, Clippy, Cargo test)
│   └── release.yml             # Multi-platform build + GitHub release
├── AGENTS.md                   # Detailed bug-fix log and architectural decisions
├── CODEBASE_AUDIT_REPORT.md    # Security audit findings + resolutions
└── package.json
```

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop runtime | Tauri 2.11 (`tauri` crate and `@tauri-apps/*` packages move in lockstep: bump a plugin's JS and Rust sides to the same minor in one change; Dependabot groups them) |
| Frontend framework | React 19 + TypeScript 5.8 |
| Build tool | Vite 7 |
| Styling | Tailwind CSS 4 (dark navy/cyan theme) |
| Icons | Lucide React |
| Charts | Recharts |
| Terminal | xterm.js 6 (`@xterm/xterm`, `@xterm/addon-fit`) |
| Network graph | vis-network + vis-data |
| Backend language | Rust (edition 2021), async via Tokio 1. Toolchain pinned in `rust-toolchain.toml` (1.98.1); MSRV `rust-version = "1.95"`, checked by CI's `msrv` job |
| Database | SQLite (rusqlite bundled, migrations via rusqlite_migration) |
| SSH/SFTP | russh 0.62 (host key + auth bundled in), russh-sftp 2.4 |
| Encryption | aes-gcm 0.11 (AES-256-GCM), argon2 0.5 (Argon2id) + hkdf 0.13 / sha2 0.11 (HKDF-SHA256 key/verifier split, `vault/kdf.rs`) |
| Secure memory | zeroize 1.8, secrecy 0.8 |
| Network scan | surge-ping, cidr-utils, dns-lookup, mdns-sd |
| System info | sysinfo 0.33 |
| Cron | cron 0.12 |
| AI (optional) | ollama-rs 0.3 |
| Auto-update | tauri-plugin-updater / `@tauri-apps/plugin-updater` 2.x (signed artifacts, verified against an embedded pubkey) |
| Tailscale | local `tailscale` CLI only — no API key, no control-plane calls (`src-tauri/src/tailscale.rs`) |
| Testing | Vitest 4, React Testing Library 16, jsdom |

---

## Development Commands

```bash
# Frontend dev server only
npm run dev

# Full Tauri dev (frontend + Rust backend) — use this for normal development
npm run tauri

# TypeScript type-check + Vite production build
npm run build

# Run frontend tests
npm test

# Frontend tests with coverage (fails below the floor in vite.config.ts)
npm run coverage

# Rust: lint
cd src-tauri && cargo clippy -- -D warnings

# Rust: unit + integration tests
cd src-tauri && cargo test
```

`npm run tauri` calls `scripts/tauri-dev.js`, which sets `CARGO_TARGET_DIR=src-tauri/target` and runs `npx tauri dev`. On Windows it uses `npx.cmd`.

The Vite dev server is fixed to port **1420** (required by Tauri).

---

## Architecture: Frontend ↔ Backend IPC

All communication goes through Tauri's IPC layer — no HTTP server.

### Commands (Frontend → Backend)

All Tauri `#[command]` functions are registered in `src-tauri/src/lib.rs`. Key groups:

**Vault**
- `initialize_vault(password)` / `unlock_vault(password)` / `lock_vault()`
- `change_master_password(old_password, new_password)`
- `get_vault_settings()` / `update_vault_settings(...)`
- `export_vault(path)` / `import_vault(path)`

**Credentials**
- `add_credential(name, credential_type, username, password?, private_key?, ...)`
- `update_credential(id, ...)` / `remove_credential(id)`
- `list_credentials()` / `get_credential(id)` — `get_credential` returns **no secrets** (only `has_password` / `has_private_key` / `has_key_passphrase` flags)
- `reveal_credential_password(credential_id)` — the only command that returns a decrypted password to the frontend; call it only at the moment of explicit reveal/copy or SFTP session start

**Hosts**
- `add_host(hostname, port, username, protocol)` / `update_host(...)` / `remove_host(id)`
- `list_hosts()` / `get_saved_hosts()`
- `upsert_saved_host(name, address, protocol, port?, username?)` — `protocol` is a free-form string (`AddHostDialog` offers `ssh` / `rdp` / `database` / `api` / `other`); only `ssh` and `rdp` have an in-app client (Connect button, port default). Other protocols are inventory/monitoring-only entries.

**SSH**
- `start_ssh_session(host_id, credential_id?, password?)` → returns `session_id`
- `write_ssh(session_id, data)` / `resize_ssh(session_id, rows, cols)` / `close_ssh_session(session_id)`
- `execute_ssh_command(host_id, command, credential_id?, password?)`
- `get_known_ssh_hosts()` / `remove_ssh_host_key(host, port)` / `update_ssh_host_trust(host, port, trustStatus)` / `trust_ssh_host_key(requestId)` / `respond_ssh_host_key_verification(requestId, accepted)`

**SFTP** — password auth only (no SSH key support in backend). Pass a vault `credentialId` (decrypted in the backend, like SSH) or a manually typed `password`. `localPath` must come from `pick_local_file` / `pick_save_location` (see Security Note 17).
- `sftp_upload_file(host, port, username, password?, credentialId?, localPath, remotePath)`
- `sftp_download_file(host, port, username, password?, credentialId?, remotePath, localPath)`
- `sftp_list_directory(host, port, username, password?, credentialId?, remotePath)`
- `pick_local_file(title?, extensions?)` / `pick_save_location(defaultName?, extensions?)` → `string | null`: backend-opened native dialogs; the chosen path is recorded as a single-use grant (read for an open dialog, write for a save dialog)

**Monitoring & Alerts**
- `get_system_metrics()` / `get_remote_hosts_health()`
- `add_alert_rule(rule)` (upsert by `rule.id`; validated: threshold 0–100, cooldown ≤ 1 day) / `remove_alert_rule(ruleId)` / `get_alert_rules()` — persisted in `alert_rules` (migration 015) and edited in the Monitoring view's Alert Rules panel

**Network Discovery & Scanning**
- `start_network_scan(cidr, timeout?)` / `get_discovered_hosts()`
- `save_discovered_host(ip, hostname, port, username)` / `remove_discovered_host(ip)`

**Scheduled Tasks**
- `list_scheduled_tasks()`
- `add_scheduled_task(name, cron_expression, host_id, task_type, command?, ..., credential_id?, enabled)`
- `update_scheduled_task(id, ...)` / `remove_scheduled_task(id)`
- `run_scheduled_task_now(task_id)`

**SSH Tunnels**
- `create_tunnel(host_id, local_port, remote_addr, remote_port, ...)` / `close_tunnel(tunnel_id)` / `list_tunnels()`

**Tailscale**
- `get_tailscale_status()` — runs `tailscale status --json` locally and returns tailnet peers; `installed: false` (not an error) when the CLI isn't found

**AI**
- `send_ai_message(message, context?)`

### Events (Backend → Frontend)

Listen with `listen()` from `@tauri-apps/api/event`:

| Event | Payload | Description |
|-------|---------|-------------|
| `ssh_data_{id}` | `string` | Terminal output chunk for session `id` |
| `ssh_closed_{id}` | `{}` | SSH session `id` terminated by server |
| `ssh_timeout_{id}` | `{}` | SSH session `id` closed after 30 min idle |
| `ssh_stats_{id}` | `{ bandwidth: string, latency: number }` | Per-session SSH stats |
| `ssh-host-key-verification` | host key object | Prompt user to trust/reject a new host key |
| `system-metrics` | `SystemMetrics` | Periodic local system metrics (CPU, mem, disk, …) |
| `alerts-triggered` | `Alert[]` | One or more alert thresholds exceeded |
| `alerts-recovered` | `AlertRecovery[]` | Alerts cleared (metric back below threshold) |
| `vault-auto-locked` | `{}` | Vault locked due to inactivity timeout |
| `host-discovered` | `DiscoveredHost` | mDNS host discovered during network scan |
| `scan_progress` | `{ scanned, total }` | Network scan progress update |
| `scan_result` | `ScanResult` | Individual host scan result |
| `scan_complete` | `{ total }` | Network scan finished |
| `scan_error` | `string` | Network scan failed with error |
| `ai-chat-response` | `string` | AI assistant streaming token |

---

## Database

SQLite database stored in the platform app-data directory as `quasar.db`.

Migrations run automatically on startup via `rusqlite_migration`. Migration files are in `src-tauri/migrations/` numbered `001` through `015` (there is no `002`). `LATEST_SCHEMA_VERSION` in `lib.rs` must equal the number of `MIGRATIONS` entries (currently 14); a test enforces it.

### Key Tables

| Table | Purpose |
|-------|---------|
| `hosts` | Saved remote servers |
| `credentials` | Encrypted passwords/SSH keys (AES-256-GCM) |
| `vault_settings` | KDF salt + verifier (`kdf_version` 2), auto-lock timeout, persisted lockout (see Security Notes 15-16) |
| `ssh_known_hosts` | SSH host key fingerprints |
| `security_audit_log` | Vault and credential access events |
| `discovered_hosts`, `host_services` | Network scan results |
| `scheduled_tasks` | Cron automation tasks |
| `scheduled_task_runs` | Task execution history |
| `system_metrics_history` | Historical monitoring data |
| `alert_rules`, `alert_history` | Alert configuration and log |

See `docs/SCHEMA.md` for full column definitions.

### Adding Migrations

1. Create `src-tauri/migrations/016_description.sql`
2. Register it in `lib.rs` where migrations are initialized
3. Use `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` for additive changes

---

## Testing

### Frontend Tests

```bash
npm test          # run all tests once
npm test -- --watch   # watch mode
```

Tests live alongside source files as `*.test.tsx`. The setup file `src/test-setup.ts` globally mocks:
- `window.__TAURI_INTERNALS__`
- `@tauri-apps/api/core` (`invoke`)
- `@tauri-apps/api/event` (`listen`, `emit`)
- `@tauri-apps/plugin-dialog`
- `@tauri-apps/api/path`

**All Tauri API calls must be mocked in tests.** Use `vi.mocked(invoke).mockResolvedValue(...)` to set return values.

#### Test file inventory (49 files / 351 tests as of September 24, 2026)

Not exhaustive — a curated subset covering the components with the most notable test
patterns or regression history. Run `find src -name "*.test.ts*"` for the full list.

| Test file | Component tested | Key scenarios |
|-----------|-----------------|---------------|
| `App.test.tsx` | App root | Routing, sidebar navigation |
| `Layout.test.tsx` | Layout | Render, vault integration |
| `ErrorBoundary.test.tsx` | ErrorBoundary | Error catching, crash UI, reload |
| `TopBar.test.tsx` | TopBar | View label mapping for all 7 views |
| `HostManagement.test.tsx` | HostList, AddHostDialog | List, filter, connect/SFTP callbacks, remove, duplicates, empty state, full protocol option list (ssh/rdp/database/api/other), required-port toggle for protocols without a backend default |
| `HostList.test.tsx` | HostList | Loading/empty states, filtering, duplicate detection/removal, single removal (both verify the row disappears after the post-removal refresh, not just that the `invoke` call happened), protocol-gated action buttons (database host has neither Connect nor SFTP). `window.confirm` is spied per-test with `afterEach` restore — a module-scope spy here previously leaked into other test files. |
| `NetworkScanner.test.tsx` | NetworkScanner | CIDR input, scan start/stop, CIDR validation error, scan failure, initialResults |
| `NetworkTopologyView.test.tsx` | NetworkTopologyView | Empty/populated states, physics/fit/zoom controls, search, click/double-click callbacks. The zoom test models the mocked network's actual scale statefully across clicks (asserting the compounded value) rather than a fixed mock return, and the topology-population assertion checks specific node/edge ids and call count. |
| `SshFileManager.test.tsx` | SshFileManager | Path bar, loading, error, file listing |
| `HealthCheckBadge.test.tsx` | HealthCheckBadge | Online/offline/checking states |
| `HostDetailDialog.test.tsx` | HostDetailDialog | Tabs, connect/close callbacks, clipboard-copy-failure logging (saves and restores `navigator.clipboard` in a `finally` — it used to overwrite the global permanently and leak into later tests) |
| `PreflightDialog.test.tsx` | PreflightDialog | Health check flow |
| `AIAssistant.test.tsx` | AIAssistant | Chat, streaming, error |
| `CredentialPrompt.test.tsx` | CredentialPrompt | Render, submit, cancel, save-credential toggle, empty-field validation (includes a whitespace-only username case, which satisfies the native `required` attribute but must still fail the component's own `.trim()` guard — a plain empty-field case never reaches the component's `handleSubmit` because jsdom blocks the native-invalid form submit first) |
| `ScheduledTasksView.test.tsx` | ScheduledTasksView | Form validation: save without a host, invalid cron expression |
| `UpdateBanner.test.tsx` | UpdateBanner | No update / available / install-and-relaunch / dismiss / check-failure |
| `vault/VaultProvider.test.tsx` | VaultProvider | Loading, init, unlock, error states |
| `vault/VaultInitDialog.test.tsx` | VaultInitDialog | Password validation (all rules), strength indicator, invoke, error |
| `vault/VaultUnlockDialog.test.tsx` | VaultUnlockDialog | Empty password, success, error, cancel, password toggle |
| `vault/CredentialSelector.test.tsx` | CredentialSelector | Loading, type/host filter (incl. SFTP ssh_key exclusion), search, select, manual entry, error |
| `vault/CredentialManager.test.tsx` | CredentialManager | List, empty, loading, add/view/edit/delete dialogs, search, confirm flows, error, camelCase `invoke` payload keys on update (regression guard), editing an SSH-key credential with no `key_path` (`has_private_key`-gated validation, regression guard), password never rendered/fetched until the reveal toggle calls `reveal_credential_password`, password controls shown for every password-backed type (e.g. rdp), edit leaves password blank and sends `password: null` (keep) |
| `vault/KnownHostsManager.test.tsx` | KnownHostsManager | List, search by host/fingerprint, remove with confirm, trust updates, error |
| `vault/AuditLogViewer.test.tsx` | AuditLogViewer | List, search, event type filter, result filter, resource info, error |
| `vault/SshHostKeyPrompt.test.tsx` | SshHostKeyPrompt | New vs changed key, trust/reject, permanent toggle, copy fingerprint, MITM warning |
| `vault/VaultSettings.test.tsx` | VaultSettings | Load/save/error, auto-lock, lock vault, change password validation |
| `dashboard/AlertFeed.test.tsx` | AlertFeed | Empty state, alert render, dismiss, acknowledge, clear all, metrics summary |
| `dashboard/QuickConnectWidget.test.tsx` | QuickConnectWidget | Loading, host list, search, connect callback, overflow indicator |
| `dashboard/AlertRules.test.tsx` | AlertRules | Rule CRUD |
| `dashboard/MetricChartCard.test.tsx` | MetricChartCard | Rendering, custom color forwarded to the chart's stroke/gradient (the Recharts `Area` mock must forward its props — a mock that discards them lets a broken color prop pass silently) |
| `dashboard/SystemHealthWidget.test.tsx` | SystemHealthWidget | Metrics display, CPU/memory/disk/alerts/hosts-online/vault-timeout, event-driven updates |
| `hooks/useUpdater.test.ts` | useUpdater | State transitions (idle/checking/available/upToDate/error/downloading), auto-check, install+relaunch (awaits the `installUpdate()` promise itself rather than polling for the intermediate `downloadAndInstall` call, which is invoked synchronously and can't be used to infer that the later `relaunch()` call has actually run), unmount cleanup |
| `lib/utils.test.ts` | `cn()`, `getErrorMessage()` | Class merging/conditionals/arrays/falsy values/Tailwind conflicts; error extraction from `Error`/string/object/non-string `.message`/null/undefined with fallback |
| `TailscalePeers.test.tsx` | TailscalePeers | Not-installed hint, needs-login hint, peer list with SSH chip/online dot, Add payload uses `preferred_address`, "Saved" state for a peer matching an existing host, empty state |
| `hooks/useTailscaleStatus.test.ts` | useTailscaleStatus, `findTailscalePeer` | Fetch on mount, error surfaced with status left `null`, `refresh()` re-fetches, concurrently mounted consumers coalesce onto one backend call, matcher matches by MagicDNS name/IPv4/hostname case-insensitively and returns `undefined` for no match/no status/no address. The poll is module-scoped and shared, so tests touching it must call `resetTailscaleStatusCache()` in `beforeEach`. |

#### Key testing patterns

- Components with `export default` (most vault components, ErrorBoundary, TopBar, AlertFeed, QuickConnectWidget) are imported with default import syntax: `import VaultSettings from './VaultSettings'`
- `window.confirm` must be stubbed via `vi.spyOn(window, 'confirm').mockReturnValue(true/false)` (not `vi.stubGlobal`) — restore with `spy.mockRestore()` after each test
- Selects without `aria-label`/`id` (e.g. AuditLogViewer filters) are queried by index: `screen.getAllByRole('combobox')[0]`
- When multiple elements match the same text (heading + button both say "Unlock Vault"), use `getAllByText(...)` or role-scoped queries like `getByRole('button', { name: ... })`
- Saved-host components mock `invoke` commands such as `get_saved_hosts`, `upsert_saved_host`, and `remove_saved_hosts`; frontend tests never open SQLite directly
- **`invoke()` payload keys must be lowerCamelCase.** Tauri's command macro matches JSON keys against the camelCased Rust parameter name and silently treats a missing key as `None` for `Option<T>` params — there is no "unknown key" error. A snake_case key (e.g. `key_path` instead of `keyPath`) doesn't fail, it just never updates that field. This bit `CredentialManager.tsx`'s update/add payloads once; `CredentialManager.test.tsx` now asserts no payload key contains `_`.
- **`get_credential` never returns any decrypted secret — password, private key, or passphrase** — only `has_password`/`has_private_key`/`has_key_passphrase` booleans (see `CredentialFrontendView` in `vault/credentials.rs`). Any form/validation built on the result of `get_credential` must treat an empty `private_key` field as "not shown", not "not stored" — check the `has_*` flag (or `key_path`, which *is* returned in cleartext) before concluding no key exists. Got this backwards once in `CredentialManager.tsx`'s edit-submit validation, which blocked saving any change to a credential whose key was originally pasted as PEM rather than a file path. Same for the password: don't prefill it in forms, and never re-add it to `CredentialFrontendView`; fetch it via `reveal_credential_password` only when the user explicitly asks or SFTP needs it (`RemoteManager.test.tsx` covers the SFTP path).

### Rust Tests

```bash
cd src-tauri && cargo test
```

Integration tests are in `src-tauri/tests/`. Unit tests use `#[cfg(test)]` modules within source files.

---

## Code Conventions

### TypeScript / React

Source: `conductor/code_styleguides/typescript.md` (Google TypeScript Style Guide).

- **`const` by default**, `let` if needed, never `var`
- **Named exports only** — no default exports
- **No `any`** — use `unknown` or a specific type
- **No type assertions** (`as SomeType`, `x!`) without a clear comment justifying it
- **No `#private` fields** — use TypeScript `private` keyword
- **No `public` modifier** (it's the default)
- **Single quotes** for strings; template literals for interpolation
- **Strict equality** — always `===` / `!==`
- **Explicit semicolons** — never rely on ASI
- **No `const enum`** — use plain `enum`
- **No `eval()`**
- **No `{}` type** — prefer `unknown`, `Record<string, unknown>`, or `object`

**Naming**:
- `UpperCamelCase` — classes, interfaces, types, enums
- `lowerCamelCase` — variables, functions, methods, properties
- `CONSTANT_CASE` — global constants and enum values
- No `_` prefix/suffix on identifiers

**Comments**: JSDoc `/** */` for documentation, `//` for inline notes. Don't restate the code.

**CSS utility**: Use the `cn()` helper from `src/lib/utils.ts` (wraps `clsx` + `tailwind-merge`) for conditional class composition.

### Rust

- Tauri commands return `Result<T, String>` — convert errors with `.map_err(|e| e.to_string())` or the error sanitization in `errors.rs`
- No `.unwrap()` or `.expect()` in non-test code
- Zeroize sensitive data (passwords, keys) when done — use `zeroize` crate
- Use `secrecy::Secret<String>` for in-memory secrets
- Validate all external inputs through `validation.rs` before processing
- Database work should use transactions for multi-step operations
- `discovery.rs` uses a singleton pattern (`Arc<AtomicBool>`) — do not bypass it
- `tailscale.rs` only shells out to the local `tailscale` CLI with fixed arguments (`status --json`) — never pass user input into that command, and never call the Tailscale control-plane API directly

### Component Patterns

- Functional components with hooks only (no class components)
- Every view stays mounted (hidden with CSS). Load lists other views can change (hosts, credentials, tasks) with `useOnViewShown` (runs on mount and each time the view is shown), not a mount-only `useEffect`, or they go stale (FE-004). Poll with `useVisiblePolling`.
- Global vault state via `VaultProvider` context (`src/components/vault/VaultProvider.tsx`)
- The app root has an `ErrorBoundary` (full-screen "Reload App"), and `Layout` wraps each view in `<ErrorBoundary scope="…">`, whose inline fallback re-mounts only that view ("Try again"), so a render error in one view no longer blanks the app or forces a reload that drops SSH sessions (FE-002)
- SSH host-key prompts are rendered once, app-wide, by `vault/HostKeyPromptHost` in `Layout` (FE-011: inside RemoteManager they were invisible from other views). Don't mount `useSshHostKeyVerification` a second time: each instance would queue its own copy of every prompt.
- SSH credential selectors must filter out `ssh_key` type when the target is SFTP (backend only supports password auth for SFTP)

---

## Security Notes

1. **Vault must be unlocked** before any credential operations. The encryption key lives only in memory; the database stores a salt and an HKDF-derived **verifier**, never anything the key can be computed from (see item 15).
2. **Credentials are encrypted** with AES-256-GCM under a key derived from the master password at unlock (`vault/kdf.rs`).
3. **SFTP only supports password credentials.** `CredentialSelector` takes `allowedTypes`; SFTP contexts pass `allowedTypes={['ssh']}` (see `RemoteManager.tsx`), which excludes `ssh_key` credentials.
4. **Discovery is a singleton.** Calling `start_network_scan` when already running returns early; there is no duplicate-thread risk.
5. **Database export** uses `VACUUM INTO` (not file copy) to get a consistent snapshot while connections are live.
6. **Input validation** (`validation.rs`) must be called on all user-supplied network values before use in commands.
7. **Interactive SSH connect errors bypass `sanitize_error()` by design.** `connect_ssh` (the terminal path in `ssh.rs`) returns `ssh_connect::connect_with_diagnostics()` errors verbatim to the frontend — including the resolved IP/port and the phase that failed (DNS/TCP/handshake) — so users can self-diagnose (wrong port, firewalled host, dead DNS record). SFTP and pooled/scheduled-task SSH paths still route through `sanitize_error(e, "sftp"/"ssh")` at the `lib.rs` command boundary.
8. **All SQLite connections must go through `db::open_connection()`.** It sets the 5s busy-timeout, WAL journal mode, and the foreign-keys pragma. `HostTracker` and `MetricsStore` used to open raw `rusqlite::Connection`s directly and got none of these — under normal write contention (scheduler + monitoring + a network scan writing concurrently) they'd fail fast with `SQLITE_BUSY` instead of waiting, and the caller only logged it, so scanned hosts and monitoring samples were silently dropped. Fixed Aug 22, 2026 by routing both through `db::open_connection()`; don't reintroduce a raw `Connection::open()` call outside `db.rs`.
9. **`VaultState.changing_password` (an `AtomicBool` outside `inner`, set by a `RotationInProgress` drop guard so a cancelled rotation can't leave auto-lock disabled) only gates `check_auto_lock()`, not `lock_vault()`.** (A second concurrent rotation queues on `credential_gate`, then fails current-password verification against the hash the first one wrote.) Credential operations are gated separately by `credential_gate` (item 14). `change_master_password` drops its write lock during the multi-second Argon2id + credential re-encryption pass (so other vault reads, e.g. an in-flight SSH credential lookup, aren't stalled for the whole operation) and only reacquires it briefly at the start and end. Because of that window, an explicit `lock_vault()` call can land mid-change — the reacquire-and-install step at the end checks `inner.master_key.is_some()` before installing the new key, so an explicit lock is respected instead of silently reverted. If you touch `change_master_password` again, preserve that check; see `.claude/agent-memory/security-reviewer/vault_rs_patterns.md` for the full locking model and a regression test at `vault::tests::test_lock_vault_during_password_change_stays_locked`.
10. **`unlock_vault`'s failed-attempt lockout (5 → 5 min, 10 → 15 min, 15 → 60 min) is persisted to `vault_settings`** (`lockout_failed_attempts` / `lockout_until_unix`), not just tracked in-memory. `VaultStateInner` is rebuilt fresh on every app launch, so an in-memory-only counter would let an attacker with local file access bypass the lockout by relaunching the app every 4 guesses. `load_persisted_lockout()` folds the persisted state into `inner` at the top of every `unlock_vault` call (only ever raising it, never lowering) and `persist_lockout()` writes it back on every failure and on success (clearing it). Regression tests: `vault::tests::test_lockout_persists_across_vault_state_restart`, `test_successful_unlock_clears_persisted_lockout`.
11. **`scan_network`'s claim (`is_scanning` check-and-set + `stop_signal` reset) must happen synchronously in the `scan_network` Tauri command, before `tokio::spawn`, via `scanner::claim_scan()` — never inside the spawned task.** The command spawns the actual scan and returns immediately; if the claim happened inside the spawned task instead, a `stop_scan()` call landing in the window between `tokio::spawn(...)` and the task actually starting would set `stop_signal = true`, only for the still-starting scan to unconditionally reset it back to `false` when its own claim ran — silently discarding the user's stop request. `scan_network()` itself now assumes the caller already claimed it: it installs `ScanRunningGuard` as its first action (before any fallible work, so the claim is always released even on early return) and checks `stop_signal` before opening the raw ICMP socket (so an already-stopped scan doesn't pay for, or need privileges for, a socket it won't use). Fixed Sep 17, 2026; regression tests: `scanner::tests::test_claim_scan_rejects_concurrent_claim`, `test_stop_request_after_claim_is_not_lost`. See `AGENTS.md` for the incident where a bot-pushed commit reverted this fix mid-review and it had to be reapplied on top of a master merge.
12. **`ssh_auth::authenticate` falls back to `none` authentication only when no password and no key material were supplied at all** — a present-but-wrong password or key still fails normally; this path exists specifically for identity-aware servers like Tailscale SSH, which authenticate the tailnet connection out-of-band and accept a bare `authenticate_none` request. Tailscale SSH "check mode" (browser re-verification over keyboard-interactive) is not implemented — it surfaces as a connection error in the terminal, same as any other rejected `none` auth attempt.
13. **Decrypted passwords only cross IPC through `reveal_credential_password`, and only for an explicit reveal/copy in the credential view.** SFTP now takes a `credentialId` like SSH (September 24, 2026, FE-001/RSEC-013); before that the SFTP tab held the revealed vault password in React state for its lifetime. `get_credential`'s `CredentialFrontendView` has `has_password` instead of a `password` field (Rust regression test `vault::credentials::tests::test_frontend_view_carries_no_secret_material`). The frontend calls `reveal_credential_password` only on an explicit reveal/copy in the view dialog, and the backend shows a native confirmation before returning it (Security Note 17g). SFTP passes the credential ID and never reveals. SSH sessions pass the credential ID and never need it. Until September 24, 2026 the view carried the plaintext, so opening any credential put it into React state.
14. **Credential code must get the master key through `VaultState::credential_access()` (user-initiated: counts as activity for auto-lock) or `credential_access_background()` (monitoring polls, scheduled runs: must not reset the auto-lock timer, RSEC-002), never `get_master_key()` (now `pub(crate)`).** `credential_access()` returns the key plus a shared hold on `credential_gate`. `change_master_password` takes that gate exclusively for the whole rotation, before it sets `changing_password` or reads the old key, so a credential encrypt/decrypt can't use a key that's about to be replaced. (`get_master_key()` bypasses the gate; it's kept for tests and the rotation internals.) Drop the `CredentialAccess` as soon as decryption is done (`connect_ssh`, `start_ssh_tunnel` and `get_remote_hosts_health` drop it before their network phase) so a slow connect doesn't stall a password change. `delete_credential` takes `credential_gate()` (no key needed). Lock order is gate → `inner`. Regression test: `vault::tests::test_credential_write_waits_for_master_password_change`. Fixed September 24, 2026 (EDW-15).
15. **Vault key derivation is v2 (`vault/kdf.rs`); never store an Argon2 hash of the master password.** One Argon2id pass (m=47104 KiB, t=2, p=1) gives `ikm`; HKDF-SHA256 expands it into the AES key (memory only) and a verifier (`vault_settings.master_password_verifier`, hex, compared in constant time), with `kdf_version = 2`. Until September 24, 2026 (v1) the vault stored `argon2.hash_password(pw, salt)` and used `hash_password_into(pw, same salt)` as the key, which are the same bytes in argon2 0.5: **the stored hash was the key** (audit RSEC-001, Critical; reproduced in `audit/raw/refresh/rsec001-proof.txt`). v1 vaults (no `kdf_version` row) are migrated on the next successful unlock: fresh salt, all credentials re-encrypted in one IMMEDIATE transaction, `master_password_hash` deleted, then VACUUM + `wal_checkpoint(TRUNCATE)` to scrub the old hash. Only rows whose password ciphertext fails AES-GCM authentication under the legacy key (orphans) are skipped: they're left untouched, listed in the `vault_kdf_migration` audit event, and the old salt is kept as `legacy_salt` so they stay recoverable. Any other row error aborts the migration. If the migration fails before commit, the unlock still succeeds on the legacy key and it's retried next time. **After the commit nothing may fail it** (audit, scrub and `.bak` cleanup are best-effort), otherwise the vault would run on a key the DB no longer uses; `change_master_password` follows the same rule, and runs the rotation as a spawned task it awaits, so dropping the caller can't cancel it between the commit and installing the new key (test `test_cancelled_password_change_clears_rotation_flag` drops the caller in that window and checks a credential written afterwards survives relock). An incomplete scrub (busy checkpoint, failed VACUUM) sets `kdf_scrub_pending`, retried on every unlock. The migration also deletes `master_password_hash` from `quasar.db.bak`. A present-but-malformed encrypted field (partial NULL triple, wrong nonce/tag length) is an error, never "absent", so re-encryption can't overwrite it with NULL. `unlock_vault` therefore takes `credential_gate` exclusively (gate → `inner`, like the rotation). **Exports and `quasar.db.bak` files made before the migration still expose their credentials: treat them as plaintext.** `db::open_connection` sets `secure_delete = ON` and makes the DB/WAL/SHM `0600`; the app data dir and exports are owner-only too (Unix; RSEC-009). Changing the Argon2 params, the HKDF labels or the salt decoding orphans every vault: `vault::kdf::tests::kdf_known_answer_*` pin them (the v2 values were computed independently in Python). Regression tests: `vault::tests::test_stored_vault_settings_never_contain_master_key`, `test_legacy_vault_is_migrated_to_v2_on_unlock`, `test_failed_legacy_migration_keeps_vault_usable_and_unchanged`, `test_change_master_password_refuses_when_key_does_not_match_database`, `test_migration_post_commit_failure_still_installs_new_key`, `test_password_change_post_commit_failure_still_installs_new_key`, `test_migration_aborts_on_non_decryption_errors`, `test_migration_keeps_legacy_salt_for_orphans_and_strips_backup_hash`, `test_legacy_unlock_waits_for_credential_gate`, `credentials::tests::test_malformed_private_key_blob_is_an_error_not_absent`, `db::tests::test_database_file_is_owner_only`.

16. **Replacing the database locks the vault.** `import_database` (via `import_database_at`, testable) holds `credential_gate` exclusively through `VaultState::lock_for_database_replacement()`, locks the vault (the imported file has its own salt/verifier, so the in-memory key is stale), emits `vault-auto-locked` so the UI asks for the imported vault's password, and rejects backups whose `user_version` is newer than `LATEST_SCHEMA_VERSION` (keep that const equal to the number of `MIGRATIONS` entries; a test enforces it). Before September 2026 credentials written after an import were orphaned, and a newer backup bricked startup (RSEC-004, RUST-004). Vault settings: only `auto_lock_timeout_minutes` is caller-settable, validated to 1-1440 server-side, persisted, and loaded into memory at unlock (it used to reset to 15 after every restart). The "require master password for credential access" toggle was removed: it was never enforced. User-facing vault errors (wrong password, lockout, the list of credentials blocking a password change) pass through `errors::user_facing_vault_error`; everything else is still sanitized. Password changes refuse, naming each credential, when some can't be decrypted with the current key. Failed unlocks, failed password changes and plaintext reveals (`credential_reveal`) are audited. Regression tests: `lib::tests::import_locks_the_vault`, `import_rejects_newer_schema_and_leaves_vault_alone`, `vault::tests::test_background_access_does_not_count_as_activity`, `test_saved_auto_lock_timeout_is_loaded_on_unlock`, `test_update_settings_validates_timeout_and_ignores_derived_fields`, `test_cancelled_password_change_clears_rotation_flag`, `test_password_change_lists_undecryptable_credentials`, `test_failed_unlock_is_audited`, `test_gate_held_until_new_key_installed`.

17. **IPC hardening (September 24, 2026, audit P7-3).** (a) **Local file paths** for SFTP transfers, DB export/import and scheduled SFTP tasks must come from a backend-opened dialog (`pick_local_file`, `pick_save_location`); `local_paths::LocalPathGrants` records the choice and the commands reject anything else. Each grant is **single-use and intent-bound**: an open dialog grants reading (upload source, import), a save dialog grants writing (download target, export), so a file picked for upload can't then be overwritten. A failed SFTP transfer gives its grant back (so a retry after a wrong password doesn't need the dialog again). A task update may keep an unchanged path only if its type is unchanged too (upload → download needs a new pick). The webview has no dialog permission and the `@tauri-apps/plugin-dialog` JS package is gone. Before, any absolute path was accepted: arbitrary local file read/write from a compromised webview (IPC-001). (b) **Host keys are trusted by pending request only**: `trust_ssh_host_key(requestId)` persists the key that handshake presented (`HostKeyApprovalState` keeps it); the webview can no longer pin arbitrary keys (IPC-002). A changed key defaults to one-time trust and needs an explicit "I verified" confirmation (RSEC-007). A known-host lookup error fails closed. (c) **A credential with a stored `host` only works against that host** (`vault::credentials::host_allowed`): connect, tunnel, monitoring poll, scheduled runs, SFTP, and `set_host_monitoring_credential` (IPC-003). (d) **Scheduled tasks** are validated (known `task_type`, existing host, bounded name/command, and a host-bound credential must match the task's host at save time) and create/update/delete/run are audited. Scheduled runs use `credential_access_background()`; "Run now" is user-initiated and uses `credential_access()`, so it resets the auto-lock timer; an unknown type no longer runs as SSH exec (IPC-011). (e) **Least-privilege capability** (no `core:event` emit, opener limited to ollama.com, no dialog) and a production CSP without `localhost:*` (dev-only via `devCsp`), pinned by `capability_and_csp_stay_least_privilege` (IPC-007/008). (f) Eight never-invoked commands were removed (IPC-010), plus `verify_ssh_host_key`. (g) **Native confirmations** (`native_confirm.rs`): commands that weaken a security decision ask through a backend-opened OS dialog the webview can't click: accepting a *changed* host key (once or permanently), `remove_ssh_host_key`, `update_ssh_host_trust` to Trusted, `update_credential` moving or clearing a bound credential's host, and `reveal_credential_password`. Declining returns the error `Cancelled` (`isUserCancelled()` in `lib/utils.ts`; treat it as a no-op) and rejects a pending handshake. Don't add a webview `confirm()` in front of these. Dialog hardening: one dialog at a time, a 3s refusal window after a decline (so a request loop can't bury the user in dialogs), an answer within 800ms counts as a decline (the confirm button is the platform default, so a webview could time a dialog to a terminal Enter), and webview-controlled text in dialogs goes through `native_confirm::display_text` (no control characters, 64 chars). The reveal checks the vault is unlocked before asking, and declined reveals are audited (`credential_reveal` / `declined`). The password Copy button works only after a reveal (the dialog would expire WebKit's clipboard gesture). Known-host matching is case-insensitive on the host (`COLLATE NOCASE`), and a presented key for a host that has a *different* trusted key on another port is treated as Changed (with that key as `old_fingerprint`), so neither letter case nor a port switch turns a pinned host into a first-seen one. A key the user marked Rejected is refused with no prompt (`ssh::interactive_decision`). Tests: `test_case_and_port_changes_keep_the_pin`, `rejected_keys_are_refused_without_a_prompt`. Still webview-decided by design: trusting a never-seen host key (TOFU) and using a credential with **no** stored host against any host, so a compromised webview can still send an unbound credential anywhere; bind credentials that matter. Import runs its SQLite backup/restore in `spawn_blocking` (holding the owned gate guard) and emits `vault-auto-locked` even when it fails after locking. `Vault is locked` errors pass through `user_facing_vault_error` so an SFTP call after auto-lock says to unlock. Tests: `native_confirm::tests::*`, `ssh::host_key_approval_tests::only_changed_keys_carry_a_native_warning`, `local_paths::tests::only_picked_paths_are_accepted`, `grants_are_single_use_and_bound_to_their_intent`, `scheduled_task_path_exemption_requires_same_type`, `ssh::host_key_approval_tests::*`, `vault::ssh_keys::tests::test_lookup_error_fails_closed`, `test_rejected_port_isolation_and_old_fingerprint`, `scheduled_task_validation`, `monitoring_binding_respects_credential_host`; FE: `useSshHostKeyVerification.test.ts`, `SshHostKeyPrompt.test.tsx`, `SshFileManager.test.tsx`, `RemoteManager.test.tsx`.

18. **SSH and scheduler reliability (audit P7-4).** (a) **The interactive handshake has its own budget** (`ssh::INTERACTIVE_HANDSHAKE_TIMEOUT` = 135s, via `ssh_connect::connect_with_timeouts`), longer than the host-key prompt's `HOST_KEY_APPROVAL_TIMEOUT` (120s); DNS/TCP keep the short per-phase timeout. Before, the 10s phase timeout also covered the handshake, so any first connection whose key prompt took over ~10s to answer failed (RUST-001). Terminal and tunnel paths use it; exec/pool/SFTP never prompt. `ssh_test_server` (cfg(test)) is an in-process russh server that records auth attempts; use it for connect/auth tests. Test: `ssh_connect::tests::slow_host_key_approval_fits_the_handshake_budget`. (b) **`ssh_exec::run_command` reads until the channel closes, not until EOF**: OpenSSH sends exit-status after EOF, so failed scheduled commands and health probes were recorded as successes (RUST-009). A signal exit is an error. Once EOF and the exit status have arrived it waits at most 2s (`CLOSE_GRACE`) for the close, since some servers wait for the client to close first (test `server_that_never_closes_does_not_hit_the_timeout`). Output is capped at 1 MiB with an `[output truncated]` marker (RSEC-015). `ssh_test_server::Policy::exec` replies to exec requests in OpenSSH's order. Tests: `ssh_exec::tests::exit_status_after_eof_is_not_lost`, `output_is_capped`, `ssh_auth::tests::*` (TEST-004). (c) **The SSH pool never disconnects a leased session** (RUST-007): the idle/lifetime sweep skips sessions a `Lease` still holds, and replacing an expired one only drops it from the pool; the connection closes when the lease drops its handle. `SshConnectionPool<H = ExecClient>` is generic so tests can drive it with `ssh_test_server`. Test: `ssh_pool::tests::expired_sessions_are_not_closed_under_an_active_lease`. (d) **Tunnels end when their SSH session dies** (RUST-008): `run_tunnel_loop` checks `handle.is_closed()` every 5s and after a failed channel open, and tunnel sessions send keepalives (15s, 3 misses), so a dead tunnel frees its local port and leaves `list_ssh_tunnels` (the view polls it every 10s). `ssh_test_server::spawn_killable` returns a `Killer` that disconnects server sessions. A tunnel id that is already active is refused (replacing it used to let the old tunnel's cleanup delete the new one's entry). Tests: `ssh_tunnel::tests::tunnel_loop_ends_when_the_session_dies`, `a_tunnel_id_can_not_be_reused_while_active`. (e) **Scheduled runs never overlap and never block the tick** (RUST-005/006): `scheduler::InFlightGuard` (a process-wide set of running task ids) is claimed synchronously before a run is spawned and held until it ends, by cron runs and "Run now" alike (the latter fails with `ALREADY_RUNNING`, shown verbatim). `spawn_due_runs` starts due runs detached, with a `Semaphore` that persists across ticks (`SchedulerState`), so a stalled run no longer delays every other task. Scheduled SFTP transfers are bounded by `SFTP_TASK_TIMEOUT` (1h). Tests: `scheduler::tests::due_runs_are_detached_and_never_overlap`, `in_flight_claims_are_exclusive_until_dropped` (TEST-006). (f) **Non-interactive host-key checks share one decision**: `SshKeyManager::check_non_interactive` (used by `ExecClient` and `SftpClient`) accepts only a key stored as trusted for that host and port; unknown, changed, rejected and lookup errors refuse. Test: `vault::ssh_keys::tests::test_non_interactive_accepts_only_a_trusted_key_for_that_port` (TEST-003).
19. **Alerting works end to end (audit P7-5).** Alert rules are persisted in `alert_rules` (migration 015) and loaded by `AlertEngine::attach_store` at startup; before, they were in memory only and lost on every restart (RUST-002). `add_rule` validates (`monitoring::validate_rule`) and writes before changing memory. The `AlertRules` editor is mounted in `MonitoringView` and shows backend errors; it was never mounted, so no rule could be created (FE-003). Tests: `monitoring::tests::alert_rules_persist_across_restart`, `invalid_alert_rules_are_rejected`; FE: `AlertRules.test.tsx`, `MonitoringView.test.tsx`.

See `CODEBASE_AUDIT_REPORT.md` and `AGENTS.md` for the full audit findings and their fixes.

---

## CI/CD

`.github/workflows/ci.yml` runs on every push/PR to `master` (the repo's only branch — it
targeted `main`/`develop` until Aug 26, 2026, neither of which exist here, so CI had never
actually run on GitHub before that fix; see `AGENTS.md`):

1. **Workflow lint** (Ubuntu): `actionlint` (checksum-pinned 1.7.7) over `.github/workflows/*.yml`. GitHub silently rejects an invalid workflow file (it never runs, it just shows a 0-job failure), so this is the only thing that turns that red. **Never put the `secrets` context in a step `if:`**: evaluate it into a job-level `env` and test the env instead (see `release.yml` `HAS_WINDOWS_CERTIFICATE`). That mistake made `release.yml` invalid from Aug 26 to Sep 24, 2026 (CI-001): no release or updater artifact was ever built in that time.
2. **Frontend** (Ubuntu): `tsc --noEmit` + `npm run coverage` (the tests, failing below the coverage floor in `vite.config.ts`: statements 72 / branches 66 / functions 67 / lines 74; raise it as coverage grows) + `npm audit --audit-level=high`
3. **MSRV** (Ubuntu): `cargo +1.95 check --locked`, so Cargo.toml's `rust-version` stays true.
4. **Backend** (Ubuntu): `cargo clippy --all-targets -- -D warnings` + `cargo test` + `cargo audit` (accepted-risk advisories suppressed in `src-tauri/.cargo/audit.toml`, documented in `SECURITY.md`) + `cargo deny check` (`src-tauri/deny.toml`: license allowlist, crates.io-only sources, yanked crates denied, advisories; its `ignore` list must match `audit.toml` plus the transitive glib unsoundness RUSTSEC-2024-0429, DEP-006)
5. **Build matrix** (Windows, Ubuntu, macOS): `tauri build` with artifact upload

**Workflow hardening (CI-002/003):** both workflows default to a read-only `GITHUB_TOKEN` (`permissions: contents: read`; only the release job gets `contents: write`), check out with `persist-credentials: false`, and pin every action to a full commit SHA with the version in a trailing comment. When bumping an action, resolve the new tag's commit (`git ls-remote --tags https://github.com/<owner>/<repo>`; use the `^{}` line for annotated tags) and keep the comment in sync. Apple signing secrets are exported only on the macOS runner and only when set. **Release gate (CI-006/008):** the tag must be semver (`vX.Y.Z[-pre]`), `ci-checks` also runs `npm audit` and `cargo audit`, and release jobs use no Rust cache (no cache poisoning from `master` runs). `cargo-audit` is installed `--locked --version 0.22.2`. `.github/workflows/audit.yml` runs both audits weekly (Mondays 06:17 UTC) and on demand. CI cancels superseded runs (`concurrency`), every job has a `timeout-minutes`, and `.github/dependabot.yml` proposes weekly npm/cargo/actions updates, grouping the Tauri JS packages and the Tauri crates (merge the two Tauri PRs together).

`.github/workflows/release.yml` triggers on `v*` tags, builds all platforms, code-signs Windows/macOS installers and notarizes the macOS build, produces signed auto-updater artifacts (`latest.json` + `.sig` files), and creates a GitHub release with bundles attached. Signing/notarization/updater secrets are documented in `docs/RELEASE_SIGNING.md` — without them the build still succeeds but produces unsigned installers. Releases are created as **drafts** (`releaseDraft: true`): the updater endpoint (`releases/latest/download/latest.json`) serves nothing until a human publishes the draft. Generate updater keys outside the repo (`~/.tauri/`); `*.key` and `.claude/settings.local.json` are gitignored.

---

## Key Files for Common Tasks

| Task | File(s) |
|------|---------|
| Add a new Tauri command | `src-tauri/src/lib.rs` (register in `tauri::generate_handler!`) |
| Add credential/host logic | `src-tauri/src/vault/credentials.rs` or `lib.rs` |
| Add a new React view | `src/components/` + register in `App.tsx` sidebar |
| Add a database table/column | New migration in `src-tauri/migrations/` + register in `lib.rs` |
| Change alert/monitoring logic | `src-tauri/src/monitoring.rs` |
| Change SSH session handling | `src-tauri/src/ssh.rs` (interactive) or `ssh_exec.rs` (one-shot) |
| Change SSH connect/timeout/error-reporting | `src-tauri/src/ssh_connect.rs` — shared by `ssh.rs`, `sftp.rs`, `ssh_exec.rs`, `ssh_pool.rs`, `ssh_tunnel.rs`; reports which phase (DNS/TCP/handshake) failed instead of one opaque timeout |
| Change cron/scheduler logic | `src-tauri/src/scheduler.rs` |
| Change Tailscale peer listing/matching | `src-tauri/src/tailscale.rs` (backend), `src/hooks/useTailscaleStatus.ts` + `src/components/TailscalePeers.tsx` (frontend) |
| Change encryption | `src-tauri/src/crypto.rs` |
| Change input validation | `src-tauri/src/validation.rs` |
| Understand DB schema | `docs/SCHEMA.md` + `src-tauri/migrations/` |
| Understand user workflows | `docs/CORE_WORKFLOWS.md` |
| Configure release code signing / auto-updater secrets | `docs/RELEASE_SIGNING.md` |
| Report a vulnerability / see security scope | `SECURITY.md` |
| Review past bug fixes | `AGENTS.md` |

---

## Important Constraints

- **SFTP backend does not support SSH key auth** — only password credentials. Do not add `ssh_key` credential support to SFTP paths without implementing it in `src-tauri/src/sftp.rs` first.
- **Tauri API cannot be called in tests** — always mock via `vi.mocked(invoke)` in test files or `test-setup.ts`.
- **Do not use `unwrap()`/`expect()` in Rust production code** — return `Result` and propagate errors.
- **Migrations are append-only** — never modify existing migration files. Add a new numbered file instead.
- **All Tauri commands must be registered** in `tauri::generate_handler!([...])` inside `lib.rs` to be callable from the frontend.
- **The Vite port is fixed at 1420** — do not change it; Tauri's CSP and dev config depend on it.

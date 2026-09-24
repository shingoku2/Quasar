# CLAUDE.md — Quasar Codebase Guide

Quasar is a Tauri 2.x desktop application for remote infrastructure management: SSH sessions, SFTP, system monitoring, network discovery, cron-scheduled automation, and a security vault. React + TypeScript frontend, Rust backend, SQLite database.

## Current Work: Deferred Audit Fixes

Status as of September 17, 2026, verified in code on master (`31f6fa99`): four of the five
September 2 findings are implemented —

- cancel SSH attempts when their terminal closes while connecting — done
  (`SshState::pending_connections` cancellation registry, `ssh.rs`);
- queue concurrent SSH host-key prompts instead of replacing the active prompt — done
  (requestId-correlated `HostKeyApprovalState` + FIFO queue in
  `src/hooks/useSshHostKeyVerification.ts`);
- reject unrelated SQLite files during database import — done (Quasar `application_id`
  "QSR1" marker + strict legacy-signature fallback in `lib.rs`);
- remove remotely closed SSH sessions from the registry immediately — done (session
  driver removes the entry when the transport future completes).

Still open: **serialize credential operations with master-password rekeying.** The planned
credential-access gate (queueing credential operations behind rotation) was never
implemented — `add_credential`/`update_credential` don't check `changing_password`, so a
write landing during the re-encryption transaction or the post-commit key-swap window is
encrypted with the old key and orphaned under the new one. Rotation itself is hardened
(single-connection transactional re-encryption with AEAD probe, auto-lock suspension).
Finding-by-finding write-up with commit attribution: `AGENTS.md`, September 17, 2026
"Deferred Bug-Audit Fixes: Status Reconciliation". (`docs/DEFERRED_AUDIT_FIX_PLAN.md`
never landed on master — it exists only on the unmerged `copilot/vscode-mu4rlxtb-cd2d`
branch.)

---

## Recent Work: Google Jules MCP Connector (September 24, 2026)

`tools/jules-mcp/server.mjs` is a dependency-free MCP server (stdio, newline-delimited
JSON-RPC) that wraps the Jules REST API (`https://jules.googleapis.com/v1alpha`, auth via the
`X-Goog-Api-Key` header). It's registered project-wide in `.mcp.json` as `jules`, and the key
comes from `JULES_API_KEY` (`${JULES_API_KEY:-}` so a missing var doesn't break config
parsing; tool calls return a clear error instead). **Never commit the key.** Tools:
`jules_list_sources`, `jules_get_source`, `jules_create_session`, `jules_list_sessions`,
`jules_get_session`, `jules_approve_plan`, `jules_send_message`, `jules_list_activities`.
Session IDs and source names are normalized and regex-validated before they're put into
URL paths. Activity responses have base64 screenshots stripped and patches capped at 20k
chars each. The tests are `npm run test:jules-mcp` (`node:test`). The test file is named
`server.check.mjs` on purpose, because Vitest's default include would pick up `*.test.mjs`
and run it under jsdom. This is dev tooling only, so the Tauri app doesn't import it.

---

## Previous Work: PR Triage (September 24, 2026)

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
- **#62's IPC audit note recorded "None found"** while the credential-write/rekey race
  (see "Current Work" above) is still open. It now records that race as an open exception.

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
│   ├── migrations/             # 12 numbered SQL migration files
│   ├── tests/                  # Rust integration tests
│   ├── Cargo.toml
│   └── tauri.conf.json
├── conductor/                  # Internal docs & style guides
│   └── code_styleguides/       # typescript.md, html-css.md, general.md
├── docs/
│   ├── SCHEMA.md               # Database schema reference
│   └── CORE_WORKFLOWS.md       # End-to-end user workflows
├── scripts/tauri-dev.js        # Tauri dev wrapper (sets CARGO_TARGET_DIR)
├── tools/jules-mcp/            # Google Jules MCP connector for Claude Code (see .mcp.json)
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
| Desktop runtime | Tauri 2.x |
| Frontend framework | React 19 + TypeScript 5.8 |
| Build tool | Vite 7 |
| Styling | Tailwind CSS 4 (dark navy/cyan theme) |
| Icons | Lucide React |
| Charts | Recharts |
| Terminal | xterm.js 6 (`@xterm/xterm`, `@xterm/addon-fit`) |
| Network graph | vis-network + vis-data |
| Backend language | Rust (edition 2021), async via Tokio 1 |
| Database | SQLite (rusqlite bundled, migrations via rusqlite_migration) |
| SSH/SFTP | russh 0.62 (host key + auth bundled in), russh-sftp 2.4 |
| Encryption | aes-gcm 0.10 (AES-256-GCM), argon2 0.5 (Argon2id) |
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

# Jules MCP connector tests (node:test)
npm run test:jules-mcp

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
- `get_ssh_known_hosts()` / `verify_ssh_host_key(...)` / `remove_known_host(hostname)`

**SFTP** — password auth only (no SSH key support in backend). These take the plaintext password directly; there is no `credential_id` lookup (unlike SSH sessions).
- `sftp_upload_file(host, port, username, password, local_path, remote_path)`
- `sftp_download_file(host, port, username, password, remote_path, local_path)`
- `sftp_list_directory(host, port, username, password, remote_path)`
- `sftp_remote_exists(host, port, username, password, remote_path)`

**Monitoring & Alerts**
- `get_system_metrics()` / `get_remote_hosts_health()`
- `add_alert_rule(host_id, metric, threshold, ...)` / `update_alert_rule(id, ...)` / `remove_alert_rule(id)`
- `list_alert_rules()` / `get_alert_history()`
- `get_metrics_history(host?, limit?)` — returns stored `metrics_history` rows

**Network Discovery & Scanning**
- `start_network_scan(cidr, timeout?)` / `get_discovered_hosts()`
- `save_discovered_host(ip, hostname, port, username)` / `remove_discovered_host(ip)`

**Scheduled Tasks**
- `list_scheduled_tasks()` / `get_scheduled_task(id)`
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

Migrations run automatically on startup via `rusqlite_migration`. Migration files are in `src-tauri/migrations/` numbered `001` through `012`.

### Key Tables

| Table | Purpose |
|-------|---------|
| `hosts` | Saved remote servers |
| `credentials` | Encrypted passwords/SSH keys (AES-256-GCM) |
| `vault_settings` | Master password hash (Argon2id), auto-lock config |
| `ssh_known_hosts` | SSH host key fingerprints |
| `security_audit_log` | Vault and credential access events |
| `discovered_hosts`, `host_services` | Network scan results |
| `scheduled_tasks` | Cron automation tasks |
| `scheduled_task_runs` | Task execution history |
| `system_metrics_history` | Historical monitoring data |
| `alert_rules`, `alert_history` | Alert configuration and log |

See `docs/SCHEMA.md` for full column definitions.

### Adding Migrations

1. Create `src-tauri/migrations/013_description.sql`
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
- Global vault state via `VaultProvider` context (`src/components/vault/VaultProvider.tsx`)
- Wrap the app (or risky subtrees) in `ErrorBoundary`
- SSH credential selectors must filter out `ssh_key` type when the target is SFTP (backend only supports password auth for SFTP)

---

## Security Notes

1. **Vault must be unlocked** before any credential operations. The vault state is in-memory only — the master password hash is stored (Argon2id) but the derived key is not persisted.
2. **Credentials are encrypted** with AES-256-GCM. The key is derived from the master password per session.
3. **SFTP only supports password credentials.** The `CredentialSelector` component filters `ssh_key` type out when `filterType="password_only"` is set — always use this prop for SFTP contexts.
4. **Discovery is a singleton.** Calling `start_network_scan` when already running returns early; there is no duplicate-thread risk.
5. **Database export** uses `VACUUM INTO` (not file copy) to get a consistent snapshot while connections are live.
6. **Input validation** (`validation.rs`) must be called on all user-supplied network values before use in commands.
7. **Interactive SSH connect errors bypass `sanitize_error()` by design.** `connect_ssh` (the terminal path in `ssh.rs`) returns `ssh_connect::connect_with_diagnostics()` errors verbatim to the frontend — including the resolved IP/port and the phase that failed (DNS/TCP/handshake) — so users can self-diagnose (wrong port, firewalled host, dead DNS record). SFTP and pooled/scheduled-task SSH paths still route through `sanitize_error(e, "sftp"/"ssh")` at the `lib.rs` command boundary.
8. **All SQLite connections must go through `db::open_connection()`.** It sets the 5s busy-timeout, WAL journal mode, and the foreign-keys pragma. `HostTracker` and `MetricsStore` used to open raw `rusqlite::Connection`s directly and got none of these — under normal write contention (scheduler + monitoring + a network scan writing concurrently) they'd fail fast with `SQLITE_BUSY` instead of waiting, and the caller only logged it, so scanned hosts and monitoring samples were silently dropped. Fixed Aug 22, 2026 by routing both through `db::open_connection()`; don't reintroduce a raw `Connection::open()` call outside `db.rs`.
9. **`VaultState.changing_password` only gates `check_auto_lock()`, not `lock_vault()`.** `change_master_password` drops its write lock during the multi-second Argon2id + credential re-encryption pass (so other vault reads, e.g. an in-flight SSH credential lookup, aren't stalled for the whole operation) and only reacquires it briefly at the start and end. Because of that window, an explicit `lock_vault()` call can land mid-change — the reacquire-and-install step at the end checks `inner.master_key.is_some()` before installing the new key, so an explicit lock is respected instead of silently reverted. If you touch `change_master_password` again, preserve that check; see `.claude/agent-memory/security-reviewer/vault_rs_patterns.md` for the full locking model and a regression test at `vault::tests::test_lock_vault_during_password_change_stays_locked`.
10. **`unlock_vault`'s failed-attempt lockout (5 → 5 min, 10 → 15 min, 15 → 60 min) is persisted to `vault_settings`** (`lockout_failed_attempts` / `lockout_until_unix`), not just tracked in-memory. `VaultStateInner` is rebuilt fresh on every app launch, so an in-memory-only counter would let an attacker with local file access bypass the lockout by relaunching the app every 4 guesses. `load_persisted_lockout()` folds the persisted state into `inner` at the top of every `unlock_vault` call (only ever raising it, never lowering) and `persist_lockout()` writes it back on every failure and on success (clearing it). Regression tests: `vault::tests::test_lockout_persists_across_vault_state_restart`, `test_successful_unlock_clears_persisted_lockout`.
11. **`scan_network`'s claim (`is_scanning` check-and-set + `stop_signal` reset) must happen synchronously in the `scan_network` Tauri command, before `tokio::spawn`, via `scanner::claim_scan()` — never inside the spawned task.** The command spawns the actual scan and returns immediately; if the claim happened inside the spawned task instead, a `stop_scan()` call landing in the window between `tokio::spawn(...)` and the task actually starting would set `stop_signal = true`, only for the still-starting scan to unconditionally reset it back to `false` when its own claim ran — silently discarding the user's stop request. `scan_network()` itself now assumes the caller already claimed it: it installs `ScanRunningGuard` as its first action (before any fallible work, so the claim is always released even on early return) and checks `stop_signal` before opening the raw ICMP socket (so an already-stopped scan doesn't pay for, or need privileges for, a socket it won't use). Fixed Sep 17, 2026; regression tests: `scanner::tests::test_claim_scan_rejects_concurrent_claim`, `test_stop_request_after_claim_is_not_lost`. See `AGENTS.md` for the incident where a bot-pushed commit reverted this fix mid-review and it had to be reapplied on top of a master merge.
12. **`ssh_auth::authenticate` falls back to `none` authentication only when no password and no key material were supplied at all** — a present-but-wrong password or key still fails normally; this path exists specifically for identity-aware servers like Tailscale SSH, which authenticate the tailnet connection out-of-band and accept a bare `authenticate_none` request. Tailscale SSH "check mode" (browser re-verification over keyboard-interactive) is not implemented — it surfaces as a connection error in the terminal, same as any other rejected `none` auth attempt.
13. **Decrypted passwords only cross IPC through `reveal_credential_password`.** `get_credential`'s `CredentialFrontendView` has `has_password` instead of a `password` field (Rust regression test `vault::credentials::tests::test_frontend_view_carries_no_secret_material`). The frontend calls `reveal_credential_password` only on an explicit reveal/copy in the view dialog, or when starting an SFTP session (SFTP commands take the password directly). SSH sessions pass the credential ID and never need it. Until September 24, 2026 the view carried the plaintext, so opening any credential put it into React state.

See `CODEBASE_AUDIT_REPORT.md` and `AGENTS.md` for the full audit findings and their fixes.

---

## CI/CD

`.github/workflows/ci.yml` runs on every push/PR to `master` (the repo's only branch — it
targeted `main`/`develop` until Aug 26, 2026, neither of which exist here, so CI had never
actually run on GitHub before that fix; see `AGENTS.md`):

1. **Frontend** (Ubuntu): `tsc --noEmit` + `npm test` + `npm audit --audit-level=high`
2. **Backend** (Ubuntu): `cargo clippy -- -D warnings` + `cargo test` + `cargo audit` (accepted-risk advisories suppressed in `src-tauri/.cargo/audit.toml`, documented in `SECURITY.md`)
3. **Build matrix** (Windows, Ubuntu, macOS): `tauri build` with artifact upload

`.github/workflows/release.yml` triggers on `v*` tags, builds all platforms, code-signs Windows/macOS installers and notarizes the macOS build, produces signed auto-updater artifacts (`latest.json` + `.sig` files), and creates a GitHub release with bundles attached. Signing/notarization/updater secrets are documented in `docs/RELEASE_SIGNING.md` — without them the build still succeeds but produces unsigned installers.

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

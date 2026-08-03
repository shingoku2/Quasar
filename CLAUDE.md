# CLAUDE.md — Quasar Codebase Guide

Quasar is a Tauri 2.x desktop application for remote infrastructure management: SSH sessions, SFTP, system monitoring, network discovery, cron-scheduled automation, and a security vault. React + TypeScript frontend, Rust backend, SQLite database.

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
│   └── *.test.tsx              # 32 test files / 208 tests (co-located with sources)
├── src-tauri/                  # Rust/Tauri backend
│   ├── src/
│   │   ├── lib.rs              # App setup, migrations, ALL Tauri commands
│   │   ├── vault.rs            # Vault state + auto-lock
│   │   ├── vault/
│   │   │   ├── credentials.rs  # AES-256-GCM encryption, CRUD
│   │   │   ├── ssh_keys.rs     # SSH host key trust management
│   │   │   └── audit.rs        # Security event logging
│   │   ├── ssh.rs              # Interactive SSH sessions (Channel::wait())
│   │   ├── ssh_exec.rs         # One-shot SSH command execution
│   │   ├── ssh_tunnel.rs       # SSH port forwarding
│   │   ├── sftp.rs             # SFTP file transfer (password-auth only)
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
| SSH/SFTP | russh 0.57, russh-keys 0.49, russh-sftp 2.0 |
| Encryption | aes-gcm 0.10 (AES-256-GCM), argon2 0.5 (Argon2id) |
| Secure memory | zeroize 1.8, secrecy 0.8 |
| Network scan | surge-ping, cidr-utils, dns-lookup, mdns-sd |
| System info | sysinfo 0.33 |
| Cron | cron 0.12 |
| AI (optional) | ollama-rs 0.3 |
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
- `list_credentials()` / `get_credential(id)`

**Hosts**
- `add_host(hostname, port, username, protocol)` / `update_host(...)` / `remove_host(id)`
- `list_hosts()` / `get_saved_hosts()`

**SSH**
- `start_ssh_session(host_id, credential_id?, password?)` → returns `session_id`
- `write_ssh(session_id, data)` / `resize_ssh(session_id, rows, cols)` / `close_ssh_session(session_id)`
- `execute_ssh_command(host_id, command, credential_id?, password?)`
- `get_ssh_known_hosts()` / `verify_ssh_host_key(...)` / `remove_known_host(hostname)`

**SFTP** — password auth only (no SSH key support in backend)
- `sftp_upload_file(host_id, local_path, remote_path, credential_id?, password?)`
- `sftp_download_file(host_id, remote_path, local_path, credential_id?, password?)`
- `sftp_list_directory(host_id, path, credential_id?, password?)`
- `sftp_remote_exists(host_id, path, credential_id?, password?)`

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

#### Test file inventory (37 files / 243 tests as of August 2026)

| Test file | Component tested | Key scenarios |
|-----------|-----------------|---------------|
| `App.test.tsx` | App root | Routing, sidebar navigation |
| `Layout.test.tsx` | Layout | Render, vault integration |
| `ErrorBoundary.test.tsx` | ErrorBoundary | Error catching, crash UI, reload |
| `TopBar.test.tsx` | TopBar | View label mapping for all 7 views |
| `HostManagement.test.tsx` | HostList, AddHostDialog | List, filter, connect/SFTP callbacks, remove, duplicates, empty state |
| `NetworkScanner.test.tsx` | NetworkScanner | CIDR input, scan start/stop, CIDR validation error, scan failure, initialResults |
| `SshFileManager.test.tsx` | SshFileManager | Path bar, loading, error, file listing |
| `HealthCheckBadge.test.tsx` | HealthCheckBadge | Online/offline/checking states |
| `PreflightDialog.test.tsx` | PreflightDialog | Health check flow |
| `AIAssistant.test.tsx` | AIAssistant | Chat, streaming, error |
| `vault/VaultProvider.test.tsx` | VaultProvider | Loading, init, unlock, error states |
| `vault/VaultInitDialog.test.tsx` | VaultInitDialog | Password validation (all rules), strength indicator, invoke, error |
| `vault/VaultUnlockDialog.test.tsx` | VaultUnlockDialog | Empty password, success, error, cancel, password toggle |
| `vault/CredentialSelector.test.tsx` | CredentialSelector | Loading, type/host filter (incl. SFTP ssh_key exclusion), search, select, manual entry, error |
| `vault/CredentialManager.test.tsx` | CredentialManager | List, empty, loading, add/view/edit/delete dialogs, search, confirm flows, error |
| `vault/KnownHostsManager.test.tsx` | KnownHostsManager | List, search by host/fingerprint, remove with confirm, trust updates, error |
| `vault/AuditLogViewer.test.tsx` | AuditLogViewer | List, search, event type filter, result filter, resource info, error |
| `vault/SshHostKeyPrompt.test.tsx` | SshHostKeyPrompt | New vs changed key, trust/reject, permanent toggle, copy fingerprint, MITM warning |
| `vault/VaultSettings.test.tsx` | VaultSettings | Load/save/error, auto-lock, lock vault, change password validation |
| `dashboard/AlertFeed.test.tsx` | AlertFeed | Empty state, alert render, dismiss, acknowledge, clear all, metrics summary |
| `dashboard/QuickConnectWidget.test.tsx` | QuickConnectWidget | Loading, host list, search, connect callback, overflow indicator |
| `dashboard/AlertRules.test.tsx` | AlertRules | Rule CRUD |
| `dashboard/MetricChartCard.test.tsx` | MetricChartCard | Rendering |
| `dashboard/SystemHealthWidget.test.tsx` | SystemHealthWidget | Metrics display |

#### Key testing patterns

- Components with `export default` (most vault components, ErrorBoundary, TopBar, AlertFeed, QuickConnectWidget) are imported with default import syntax: `import VaultSettings from './VaultSettings'`
- `window.confirm` must be stubbed via `vi.spyOn(window, 'confirm').mockReturnValue(true/false)` (not `vi.stubGlobal`) — restore with `spy.mockRestore()` after each test
- Selects without `aria-label`/`id` (e.g. AuditLogViewer filters) are queried by index: `screen.getAllByRole('combobox')[0]`
- When multiple elements match the same text (heading + button both say "Unlock Vault"), use `getAllByText(...)` or role-scoped queries like `getByRole('button', { name: ... })`
- Saved-host components mock `invoke` commands such as `get_saved_hosts`, `upsert_saved_host`, and `remove_saved_hosts`; frontend tests never open SQLite directly

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

See `CODEBASE_AUDIT_REPORT.md` and `AGENTS.md` for the full audit findings and their fixes.

---

## CI/CD

`.github/workflows/ci.yml` runs on every push/PR to `main` or `develop`:

1. **Frontend** (Ubuntu): `tsc --noEmit` + `npm test`
2. **Backend** (Ubuntu): `cargo clippy -- -D warnings` + `cargo test`
3. **Build matrix** (Windows, Ubuntu, macOS): `tauri build` with artifact upload

`.github/workflows/release.yml` triggers on `v*` tags, builds all platforms, and creates a GitHub release with bundles attached.

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
| Change cron/scheduler logic | `src-tauri/src/scheduler.rs` |
| Change encryption | `src-tauri/src/crypto.rs` |
| Change input validation | `src-tauri/src/validation.rs` |
| Understand DB schema | `docs/SCHEMA.md` + `src-tauri/migrations/` |
| Understand user workflows | `docs/CORE_WORKFLOWS.md` |
| Review past bug fixes | `AGENTS.md` |

---

## Important Constraints

- **SFTP backend does not support SSH key auth** — only password credentials. Do not add `ssh_key` credential support to SFTP paths without implementing it in `src-tauri/src/sftp.rs` first.
- **Tauri API cannot be called in tests** — always mock via `vi.mocked(invoke)` in test files or `test-setup.ts`.
- **Do not use `unwrap()`/`expect()` in Rust production code** — return `Result` and propagate errors.
- **Migrations are append-only** — never modify existing migration files. Add a new numbered file instead.
- **All Tauri commands must be registered** in `tauri::generate_handler!([...])` inside `lib.rs` to be callable from the frontend.
- **The Vite port is fixed at 1420** — do not change it; Tauri's CSP and dev config depend on it.

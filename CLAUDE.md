# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Run the full app (Vite dev server + Tauri/Rust backend)
npm run tauri dev

# Build for production (outputs to src-tauri/target/release/bundle/)
npm run tauri build

# Run frontend tests (Vitest)
npm test

# Run a single frontend test file
npx vitest run src/components/TerminalComponent.test.tsx

# Run Rust backend tests
cd src-tauri && cargo test

# Run a specific Rust test module
cd src-tauri && cargo test scheduler::

# Frontend-only Vite dev server (no Tauri)
npm run dev
```

## Architecture

Quasar is a **Tauri v2** desktop app: React/TypeScript frontend compiled by Vite, communicating with a Rust backend via `invoke()` calls.

### Frontend (`src/`)

- **Entry**: `main.tsx` → `App.tsx` → `Layout.tsx`
- **Navigation**: `Layout.tsx` renders all view components simultaneously and toggles visibility with `block`/`hidden` CSS classes. Views are never unmounted, so SSH sessions persist when navigating away.
- **Views**: Dashboard, RemoteManager (SSH/SFTP sessions), AIAssistant, MonitoringView, ScheduledTasksView, SecurityView, SettingsView — all wired in `Layout.tsx`.
- **Vault state**: `src/components/vault/VaultProvider.tsx` wraps the app and exposes vault/credential state via React Context. Always check vault locked state before accessing credentials.
- **Database (frontend)**: `src/db.ts` uses `@tauri-apps/plugin-sql` for the `hosts` table only. All other data goes through Tauri `invoke()` commands.
- **Terminal**: `src/components/TerminalComponent.tsx` uses xterm.js. Terminal theme is driven by the `theme` prop only — it does **not** sync with the app theme. In-place theme/font updates are applied without reconnecting the SSH session.
- **Theming**: Dark/light themes use CSS custom properties. Light mode is implemented via `[data-theme="light"]` overrides in `App.css` that remap Tailwind text classes (`text-white`, `text-gray-*`, etc.) to dark-on-light values.

### Backend (`src-tauri/src/`)

All Tauri commands are defined in `lib.rs` and registered in the `tauri::Builder`. Modules:

| Module | Purpose |
|--------|---------|
| `vault/` | Encrypted credential store (AES-256-GCM, Argon2id). `VaultState` is the central Tauri-managed state. |
| `ssh.rs`, `ssh_auth.rs`, `ssh_exec.rs` | SSH connection, authentication, and command execution (russh) |
| `sftp.rs` | SFTP file transfer (russh-sftp) |
| `scheduler.rs` | Cron task runner; polls every 60 s; supports SSH command, SFTP upload, SFTP download |
| `monitoring.rs` | Real-time host metrics (CPU, memory, disk) |
| `scanner.rs`, `discovery.rs`, `host_tracker.rs` | Network CIDR scanning and device discovery |
| `health.rs` | Pre-flight ping + SSH validation |
| `db.rs` | `open_connection()` helper — always enables `PRAGMA foreign_keys = ON` |
| `crypto.rs` | AES-256-GCM encrypt/decrypt primitives |
| `validation.rs` | Input validation (IPs, hostnames, CIDR, credentials) |
| `errors.rs` | `sanitize_error()` strips internal detail from user-facing errors |
| `ai.rs`, `launcher.rs` | AI assistant (ollama-rs) and process launchers |

### Database

- SQLite at `quasar.db` (in Tauri app data dir at runtime, or project root during tests).
- **Migrations**: numbered SQL files in `src-tauri/migrations/` (001–012), applied by `rusqlite-migration` on startup in `lib.rs`. To add a migration, create the next numbered `.sql` file and register it in the `Migrations::from_iter([...])` call in `lib.rs`.
- Schema reference: `docs/SCHEMA.md`.

### Vault Security Model

- Master password → Argon2id (47 MiB, 2 iterations) → 32-byte master key held in `VaultState` (in-memory only, zeroized on drop/lock).
- All credentials encrypted at rest with AES-256-GCM using the master key.
- Auto-lock after configurable idle timeout. Lockout after failed attempts (5/10/15 thresholds).
- Changing password re-encrypts all credentials in a single transaction with validation before commit.

### Scheduled Tasks (cron)

- Cron expressions are **6-field** format: `sec min hour day month dow` (e.g., `0 0 9 * * *` = 9:00 AM daily).
- Task types: `ssh_command`, `sftp_upload`, `sftp_download`. File transfer tasks require `local_path` and `remote_path`.
- Scheduler runs in a background `tokio` task; checks every 60 s; uses an in-memory cooldown map to prevent double-firing.

## Testing

**Frontend**: Vitest + React Testing Library. All Tauri APIs are globally mocked in `src/test-setup.ts` (covers `@tauri-apps/api/core`, `event`, `plugin-dialog`, `plugin-sql`). Add per-test `vi.mock()` calls for component-specific behavior.

**Backend**: Standard `cargo test`. Tests use unique timestamped temp `.db` files and clean them up in a `cleanup_test_db()` helper pattern. The scheduler has integration tests: run them with `cargo test scheduler::`.

## Code Style

Follows Google TypeScript Style Guide (`conductor/code_styleguides/typescript.md`):
- **Named exports only** — no default exports.
- `const`/`let` only; no `var`.
- Single quotes for strings; template literals for interpolation.
- 2-space indentation; explicit semicolons.
- Avoid `any`; avoid type assertions (`as`/`!`) without justification.
- `UpperCamelCase` for types/components; `lowerCamelCase` for variables/functions.
- No `_` prefix/suffix on identifiers.

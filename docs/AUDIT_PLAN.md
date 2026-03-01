# Quasar — Complete Code Audit Plan

**Date:** 2026-02-28
**Scope:** Full codebase — 10,818 frontend LOC (62 TS/TSX files) + 8,806 backend LOC (24 Rust files) + 12 migrations + CI/CD
**Branch:** `claude/add-claude-documentation-Fy6A5`

---

## Executive Summary of Findings

Pre-audit reconnaissance identified the following category totals requiring action:

| Category | Critical | High | Medium | Low |
|----------|----------|------|--------|-----|
| Security / Config | 3 | 4 | 3 | 2 |
| Rust Backend | 0 | 2 | 6 | 5 |
| Frontend TS/React | 0 | 6 | 10 | 8 |
| Accessibility | 0 | 4 | 8 | 6 |
| Database | 0 | 1 | 3 | 2 |
| CI/CD | 0 | 2 | 3 | 2 |

**Total: ~3 Critical, ~19 High, ~33 Medium, ~25 Low**

---

## Phase 1 — Security & Configuration Audit

**Priority: P0 — Do these first.**

### 1.1 `src-tauri/tauri.conf.json`

| Finding | Severity | Action |
|---------|----------|--------|
| `"pubkey": ""` — updater accepts unsigned binaries | **Critical** | Generate ed25519 keypair; populate `pubkey` field; note if not using updater, disable the plugin entirely |
| Updater `endpoints` URL contains `user` placeholder | **Critical** | Replace with real repo URL or remove updater block |
| CSP: `style-src 'unsafe-inline'` allows style injection | High | Use hash or nonce; or remove inline styles in favour of CSS classes |
| CSP: `img-src data: https:` — `data:` allows base64 XSS vectors | High | Restrict to `img-src 'self' asset: data:` (narrow to Tauri asset protocol only) |
| Main window minimum size 800×600 — unusable on smaller displays | Low | Add `minWidth`/`minHeight` |

### 1.2 Vault & Cryptography (`src-tauri/src/vault.rs`, `crypto.rs`, `vault/credentials.rs`)

| Finding | Severity | Action |
|---------|----------|--------|
| Verify Argon2id params match OWASP M=47104 KB, t=2, p=1 | Verify | Read and confirm current constants against OWASP ASVS 2.4.4 |
| Binary salt `decode_b64()` fix applied — confirm no UTF-8 path remains | Verify | Search all salt-related code for `.as_bytes()` on base64-decoded strings |
| `change_master_password` re-encryption uses transaction — verify rollback on partial failure | Verify | Trace the full transaction path in `vault.rs` |
| Credential fields (nonce/tag) stored as base64 TEXT — verify length validation on decode | Medium | Add length guard before decrypt to prevent panic on malformed DB rows |

### 1.3 Input Validation (`src-tauri/src/validation.rs`)

| Finding | Severity | Action |
|---------|----------|--------|
| IPv4 regex allows leading zeros (`001.002.003.004` is technically octal) | Medium | Tighten regex to `(25[0-5]\|2[0-4]\d\|1?\d?\d)` for each octet |
| Hostname regex — verify max label length 63 chars enforced | Medium | Check regex against RFC 1123 |
| No validation on SSH tunnel `remote_addr` parameter in `lib.rs` | High | Add `validate_hostname` or `validate_ip` call before `start_tunnel` |
| No validation on `remote_path` / `local_path` for SFTP/scheduler | Medium | Add path sanitisation (no null bytes, no `..` traversal component) |

### 1.4 SQL Safety (all DB-touching modules)

| Finding | Severity | Action |
|---------|----------|--------|
| Verify every `execute()` / `query()` uses `?` placeholders, not string format | Verify | Grep for `format!(` inside SQL strings across all Rust files |
| `audit.rs` dynamic query built with `push_str` — verify all values bound, never interpolated | Verify | Read query builder carefully |
| No SQL injection vectors found in prior analysis, but confirm | Confirm | `grep -n "format!.*SELECT\|UPDATE\|INSERT\|DELETE"` across src-tauri |

### 1.5 Error Information Disclosure (`src-tauri/src/errors.rs`)

| Finding | Severity | Action |
|---------|----------|--------|
| `sanitize_error()` pattern-matches on prefix strings — verify coverage is complete | Verify | Check that every command in `lib.rs` routes errors through `sanitize_error()` |
| Some `lib.rs` commands use `.map_err(\|e\| e.to_string())` directly — no sanitisation | High | Audit every `#[command]` return for direct `e.to_string()` leaking internal details |

### 1.6 Clipboard Exposure (`src/components/vault/CredentialManager.tsx`)

| Finding | Severity | Action |
|---------|----------|--------|
| `navigator.clipboard.writeText(credential.password)` — clipboard persists after app close | High | Add auto-clear after 30 seconds with `setTimeout(() => navigator.clipboard.writeText(''), 30000)` |

---

## Phase 2 — Rust Backend Code Quality

### 2.1 `src-tauri/src/lib.rs` (1,361 lines)

| Finding | Severity | Action |
|---------|----------|--------|
| `lib.rs` contains all 70+ command *implementations* inline — makes file 1,361 lines | Medium | Commands that are already thin wrappers around modules are fine; audit for commands with >20 lines of logic that belong in their own module |
| `greet()` test command still registered in production handler | Low | Remove or gate behind `#[cfg(debug_assertions)]` |
| SSH timeout event (`ssh_timeout_*`) emitted but no corresponding frontend listener found | Medium | Verify frontend handles this event or remove dead code |
| Direct `e.to_string()` in some `#[command]` returns without sanitisation | High | Replace with `sanitize_error(&e.to_string())` calls |

### 2.2 `src-tauri/src/scheduler.rs` (797 lines)

| Finding | Severity | Action |
|---------|----------|--------|
| Cron schedule assumes UTC — no timezone field in `scheduled_tasks` table | Medium | Document UTC assumption in UI; or add `timezone` column (migration 013) |
| 60-second polling interval means tasks can fire up to 60 s late | Low | Document acceptable drift; or make interval configurable |
| Output truncated to 4 KB silently — user may lose diagnostic output | Low | Log a warning when truncation occurs; show truncation indicator in UI |
| Credential lookup from vault fails silently if vault locked — task marked failed but no user alert | Medium | Emit event or store error noting "vault was locked" specifically |

### 2.3 `src-tauri/src/ssh.rs` (400 lines)

| Finding | Severity | Action |
|---------|----------|--------|
| VecDeque pending ops has no capacity cap — adversarial rapid writes could exhaust memory | Low | Add `max_capacity` guard (e.g. 1000 ops) |
| No session-level activity timeout after connection — idle sessions live forever | Medium | Add inactivity timeout tracked in session state, configurable via settings |
| `write_ssh` / `resize_ssh` error: JSON-stringified error matched against string literal | Medium | Use typed error matching instead of `JSON.stringify(e).includes(...)` on frontend |

### 2.4 `src-tauri/src/discovery.rs` (96 lines)

| Finding | Severity | Action |
|---------|----------|--------|
| Background thread has no clean shutdown path — kills only on process exit | Medium | Add a second `AtomicBool` stop signal; `start_discovery` sets it; thread checks it each iteration |
| `ServiceDaemon::new()` failure returns silently — user unaware discovery is broken | Medium | Return error to frontend instead of silent early return |

### 2.5 `src-tauri/src/monitoring.rs` (1,036 lines)

| Finding | Severity | Action |
|---------|----------|--------|
| `.unwrap_or_else(\|e\| e.into_inner())` on poisoned mutex — verify all locks have this pattern | Verify | Grep for `.lock().unwrap()` without poison recovery |
| Per-core CPU tracking allocates vector every poll cycle | Low | Pre-allocate or cache core count |

### 2.6 `src-tauri/src/ssh_exec.rs` (361 lines)

| Finding | Severity | Action |
|---------|----------|--------|
| System metrics commands hardcoded for Linux (`top`, `free`, `/proc/*`) — silently broken on macOS/BSD | Medium | Add platform detection or document "Linux remote targets only"; return error on non-Linux |
| `execute_ssh_commands_batch` divides timeout by count — each command gets `max(total/n, 5s)` | Verify | Confirm behaviour is documented and acceptable |

### 2.7 `src-tauri/src/db.rs`

| Finding | Severity | Action |
|---------|----------|--------|
| No connection pooling — each operation opens a new SQLite connection | Medium | Consider `r2d2` + `r2d2_sqlite`; or at minimum cache connection in AppState |

### 2.8 Cross-cutting: `unwrap` / `expect` audit

| Finding | Severity | Action |
|---------|----------|--------|
| Run `grep -n "\.unwrap()\|\.expect(" src-tauri/src/*.rs src-tauri/src/vault/*.rs` and verify all occurrences are in `#[cfg(test)]` blocks | High | Any production `unwrap`/`expect` outside tests must be converted to `?` or handled |

---

## Phase 3 — Frontend TypeScript / React Quality

### 3.1 Type Safety

| File | Finding | Severity | Action |
|------|---------|----------|--------|
| `DashboardView.tsx:89` | `SavedHost.id: number` but string used everywhere else | High | Align to `string`; update interface and all usages |
| `AlertRules.tsx:82` | `convertToBackendRule()` returns `any` | High | Define proper discriminated union type for backend metric enum |
| `AlertRules.tsx:67` | `Object.keys(rule.metric)[0]` to extract enum variant | High | Use exhaustive type guard or discriminated union switch |
| `CredentialManager.tsx:294-296` | `as { key_path?: string }` type assertion | Medium | Define `SshKeyCredential` and `PasswordCredential` union type; use type narrowing |
| `TerminalComponent.tsx:142` | Payload cast `<{bandwidth: string, latency: number}>` without validation | Medium | Add runtime validation of event payload shape |
| `useSshHostKeyVerification.ts:42` | `keyBytes: number[]` — not clear how this serialises | Medium | Verify JSON serialisation of byte arrays between Rust `Vec<u8>` and TS `number[]` |

### 3.2 React Hook Correctness

| File | Finding | Severity | Action |
|------|---------|----------|--------|
| `VaultProvider.tsx:99,119` | `unlisten` stored in closure variable, not ref; race on unmount | High | Store `unlistenRef = useRef<(() => void) \| null>(null)`; call in cleanup |
| `MonitoringView.tsx:109-119` | `unlistenPromiseRef.current?.then(fn => fn())` — if promise rejected, cleanup silently skipped | High | `await unlistenRef.current?.()` in cleanup with try/catch |
| `MonitoringView.tsx:155` | `setInterval(fetchRemoteHealth, 30000)` — interval ID not stored in some code paths | High | Always store `intervalRef = useRef<number>`; clear in cleanup |
| `TerminalComponent.tsx:221` | `useEffect` dep array includes `password` — changing props reconnects live session | High | Gate reconnect on `sessionId` only; separate credential effect |
| `SettingsView.tsx:576-580` | Module-level code runs on import, not in component | Medium | Move to `useEffect` in root component or `main.tsx` |
| `SshTunnelsView.tsx:47` | `setInterval(loadTunnels, 2000)` — agressive 2s polling, never paused | Medium | Use 10s interval; only poll when view is visible (`useVisibilityChange`) |
| `NetworkScanner.tsx:107` | `onHostFound` in dep array — reinstalls listener mid-scan if callback reference changes | Medium | Wrap callback in `useRef`; don't include in dep array |
| `AlertFeed.tsx` | Event listener cleanup promises not awaited | Medium | Follow same `unlistenRef` pattern as VaultProvider fix |

### 3.3 Missing Input Validation

| File / Location | Finding | Severity | Action |
|-----------------|---------|----------|--------|
| `NetworkScanner.tsx:204` | CIDR input passed raw to backend with no client-side validation | High | Add regex/format check before invoking `scan_network`; show inline error |
| `ScheduledTasksView.tsx:289-295` | Cron expression accepted as free-form string | High | Integrate lightweight cron-parser validation (or regex for 6-field format) |
| `SshTunnelsView.tsx:127,158,172` | Port inputs: `parseInt() \|\| fallback` silently accepts NaN | Medium | Use `Number.isInteger(v) && v >= 1 && v <= 65535` guard; show error |
| `CredentialManager.tsx:467` | Port field: `parseInt(e.target.value) \|\| 22` silently coerces NaN | Medium | Same integer range guard |
| `AlertRules.tsx:260` | Threshold input `type="number"` with no min/max | Low | Add `min="0" max="100"` for percentage metrics; validate on submit |
| `ScheduledTasksView.tsx:101` | Default host_id `hosts[0]?.id ?? ''` — empty string passes validation | Medium | Treat empty string as invalid; show "No hosts configured" state |

### 3.4 Error Handling

| Finding | Severity | Action |
|---------|----------|--------|
| Multiple components use `alert()` for errors | Medium | Replace with inline error `<div role="alert">` or a toast notification system |
| `DashboardView.tsx:62-74` `load()` has no try-catch — errors silently ignored | Medium | Wrap in try-catch; show error state in UI |
| `RemoteManager.tsx:142` uses `alert()` for RDP error | Medium | Same — use proper error component |
| `SshFileManager.tsx:56` treats empty string `""` as no password (falsy) | Medium | Change guard to `if (password === null \|\| password === undefined) return` |

### 3.5 Miscellaneous Frontend Issues

| File | Finding | Severity | Action |
|------|---------|----------|--------|
| `src/db.ts` | Creates `hosts` table client-side — duplicates backend migration 001 | Medium | Remove `initDatabase()` call from frontend; let backend own all schema |
| `index.html` | `<title>Tauri + React + Typescript</title>` | Low | Change to `<title>Quasar</title>` |
| `RemoteManager.tsx:223` | `processedQuickConnects.delete` deferred — 60ms race window | Medium | Delete immediately on dedup check, not after timeout |
| `CredentialManager.tsx:298,440` | Single `showPassword` boolean controls multiple inputs | High | Use `showPasswordFor: string \| null` (keyed by field name) |
| `ScheduledTasksView.tsx:207` | `new Date(ts * 1000)` — comment if ts is Unix seconds not ms | Low | Add assertion or document expected unit |

---

## Phase 4 — Accessibility (WCAG 2.1 AA)

These apply across multiple components. Tackle by component type:

### 4.1 Modal Dialogs
**Files:** `VaultInitDialog`, `VaultUnlockDialog`, `AddHostDialog`, `HostDetailDialog`, `CredentialManager` (view/edit/add dialogs), `PreflightDialog`, `SshHostKeyPrompt`

| Action | WCAG |
|--------|------|
| Add `role="dialog"` to dialog container | 4.1.2 |
| Add `aria-modal="true"` | 4.1.2 |
| Add `aria-labelledby` pointing to dialog title `id` | 1.3.1 |
| Trap focus within dialog while open (Tab cycles inside) | 2.1.2 |
| Return focus to trigger element on close | 2.4.3 |

### 4.2 Form Labels
**Files:** All forms in `ScheduledTasksView`, `AlertRules`, `SshTunnelsView`, `CredentialManager`, `AddHostDialog`

| Action | WCAG |
|--------|------|
| Every `<input>` / `<select>` must have `<label htmlFor>` or `aria-label` | 1.3.1, 4.1.2 |
| Error messages must use `aria-describedby` pointing to error element id | 1.3.1 |

### 4.3 Custom Interactive Controls
**Files:** `AlertRules.tsx` toggle div, `NetworkScanner.tsx` list items, `NetworkTopologyView`, quick-connect items

| Control | Action | WCAG |
|---------|--------|------|
| `<div onClick>` toggle in AlertRules | Replace with `<button role="switch" aria-checked={enabled}>` | 4.1.2, 2.1.1 |
| Clickable scan result rows | Add `role="button" tabIndex={0} onKeyDown` | 2.1.1 |
| Icon-only buttons (edit/delete/view) | Add `aria-label="Edit credential"` etc. | 1.1.1 |

### 4.4 Live Regions
**Files:** `AlertFeed`, `MonitoringView`, `VaultProvider` error display, all error state renders

| Action | WCAG |
|--------|------|
| Error messages: wrap in `<div role="alert" aria-live="assertive">` | 4.1.3 |
| Status updates (scan progress, task result): `<div aria-live="polite">` | 4.1.3 |

### 4.5 Terminal Accessibility
**File:** `TerminalComponent.tsx`

| Action | WCAG |
|--------|------|
| Add `role="application"` + `aria-label="SSH Terminal"` to container | 4.1.2 |
| Provide keyboard shortcut documentation visible to screen readers | 2.4.1 |

---

## Phase 5 — Database & Migration Integrity

### 5.1 Migration Numbering Gap

| Finding | Severity | Action |
|---------|----------|--------|
| Migration `002` is absent (jump from 001 → 003) | Medium | Document why 002 was skipped in `docs/SCHEMA.md`; add comment in `lib.rs` migration list |

### 5.2 Frontend `db.ts` Conflict

| Finding | Severity | Action |
|---------|----------|--------|
| `src/db.ts` calls `CREATE TABLE IF NOT EXISTS hosts` — potentially races with backend migration on first launch | High | Remove the `execute()` call from `initDatabase()`; the function can remain for DB connection if used, but schema must come from Rust migrations only |

### 5.3 Missing Indexes

Audit the migrations for indexes on frequently queried columns:

| Table | Column(s) | Used in |
|-------|-----------|---------|
| `security_audit_log` | `created_at`, `event_type` | `audit.rs:get_audit_logs()` filters and sorts |
| `credentials` | `name`, `credential_type` | `search_credentials()`, `list_credentials()` |
| `discovered_hosts` | `ip_address` | `host_tracker.rs` upsert lookups |
| `scheduled_tasks` | `enabled`, `next_run_at` | `scheduler.rs` due-task query |

Action: Add migration `013_indexes.sql` if any of these are missing.

### 5.4 Schema / Type Alignment

| Finding | Severity | Action |
|---------|----------|--------|
| `SavedHost.id` typed as `number` in `DashboardView.tsx` but as `string` elsewhere | High | Verify actual SQLite type (TEXT vs INTEGER); align all TS interfaces |
| `scheduled_tasks.credential_id` — nullable TEXT in SQL; TS sends `string \| null` — verify backend handles empty string correctly | Medium | Confirm backend treats `""` same as `null` or enforce `null` on frontend |
| `alert_rules` metric stored as JSON — verify deserialization is exhaustive | Medium | Check all metric variant names match between Rust enum and TS interface |

### 5.5 Transaction Safety Review

| Finding | Severity | Action |
|---------|----------|--------|
| `change_master_password` re-encrypts all credentials in a transaction — verify entire operation is atomic | Verify | Trace `vault.rs` transaction scope |
| `scheduler.rs:set_run_result` — multiple UPDATE statements, not wrapped in transaction | Low | Wrap in transaction for atomicity |

---

## Phase 6 — IPC Layer & API Contract Review

### 6.1 Command Parameter Naming Inconsistency

The Tauri `invoke()` call serialises camelCase keys from TS to snake_case in Rust (Tauri does this automatically via serde). Verify the following are consistent:

| Command | TS Parameter | Rust Parameter | Status |
|---------|-------------|----------------|--------|
| `add_scheduled_task` | `cronExpression` | `cron_expression` | Verify Tauri's rename attribute |
| `update_credential` | `credentialId` | `credential_id` | Verify |
| `add_alert_rule` | All camelCase params | All snake_case | Verify |

Action: Run a systematic grep for all `invoke(` calls in frontend; compare against `#[command]` signatures.

### 6.2 Event Name Consistency

| Event (Rust `emit`) | Event (TS `listen`) | Verified? |
|--------------------|---------------------|-----------|
| `ssh_output` | `ssh_output` | Check |
| `ssh_closed` | `ssh_closed` | Check |
| `system-metrics` (hyphen) | `system-metrics` (hyphen) | Check |
| `vault-auto-locked` (hyphen) | `vault-auto-locked` | Check |
| `ssh_timeout_{session_id}` | Not found in any listener | Dead code? |
| `alerts-triggered` (hyphen) | `alerts-triggered` | Check |

### 6.3 Missing Frontend Commands

| Tauri command registered | Used in frontend? |
|--------------------------|-------------------|
| `get_app_info` | Verify |
| `clear_metrics_data` | Verify |
| `export_database` / `import_database` | In VaultSettings? Verify |
| `get_metrics_history` | Verify (different from get_system_metrics) |

---

## Phase 7 — CI/CD & Infrastructure

### 7.1 `ci.yml`

| Finding | Severity | Action |
|---------|----------|--------|
| No `cargo audit` step — Rust CVE tracking absent | High | Add `cargo install cargo-audit && cargo audit` after `cargo clippy` |
| No `npm audit` step | High | Add `npm audit --audit-level=high` after `npm test` |
| No test coverage reporting | Medium | Add `npm run test -- --coverage` and upload to codecov or similar |
| No ESLint step (no ESLint config exists) | Medium | Add ESLint config (`eslint.config.js`) + `npm run lint` step |
| TypeScript check has no timeout | Low | Add `timeout-minutes: 10` to job |

### 7.2 `release.yml`

| Finding | Severity | Action |
|---------|----------|--------|
| `releaseDraft: true` — releases never auto-published | High | Change to `releaseDraft: false` or document manual publish step |
| No code signing for Windows/macOS | High | Set up GitHub secrets for signing certificates |
| Version tag format not validated | Medium | Add step: `echo $GITHUB_REF_NAME \| grep -E '^v[0-9]+\.[0-9]+\.[0-9]+'` |

### 7.3 Missing CI Checks

| Check | Action |
|-------|--------|
| No `cargo fmt --check` | Add to backend-checks job |
| No Rust Clippy for `tests/` directory | Ensure `--all-targets` flag used |
| Build matrix doesn't test Tauri build on PR — only on main/develop | Consider adding build check on PRs at least for one platform |

---

## Phase 8 — Code Cleanup & Documentation

### 8.1 Dead Code

| Location | Finding | Action |
|---------|---------|--------|
| `lib.rs:greet()` | Debug test function registered in production | Remove or gate with `#[cfg(debug_assertions)]` |
| `src-tauri/src/launcher.rs` | `launch_rdp()` does nothing on non-Windows | Add compile-time warning or `#[cfg(target_os = "windows")]` |
| `RemoteManager.tsx:136` | RDP connection shows placeholder UI — no actual RDP implemented | Display clear "RDP not yet supported" message or remove the connect flow |

### 8.2 Code Style

| Finding | Action |
|---------|--------|
| No ESLint config exists (`eslint.config.js` missing) | Add ESLint with `@typescript-eslint/recommended` rules |
| No Prettier config | Optional: Add `.prettierrc` |
| `conductor/code_styleguides/typescript.md` says "no default exports" — `App.tsx` uses `export default App` | Fix: `export { App }` or note this is the Vite entry-point convention exception |

### 8.3 Documentation Gaps

| Finding | Action |
|---------|--------|
| `index.html` title: "Tauri + React + Typescript" | Change to "Quasar" |
| Migration gap at 002 undocumented | Add note in `docs/SCHEMA.md` |
| `ssh_timeout_*` event undocumented / apparently unused | Document or remove |
| `get_metrics_history` command — not documented in `CLAUDE.md` IPC table | Add to IPC table |
| Linux-only assumption for remote SSH metrics commands not documented | Add note in `docs/CORE_WORKFLOWS.md` |

---

## Execution Order

The phases above are listed by priority. Suggested execution order for maximum impact:

```
Phase 1  →  Phase 2.8 (unwrap audit)  →  Phase 3.1 (type safety)
         →  Phase 3.2 (hook cleanup)  →  Phase 5.2 (db.ts conflict)
         →  Phase 6 (IPC contracts)   →  Phase 4 (accessibility)
         →  Phase 7 (CI/CD)           →  Phase 8 (cleanup)
```

Each phase should:
1. Make changes on branch `claude/add-claude-documentation-Fy6A5`
2. Run `npm test` (frontend) and `cargo clippy -- -D warnings && cargo test` (backend) after each phase
3. Commit with message referencing the audit phase (e.g. `audit(security): fix CSP and updater config`)

---

## Files to Modify (Master List)

### Configuration
- `src-tauri/tauri.conf.json` — CSP, updater pubkey/URL
- `tsconfig.json` — Verify `allowImportingTsExtensions`

### Rust Backend
- `src-tauri/src/lib.rs` — remove greet, fix unsanitised error returns, document ssh_timeout event
- `src-tauri/src/validation.rs` — IPv4 leading-zero fix, add path validation
- `src-tauri/src/discovery.rs` — add stop signal, error reporting
- `src-tauri/src/scheduler.rs` — timezone doc, vault-locked error, truncation warning
- `src-tauri/src/ssh.rs` — session activity timeout, VecDeque cap
- `src-tauri/src/ssh_exec.rs` — Linux platform note/guard
- `src-tauri/src/db.rs` — connection reuse investigation
- `src-tauri/migrations/013_indexes.sql` — new migration if indexes missing

### Frontend
- `src/index.html` — title
- `src/db.ts` — remove schema creation
- `src/App.css` or `App.tsx` — move module-level code from SettingsView
- `src/components/vault/VaultProvider.tsx` — unlisten race fix
- `src/components/vault/CredentialManager.tsx` — type assertions, showPasswordFor fix, clipboard auto-clear
- `src/components/MonitoringView.tsx` — interval cleanup, listener await
- `src/components/TerminalComponent.tsx` — dep array reconnect fix, error matching
- `src/components/NetworkScanner.tsx` — CIDR validation, callback ref
- `src/components/ScheduledTasksView.tsx` — cron validation, host_id guard, task type clarity
- `src/components/SshTunnelsView.tsx` — polling interval, port validation
- `src/components/RemoteManager.tsx` — quick-connect race, alert() replacement
- `src/components/SshFileManager.tsx` — falsy password guard
- `src/components/dashboard/DashboardView.tsx` — SavedHost ID type, try-catch
- `src/components/dashboard/AlertRules.tsx` — any type, toggle semantics, accessibility
- `src/components/dashboard/AlertFeed.tsx` — listener cleanup
- `src/hooks/useSshHostKeyVerification.ts` — promise resolution, callback cleanup

### CI/CD
- `.github/workflows/ci.yml` — cargo audit, npm audit, coverage, ESLint
- `.github/workflows/release.yml` — draft flag, signing
- Add `eslint.config.js`

### Documentation
- `docs/SCHEMA.md` — migration 002 gap note
- `docs/CORE_WORKFLOWS.md` — Linux metric assumption
- `CLAUDE.md` — get_metrics_history command, ssh_timeout event

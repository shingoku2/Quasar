# CLAUDE.md — Quasar

Quasar is a Tauri 2 desktop app for remote infrastructure management: SSH terminals, SFTP, tunnels, monitoring and alerts, network discovery, cron-scheduled tasks, and an encrypted credential vault. React + TypeScript frontend, Rust backend, one SQLite database. This file is current-state only: history is in `CHANGELOG.md`, rationale in `docs/ARCHITECTURE.md`.

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
## Commands

```bash
npm run tauri            # full app (Vite on :1420 + Rust); wraps `tauri dev` with CARGO_TARGET_DIR=src-tauri/target
npm run dev              # frontend only
npm run build            # tsc && vite build
npm test                 # vitest run
npm run coverage         # tests + coverage floor (vite.config.ts); what CI runs
npx tsc --noEmit
cd src-tauri && cargo clippy --all-targets -- -D warnings
cd src-tauri && cargo test
cd src-tauri && cargo deny check   # and `cargo audit`; both run in CI
```

Node 24.15.0 (`.nvmrc`), npm 12. Rust toolchain pinned in `rust-toolchain.toml` (1.98.1); MSRV 1.95 (`rust-version`, checked by CI). Linux needs the WebKitGTK/libsoup/GTK dev packages listed in `README.md`.

## Repository map

```
src/                      React frontend
  components/             views and dialogs (vault/ = vault UI, dashboard/ = widgets); tests co-located
  hooks/                  useViewVisibility (useOnViewShown, useVisiblePolling), useSshHostKeyVerification,
                          useTailscaleStatus, useUpdater
  lib/utils.ts            cn(), getErrorMessage(), isUserCancelled()
  test-setup.ts           global Tauri mocks
  jest-dom-vitest.d.ts    jest-dom matcher types for vitest 5 (drop once jest-dom ships them)
src-tauri/src/
  lib.rs                  setup, migrations list, every #[tauri::command] wrapper, generate_handler!
  vault.rs, vault/        VaultState (lock, gate, auto-lock); kdf.rs, credentials.rs, ssh_keys.rs, audit.rs
  ssh.rs                  interactive sessions + host-key prompts; ssh_connect.rs (phased connect),
                          ssh_auth.rs, ssh_exec.rs (one-shot), ssh_pool.rs, ssh_tunnel.rs, sftp.rs
  scheduler.rs  monitoring.rs  scanner.rs  discovery.rs  host_tracker.rs  health.rs  tailscale.rs
  launcher.rs  ai.rs  local_paths.rs  native_confirm.rs  crypto.rs  validation.rs  errors.rs  db.rs
  ssh_test_server.rs      cfg(test) in-process russh server for connect/auth/exec/pool/tunnel tests
src-tauri/migrations/     001, 003–015 (no 002, on purpose)
docs/                     ARCHITECTURE, SCHEMA, SECURITY_MODEL, CORE_WORKFLOWS, RELEASE_SIGNING, style/, archive/
audit/, AUDIT.md          September 2026 audit (findings, plan, verification)
```

## Stack

Tauri 2.11 (`tauri` crate and `@tauri-apps/*` move in lockstep: bump both sides of a plugin to the same minor together; Dependabot groups them). React 19, TypeScript 7, Vite 8, Tailwind 4, xterm.js 6, Recharts 3, vis-network 10, lucide-react. Rust 2021 on Tokio; rusqlite 0.40 (bundled) + rusqlite_migration; russh 0.63 / russh-sftp 3; aes-gcm 0.11, argon2 0.6, hkdf 0.13, sha2 0.11, subtle, zeroize 1.9, secrecy 0.10 (`SecretString`); sysinfo 0.39, cron 0.17, mdns-sd 0.21, surge-ping, dns-lookup 4, ollama-rs 0.3. Tests: Vitest 5, React Testing Library 16, jest-dom 7, jsdom.

## IPC

- 66 commands, all registered in `generate_handler!` in `lib.rs`. The full list with args and return types is in `docs/ARCHITECTURE.md` (a test keeps it complete); events are listed there too. Add a command there when you add one.
- **`invoke()` payload keys are lowerCamelCase.** A snake_case key for an `Option<T>` param silently arrives as `None` (no error). `CredentialManager.test.tsx` asserts no payload key contains `_`.
- Commands return `Result<T, String>`. Sanitize errors with `errors::sanitize_error`; only messages the user must read go through `errors::user_facing_vault_error`. The interactive terminal connect returns raw `ssh_connect` diagnostics by design.
- A `Cancelled` error means the user declined a native confirmation: treat it as a no-op (`isUserCancelled()`).

## Database

- Schema: `docs/SCHEMA.md` (column tables checked by `schema_doc_lists_every_table_and_column`: after a schema change, run it and paste its output). Tables: hosts, credentials, vault_settings, ssh_known_hosts, security_audit_log, discovered_hosts, host_services, scheduled_tasks, metrics_history, alert_history, alert_rules, monitoring_host_credential.
- Every connection goes through `db::open_connection()`. Never `Connection::open` elsewhere.
- New migration: `src-tauri/migrations/016_*.sql` (highest + 1), appended to `MIGRATIONS` in `lib.rs`, with `LATEST_SCHEMA_VERSION` bumped (a test checks they match). Append-only; never edit a shipped migration or create a `002`. SQLite has **no** `ADD COLUMN IF NOT EXISTS`: use an `M::up_with_hook` that checks `PRAGMA table_info` (see 011/012).

## Testing

- Frontend tests mock all Tauri APIs (`test-setup.ts` mocks `invoke`, `listen`, dialog, path). Set results with `vi.mocked(invoke).mockResolvedValue(...)`. Because every test mocks `invoke`, a wrong command name or arg key passes tests and fails at runtime: check names against `docs/ARCHITECTURE.md`.
- Components are default exports (`import VaultSettings from './VaultSettings'`); hooks and utils are named exports.
- Stub `window.confirm` with `vi.spyOn(window, 'confirm')` per test and restore it; never at module scope (it leaked across files once). Restore anything global you replace (e.g. `navigator.clipboard`) in `finally`/`afterEach`.
- Mocks must forward the props a test asserts on (a Recharts mock that dropped props once hid a broken color).
- `useTailscaleStatus` shares a module-level poll: call `resetTailscaleStatusCache()` in `beforeEach`.
- Rust: unit tests in `#[cfg(test)]` modules, integration tests in `src-tauri/tests/`. Use `ssh_test_server` for anything that connects over SSH.

## Conventions

- TypeScript: `docs/style/typescript.md`. `const` by default, no `any`, no unexplained type assertions, `===`, single quotes, semicolons. `cn()` for conditional classes.
- React: function components and hooks. Every view stays mounted (hidden with CSS): load lists other views can change with `useOnViewShown`, poll with `useVisiblePolling`. Vault state comes from `VaultProvider`. Each view is wrapped in `<ErrorBoundary scope=…>`. The host-key prompt is mounted once (`vault/HostKeyPromptHost` in `Layout`); don't mount `useSshHostKeyVerification` again.
- Rust: no `unwrap`/`expect` outside tests. `SecretString` for passwords, `Zeroizing`/`zeroize` for key bytes. Validate network inputs with `validation.rs`. Multi-step DB writes in a transaction.
- `tailscale.rs` only runs `tailscale status --json` with fixed args: never pass user input, never call the control-plane API.

## Security invariants

Each rule has regression tests; don't weaken one without replacing its test. Rationale: `docs/ARCHITECTURE.md`, `docs/SECURITY_MODEL.md`.

1. **KDF v2** (`vault/kdf.rs`): Argon2id (47104 KiB, t=2, p=1) → HKDF → AES key (memory only) + stored verifier. Never store a password hash. Changing params, labels or salt decoding orphans every vault. Tests: `kdf_known_answer_*`, `legacy_phc_from_argon2_0_5_still_verifies`, `test_stored_vault_settings_never_contain_master_key`, `test_legacy_vault_is_migrated_to_v2_on_unlock`.
2. **After a key-changing commit nothing may fail the operation** (v1 migration, password change): audit, scrub and `.bak` cleanup are best-effort. Tests: `test_migration_post_commit_failure_still_installs_new_key`, `test_password_change_post_commit_failure_still_installs_new_key`, `test_cancelled_password_change_clears_rotation_flag`.
3. **Credential code gets the key only via `credential_access()` / `credential_access_background()`** (shared `credential_gate`). Rotation, migration and import hold it exclusively. Lock order gate → `inner`. Drop the access before any network phase. Background work uses `_background` (doesn't reset auto-lock). Tests: `test_credential_write_waits_for_master_password_change`, `test_gate_held_until_new_key_installed`, `test_background_access_does_not_count_as_activity`.
4. **A lock during a password change is respected** (re-check `master_key.is_some()` before installing). Test: `test_lock_vault_during_password_change_stays_locked`.
5. **Unlock lockout is persisted** (5/10/15 → 5/15/60 min). Tests: `test_lockout_persists_across_vault_state_restart`, `test_successful_unlock_clears_persisted_lockout`.
6. **No decrypted secret in `get_credential`** (only `has_*` flags). Passwords cross IPC only via `reveal_credential_password` (native confirm, audited); SFTP and SSH take a `credentialId`. Forms treat an empty secret field as "not shown", not "not stored", and never prefill passwords. Test: `test_frontend_view_carries_no_secret_material`.
7. **A present-but-malformed encrypted field is an error, never "absent".** Test: `test_malformed_private_key_blob_is_an_error_not_absent`.
8. **Host keys**: interactive trust by pending request id only (`trust_ssh_host_key(requestId)`); changed keys need a native confirm; Rejected keys are refused without a prompt; matching is case-insensitive, and a key differing from another port's trusted key counts as Changed. Non-interactive paths accept only a key trusted for that host+port (`check_non_interactive`). Fingerprint via `presented_key_bytes`. Tests: `host_key_approval_tests::*`, `test_case_and_port_changes_keep_the_pin`, `test_non_interactive_accepts_only_a_trusted_key_for_that_port`, `test_lookup_error_fails_closed`, `presented_key_bytes_are_stable_and_certificates_pin_their_key`.
9. **Local paths only from backend dialogs** (`pick_local_file` / `pick_save_location` → single-use, intent-bound `LocalPathGrants`). Tests: `only_picked_paths_are_accepted`, `grants_are_single_use_and_bound_to_their_intent`.
10. **Host-bound credentials only work against their host**, on every path. Tests: `monitoring_binding_respects_credential_host`, `scheduled_task_validation`.
11. **Native confirmations** (`native_confirm.rs`) guard changed host keys, pin removal, marking trusted, moving/clearing a credential's host, and reveals. Never put a webview `confirm()` in front of these. Tests: `native_confirm::tests::*`.
12. **Least-privilege webview**: no `core:event` emit, no dialog permission, opener limited to ollama.com, production CSP without `localhost:*`. Test: `capability_and_csp_stay_least_privilege`.
13. **Import locks the vault** and refuses newer schemas. Tests: `import_locks_the_vault`, `import_rejects_newer_schema_and_leaves_vault_alone`.
14. **Scan claim happens before `tokio::spawn`** (`scanner::claim_scan()`); same rule for any command that spawns work and returns. Tests: `test_claim_scan_rejects_concurrent_claim`, `test_stop_request_after_claim_is_not_lost`.
15. **`ssh_auth` falls back to `none` auth only when no password and no key were supplied** (Tailscale SSH). Tests: `ssh_auth::tests::*`.
16. **SSH reliability**: the handshake budget (135 s) exceeds the host-key prompt (120 s); exec reads until close; the pool never closes a leased session; tunnels end with their session; scheduled runs never overlap (`InFlightGuard`). Tests: `slow_host_key_approval_fits_the_handshake_budget`, `exit_status_after_eof_is_not_lost`, `expired_sessions_are_not_closed_under_an_active_lease`, `tunnel_loop_ends_when_the_session_dies`, `due_runs_are_detached_and_never_overlap`.
17. **Exports and `.bak` files made before the v2 migration expose credentials**: treat them as plaintext.

## CI

`.github/workflows/ci.yml` (push/PR to `master`): actionlint; frontend (`tsc`, `npm run coverage`, `npm audit`); MSRV check; backend (clippy `--all-targets`, `cargo test`, `cargo audit`, `cargo deny`); build matrix. `release.yml` (`v*` semver tags) builds signed bundles as a **draft** release (publish it by hand; see `docs/RELEASE_SIGNING.md`). `audit.yml` runs audits weekly. Rules: actions pinned to a commit SHA with the version in a comment; read-only `GITHUB_TOKEN` by default; **never use the `secrets` context in a step `if:`** (map it to a job-level env; that bug made `release.yml` invalid for a month). Accepted advisories: `src-tauri/.cargo/audit.toml` and `deny.toml` `ignore` (keep them in sync), documented in `SECURITY.md`.

## Constraints

- SFTP supports password credentials only (`CredentialSelector allowedTypes={['ssh']}`). Add key auth in `sftp.rs` before exposing `ssh_key` credentials there.
- The Vite port is fixed at 1420 (Tauri config and CSP depend on it).
- `discovery.rs` is a singleton; don't bypass it.
- Only `ssh` and `rdp` hosts have an in-app client; other protocols are inventory-only.
- Updater keys live outside the repo (`~/.tauri/`); `*.key` is gitignored.

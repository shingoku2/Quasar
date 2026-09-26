# Changelog

Notable changes, newest first. The detailed record for September 2026 is `AUDIT.md` (findings, fix order, commits) and the git history. The long-form notes from before that are in git history under `AGENTS.md` (up to commit `c36207d`).

## 2026-09-26: Test gap fill

- New frontend tests for the weakest-covered views: Dashboard host handlers (connect, save, delete), RemoteManager connect flows (credential selector, manual entry, saving a credential, RDP, split view, quick connect), TerminalComponent sessions (connect payload, event channels, resize, cleanup, in-place theme changes), ScheduledTasksView (add, edit, SFTP pickers, credential filtering, delete, run now), SshTunnelsView (start payload, port clamping, errors) and SessionContainer tab interactions. Frontend coverage rose from 73.6/68.2/68.6/75.9 to 83.5/78.2/80.0/85.3 (statements/branches/functions/lines), and the CI floor is raised to 82/76/78/84.
- New Rust tests: the audit log's `record` and `get_audit_logs` (every filter, search, paging, newest-first order, the default 100-row cap, parameter binding), tunnel state (stop signal, listing, shared clones) and `copy_bidirectional`, and the SFTP password requirement (which had only an empty placeholder test).

## 2026-09-26: Refactors (P7-9)

- A credential whose stored password is only partly present (a corrupt row) now fails to load instead of loading with an empty password, the same rule the key fields already followed. An empty password with no key would have made SSH fall back to `none` auth.
- Dialogs close with Escape, keep keyboard focus inside while open, return it to where it was on close, and are announced by their title (FE-020).
- Form labels are tied to their inputs in the task, credential, vault-settings, audit-log and monitoring forms, so screen readers name each field and clicking a label focuses it (FE-021).
- Session tabs are a keyboard tablist (arrow keys, Home, End), SFTP entries are buttons (Enter opens a folder), and scanner host cards open with Enter or Space (FE-022).
- A revealed credential password hides itself after 30 seconds and as soon as the vault locks (it stayed on screen after auto-lock). A copied password is wiped from the clipboard after 30 seconds or on lock, but only if the clipboard still holds it, so something you copied since survives (FE-009).
- The credential list can no longer be overwritten by an older, slower load after a newer search (FE-014).
- CredentialManager is split into `vault/credentials/` (card, form dialog, view dialog, `buildCredentialPayload`, `useRevealedPassword`, `useCredentials`) (FE-027).
- SettingsView is split into `settings/` (one file per panel, plus `appearanceStore` for the saved theme). The theme is applied once at startup from `main.tsx`; the duplicate apply at module import is gone (FE-027).
- ScheduledTasksView is split into `scheduled/` (form, list, run-result banner, and a pure `taskForm.ts` for validation, payload, host and credential filtering, now unit-tested) (FE-027).
- MonitoringView is split into `monitoring/` (system info bar, disk card, process cards, remote hosts list, `useRemoteHostsHealth`, format helpers) (FE-027).
- The Dashboard widgets and the Monitoring view share one `system-metrics` listener and one initial fetch (`useSystemMetrics`), instead of one each (FE-024).
- An RDP session opened from the Inventory while split view is on now joins the split; it opened hidden, because the Inventory's handlers were frozen into tab state when the tab was built (FE-018). RemoteManager now keeps open sessions as data (`remote/SessionContent`), and is split into `remote/` (`InventoryPanel`, `useConnectFlow`) (FE-027).

## 2026-09-26: Repo cleanup

- Deleted 61 superseded files: GEMINI.md, the `conductor/` planning scaffolding, `_archived/` (the removed automation engine), `.codex/`, `.windsurf/`, the MONITORING_PHASE reports, the old workflow test fixtures, three unused template SVGs and a never-compiled scratch file. Their useful content was already in `docs/archive/` and `docs/style/`; the rest is in git history (last present at `5cc7b55`).

## 2026-09-25: PR #68 review fixes

- Legacy `password`-type SSH credentials show up again in the terminal, SFTP and tunnel credential pickers. They had been filtered out, though the backend accepts them.
- The alert-rule editor reloads its rules each time the Monitoring view is shown, so after an import, editing a rule can't overwrite the imported rules with stale ones.
- Scheduled tasks can only target SSH hosts. Database, API, RDP and "other" hosts are no longer offered, and are refused when a task is saved and when it runs.
- A host key marked Rejected can no longer be quietly moved back to "unknown" or "changed" (which turned the refusal into an ordinary prompt): any change out of Rejected asks through a native dialog.
- A saved SFTP task can no longer be pointed at another host or remote path without picking its local file again, and moving a host that scheduled transfers use asks through a native dialog.
- After importing another vault, the old vault's unlock lockout no longer blocks the imported vault's password.
- A monitoring tick that evaluated the old rules just before an import can no longer save their alerts into the imported database.
- An SFTP transfer that fails no longer hands its local-file pick back: every pick is good for one transfer, so a compromised webview can't replay it against another host.
- If migrating an imported backup fails and putting the previous database back fails too, the import now says so, and names the `.bak` file to recover from.
- Monitoring pauses its alert rules while an import swaps the database, so it can't record an old rule's alert into the imported file, and reloads them afterwards (the old ones again if the import failed). If an imported database's alert rules can't be read, the import reports it and monitoring runs with no rules, not the old database's. Reloading also forgets which rules were triggered, so no recovery is announced for an alert of the old database.
- A single SSH-key credential (no stored password) no longer blocks every master-password change, or stops a legacy vault from migrating off the v1 key-equivalent format. The rekey step now checks each row by whichever encrypted field it has.
- The release gate runs `cargo deny` and the coverage floor, like CI.
- The v1 vault migration no longer makes `quasar.db.bak` (the vault an import replaced) impossible to unlock: it migrates that copy too when the same password unlocks it, and leaves it alone otherwise.
- A pooled SSH command that fails on a shared session no longer disconnects commands still running on it, and no longer evicts a fresh session another task just created.
- After an import, monitoring uses the imported database's alert rules; it kept the old ones until restart.
- Importing a backup from an older version migrates it to the current schema; before, the app ran on the old schema until restarted (e.g. no alert rules table).
- A host with different keys trusted on different ports: a new port presenting one of them now gets the changed-key warning and native confirmation, not a first-use prompt.
- Only SSH-type credentials authenticate SSH connections, and SFTP takes SSH passwords only. Before, an API, database, RDP or other credential could be picked for an SFTP or SSH task (or a monitoring binding) and its secret sent to the SSH server. Checked at save and at use; pickers filter to matching types.
- Queued host-key prompts no longer inherit the previous prompt's "I verified" and permanent-trust choices.

## 2026-09-24 – 2026-09-25: Full audit and fixes (EDW-17 to EDW-26)

- **Vault key separation (Critical, RSEC-001).** The vault stored an Argon2 hash of the master password that was byte-for-byte the encryption key. KDF v2 derives the key and a stored verifier separately (Argon2id → HKDF-SHA256). v1 vaults migrate on their next unlock, and the old hash is scrubbed from the file. `secure_delete` is on and the database files are owner-only.
- **Vault state.** Importing a database locks the vault and rejects newer schemas. The auto-lock timeout is validated, persisted and loaded at unlock. Background polls no longer count as activity. The unenforced "require master password" toggle is removed. More vault events are audited.
- **IPC hardening.** Local file paths come only from Rust-side dialogs (single-use, intent-bound grants). Host keys are trusted by pending request id, and changed keys need a native confirmation. Credentials with a stored host only work against it. SFTP takes a credential id instead of a revealed password. The production CSP is least-privilege, and there's no dialog or event-emit capability. Scheduled tasks are validated and audited. Eight dead commands are removed.
- **SSH and scheduler reliability.**
  - The host-key prompt no longer times out the handshake.
  - Failed remote commands are recorded as failures (exit status after EOF).
  - The pool never closes a leased session, and tunnels end when their session dies.
  - Scheduled runs never overlap and never block the tick.
- **Broken features fixed.** Alert rules are persisted and editable in the Monitoring view (they were in memory only, with no UI). Cron and known-hosts dates are fixed. Lists refresh when their view is shown. Per-view error boundaries mean a render error no longer blanks the app. There's one app-wide host-key prompt.
- **CI and supply chain.**
  - Actions are SHA-pinned, the token is read-only, and there's a coverage floor.
  - An MSRV job was added, with the toolchain pinned.
  - `cargo deny`, a weekly audit workflow and Dependabot were added.
  - The release gate is semver-only and runs audits. The release workflow had been invalid since 2026-08-26, so no release was built in that window.
- **Dependencies.**
  - Yanked crates removed. mdns-sd 0.21 fixes a LAN-triggerable panic, and discovery now exits when its daemon dies.
  - Tauri 2.11 (JS and Rust in lockstep).
  - argon2 0.6 (KDF outputs unchanged, pinned by known-answer tests).
  - russh 0.63 / russh-sftp 3, dns-lookup 4 and vitest 5.
- **Docs and repo.**
  - CLAUDE.md rewritten against the code. `docs/ARCHITECTURE.md` and `docs/SCHEMA.md` are checked by tests.
  - AGENTS.md is now a pointer, and this changelog was added.
  - Historical reports moved to `docs/archive/`, and the security-reviewer agent's config was corrected.

## 2026-09-24

- Credential writes are serialized with master-password rekeying via `credential_gate` (EDW-15). This closed the last deferred finding from the September 2 audit.
- Dependabot alert #1 was identified as `rsa` / RUSTSEC-2023-0071, an accepted risk with no upstream fix. It's documented in `SECURITY.md` (EDW-16).
- PR triage (#54–#62). The "plaintext password exposure" fix didn't fix it: `get_credential` now returns only `has_password`, and passwords are revealed on demand. The timezone-format cache missed same-offset zone switches and was removed.

## 2026-09-19

- Tailscale integration: tailnet peers in Remote → Inventory (via the local `tailscale status --json` only), add a peer as a host, a Tailscale badge on saved hosts, and `none` auth for Tailscale SSH peers.

## 2026-09-17

- 26 bot-authored PRs triaged and merged, 9 of them fixed first.
- The network-scan stop race is really fixed: the scan is claimed synchronously before `tokio::spawn`. The bot's own "fix" was a no-op, and a bot commit later reverted the real fix mid-review; it was re-applied.

## 2026-09-02 – 2026-09-16

- Deferred bug-audit fixes reconciled; the SSH connect/disconnect race was fixed with pending-connection cancellation.

## 2026-08-26

- Code signing (Windows, macOS notarization) and the signed auto-updater.
- The unlock lockout is persisted across restarts.
- RUSTSEC-2023-0071 accepted and documented. CI targets `master` (it had never run before).

## 2026-08-22

- Full bug audit:
  - Adding a host no longer drops every open session tab.
  - Scan events are no longer lost mid-scan.
  - The alert-rule toggle is an atomic upsert.
  - All DB access goes through `db::open_connection()`, so contended writes wait instead of being dropped.
  - Password change no longer holds the vault lock for the whole re-encryption, and an explicit lock during it is respected.
  - Scheduled tasks run concurrently.
  - `getErrorMessage` is used everywhere.

## 2026-08-21

- Phase-aware SSH connect errors (DNS/TCP/handshake, `ssh_connect.rs`).
- Credential edits now save: `invoke` keys must be camelCase, and a PEM-pasted key no longer blocks edits.
- Host protocols include database, API and other.

## 2026-03-06

- Test coverage expansion: 12 new test files, 208 tests.

## 2026-02-20

- Light-theme app UI; the terminal theme is independent (Solarized Light fixed).

## 2026-02-18

- Code audit AUD-01..07 fixed (see `docs/archive/codebase-audit-2026-02-18.md`).
- Scheduled tasks (cron, SSH command, SFTP upload/download, last-run results, Run now).
- SSH terminal hang fixed with `Channel::wait()`, plus output batching.
- Dashboard health fixes.

## 2026-02-16

- Credential edit and type-switch fixes; migration 009 makes the password columns nullable.

## 2026-02-12

- UI refactor to the dark navy/cyan design, Quasar logo, and the new dashboard layout.

## 2026-02-03

- Migration system fixed for fresh installs (initial schema 001, migration 005 re-enabled).

## 2026-02-02

- Network scanner: 13-port scan, device detection, topology view, persistent host tracking.
- The automation canvas was removed (see `docs/archive/automation-removal-2026-02.md`).

## January 2026

- MVP: SSH terminal, SFTP, credential vault (phases 1-3), monitoring and alerts, local AI assistant (Ollama), remote manager. The project was renamed from Project Titan to Quasar.

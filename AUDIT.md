# Quasar Full Audit — AUDIT.md

**Date:** 2026-09-24 · **Audited commit:** master `97dd84a` (the EDW-15 credential gate, #63, #64 and #65 are all included) · **Linear:** EDW-25, project "Quasar: Full Audit + Cleanup"
**Inputs:** `audit/00-recon.md`, `01-baseline.md`, `02a-rust-security.md`, `02b-ipc-boundary.md`, `03-rust-quality.md`, `04-frontend.md`, `05-tests-deps-ci.md`, `06-cleanup.md`. The partial files hold the full evidence, impact and fix text for every finding. This file de-duplicates them, verifies them and orders the work.
**Status:** ⛔ **Awaiting approval.** Phase 7 (EDW-26) doesn't start until Edward records approval on EDW-25.

**Threat model** (shared by all phases). Quasar is a single-user desktop app. The attackers that matter are:
1. anyone who can read `quasar.db`, an export or `quasar.db.bak`: backups, a stolen disk, other local users;
2. a malicious network or remote host (SSH, SFTP, mDNS, LAN);
3. a compromised webview (XSS or a malicious dependency) calling IPC. Phase 2B found **no working XSS path**, so findings that need a compromised webview are defence in depth and capped at Medium;
4. the supply chain: dependencies, CI actions, and the release pipeline that signs auto-updates.

---

## 1. Executive summary

121 distinct findings, after 18 cross-phase duplicates were merged: **1 Critical, 1 High, 29 Medium, 69 Low, 21 Info.** The build is healthy: tsc, clippy and every test pass on the audited commit (see §2). The problems are design defects that no test catches, not red builds.

The ten items that matter most, worst first:

1. **RSEC-001 (Critical): the vault's stored password hash *is* the encryption key.** Verified by the main agent and reproduced at runtime (`audit/raw/refresh/rsec001-proof.txt`). `vault.rs` saves `argon2.hash_password(pw, salt)` as the "verifier". It derives the AES-256-GCM key with `hash_password_into(pw, same salt bytes, 32 bytes)`, using the same Argon2 instance. In argon2 0.5.3 those produce identical bytes (`src/lib.rs:579-587`). **Anyone holding `quasar.db`, an export or `quasar.db.bak` can decrypt every stored password and SSH key without the master password.** Argon2, the 12-character minimum and the lockout protect nothing against that attacker. RSEC-009 (the DB is created world-readable under the default umask, and the `.bak` is never removed) widens who that attacker can be. The fix needs a vault format migration. Until then, treat existing exports and `.bak` files as plaintext.
2. **CI-001 (High): the release and auto-update pipeline has never run.** Verified. `release.yml:123` uses `secrets.*` in a step `if:`, which makes the file invalid. There have been 169 runs, every one failing with 0 jobs, including one per push today. No signed installer or `latest.json` has ever been produced, and pushing a `v*` tag would build nothing. It also puts a red ✗ on every push.
3. **RSEC-002 (Medium): auto-lock never fires in normal use.** Verified. The Dashboard polls `get_remote_hosts_health` every 30 s and it always calls `credential_access()`, which resets the idle timer. Scheduled tasks do the same. The vault stays unlocked on an unattended machine. RUST-003 adds that the configured timeout isn't reloaded after a restart, so it silently falls back to 15 min.
4. **RSEC-004 + RUST-004 (Medium): `import_database` corrupts vault state.** Verified. It never locks the vault or takes `credential_gate`, so credentials written after an import are encrypted with the old key and are lost at the next unlock. Importing a DB from a newer build bricks startup (`DatabaseTooFarAhead`, then `.expect`). This is the one path the EDW-15 fix doesn't cover.
5. **IPC-001 / IPC-002 / IPC-003 (Medium): a compromised webview has full local file read/write, can silently trust MITM keys, and can steal vault passwords.** IPC-001 was verified by the main agent. `validate_path` only rejects `..`, so SFTP, scheduled SFTP and DB export/import accept any absolute path. `trust_ssh_host_key` stores whatever key the webview sends, and nothing is written to the audit log. Any credential can be bound to any host, and the 30 s monitoring poll then sends it there. There's no XSS vector today, but these three together turn any future one into local code execution plus exfiltration of every credential.
6. **RUST-001 (Medium): the first connection to a new SSH host fails unless you click Trust within about 10 s.** Verified. The 10 s handshake timeout (`ssh.rs:258`, `ssh_tunnel.rs:171`) wraps the host-key prompt, which is supposed to allow 120 s (`ssh.rs:111`). The key gets saved anyway, so the user sees a failure followed by a "trusted" key.
7. **RUST-002 + FE-003 (Medium): alerting is dead end to end.** Verified. The rules UI (`AlertRules.tsx`) has never been mounted in any commit, and rules live only in memory: no table exists and they're lost on restart. `alerts-triggered` never fires, and the Dashboard alert widgets are always empty. CLAUDE.md and the README both advertise the feature.
8. **FE-001 (Medium): decrypted SFTP and SSH passwords live in React state for the lifetime of the tab, after the vault locks.** The SFTP commands take the plaintext password on every call (RSEC-013). The fix is for SFTP to accept `credential_id`, the way `connect_ssh` does.
9. **CLEAN-001 (Medium): `.claude/settings.local.json` is committed.** Verified. It pre-approves `python -c '*'`, `npm update`, `cargo update` and `git fetch` for every Claude session on every clone, so a prompt-injected agent skips the permission gate. A related gap (CLEAN-013) is that `.gitignore` doesn't cover the updater private key that `docs/RELEASE_SIGNING.md` tells you to generate in the repo root.
10. **CLEAN-002 / CLEAN-003 / RUST-018 (Medium/Low): the agent instruction files are wrong.** CLAUDE.md lists 21 IPC commands that don't exist, 3 tables that don't exist, a nonexistent prop, stale versions, and invalid SQLite syntax for migrations (`ADD COLUMN IF NOT EXISTS`). AGENTS.md (130 KB) contradicts itself. Every frontend test mocks `invoke`, so an agent following CLAUDE.md gets green tests and a runtime failure.

Also notable: the EDW-15 rekey fix itself is sound. Four phases independently traced every credential call site through the gate, and the regression test fails if the gate is removed from either side. One mutation isn't caught (TEST-008). Dependabot #1 (`rsa`, RUSTSEC-2023-0071) **isn't reachable** as a Marvin decryption oracle (DEP-005); dismissing it is justified.

## 2. Baseline

Two runs. **Phase 1** (`01-baseline.md`) ran on `fcc58d2`, before EDW-15. **EDW-25 re-run** ran on the audited tree (HEAD `1845f37` = master `97dd84a` + `audit/` only), with node 24.15.0, npm 12.0.2 and rustc 1.98.1 (raw output in `audit/raw/refresh/`). The first Rust attempt on this container failed only because the GTK/WebKit system libraries were missing (`gdk-3.0`, `cargo-clippy-nolibs.txt`). After apt-installing the same package list CI uses, it was re-run.

| Check | Phase 1 (`fcc58d2`) | EDW-25 re-run (`97dd84a`) |
|---|---|---|
| `npx tsc --noEmit` | ✅ clean | ✅ clean (`refresh/tsc.txt`) |
| Vitest | ✅ 49 files / 351 tests | ✅ 49 files / 351 tests (`refresh/vitest.txt`) |
| `npm run build` | ✅ main chunk 1,313.95 kB (> 500 kB warning) | not re-run (no frontend build input changed that would affect size meaningfully) |
| `cargo clippy --all-targets -- -D warnings` | ✅ 0 warnings | ✅ 0 warnings (`refresh/cargo-clippy.txt`) |
| `cargo test` | ✅ 131 lib + 4 integration passed, 1 ignored | ✅ **132** lib + 4 integration passed, 1 ignored. The new test is `vault::tests::test_credential_write_waits_for_master_password_change` (EDW-15) ✅ |
| FE coverage (v8) | Lines 72.83% · Stmts 70.64% · Branches 66.13% · Funcs 65.63% | not re-run (needs the undeclared `@vitest/coverage-v8`, CI-007) |
| RS coverage (llvm-cov) | Lines 52.60% · Regions 52.01% · Funcs 36.00%. Weakest: `ssh_auth`/`ssh_tunnel`/`ai`/`discovery` 0%, `sftp` 0.5%, `lib.rs` 21.5% | not re-run |
| `npm audit` | 0 vulnerabilities (271 deps) | — |
| `cargo audit` | 0 unignored; 1 ignored (RUSTSEC-2023-0071); 6 unmaintained, 1 unsound, 2 yanked (the yanked check is incomplete) | — |
| `cargo deny` | ❌ no `deny.toml` (config gap, DEP-003) | — |
| Outdated | npm 18 (1 major); cargo 13 (5 majors) | — |

**Bottom line:** no build, lint, type or test failures on the audited commit. Every finding in this audit is one the existing gates can't see.

## 3. Findings table

These are the de-duplicated findings. Each keeps its original ID from the partial file, where the evidence, impact and fix are written out in full. Merged duplicates are listed at the end of the table and in §9.
**Verified** column: **Main**: the main agent re-opened the file:line (or reproduced it) during EDW-25. **Sub**: verified by the subagent with file:line evidence, but not independently re-checked. **Sub ×N**: found independently by N phases.
Area: Vault / IPC / SSH / Sched (scheduler) / Mon (monitoring) / DB / FE / Test / Dep / CI / Docs.

| ID | Sev | Area | Location | Summary | Effort | Verified |
|---|---|---|---|---|---|---|
| RSEC-001 | **Critical** | Vault | `vault.rs:152-160,194-214,387-408,593-623` | Stored Argon2 PHC verifier == AES master key (same instance/salt/output len); DB/export/.bak holder decrypts all credentials without the password | M | **Main, reproduced** (crate source + runtime, `raw/refresh/rsec001-proof.txt`) |
| CI-001 | **High** | CI | `.github/workflows/release.yml:123` | `secrets.*` in step `if:` makes release.yml invalid; 169/169 runs failed with 0 jobs; release + signed updater has never run | S | **Main** (Actions API) |
| RSEC-002 | Medium | Vault | `vault.rs:477-493`; `lib.rs:998`; `scheduler.rs:373`; `SystemHealthWidget.tsx:117` | Background `credential_access()` (30 s health poll, cron runs) resets idle timer → auto-lock never fires | S | **Main** |
| RSEC-004 | Medium | Vault/DB | `lib.rs:1726-1798` | `import_database` doesn't lock vault / take `credential_gate`; post-import credential writes orphaned; can interleave with rekey (merges TEST-001, part of IPC-009) | S | **Main** ×3 |
| RUST-004 | Medium | DB | `lib.rs:1726-1798,1851-1854,2033` | Import doesn't check `user_version`; a newer-build DB bricks startup (`DatabaseTooFarAhead` → `.expect`) | M | Sub (rusqlite_migration src) |
| RUST-003 | Medium | Vault | `vault.rs:94-107,451-468,744-759` | Persisted `auto_lock_timeout` never loaded into `inner` → reverts to 15 min after restart while UI shows the saved value (merges RSEC-008 part) | S | Sub ×2 |
| IPC-001 | Medium | IPC | `validation.rs:145-160`; `lib.rs:1555,1586,839-846,880-887,1703,1728` | `validate_path` only rejects `..`; SFTP / scheduled SFTP / DB export+import take any absolute path → arbitrary local file R/W from webview (merges RSEC-005) | M | **Main** ×2 |
| IPC-002 | Medium | IPC/SSH | `lib.rs:1413-1444,1482-1496`; `ssh.rs:13-15`; `vault/ssh_keys.rs:178-245` | `trust_ssh_host_key` persists caller-supplied fingerprint/key for any host; not bound to the pending request; no audit (merges RSEC-006, rated Low there) | S | Sub ×2 |
| IPC-003 | Medium | IPC/Vault | `lib.rs:219-242,284-307,998-1013,1029-1060` | Any credential usable against any host; `set_host_monitoring_credential` binds without checks → monitoring poll sends password to attacker host | M | Sub |
| IPC-004 | Medium | Vault/FE | `VaultSettings.tsx:188-205`; `vault.rs:27,35,534,744-758` | "Require master password for credential access" toggle never enforced nor persisted (merges RSEC-003) | S/M | Sub ×2 |
| FE-001 | Medium | FE/Vault | `RemoteManager.tsx:83-125,200`; `SshFileManager.tsx:26-37`; `TerminalComponent.tsx:27,159,228` | Plaintext SFTP/SSH passwords held in tab JSX state for tab lifetime, survive vault lock (merges RSEC-013) | M | Sub ×2 |
| RUST-001 | Medium | SSH | `ssh_connect.rs:82-95`; `ssh.rs:111,253-260`; `ssh_tunnel.rs:166-173` | 10 s handshake timeout wraps the 120 s host-key prompt → first connect to new host fails unless approved in ~10 s | S | **Main** |
| RUST-002 | Medium | Mon | `monitoring.rs:433-473,91` | Alert rules in-memory only; no table; lost on restart (merges CLEAN-015) | M | **Main** |
| FE-003 | Medium | FE/Mon | `dashboard/AlertRules.tsx` | Alert-rules UI never mounted in any commit → no rule can exist → alerting dead | M | **Main** |
| RUST-005 | Medium | Sched | `scheduler.rs:496-549,551-630` | No in-flight guard: same task can run concurrently (cron + Run now, double click) | S | Sub |
| RUST-006 | Medium | Sched | `scheduler.rs:621-629,748-757`; `sftp.rs:152-172` | Tick awaits all runs; unbounded SFTP transfer stalls every other task's schedule | M | Sub |
| RUST-007 | Medium | SSH | `ssh_pool.rs:96-103,193-226` | Idle sweep can disconnect a session with an active lease near MAX_LIFETIME → command killed mid-run | S | Sub |
| RUST-008 | Medium | SSH | `ssh_tunnel.rs:80-126,213-229` | Tunnel loop never notices session death; tunnel stays "active", port bound, all connections fail | S | Sub |
| FE-002 | Medium | FE | `main.tsx:16-20`; `ErrorBoundary.tsx:47` | Only a root error boundary; any render error blanks app; reload skips `disconnect_ssh` | S | Sub |
| FE-004 | Medium | FE | `Layout.tsx:29-67`; `ScheduledTasksView.tsx:93-117` + 4 more | Always-mounted views fetch hosts/credentials once; lists go stale across views | M | Sub |
| FE-005 | Medium | FE/Sched | `ScheduledTasksView.tsx:50-67,140-143` | Client cron validator rejects `*/5`, accepts DOW 0 (backend rejects); "every 5 minutes" can't be saved | S | Sub |
| CI-002 | Medium | CI | `ci.yml` (no `permissions:`); `release.yml:56-57,138-154` | Default token scope + persisted checkout credentials; signing secrets exposed to all build steps on all OSes | S | Sub |
| CI-003 | Medium | CI | `ci.yml`, `release.yml` (all `uses:`) | All actions pinned to mutable tags/branch, incl. inside the signing job | S | Sub |
| TEST-002 | Medium | Test | `lib.rs` (21.5% lines); `Cargo.toml`; `vault.rs:477` | No command-layer tests; 30/73 commands untested anywhere; `get_master_key` still `pub` so a call site can bypass the gate with green tests | M | Sub |
| TEST-003 | Medium | Test/SSH | `ssh.rs:62-122`; `ssh_exec.rs:30-58`; `sftp.rs:34-60`; `ssh_keys.rs:107` | Host-key decision paths untested; DB error → "unknown host" re-prompt untested | M | Sub |
| TEST-004 | Medium | Test/SSH | `ssh_auth.rs:60-140` (0%) | `none`-auth fallback rule (Security Note 12) and RSA hash selection untested | M | Sub |
| TEST-005 | Medium | Test/Vault | `tests/vault_salt_fix_test.rs`; `vault.rs:152,350,593` | No KDF known-answer test; argon2 upgrade could silently orphan every vault | S | Sub |
| TEST-006 | Medium | Test/Sched | `scheduler.rs:362-500,551-745` | Execution paths untested; unknown `task_type` silently runs as SSH exec | M | Sub |
| CLEAN-001 | Medium | Docs/Supply | `.claude/settings.local.json` | Tracked personal permissions file pre-approves `python -c '*'`, `npm/cargo update`, `git fetch` for every clone | S | **Main** |
| CLEAN-002 | Medium | Docs | `CLAUDE.md:217-273,105,316-318,455,463` | 21 nonexistent IPC commands, fake `db.ts`, 3 fake tables, fake `filterType` prop | M | Sub (mechanical diff) |
| CLEAN-003 | Medium | Docs | `AGENTS.md:1921,1974,1987,2005,2013` | 130 KB agent file with stale/contradictory instructions read by Codex/Jules | M | Sub |
| RSEC-007 | Low | SSH/FE | `SshHostKeyPrompt.tsx:25,152-156` | Changed host key replaced with one click; "trust permanently" defaults on | S | Sub |
| RSEC-009 | Low | DB | `db.rs:32`; `lib.rs:1716,1761-1776,1810` | DB/WAL/.bak/exports created with umask perms; `.bak` never removed; no `secure_delete` (amplifies RSEC-001) | S | **Main** (grep: no `set_mode`/`secure_delete`) |
| RSEC-010 | Low | Vault | `vault/credentials.rs:7-27`; `vault.rs:563` | Decrypted secrets are plain `String` with `Debug`/`Serialize`; `old_key` not zeroized | M | Sub |
| RSEC-011 | Low | Vault | `vault.rs:357-385,604-606`; `credentials.rs:326-334` | Failed unlocks/lockouts/password-change failures not audited; reveal indistinguishable from background access (merges RUST-029, IPC-005) | S | Sub ×3 |
| RSEC-012 | Low | Vault | `vault.rs:637-639` | One undecryptable credential aborts rotation with a generic error | S | Sub |
| RSEC-014 | Low | DB | `lib.rs:181-189,1739-1789` | Import trusts foreign schema (triggers/views/known-host trust); no integrity check (part of IPC-009) | M | Sub |
| RSEC-015 | Low | SSH | `ssh_exec.rs:103-122` | Unbounded remote output buffer → memory DoS by hostile host | S | Sub |
| IPC-006 | Low | IPC/Vault | `lib.rs:1229-1238`; `vault.rs:459,744-758` | `update_vault_settings` unvalidated; `*60` overflow kills auto-lock task in debug; works while locked (merges RUST-015, RSEC-008 part) | S | Sub ×3 |
| IPC-007 | Low | IPC | `tauri.conf.json` CSP | `connect-src ws://localhost:* http://localhost:*` shipped in prod; unused `asset:` | S | Sub |
| IPC-008 | Low | IPC | `capabilities/default.json` | Not least-privilege: redundant event perms (emit), unused `opener:default` | S | Sub |
| IPC-010 | Low | IPC | `build.rs`; `lib.rs:347,501,512,815,1111,1135,1530,1622` | No per-command ACL; 8 never-invoked commands (12 counting dead UI, FE-025) | S | Sub ×3 |
| IPC-011 | Low | IPC/Sched | `lib.rs:823-919`; `scheduler.rs:385,465` | Task creation unvalidated (`task_type`, host_id), unaudited | S | Sub |
| RUST-009 | Low | SSH | `ssh_exec.rs:106-139` | Loop breaks on EOF; late ExitStatus lost → failures recorded as success (frequency UNVERIFIED) | S | Sub |
| RUST-010 | Low | DB | `migrations/007`, `014`; `lib.rs:1002-1010` | No FKs on monitoring/task credential refs; deletes leave dangling ids, errors swallowed | M | Sub |
| RUST-011 | Low | DB | `migrations/014_scheduled_tasks_cascade.sql` | Migration 014 fails on orphaned tasks under FK=ON (reproduced in sqlite3) | S | Sub (reproduced) |
| RUST-012 | Low | Rust | `lib.rs:491-530,1098-1108,1652-1798`; `vault.rs:133-428` | Sync commands on main thread; Argon2 on tokio workers; std Mutex over DB I/O (merges RUST-019) | M | Sub |
| RUST-013 | Low | Rust | `ssh_keys.rs:87-107`; `credentials.rs:121-123`; `health.rs:92`; `vault.rs:241-261` | Swallowed errors; two fail open on security controls (host-key lookup, lockout load) | S | Sub |
| RUST-014 | Low | Vault/DB | `credentials.rs:154-545`; `vault.rs:633-653` | Write+audit not transactional; reads write `last_used_at`; rotation stamps every credential | S | Sub |
| RUST-016 | Low | Rust | `ssh.rs`, `ssh_keys.rs`, `lib.rs`, `scanner.rs` | Inconsistent mutex-poison policy (latent) | S | Sub |
| RUST-017 | Low | DB/Test | `lib.rs:100-127,2208-2222` | Migration tests only cover empty DB; 011/012 logic lives in Rust hooks; hand-written test schemas | M | Sub |
| RUST-018 | Low | Docs/DB | `docs/SCHEMA.md`; CLAUDE.md DB sections | Schema docs missing columns/tables; CLAUDE.md prescribes invalid `ADD COLUMN IF NOT EXISTS` (merges part of CLEAN-014) | S | Sub |
| RUST-020 | Low | Rust | `lib.rs` 2254 ln, `monitoring.rs`, `vault.rs`, `scheduler.rs`, `credentials.rs` | Duplication (DB path ×11, KDF ×3, audit insert ×3, row mapper ×4, SFTP auth ×4); split plan in 03 | L | Sub |
| RUST-021 | Low | Test | `tests/vault_salt_fix_test.rs`; `ssh_exec.rs:431`; `launcher.rs:83`; `scheduler.rs:781` | Integration test never calls app code; three no-op tests (with TEST-010) | S | Sub ×2 |
| RUST-022 | Low | Mon | `monitoring.rs:891`; `lib.rs:1840-1842` | Two independent MetricsCollectors; command CPU% depends on call spacing | S | Sub |
| RUST-023 | Low | FE/AI | `ai.rs:41-72`; `AIAssistant.tsx:157-198` | Stream end without `done` never emitted → chat stuck (merges FE-016) | S | Sub ×2 |
| FE-006 | Low | FE | `KnownHostsManager.tsx:6-15,180-185`; `ssh_keys.rs:40-49` | TS reads `first_seen`, Rust sends `first_seen_at` (i64) → "Invalid Date" on the host-key screen; mocks hide it (merges TEST-007) | S | **Main** |
| FE-007 | Low | FE | `TerminalComponent.tsx:125-152`; `NetworkScanner.tsx`; `useSshHostKeyVerification.ts`; `VaultProvider.tsx` | `listen()` unlisten race leaks listeners (dev: duplicate host-key prompts) | S | Sub |
| FE-008 | Low | FE | `VaultSettings.tsx:61-69` | "Lock now" bypasses context → sidebar says Unlocked | S | Sub |
| FE-009 | Low | FE | `CredentialManager.tsx:586-635` | Revealed password persists after auto-lock; clipboard clear clobbers user clipboard | S | Sub |
| FE-010 | Low | FE/SSH | `useSshHostKeyVerification.ts:57-65`; `ssh_keys.rs:160-163` | Changed-key dialog shows prose incl. new fingerprint as "previous fingerprint" | S | Sub |
| FE-011 | Low | FE/SSH | `RemoteManager.tsx:420-431` | Host-key prompt invisible when another view is active | S | Sub |
| FE-012 | Low | FE | `NetworkTopologyView.tsx:34-206`; `DashboardView.tsx` | Unstable callbacks rebuild the whole graph on every click | S | Sub |
| FE-013 | Low | FE | `SettingsView.tsx:37-62,225-243` | Theme applied only when Appearance opened; notification toggles do nothing | S | Sub |
| FE-014 | Low | FE | `CredentialManager.tsx`, `SshFileManager.tsx`, `useViewVisibility.tsx`, `MonitoringView.tsx` | Unsequenced async results (out-of-order responses) | S | Sub |
| FE-015 | Low | FE | `NetworkScanner.tsx:154-155` | Fast-failing scan can leave UI stuck "scanning" | S | Sub |
| FE-017 | Low | FE/Mon | `monitoring.rs:18-21,265-267` | Integer MB/s → network/disk charts read 0 below 1 MB/s | S | Sub |
| FE-018 | Low | FE | `RemoteManager.tsx:56-62,224-327` | JSX in state → stale closures | M | Sub |
| FE-019 | Low | FE | `DashboardView.tsx:100-111` | Topology double-click can SSH to first stored port (80/23) | S | Sub |
| FE-020 | Low | FE/a11y | 10 dialogs | No Escape/focus trap/labels on modals | M | Sub |
| FE-021 | Low | FE/a11y | ~35 labels | Sibling labels without `htmlFor` (incl. master-password form) | S | Sub |
| FE-022 | Low | FE/a11y | `SessionContainer.tsx:49`; `SshFileManager.tsx:283`; `NetworkScanner.tsx:316` | Click-only divs; keyboard users can't switch tabs/open folders | S | Sub |
| FE-023 | Low | FE/Types | ~40 hand-written TS mirrors of ~20 Rust structs | No generated bindings; drift proven (FE-006); recommend tauri-specta | M | Sub |
| FE-024 | Low | FE | `MonitoringView.tsx:202-244`; `useTailscaleStatus.ts:118-129` | 4 always-on metrics listeners; Tailscale subprocess every 30 s for app lifetime | S | Sub |
| FE-025 | Low | FE | `PreflightDialog`, `AlertRules`, `DiscoveryWidget`, `NetworkMapWidget`, `verifyHostKey`, clipboard toggle | ~900 lines dead UI; fake "Connected"/clipboard indicators | S | Sub |
| TEST-008 | Low | Test/Vault | `vault.rs:1075-1175` | EDW-15 test misses "gate released before key install" mutation | S | Sub (mutation reasoning) |
| TEST-009 | Low | Test | `crypto.rs`; `credentials.rs` | No negative AEAD tests (wrong key / tamper / nonce freshness) | S | Sub |
| TEST-010 | Low | Test | 4 FE + 6 Rust tests | Tests that assert nothing or only "invoke was called" | S | Sub |
| TEST-011 | Low | Test | `vite.config.ts`; 19 test files | No `clearMocks/restoreMocks`; order-dependent `mockResolvedValueOnce` chains | S | Sub |
| TEST-012 | Low | Test/FE | `useSshHostKeyVerification.ts:121-183,84-89` | "Uncovered" lines are dead `verifyHostKey`; live trust-once branch untested | S | Sub |
| TEST-013 | Low | Test | `vault.rs:982-983` | Lock-during-rekey test can pass vacuously on fast hardware | S | Sub |
| DEP-001 | Low | Dep | `mdns-sd 0.20.3` | LAN-triggerable panic in HINFO parsing kills discovery silently; fixed in 0.21.2 | S | Sub (crate src + changelog) |
| DEP-002 | Low | Dep | `Cargo.lock` chacha20 0.10.1, wnaf 0.14.0 | Two yanked crates; chacha20 SSE4.1 crash on old CPUs | S | Sub |
| DEP-003 | Low | Dep/CI | no `deny.toml` | No license/ban gate; ollama-rs missing license field | S | Sub |
| CI-004 | Low | CI | `ci.yml`, `release.yml`; `package.json` engines | CI uses npm 11 while engines require npm 12 | S | Sub |
| CI-005 | Low | CI | no `rust-toolchain.toml` | Floating stable + `-D warnings`; MSRV 1.95 never built | S | Sub |
| CI-006 | Low | CI | `ci.yml:3-7,66-70`; `release.yml` | No scheduled audit; release gate lacks npm/cargo audit; unpinned `cargo install` | S | Sub |
| CI-007 | Low | CI/Test | `package.json` | `@vitest/coverage-v8` missing → coverage fails on clean checkout; no thresholds | S | **Main** (baseline) |
| CI-008 | Low | CI | `release.yml:102-154`; `RELEASE_SIGNING.md` | Tag not validated as semver; empty Apple secrets; release restores master cache | S | Sub |
| CLEAN-004 | Low | Docs | `README.md:3-7`; `CODEBASE_AUDIT_REPORT.md`; `GEMINI.md:25-33` | Links to nonexistent DEFERRED_AUDIT_FIX_PLAN.md; false "not implemented" banner | S | Sub |
| CLEAN-005 | Low | Docs | `.claude/agents/security-reviewer.md`; `.codex/…toml` | Stale open-finding, wrong Argon2 floor (flags correct code HIGH), `G:\` path | S | Sub |
| CLEAN-006 | Low | Docs | `README.md` | False feature claims (SFTP progress, shadcn, historical data); embedded changelog | S | Sub |
| CLEAN-007 | Low | Docs | `CLAUDE.md` | Stale versions/counts; invalid migration syntax; self-contradictory export rule; 40 KB | M | Sub + recon |
| CLEAN-008 | Low | Docs | 9 docs | Committed merge-conflict markers | S | Sub |
| CLEAN-009 | Low | Docs | `conductor/`, `.windsurf/` | Dead planning system with wrong KDF params and live pointers | S | Sub |
| CLEAN-010 | Low | Docs | `.Jules/` vs `.jules/` | Case-split dir (latent collision on Windows/macOS) | S | Sub |
| CLEAN-011 | Low | Docs | `_archived/`, `test-workflow-*.json` | 250 KB dead code/fixtures | S | Sub |
| CLEAN-012 | Low | Rust | `src-tauri/test_salt_api.rs` | Never-compiled scratch file recommending the old salt bug (merges RUST-024, TEST-014) | S | Sub ×3 |
| CLEAN-013 | Low | Docs/Supply | `.gitignore`; `RELEASE_SIGNING.md` §1 | No ignore for updater private key, `.env`, coverage, OS junk; docs generate the key in-repo | S | **Main** (grep) |
| CLEAN-014 | Low | Docs | `SCHEMA.md:28`; `CORE_WORKFLOWS.md:18`; `RELEASE_SIGNING.md` | Wrong next-migration number; nonexistent UI; draft-release step undocumented | S | Sub |
| RSEC-016 | Info | Vault | `vault.rs:556-558,575,720-724` | `changing_password` reset not drop-guarded (theoretical); dead branch (merges RUST-027) | S | Sub ×2 |
| RSEC-017 | Info | Vault | `crypto.rs:47-66` | AES-GCM without AAD (bind to credential id/field on re-encrypt) | S | Sub |
| RSEC-018 | Info | IPC | `tailscale.rs:121-163` | `tailscale` resolved from PATH first | S | Sub |
| RSEC-019 | Info | SSH | `ssh.rs:69` etc. | `key_type` hard-coded; monitoring verifies by IP (fails closed) | S | Sub |
| IPC-012 | Info | IPC | `lib.rs:1206`; `errors.rs:14` | Lockout message sanitized away; some raw rusqlite errors leak (local only) | S | Sub |
| IPC-013 | Info | FE | `TerminalComponent.tsx:94-106` | xterm OSC 8 links → confirm + `window.open` (phishing only) | S | Sub |
| RUST-025 | Info | DB | schema | Never-written columns; redundant indexes | S | Sub |
| RUST-026 | Info | Rust | spawn sites | Unstoppable discovery thread; orphaned DNS blocking threads; zombie children | S | Sub |
| RUST-028 | Info | SSH | `ssh.rs:185-197,585` | `pending_connections` tombstones accumulate | S | Sub |
| FE-026 | Info | FE | build | 1.31 MB main chunk; only terminal is lazy | S | Sub + baseline |
| FE-027 | Info | FE | 5 large components | Split plan for CredentialManager/Settings/ScheduledTasks/Monitoring/RemoteManager | M | Sub |
| FE-028 | Info | FE | various | `DataSet<any>`, unchecked casts, 43 default exports vs CLAUDE.md rule | S | Sub |
| FE-029 | Info | Dep | `package.json`; `postcss.config.js` | Unused `@tauri-apps/plugin-opener` JS; redundant autoprefixer (merges CLEAN-018) | S | Sub ×2 |
| DEP-004 | Info | Dep | Cargo/npm | 5 cargo + 1 npm majors pending; ordering in §6 | M | Sub |
| DEP-005 | Info | Dep | `rsa 0.10.0-rc.18` | Dependabot #1 / RUSTSEC-2023-0071 not reachable as a decryption oracle; dismiss | S | Sub (crate src) |
| DEP-006 | Info | Dep | Tauri/GTK tree | Unmaintained/unsound transitive advisories, not reachable | S | Sub |
| CI-009 | Info | CI | `ci.yml` | No linter despite job name; no Dependabot config; no concurrency/timeouts | S | Sub |
| CLEAN-016 | Info | Docs | `index.html:5`, template SVGs | Vite favicon; unused template assets | S | Sub |
| CLEAN-017 | Info | Docs | `src-tauri/.cargo/config.toml` | Misleading comment on CARGO_TARGET_DIR precedence | S | Sub |
| CLEAN-019 | Info | Repo | remote branches | 5 merged/superseded branches; 1 unmerged Jules-connector branch needs a decision | S | Sub |
| CLEAN-020 | Info | Docs | repo root | 11 historical Feb reports at root | S | Sub |

**Merged duplicates (not counted separately):** RSEC-003→IPC-004 · RSEC-005→IPC-001 · RSEC-006→IPC-002 · RSEC-008→RUST-003+IPC-006 · RSEC-013→FE-001 · RUST-015→IPC-006 · RUST-019→RUST-012 · RUST-024→CLEAN-012 · RUST-027→RSEC-016 · RUST-029→RSEC-011 · IPC-005→RSEC-011 · IPC-009→RSEC-004+RSEC-014 · TEST-001→RSEC-004 (test kept in §5) · TEST-007→FE-006 · TEST-014→CLEAN-012 · FE-016→RUST-023 · CLEAN-015→RUST-002 · CLEAN-018→FE-029.

## 4. Detailed findings by severity

The full evidence, impact and fix text is in the partial file named by each ID's prefix: `RSEC`→02a, `IPC`→02b, `RUST`→03, `FE`→04, `TEST`/`DEP`/`CI`→05, `CLEAN`→06. This section adds the main agent's verification notes and the details that change how the fix should be done.

### Critical

**RSEC-001 — the vault verifier is the vault key.**
- **Evidence.** Initialization (`vault.rs:152-160`) stores `argon2.hash_password(master_password, &salt)` with `Params::new(47104, 2, 1, Some(32))` as `master_password_hash`. `vault.rs:194-214` (initialization) and `:387-408` (unlock) derive the key with `argon2.hash_password_into(master_password, salt_decoded_bytes, &mut [0u8; 32])`, using the same `Argon2` value. In argon2 0.5.3, `PasswordHasher::hash_password` (`src/lib.rs:572-597`) does `salt.decode_b64(&mut salt_arr)` and then `self.hash_password_into(password, salt_bytes, out)` with `output_len` = 32. So the base64 `$hash` field of the stored PHC string is the key, byte for byte. Rotation (`:593-623`) repeats the same pattern, and old verifiers can survive in free pages or the WAL.
- **Reproduced at runtime.** A scratch crate outside the repo mirrors `initialize_vault` with argon2 `=0.5.3`, `Params::new(47104,2,1,Some(32))` and Argon2id v0x13. It prints `hash field == master key: true` (`audit/raw/refresh/rsec001-proof.txt`). The in-repo regression test (`assert_ne!` of the stored hash bytes against the key) should be the first test Phase 7 writes, before any fix.
- **Fix outline.** First pin the current behaviour with TEST-005, a known-answer test on 0.5.3. Then derive one Argon2id output and use HKDF-expand to get `enc_key` (label "enc") and a verifier (label "verify"), and compare the verifier in constant time. Migrate on the next unlock: derive the legacy key, re-encrypt every credential in one transaction under the new key (optionally with AAD, RSEC-017), and write the v2 verifier. Then set `PRAGMA secure_delete=ON`, run `VACUUM` and a WAL checkpoint, and apply RSEC-009 permissions. Hold `credential_gate` exclusively for the migration, the same way `change_master_password` does.
- **User advisory.** Every existing export and `quasar.db.bak` should be considered equivalent to plaintext. Rotating the remote passwords stored in the vault is advisable if any copy has left the machine.

### High

**CI-001 — `release.yml` is invalid.**
- **Evidence.** GitHub doesn't provide the `secrets` context in `steps[*].if`, so the workflow file fails validation. Every push creates a failed `release.yml` run that is titled with the file path and has 0 jobs. That's the invalid-workflow signature, because a valid file with `on: push: tags: ['v*']` would never run on a branch push. The main agent confirmed it through the Actions API: 169 runs, all `failure`, including the pushes from this session (runs 167–169 on commits `0e803b0`, `22db17c`, `47198b7`). The line came in with `3593e66` (2026-08-26), and there are no tags.
- **Severity.** Kept at High even though the app isn't distributed yet. It blocks every release, the signed updater path has never been exercised end to end, and a permanent red check trains people to ignore CI.
- **Fix.** Map the secret into a job-level env (`HAS_WIN_CERT: ${{ secrets.WINDOWS_CERTIFICATE != '' }}`) and test `env.HAS_WIN_CERT == 'true'` in the step. Add `actionlint` to CI, and do a dry-run tag before relying on the updater. CI-008 lists the follow-up issues that will surface once the workflow actually runs.

### Medium (29)

These are grouped by fix area; each ID's partial file has the full text.
- **Vault state:** RSEC-002, RUST-003, IPC-004, RSEC-004, RUST-004. RSEC-004 needs the fix in `import_database`: take `credential_gate.write_owned()`, call `lock_vault()`, check `user_version` (RUST-004), and emit an event that forces re-unlock in the UI.
- **IPC hardening:** IPC-001, IPC-002, IPC-003, FE-001. The common fix is to stop trusting webview-supplied security-relevant values:
  - paths should come from dialogs opened in Rust;
  - host keys should come from the backend's own pending request;
  - credentials should be bound to their host;
  - SFTP should take `credential_id`.
- **SSH and scheduler reliability:** RUST-001, RUST-005, RUST-006, RUST-007, RUST-008.
- **Broken features:** RUST-002 together with FE-003 (alerting), FE-005 (cron), FE-004 (stale lists), FE-002 (error boundaries).
- **Supply chain / CI:** CI-002, CI-003, CLEAN-001.
- **Test gaps that gate risky work:** TEST-002, TEST-003, TEST-004, TEST-005, TEST-006.
- **Agent-instruction accuracy:** CLEAN-002, CLEAN-003.

### Low (69) / Info (21)

See the table and the partial files. None of them blocks a release. Most are S-effort items that ride along with the Medium batches in §8.

## 5. Test gap plan

Condensed from `05-tests-deps-ci.md` §"Prioritized tests to write", with the RSEC-001 proof test added. P0 has to land **before** the vault fix and before any argon2 or russh upgrade, because it pins behaviour those changes could silently break.

**P0 — vault, host keys, auth**
1. `vault::tests::verifier_is_not_key`. It fails today, and it's the RSEC-001 proof and regression test.
2. `vault::tests::kdf_known_answer` (TEST-005): a fixture PHC string, salt and password from a 0.5.3-era vault. Assert the derived key matches a pinned value and that a fixture credential decrypts. After RSEC-001 is fixed, add a second fixture that proves the legacy → v2 migration.
3. An import test through an extracted `import_database_at` (TEST-001 → RSEC-004 / RUST-004). Scenarios: the vault is locked after an import; a non-Quasar file leaves the live DB unchanged; a DB with `user_version` above the known migrations is rejected; a failed restore leaves a usable `.bak`.
4. `vault::tests::test_gate_held_until_new_key_installed` (TEST-008).
5. Make `VaultState::get_master_key` private (TEST-002). That's a compile-time guard, not a test.
6. Host-key tests (TEST-003):
   - `ssh_keys`: a `rejected` key isn't allowed; port isolation; a DB error fails closed (RUST-013 item 1).
   - A table test of the non-interactive decision.
   - `ssh::Client::check_server_key` with a mock app and paused time.
7. `ssh_auth` in-process russh server tests (TEST-004): `none` auth only when nothing was supplied, key→password fallback, and `rsa-sha2-256` selection.

**P1 — command layer and scheduler**
8. Add `tauri` with the `test` feature as a dev-dependency, and a `commands_tests` module (TEST-002). Scenarios: credential add/update queue behind a rotation; the launcher/RDP commands reject metacharacters; `add_scheduled_task` rejects an unknown `task_type` (IPC-011 / TEST-006).
9. Scheduler (TEST-006): with the vault locked, a run is recorded as failed and never connects; the double-run guard holds (RUST-005); FE and backend accept the same cron grammar (FE-005).
10. Negative AEAD tests (TEST-009).
11. A regression test for RUST-001: a handler that sleeps longer than the phase timeout still connects once it's approved.

**P2 — frontend contract and hygiene**
12. IPC fixture contract tests generated from the Rust structs, or tauri-specta bindings (FE-023 / FE-006).
13. The trust-once path in `useSshHostKeyVerification` (TEST-012). Delete `verifyHostKey`.
14. SettingsView import/export, TerminalComponent's `credentialId` payload, and ScheduledTasksView's exact payloads.
15. Global mock hygiene (TEST-011):
    - set `clearMocks`/`restoreMocks`;
    - add a dispatching `mockInvoke` helper that throws on unmocked commands;
    - add a global guard against `_` in `invoke` payload keys.
16. Replace the tests that assert nothing (TEST-010, RUST-021).

**P3:** `ssh_tunnel` (loopback bind only; dead-session detection, RUST-008), `ai.rs`, `health.rs`. Delete the dead commands rather than testing them.

## 6. Dependency upgrade batches (ordered)

| # | Batch | Contents | Depends on | Risk |
|---|---|---|---|---|
| D1 | Lockfile-only | `cargo update -p chacha20 --precise 0.10.2 -p wnaf --precise 0.14.1` (DEP-002); re-run `cargo audit` with a healthy index to finish the yanked check | none | Very low |
| D2 | Compatible, Tauri in lockstep | Rust: tauri 2.11.6, plugin-dialog 2.7.3, plugin-opener 2.5.5, plugin-updater 2.12.0, uuid, rand, log, surge-ping. npm: @tauri-apps/cli 2.11.5 and the **same** plugin versions, react/react-dom 19.3, vite 8.3, lucide-react, jsdom, tailwind-merge, testing-library patches. **JS and Rust plugin versions must land in one PR** | D1 | Low |
| D3 | mdns-sd 0.21.4 | Fixes the DEP-001 panic. The only breaking change is the default packet size. Also make `discovery.rs` exit on `Disconnected` | D2 | Low |
| D4 | dns-lookup 4 | One call site (`scanner.rs:135`); changelog UNVERIFIED | D2 | Low |
| D5 | vitest 5 + `@vitest/coverage-v8@5.0.1` | Adds the missing coverage devDependency (CI-007); 49 test files; changelog UNVERIFIED | D2, TEST-011 hygiene | Low–Med |
| D6 | argon2 0.6 / password-hash 0.6 | `Salt`/`SaltString`/`PasswordHash` moved to `phc`. **Only after** RSEC-001 is fixed on 0.5.3 and the KAT (TEST-005) passes both before and after | RSEC-001 fix, TEST-005 | **High** if done blind: it can orphan every vault |
| D7 | russh 0.63 + russh-sftp 3 | About 74 `russh::` references in files at 0–40% coverage; changelogs UNVERIFIED | TEST-003, TEST-004, RUST-001 fix | Med–High |
| — | Add `deny.toml` (DEP-003) and a Dependabot config (CI-009) | Can be done at any point, preferably with D1 | none | none |

Dependabot #1 (RUSTSEC-2023-0071): no upgrade is available. Dismiss it as tolerable risk (DEP-005) and re-check when russh moves past `rsa 0.10.0-rc.18`.

## 7. Cleanup plan

The full per-file classification from Phase 6 follows (80 rows covering all 144 tracked non-source files; counts: KEEP 23 · UPDATE 16 · MERGE 4 · ARCHIVE 12 · DELETE 25). "DELETE" means recoverable from git history.

| Path | Class | Reason (one line) | Evidence | Unique info lost if deleted? |
|---|---|---|---|---|
| `audit/` | KEEP | Output of this audit | EDW-17..26 | n/a |
| `.github/workflows/ci.yml` | KEEP | Live CI (content is reviewed in Phase 5) | runs on push/PR to master | n/a |
| `.github/workflows/release.yml` | KEEP | Live release pipeline (Phase 5 content) | `v*` tag trigger | n/a |
| `.gitignore` | UPDATE | Missing entries | CLEAN-013 | n/a |
| `.nvmrc` | KEEP | Node pin 24.15.0, matches `engines` and CI | `package.json` engines | n/a |
| `.vscode/extensions.json` | KEEP | Whitelisted in .gitignore, useful | `.gitignore` `!.vscode/extensions.json` | n/a |
| `LICENSE` | KEEP | MIT, referenced by README and package.json | `package.json` `"license": "MIT"` | n/a |
| `README.md` | UPDATE | Stale banner, false feature claims, embedded changelog | CLEAN-004, CLEAN-006 | n/a |
| `SECURITY.md` | KEEP | Accurate disclosure policy and accepted-risk record | matches `src-tauri/.cargo/audit.toml` | n/a |
| `CLAUDE.md` | UPDATE | Canonical agent file, but 21 fake commands, stale versions, history sections | CLEAN-002, CLEAN-007 | n/a |
| `AGENTS.md` | MERGE into CLAUDE.md + `CHANGELOG.md` + `docs/ARCHITECTURE.md`, then stub | 130 KB history log with stale instructions | CLEAN-003 | Yes, if deleted outright: the Sep 2026 design rationale (credential gate, claim_scan, lockout persistence). See the keep map below. |
| `GEMINI.md` | DELETE | Stale status and "next session" pointing at a missing plan; duplicates AGENTS.md | GEMINI.md:25-33 | No: every section is a subset of AGENTS.md entries of the same dates |
| `CODEBASE_AUDIT_2026-02-01.md` | ARCHIVE | Feb 1 audit, since superseded ("SSH server key validation disabled" etc. is now fixed) | host-key verify in `ssh.rs` (recon §4.2) | Yes: historical record of the Feb findings |
| `IMPLEMENTATION_SUMMARY_2026-02-01.md` | ARCHIVE (next to the Feb 1 audit) | Fix log for the Feb 1 audit | "Status: COMPLETED" | Minor: fix-by-fix notes, also in AGENTS.md:1679-1747 |
| `CODEBASE_AUDIT_REPORT.md` | ARCHIVE | Feb 18 audit; all items closed; preamble links a missing plan | CLEAN-004; "all five are fixed" note in the file | Yes: AUD-01..07 history |
| `code-review-projecttitan-8fd249.md` | ARCHIVE | Feb 2 review under the old project name, with conflict markers | CLEAN-008 | Yes: the Feb 2 review record |
| `MONITORING_COMPLETE.md` | ARCHIVE | Monitoring feature report (Feb 2) | Its usage example queries history the UI never calls (recon §3.2) | Minor: design rationale (5 s sample, 30-day retention, 300 s cooldown) |
| `MONITORING_PHASE1_COMPLETE.md` | DELETE | Superseded by MONITORING_COMPLETE.md | covered by MONITORING_COMPLETE §Phase 1 | No: field list is in `monitoring.rs` |
| `MONITORING_PHASE2_COMPLETE.md` | DELETE | Superseded, has conflict markers | CLEAN-008; schema is in `migrations/004_monitoring.sql` | No |
| `MONITORING_PHASE3_COMPLETE.md` | DELETE | Superseded | cooldown/recovery live in `monitoring.rs` | No |
| `MVP_COMPLETION_ROADMAP.md` | ARCHIVE | "~90% complete, Feb 18" roadmap; still linked from README:325 | file header | Minor: post-MVP idea list (Phases 8-11), which belongs in Linear |
| `migration-options-comparison-5460b3.md` | ARCHIVE | Decision record; option 3A was chosen | `Cargo.toml:57` `rusqlite_migration = "2.6.0"` | Yes: why rusqlite_migration was chosen, and the 005 idempotency incident |
| `test-workflow-basic-health-check.json` | DELETE | Fixture for the removed workflow engine | CLEAN-011 | No |
| `test-workflow-conditional-disk-check.json` | DELETE | Same | CLEAN-011 | No |
| `index.html` | UPDATE | Favicon is the Vite template logo | CLEAN-016 | n/a |
| `package.json` | KEEP | Build config (engines vs CI npm mismatch is Phase 5) | recon §1 | n/a |
| `package-lock.json` | KEEP | Lockfile | `npm ci` | n/a |
| `postcss.config.js` | UPDATE | Drop redundant autoprefixer | CLEAN-018 | n/a |
| `tsconfig.json` / `tsconfig.node.json` | KEEP | Used by `tsc && vite build` | `package.json` scripts | n/a |
| `vite.config.ts` | KEEP | Used; port 1420 pinned | `vite.config.ts:18-19` | n/a |
| `public/tauri.svg` | DELETE | Unreferenced template asset | grep: 0 references | No |
| `public/vite.svg` | DELETE (after the index.html fix) | Template favicon | `index.html:5` | No |
| `src/assets/react.svg` | DELETE | Unreferenced template asset | grep: 0 references | No |
| `src/assets/quasar-logo.svg` | KEEP | Used | `Sidebar.tsx:15` | n/a |
| `src/App.css` | KEEP | Used (light-theme overrides) | `App.tsx:2` | n/a |
| `scripts/tauri-dev.js` | KEEP | Used by `npm run tauri`; accurate | `package.json` `"tauri"` | n/a |
| `docs/SCHEMA.md` | UPDATE | Stale "next = 013", incomplete table list | CLEAN-014 | n/a |
| `docs/CORE_WORKFLOWS.md` | UPDATE | Mostly accurate; one false claim | CLEAN-014 (`alerts-recovered` → Recent Activity **is** true: `AlertFeed.tsx:110`) | n/a |
| `docs/RELEASE_SIGNING.md` | UPDATE | Accurate on secrets; missing the draft-release step; key path inside repo | CLEAN-013, CLEAN-014 | n/a |
| `docs/AUDIT_PLAN.md` | ARCHIVE | Feb 28 plan; header says "historical"; references removed `db.ts` | AUDIT_PLAN.md:3-12, :270 | Yes: the Feb plan record |
| `src-tauri/Cargo.toml` / `Cargo.lock` / `build.rs` | KEEP | Build | n/a | n/a |
| `src-tauri/tauri.conf.json` | KEEP | Build (CSP and updater content is Phase 2B/5) | n/a | n/a |
| `src-tauri/capabilities/default.json` | KEEP | Only capability file | recon §2 | n/a |
| `src-tauri/.cargo/audit.toml` | KEEP | Documented accepted risk | SECURITY.md | n/a |
| `src-tauri/.cargo/config.toml` | UPDATE | Misleading comment | CLEAN-017 | n/a |
| `src-tauri/.gitignore` | KEEP | Correct for target/, gen/schemas, test DBs | file | n/a |
| `src-tauri/icons/` (17 files) | KEEP | 5 referenced by `bundle.icon`; Square*/StoreLogo are the standard `tauri icon` set | `tauri.conf.json` bundle.icon | n/a |
| `src-tauri/migrations/` (13 files) | KEEP | Append-only; all registered | `lib.rs:104-125` | n/a |
| `src-tauri/test_salt_api.rs` | DELETE | Never-compiled scratch file | CLEAN-012 | No |
| `.claude/settings.local.json` | DELETE (untrack; keep locally) | Personal permissions file | CLEAN-001 | No |
| `.claude/agents/security-reviewer.md` | UPDATE | Stale open-finding item, wrong Argon2 floor, hardcoded `G:\` path | CLEAN-005 | n/a |
| `.claude/agent-memory/security-reviewer/MEMORY.md` | UPDATE | Header says "open credential-write/rekey race" (:5) | EDW-15 | n/a |
| `.claude/agent-memory/security-reviewer/patterns.md` | UPDATE | Mar 2 findings list, partly fixed; keep the claim_scan section (:1-66) | file headings | n/a |
| `.claude/agent-memory/security-reviewer/vault_rs_patterns.md` | UPDATE | Referenced by CLAUDE.md item 9; fix the :87 MiB claim | CLEAN-005 | n/a |
| `.codex/agents/security-reviewer.toml` | DELETE (or regenerate from `.claude`) | Diverged older copy | CLEAN-005 | No: strict subset of the `.claude` version |
| `.jules/bolt.md` | KEEP | Active Jules persona journal | last commit 2026-09-24 | n/a |
| `.jules/sentinel.md` | KEEP | Active Jules persona journal | last commit 2026-09-24 | n/a |
| `.Jules/palette.md` | MERGE into `.jules/palette.md` | Case-split directory | CLEAN-010 | No |
| `.windsurf/workflows/plan_track.md` | DELETE | Drives the dead conductor/GEMINI flow | CLEAN-009 | No |
| `_archived/automation_20260202/README.md` | ARCHIVE → `docs/archive/automation-removal-2026-02.md` | Why and what was removed | CLEAN-011 | Yes (small): removal rationale |
| `_archived/automation_20260202/REMOVAL_SUMMARY.md` | MERGE into the archived README above | Same topic | CLEAN-011 | No, once merged |
| `_archived/automation_20260202/backend/` (3 .rs) | DELETE | Dead code, not built | CLEAN-011 | No: in git history (e.g. 97dd84a) |
| `_archived/automation_20260202/frontend/` (12 .tsx) | DELETE | Dead code, not built | CLEAN-011 | No: in git history |
| `_archived/automation_20260202/plans/` (5 .md) | DELETE | Plans for a removed feature | CLEAN-011 | Minor: design notes for an abandoned feature, in git history |
| `conductor/index.md`, `tracks.md`, `setup_state.json` | DELETE | Conductor scaffolding, stale | CLEAN-009 | No |
| `conductor/tech-stack.md` | DELETE | Lists crates and sidecars that don't exist | CLEAN-009 | No |
| `conductor/workflow.md` | DELETE | Unfilled template | CLEAN-009 | No |
| `conductor/product.md` | ARCHIVE | Original product vision (RDP/VNC, sidecars) | file | Yes: vision statement and success metrics (<2 s startup, <100 MB idle) |
| `conductor/product-guidelines.md` | ARCHIVE | UX principles | file | Yes (small): design principles; could be lifted into `docs/ARCHITECTURE.md` |
| `conductor/design_concept.pdf` (459 KB, largest tracked binary) | ARCHIVE | 6-page "SysAdmin Nexus" concept doc; unreferenced | PDF metadata title | Yes: early UI concept |
| `conductor/code_styleguides/typescript.md` | UPDATE (move to `docs/style/`) | Referenced by CLAUDE.md:414; its "named exports only" rule contradicts the codebase | CLEAN-007 | n/a |
| `conductor/code_styleguides/general.md` | UPDATE (move to `docs/style/`) | Short general rules | README:204 | n/a |
| `conductor/code_styleguides/html-css.md` | UPDATE (move to `docs/style/`) | Referenced by README:206 | n/a | n/a |
| `conductor/code_styleguides/javascript.md` | DELETE | No hand-written JS besides `scripts/` and configs; TS guide covers it | README:205 mislabels it | No |
| `conductor/tracks/asset_discovery_monitoring_20260130/` (4 files) | DELETE | Implemented (scanner, discovery, health all exist); conflict markers | CLEAN-008; `scanner.rs`, `discovery.rs`, `health.rs` | No |
| `conductor/tracks/automation_canvas_20260130/plan.md` | DELETE | Feature removed Feb 2 | REMOVAL_SUMMARY.md | No |
| `conductor/tracks/monitoring_alerts_20260130/plan.md` | DELETE | Implemented | `monitoring.rs` | No |
| `conductor/tracks/security_credential_vault_20260130/{index,plan,spec}.md` | DELETE | Implemented (vault, credentials, known hosts, audit) | `vault.rs`, `vault/*.rs` | No |
| `conductor/tracks/security_credential_vault_20260130/security-model.md` | MERGE into new `docs/SECURITY_MODEL.md` (fix params) | The repo's only threat model | CLEAN-009 (wrong Argon2 params :94-97) | **Yes**: threat actors, attack vectors, and does/doesn't-protect lists |
| `conductor/archive/` (4 track dirs, 12 files) | DELETE | Completed Jan tracks; 2 have conflict markers | CLEAN-008 | No |

### Target doc structure

```
CLAUDE.md               ≤15 KB, current state only (session start, real command list, repo map,
                        conventions, security invariants as rule + regression-test name, constraints)
AGENTS.md               3-line stub → CLAUDE.md (Codex/Jules read this name; no symlink, Windows)
README.md               verified features, install/dev/build; no changelog
CHANGELOG.md            NEW: dated one-liners from README "Recent Updates" + AGENTS.md history
SECURITY.md             unchanged
docs/ARCHITECTURE.md    NEW: generated IPC list, events, managed state, vault lock model,
                        scan-claim pattern, ssh_connect phases, DB access rule
docs/SECURITY_MODEL.md  NEW: from conductor security-model.md, params corrected
docs/SCHEMA.md          full column tables; "next migration = highest + 1"; hook pattern for ADD COLUMN
docs/CORE_WORKFLOWS.md  minor fixes
docs/RELEASE_SIGNING.md draft-release step; key generated outside the repo
docs/style/             typescript.md (export rule reconciled), general.md, html-css.md
docs/archive/           Feb audits, roadmap, migration decision, AUDIT_PLAN, product vision,
                        design_concept.pdf, automation-removal-2026-02.md
```

Removed from the tree: GEMINI.md, `.codex/`, `.windsurf/`, `conductor/` (after the moves), `_archived/` (after a one-page summary), the root report files, `test-workflow-*.json`, `test_salt_api.rs` and the template SVGs. The AGENTS.md sections worth keeping, and where each one goes, are mapped in `06-cleanup.md` §"AGENTS.md: what to keep and where". The CLAUDE.md fix list (11 items) is in the same file.

**Remote branches (list only).** Five are merged or superseded and can be deleted: `bolt/react-derived-state-…`, `bolt/optimize-date-formatting-…`, `palette/alertfeed-a11y-…`, `palette/improve-search-a11y-…` and `claude/nifty-pasteur-36u236`. `claude/google-jules-connector-v8hhwq` is unmerged, has no PR and adds a `.mcp.json` that reads `JULES_API_KEY`, so it needs Edward's decision. `claude/quasar-audit-cleanup-3srz1j` is superseded by this branch once the audit lands. **Nothing has been deleted.**

## 8. Recommended fix order (Phase 7 batches)

Phase 7 rules (EDW-26): one agent, sequential, one logical change per commit, with the repo's fast checks run before every push. Each batch should be its own PR.

| Batch | Contents | Why here | Size |
|---|---|---|---|
| **P7-0: Stop-the-bleeding hygiene** | CLEAN-001 (untrack `.claude/settings.local.json`, which stays on disk locally) + CLEAN-013 `.gitignore` (updater key, `.env`, `coverage/`, OS junk) + RELEASE_SIGNING key path; CI-001 (fix `release.yml` `if:`) + `actionlint` | Minutes of work. Removes a supply-chain gap and the permanent red check | S |
| **P7-1: Vault key separation (Critical)** | P0 tests 1, 2, 5 first → RSEC-001 KDF split + v2 migration + `secure_delete`/VACUUM → RSEC-009 file perms + `.bak` cleanup → RSEC-017 AAD during the re-encrypt (optional) | The only Critical. Everything else in the vault builds on the new format | M |
| **P7-2: Vault state correctness** | RSEC-004 + RUST-004 import (gate, lock, `user_version`), RSEC-002 background activity, RUST-003 + IPC-006 settings load/validate, IPC-004 (recommend: remove the toggle), RSEC-012, RSEC-016, RSEC-011 audit events, TEST-008 | Same files as P7-1; closes the EDW-15 residual | M |
| **P7-3: IPC hardening** | IPC-002 (trust by `request_id`) + RSEC-007 changed-key UX, IPC-001 (Rust-side dialogs), IPC-003 (credential↔host binding), FE-001 + RSEC-013 (SFTP `credential_id`), IPC-007 CSP → `devCsp`, IPC-008 capabilities, IPC-010 remove dead commands, IPC-011 task validation/audit | Closes webview → RCE/exfil chains before any future XSS | M–L |
| **P7-4: SSH & scheduler reliability** | RUST-001 (+ test), RUST-005, RUST-006, RUST-007, RUST-008, RUST-009, RSEC-015, plus P0 tests 6–7 and P1 test 9 | User-visible failures; tests unblock D7 | M |
| **P7-5: Broken features** | Alerting (RUST-002 migration 015 + mount FE-003), FE-005 cron, FE-006 known-hosts dates, FE-004 stale lists, FE-002 per-view boundaries, FE-011, FE-008, FE-013, FE-017, RUST-023 | Features advertised but not working | M |
| **P7-6: CI & supply chain** | CI-002, CI-003, CI-004, CI-005, CI-006, CI-007, CI-008, DEP-003 `deny.toml`, CI-009 Dependabot config | Needs CI-001 fixed first to see the release path | S–M |
| **P7-7: Dependencies** | D1 → D2 → D3 → D4 → D5, then D6 after P7-1 and D7 after P7-4 | Order in §6 | M |
| **P7-8: Docs & repo cleanup** | CLAUDE.md rewrite (CLEAN-002/007, RUST-018), AGENTS.md stub + CHANGELOG (CLEAN-003), README (CLEAN-004/006), the §7 moves/deletes, CLEAN-005, CLEAN-010, CLEAN-012 | Can run any time, but after P7-1 to P7-3 so the docs describe the fixed state | M |
| **P7-9: Refactors (optional)** | RUST-020 module splits, FE-027 component splits, FE-023 tauri-specta, FE-020/021/022 a11y, remaining Low/Info | No risk reduction on its own; do it last | L |

**Recommendation for which batches go first:** approve **P7-0 and P7-1** now. P7-0 is trivial and safe. P7-1 is the only Critical: it changes the on-disk vault format, so it gets its own PR with the KAT fixtures reviewed before merge. P7-2 and P7-3 should follow directly, because they touch the same vault and IPC code.

## 9. Verification log

**Personally verified by the main agent** (file:line re-opened or reproduced during EDW-19..25):

| ID | How | Result |
|---|---|---|
| RSEC-001 | Read `vault.rs:140-220, 380-412`, then argon2 0.5.3 `src/lib.rs:570-592` in `~/.cargo/registry`; **reproduced** with a scratch crate using the same params (`raw/refresh/rsec001-proof.txt`) | **Confirmed**: the stored PHC hash field equals the AES key byte for byte |
| CI-001 | `release.yml:118-126`, plus the GitHub Actions API `list_workflow_runs release.yml` | **Confirmed**: 169 runs, all `failure`, 0 jobs, including runs on this branch's pushes |
| RSEC-002 | `vault.rs:474-500` (`get_master_key` → `update_activity`); `lib.rs:998`; `SystemHealthWidget.tsx:110-117` (`useVisiblePolling(…, 30000)`) | Confirmed |
| RSEC-004 | `lib.rs:1726-1800`: no reference to the vault, gate or lock | Confirmed |
| IPC-001 | `validation.rs:140-160` | Confirmed: only empty, NUL and `..` are rejected |
| RUST-001 | `ssh_connect.rs:78-95` (timeout wraps `connect_stream`); `ssh.rs:111` (120 s) vs `ssh.rs:258`, `ssh_tunnel.rs:171` (10 s) | Confirmed structurally; not reproduced at runtime |
| RUST-002 / FE-003 | `monitoring.rs:434,443` (`Vec::new()`); `grep alert_rules migrations/` → none; `grep AlertRules src` (non-test imports) → none | Confirmed |
| FE-006 | `KnownHostsManager.tsx:13-14,180-185` vs `ssh_keys.rs:42,48-49` | Confirmed |
| RSEC-009 | `grep set_mode\|PermissionsExt\|secure_delete src-tauri/src` → none | Confirmed |
| CLEAN-001 / CLEAN-013 | `git ls-files .claude/settings.local.json` (tracked); read lines 10-30; `grep settings.local .gitignore` → none | Confirmed |
| CI-007 | Baseline `raw/vitest-coverage.txt` (MISSING DEPENDENCY) | Confirmed |
| Baseline | Re-ran tsc, vitest, clippy and cargo test on the audited tree (§2) | Green |

**Severity changes and merges:**
- **IPC-002 / RSEC-006.** 2A rated it Low and 2B rated it Medium. Kept at **Medium**, because together with IPC-003 it turns webview trust into a silent MITM on the unattended SSH paths.
- **RSEC-008 (Low) was split.** The load bug goes to RUST-003 (Medium), because a security control silently diverges from the UI after every restart. The validation and overflow part goes to IPC-006 (Low).
- **RSEC-003 / RSEC-005 / RSEC-013.** Merged into the 2B or FE IDs at the same severity.
- **TEST-001 (Medium)** is merged into RSEC-004. The test itself stays in §5.
- **CLEAN-015 (Low, doc)** is merged into RUST-002 (Medium, code).
- **CI-001 stays High** although the app has no users yet. The rationale is in §4.
- **Nothing was dropped.** No Critical or High finding failed verification.

**Corrections to earlier phases:**
- **Recon §4.1** ("rekey race CONFIRMED open") is superseded. EDW-15 landed after recon. Phases 2A, 2B, 3 and 5 each traced every credential call site through `credential_gate`, and the only gap is `import_database` (RSEC-004).
- **Recon §3.3** ("`alerts-recovered` has no listener") is wrong. `AlertFeed.tsx:110` and `SystemHealthWidget.tsx:89` both listen. The event just never fires (FE-003).
- **The 01-baseline data was recorded at `fcc58d2`**, before EDW-15. §2 re-runs the gates on the audited tree.

**Still UNVERIFIED (carried into Phase 7):**
- (RSEC-001 was UNVERIFIED at runtime in Phase 2A; it is now reproduced, see the verification table above.)
- RUST-009 frequency on real servers.
- IPC-007: whether Tauri adds its own IPC origins once the localhost wildcards are removed.
- IPC-013: `window.open` behaviour.
- The Tauri bundler's handling of empty `APPLE_*` secrets (CI-008).
- Changelogs for russh 0.63, russh-sftp 3, dns-lookup 4 and vitest 5.
- The repo's default `GITHUB_TOKEN` scope (CI-002).
- Which Dependabot alert #1 is (API 403; identified by elimination).
- FE-016's stream-end frequency.
- The per-library bundle split (FE-026).

---

## 10. Changes made (Phase 7, EDW-26)

Approval: Edward approved **P7-0 and P7-1** on 2026-09-24 (recorded on EDW-25). P7-2 to P7-7 followed in the same work, reported batch by batch on EDW-26, and on 2026-09-25 he approved all of P7-8 ("Approve all"). P7-9 (optional refactors) hasn't started. The table below details P7-0/P7-1; "Later batches" lists the rest. Branch `claude/stoic-einstein-j4ajfe`. Raw outputs are in `audit/raw/phase7/`.

### Commits

| Commit | Batch | Finding(s) | Change |
|---|---|---|---|
| `eee12f3` | P7-0 | CLEAN-001, CLEAN-013 | Untracked `.claude/settings.local.json` (the file stays on disk). `.gitignore` gains that file plus `*.key`/`*.key.pub`, `.env*`, coverage output and Windows junk. |
| `2f9e6a7` | P7-0 | CI-001 | `release.yml`: `secrets` in the step `if:` replaced by a job-level `HAS_WINDOWS_CERTIFICATE` env. New `workflow-lint` job in `ci.yml` runs actionlint 1.7.7 (checksum-pinned). RELEASE_SIGNING now generates the key in `~/.tauri/`. |
| `12a7355` | P7-1 | RSEC-001, TEST-005 | New `vault/kdf.rs`: Argon2id → HKDF-SHA256 into the encryption key (memory only) and a verifier (stored). v1 vaults migrate on unlock under a fresh salt, followed by VACUUM and a checkpoint. `unlock_vault` takes the credential gate. Password change writes v2 and refuses when the key doesn't match the DB. KATs pin the KDF. |
| `d2d2eff` | P7-1 | RSEC-009 | `secure_delete = ON` on every connection. The DB, WAL and SHM files are 0600, the app data dir 0700, and exports 0600 (Unix). |
| `57e109a` | P7-1 | RSEC-001 follow-up (security review) | Post-commit steps are best-effort, so the vault can't fall back to a stale key. Only AEAD-failing rows are skipped, and `legacy_salt` is kept for them. Malformed key blobs are an error rather than being silently wiped. The busy result of the WAL checkpoint is honoured, with `kdf_scrub_pending` retry. `.bak` loses the legacy hash. Stored hex parsing is strict. |

### Later batches

Each commit message carries the finding ids and what was verified.

| Batch | Commits (oldest → newest) |
|---|---|
| P7-2 vault state | `6f84c2a`, review follow-up `93a6517` |
| P7-3 IPC hardening | `6b2a1f5` `5e6fc25` `127cd93` `b4a3b5f` `39fe520` `6687871` `3498f10` `d8d9ad5` |
| P7-4 SSH & scheduler | `1b8da8d` `6a9b813` `15ce19d` `c1bb824` `91dc471` `6f687a4` `a7df035`, review follow-ups `f03ce5e` `21a3794` `79dde75` `95bba5d` `906597c` `279de83` `fb533d7` |
| P7-5 broken features | `44b1321` `904f1d2` `046fc05` `41f2c21` `75f4b4d` `be6e962` `6cbbd56` `ccd9f45` `05ad847` `61a0901` `06191ec` |
| P7-6 CI & supply chain | `3c99613` `45a35c1` `56398de` `9b7a82a` `272670c` `81a313c` `22409b8` |
| P7-7 dependencies | D1 `773368b`, D2 `35bbd74`, D3 `4bdd56e`, D4 `e6fa0ff`, D5 `1ed3032`, D6 `628449d`, D7 `58a3596` |
| P7-8 docs & cleanup | `150b8bc` (archive/moves, CLEAN-008/010/016/017, FE-029), `3733af4` (CLEAN-005), `c36207d` (SCHEMA/workflows/threat model, RUST-018, CLEAN-014), `5a0da03` (CLAUDE.md, ARCHITECTURE, AGENTS stub, CHANGELOG, README; CLEAN-002/003/004/006/007) |

**Not done in P7-8:** deleting the superseded files (GEMINI.md, `conductor/` remainder, `_archived/`, `.codex/`, `.windsurf/`, the three MONITORING_PHASE reports, `test-workflow-*.json`, `public/tauri.svg`, `public/vite.svg`, `src/assets/react.svg`, `src-tauri/test_salt_api.rs`: CLEAN-009, CLEAN-011, CLEAN-012, the rest of CLEAN-016). The session's permission policy blocked the `git rm`; it needs a human-run or explicitly allowed commit. Remote branch cleanup (CLEAN-019) is also left.

### Verification

- **RSEC-001.** The new test `test_stored_vault_settings_never_contain_master_key` failed on the old code with `master_password_hash PHC hash field is the master key` (`raw/phase7/rsec001-test-before-fix.txt`) and passes now.
- **Mutation checks.** Each of these mutations makes the named test fail:
  - removing VACUUM fails the migration scrub test;
  - restoring `?` on the post-commit audit fails both post-commit tests;
  - removing the gate from `unlock_vault` fails the gate test.
- **KATs.** The Argon2 output matches the standalone reproduction. The HKDF outputs were computed independently in Python.
- **CI-001.** actionlint flags the old `release.yml:123` (`context "secrets" is not allowed here`) and passes the new workflows, including shellcheck.
- **Independent review.** A read-only reviewer subagent reviewed P7-1 before its follow-up commit. It found one blocker and six should-fix items. Everything in P7-1 scope was fixed in `57e109a`. Items left open:
  - `import_database` doesn't gate or lock the vault. That is RSEC-004 in P7-2, which isn't approved yet.
  - Two nits: legacy unlock runs Argon2 two or three times (once per vault, only during migration), and pre-existing unzeroized stack copies of the key (RSEC-010).

### Before / after

| Metric | Before (`97dd84a`, EDW-25 re-run) | After (`57e109a`) |
|---|---|---|
| npm vulnerabilities | 0 | 0 |
| cargo audit vulnerabilities (unignored) | 0 (1 ignored: RUSTSEC-2023-0071) | 0 (same ignore); 8 allowed warnings, unchanged. The yanked check is still hitting registry 503s. |
| Clippy `--all-targets -D warnings` | 0 | 0 |
| Rust tests | 132 lib + 4 integration | **148 lib** + 4 integration |
| Frontend tests | 351 | 351 |
| RS line coverage | 52.60% (baseline, `fcc58d2`) | **55.51%** (`vault.rs` 85.19% → 89.78%, `vault/kdf.rs` 93.55%, `db.rs` 90.91%) |
| FE line coverage | 72.83% | 72.88% (no frontend change) |
| Main JS chunk | 1,313.95 kB (baseline) / 1,314.05 kB (97dd84a) | 1,314.05 kB (unchanged) |
| Tracked files | 273 at `97dd84a`; 319 with `audit/` | 319: `settings.local.json` removed, `vault/kdf.rs` added |
| Open Critical / High | 1 / 1 (RSEC-001, CI-001) | **0 / 0** |

### Advisories for Edward

- **Treat every export and every `quasar.db.bak` made before this fix as plaintext-equivalent.** The migration fixes the live DB and strips the hash from the `.bak` in the app dir. It can't reach copies elsewhere, such as backups or exports on other disks. Consider rotating remote passwords that were stored in the vault if a copy may have left the machine.
- **Pulling `eee12f3` on another clone deletes that clone's `.claude/settings.local.json`,** because git removes files that the pulled commit untracks. Back it up before pulling on the Windows machine if you want to keep it.
- **`release.yml` now parses, so the next `v*` tag will actually run it for the first time.** Expect the CI-008 follow-ups to surface: tag validation, empty Apple secrets and the release cache.

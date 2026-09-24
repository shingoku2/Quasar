# Phase 6 — Repo cleanup inventory (Linear EDW-24)

Date: 2026-09-24 · Audited commit: `97dd84a` (master). Baseline data (`audit/00-recon.md`, `audit/01-baseline.md`, `audit/raw/*`) was recorded at `fcc58d2`. Nothing in this phase depends on the `fcc58d2..97dd84a` code drift except CLAUDE.md line numbers, which are cited against the current tree.
Read-only phase: the only file written is this one. No files were moved or deleted and no branches were touched.

## Scope & method

- **Enumeration:** `git ls-files` returned 298 tracked files. After excluding `src/**/*.ts(x)`, `src-tauri/src/**`, `src-tauri/tests/**` and `audit/**`, 144 files remain, and every one is classified below, grouped where a directory's contents are uniform. Each file's size and last commit came from `stat` and `git log -1 -- <path>`.
- **References:** each candidate was grepped repo-wide (excluding `.git`, `audit`, `node_modules`) to see whether the build, tests or another doc point at it. The build inputs checked were `tsconfig.json` (`include: ["src"]`), `tsconfig.node.json`, `vite.config.ts`, `postcss.config.js`, `index.html`, `src-tauri/Cargo.toml`, `build.rs`, `tauri.conf.json` and both workflows.
- **Doc claims checked against code:** README feature list, CLAUDE.md (command list diffed mechanically against `generate_handler!` at `src-tauri/src/lib.rs:1957`; versions against `Cargo.toml`/`Cargo.lock`/`package.json`), `docs/SCHEMA.md` vs `src-tauri/migrations/` + `lib.rs:104-125`, `docs/CORE_WORKFLOWS.md` vs components, `docs/RELEASE_SIGNING.md` vs `release.yml`, and the conductor security model vs `crypto.rs`.
- **Large AI files:** AGENTS.md (130 KB) and GEMINI.md (12.5 KB) were never loaded whole. I grepped headings (`grep -n '^#'`) and read only the tail sections (AGENTS.md:1842-2030).
- **Merge-conflict scan:** `git grep -nE '^(<<<<<<<|>>>>>>>) '`.
- **Branches:** `git ls-remote origin` (read-only). Local refs came from `git log`/`git merge-base --is-ancestor`. For SHAs missing from this shallow clone, dates and PR state came from the GitHub API (read-only `get_commit`, `list_pull_requests`).
- **Not run:** `npx knip` (no `node_modules`; installing is forbidden) and `gitleaks` (not installed). Unused assets were found by grep instead. I scanned the tracked non-source files by eye for secrets. The only key material is the updater **public** key in `tauri.conf.json`, which is intended. `.claude/settings.local.json` contains no credentials.
- **Threat model:** this is repo hygiene for a single-maintainer desktop app. The realistic harms are (a) coding agents (Claude/Codex/Jules/Gemini, several of them active and bot-committing here) acting on wrong or stale instructions or on over-broad pre-approved permissions, (b) accidentally committing a secret, and (c) contributor confusion. Nothing here is directly exploitable in the shipped app, so nothing is rated above Medium.

## Findings

| ID | Severity | file:line | Evidence | Impact | Fix | Effort |
|---|---|---|---|---|---|---|
| CLEAN-001 | Medium | `.claude/settings.local.json:15,28,10,14,37` | The file is **tracked** (last commits 223a4e5, 44cda63, 1d1caea) and pre-approves `Bash(python -)`, `Bash(python -c ' *)`, `Bash(npm update *)`, `Bash(cargo update *)`, `Bash(git fetch *)`, two one-off `sed -i` rewrites of source files (:22-23), and `mcp__Desktop_Commander__read_file` (:4). `.gitignore` has no entry for it: `*.local` only matches names *ending* in `.local`. | Every clone's Claude Code session runs arbitrary Python and dependency updates **without a permission prompt**. A prompt-injected agent (e.g. from SSH output, a bot PR or a fetched page) skips the permission gate. The file is also personal machine state that churns in diffs. | `git rm --cached .claude/settings.local.json`, then add `.claude/settings.local.json` to `.gitignore`. If shared permissions are wanted, commit a reviewed minimal `.claude/settings.json` with no interpreter wildcards. | S |
| CLEAN-002 | Medium | `CLAUDE.md:217-273` (IPC list), `:105`, `:316-318`, `:455`, `:463` | A mechanical diff of backticked command names against `generate_handler!` (`lib.rs:1957`) finds **21 non-existent commands**: `add_host close_ssh_session close_tunnel create_tunnel execute_ssh_command export_vault get_ssh_known_hosts import_vault list_alert_rules list_hosts list_tunnels remove_credential remove_discovered_host remove_host remove_known_host save_discovered_host send_ai_message start_network_scan start_ssh_session update_alert_rule update_host`. The same file also cites things that don't exist: `src/db.ts` (:105), the tables `alert_rules`/`scheduled_task_runs`/`system_metrics_history` (:316-318; the real ones are `metrics_history`, `alert_history`, `monitoring_host_credential`; alert rules are in-memory, `monitoring.rs:434-443`), and a `CredentialSelector filterType="password_only"` prop (:455, :463; the real prop is `allowedTypes`, used as `allowedTypes={['ssh']}` at `RemoteManager.tsx:367`). | CLAUDE.md is the binding agent-instructions file. Every frontend test mocks `invoke`, so an agent that writes `invoke('start_ssh_session')` from this list gets **green tests and a runtime "command not found"**. | Regenerate the IPC section from `generate_handler!` (or move it to `docs/IPC.md` generated by a script) and fix the table list and prop name. The full fix list is under "CLAUDE.md fix list" below. | M |
| CLEAN-003 | Medium | `AGENTS.md:1921,2005,2013,1935-1963,1974,1987` | The 130 KB file is 2,030 lines, 22 dated history entries plus Jan-Feb sections. It contains stale **instructions**: `npm run tauri:dev` (:1921, :2005), which isn't a script in `package.json`; "tsc reports pre-existing type errors" (:2013), while baseline `tsc` is clean (`audit/raw/tsc.txt`); a file tree listing `automation.rs`, `components/automation/` and `db.ts` (:1935-1963), all removed; "Rust 1.85+", "72 tests" and "79 tests" in the Cursor section (:1991-2020); "**Update this file** when making architectural changes" (:1974); a footer saying "rotation/credential-write race still open" (:1987) that contradicts its own entry at :10 (EDW-15 fixed). | AGENTS.md is the file Codex and Jules read by convention, and `google-labs-jules[bot]` commits here actively. Agents load 130 KB of mostly obsolete, self-contradicting context, and :1974 guarantees it keeps growing. | Extract the durable parts (see "AGENTS.md: what to keep and where") into CLAUDE.md/docs/CHANGELOG, archive the history, and replace AGENTS.md with a short stub pointing at CLAUDE.md. | M |
| CLEAN-004 | Low | `README.md:3-7`, `CODEBASE_AUDIT_REPORT.md:3-8`, `GEMINI.md:25,28-33` | All three link to `docs/DEFERRED_AUDIT_FIX_PLAN.md`, which has never existed on master (`git log --all -- docs/DEFERRED_AUDIT_FIX_PLAN.md` is empty in this clone; AGENTS.md:427 says it lives only on an unmerged branch). README's banner still says the five fixes are "not yet implemented". GEMINI.md's "Next Session Start" tells agents to *implement* that plan, but all five were done by 2026-09-24 (EDW-15). | The first thing a reader or agent sees is a broken link and a false status. A Gemini session is told to redo finished security work. | Delete the README banner and the CODEBASE_AUDIT_REPORT preamble (archive that file anyway), and delete GEMINI.md. | S |
| CLEAN-005 | Low | `.claude/agents/security-reviewer.md:28,31,99`; `.claude/agent-memory/security-reviewer/vault_rs_patterns.md:87`; `.codex/agents/security-reviewer.toml:24,92` | (a) :31 says the rekey/credential race is "an open finding" "until `docs/DEFERRED_AUDIT_FIX_PLAN.md` is implemented", which is stale since EDW-15. (b) :28 sets the Argon2 floor at "≥ 47 MiB (47 * 1024 KiB)", but the code uses `Params::new(47104, …)`, which is 46 MiB (`crypto.rs:22,39`, `vault.rs:152`), so the agent's own rule flags correct code as a HIGH. `vault_rs_patterns.md:87` then claims 46 MiB "matches >= 47 MiB". (c) :99 hardcodes `G:\GitHub\Quasar\.claude\agent-memory\…`. (d) The `.codex` TOML is an older fork of the same agent: it lacks three checklist items and the ssh_connect exception, says "contradicts AGENTS.md" instead of CLAUDE.md, and points its memory dir at `G:\…\.Codex\…`. | False-positive HIGH findings, stale "open" guidance, and a memory path that only resolves on one Windows machine. | Fix the floor to "≥ 46 MiB (47104 KiB), t ≥ 2" (the OWASP 2nd-tier baseline), drop the stale open-finding item, and use a repo-relative memory path. Delete the `.codex` copy, or generate it from the `.claude` source if Codex is still used. | S |
| CLEAN-006 | Low | `README.md:26,36-37,55,68,99,161,205,219,225-316,318-327,331` | False claims: "SFTP … with **progress tracking**" (:26), but `sftp_upload_file`/`sftp_download_file` pass `None` as the progress callback (`lib.rs:1557-1566`) and `SshFileManager.tsx` has no progress UI. "**shadcn/ui** components" (:55, :331): no shadcn/Radix dependency in `package.json`. "Historical Data" / "View historical data and trends" (:37, :161): `get_metrics_history`/`get_alert_history` are never invoked (recon §3.2). "Alert System" (:36): rules aren't persisted (see CLEAN-015). Stale: "344 tests" (:68, now 351); `github.com/yourusername` (:99); "branch from `main`" (:219, the only branch is `master`); `javascript.md` labelled as the TS guide (:205). Missing features: SSH tunnels, the AI assistant (Ollama), RDP launch and the auto-updater aren't in the feature list. There are 90 lines of changelog under "Recent Updates" (:225-316), and the "Documentation" index (:318-327) points at files slated for archive. | Users and contributors are misled about capabilities. The README duplicates a changelog. | Rewrite the feature list from verified code, move "Recent Updates" to `CHANGELOG.md`, fix the clone URL and branch name, and repoint the docs index. | S |
| CLEAN-007 | Low | `CLAUDE.md:35-94,100,138,163-178,303,324,326,392,417,444` | Stale versions: TypeScript 5.8 (actual `^7.0.2`), Vite 7 (`^8.2.0`), aes-gcm 0.10 (`0.11.0`), secrecy 0.8 (`0.10.3`), sysinfo 0.33 (`0.39.6`), cron 0.12 (`0.17.0`), zeroize 1.8 (`1.9.0`). "55+ components" (43 non-test `.tsx` under `components/`). "12 numbered migrations, 001–012" (13 files: 001 and 003-014). "Create `013_description.sql`" (:324; 013 and 014 exist, so the next is 015). "`ALTER TABLE … ADD COLUMN IF NOT EXISTS`" (:326) is **not valid SQLite**; the repo uses Rust hooks for exactly this reason (`lib.rs:117-121`), and `lib.rs:2209` would catch it in `cargo test`. "Use `secrecy::Secret<String>`" (:444), but the code uses `SecretString` (`vault.rs:12`). "Named exports only" (:417) contradicts :392 and the 43 non-test files that use `export default`. :35-94 are dated history sections, and the file is 40 KB. | Agents follow wrong syntax and APIs. The conventions section contradicts itself. | Apply the "CLAUDE.md fix list" and cut the file to about 15 KB of current state only. | M |
| CLEAN-008 | Low | 9 files, e.g. `conductor/tracks/asset_discovery_monitoring_20260130/plan.md:3-7`, `MONITORING_PHASE2_COMPLETE.md:311-315`, `code-review-projecttitan-8fd249.md:73-77`, `migration-options-comparison-5460b3.md:475-479` | `git grep` finds committed `<<<<<<< HEAD` … `>>>>>>> 30e7e777…` blocks in 9 docs. Also affected: `conductor/archive/{local_ai,remote_manager}*/spec.md:4-8`, `conductor/tracks/asset_discovery_monitoring_20260130/{index.md:3,spec.md:5}`, and `conductor/tracks/security_credential_vault_20260130/plan.md:3`. They are all "Quasar" vs "Project Titan" renames. | Shows these docs haven't been read or maintained since February, and it breaks Markdown rendering. | These files are ARCHIVE/DELETE below. Resolve the markers only in the ones being archived. | S |
| CLEAN-009 | Low | `conductor/tracks.md:8-14`, `conductor/tech-stack.md`, `conductor/tracks/security_credential_vault_20260130/security-model.md:94-97`, `conductor/workflow.md:151-173`, `.windsurf/workflows/plan_track.md` | tracks.md lists the vault track as "Not Started" and only 2 of the 5 track directories that exist (4 active + the archive), but the vault is implemented. tech-stack.md lists `ironrdp`, `rust-vnc`, `russh-keys`, an nmap sidecar and a Glances agent, none of which are in `Cargo.toml:28-62`. The security model gives Argon2 as 64 MiB / 3 iter / 4 lanes, but the code uses 47104 KiB / 2 / 1. workflow.md is an unfilled template ("e.g., for a Go project…"). The windsurf workflow tells agents to create conductor tracks and update GEMINI.md. README:201-206 and CLAUDE.md:414 still point into `conductor/code_styleguides/`. | A dead planning system that still has live pointers and gives agents instructions. The only threat-model document carries wrong KDF parameters. | Delete the conductor/windsurf planning scaffolding. Move the style guides to `docs/style/` and the security model to `docs/SECURITY_MODEL.md` (with corrected params). | S |
| CLEAN-010 | Low | `.Jules/palette.md` vs `.jules/{bolt,sentinel}.md` | `git log --all --name-only` shows exactly `.Jules/palette.md`, `.jules/bolt.md` and `.jules/sentinel.md`, so **no file-level collision today**. On case-insensitive filesystems (Windows/macOS defaults) both spellings land in one directory, and git keeps two index prefixes. Adding a `palette.md` under `.jules/` or a `bolt.md` under `.Jules/` would collide, making the checkout lose one file or show a phantom modification. The palette persona writes to `.Jules` (commit e1cf703: "Updated .Jules/palette.md"). | A latent checkout and diff hazard on the maintainer's Windows machine (paths in CLEAN-005 are `G:\`). | `git mv .Jules/palette.md .jules/palette.md` (needs a two-step rename on case-insensitive filesystems) and set the Jules palette persona to `.jules/`. | S |
| CLEAN-011 | Low | `_archived/automation_20260202/**` (22 files, ~250 KB), `test-workflow-*.json` | The workflow engine was removed on 2026-02-02 (`_archived/…/REMOVAL_SUMMARY.md`). Nothing references `_archived` or the two JSON fixtures: they aren't in tsconfig `include`, aren't Cargo paths, contain no `*.test.*` files, and grep finds only self-references. The fixtures use the removed engine's node schema (`"node_type": "trigger"`). | 250 KB of dead TSX/Rust pollutes code search, agent context and scanners. The removed code stays recoverable from git. | Delete the code and fixtures. Keep a one-page `docs/archive/automation-removal-2026-02.md` built from its README and REMOVAL_SUMMARY. | S |
| CLEAN-012 | Low | `src-tauri/test_salt_api.rs:1` | "// Temporary test file to understand Salt API". It isn't a Cargo target (not in `src/bin`, `examples/`, `tests/`, or `[[bin]]` in Cargo.toml), so it's never compiled, and it uses `.unwrap()` throughout. | Dead scratch file. It looks like a test but never runs. | Delete it. | S |
| CLEAN-013 | Low | `.gitignore` (whole file), `docs/RELEASE_SIGNING.md` §1 | Missing: `.claude/settings.local.json` (CLEAN-001); `coverage/` (vitest's default output; baseline left about 5 MB of untracked coverage HTML under `audit/`); `*.key` / `quasar-updater.key`, even though RELEASE_SIGNING.md tells maintainers to run `npx tauri signer generate -w quasar-updater.key` "locally", i.e. possibly in the repo root; `Thumbs.db`, `desktop.ini` (the maintainer works on Windows); `.env`, `.env.*`; `src-tauri/gen/` (only `/gen/schemas` is ignored). | One `git add -A` away from committing the **updater private key**, the one secret that would let an attacker sign updates. Coverage HTML and OS junk are also at risk. | Add the entries above. Change RELEASE_SIGNING.md to write the key outside the repo (e.g. `-w ~/.tauri/quasar-updater.key`). | S |
| CLEAN-014 | Low | `docs/SCHEMA.md:28`, `docs/SCHEMA.md:7-17`, `docs/CORE_WORKFLOWS.md:18`, `docs/RELEASE_SIGNING.md` vs `release.yml` | SCHEMA.md: "The next migration to add must be numbered `013_`", but 013 and 014 exist. The table list omits `metrics_history`, `alert_history`, `monitoring_host_credential`, the 013 indexes and the 014 cascade rebuild. CORE_WORKFLOWS:18 says adding a host lets you "Optionally link a credential", but `AddHostDialog.tsx` has no credential field (grep "credential" returns 0 hits). RELEASE_SIGNING.md doesn't mention that `release.yml` sets `releaseDraft: true`, so the updater endpoint `releases/latest/download/latest.json` (`tauri.conf.json`) serves nothing until someone publishes the draft by hand. | Wrong migration number (not unsafe, since position-indexed migrations would still append). The workflow doc describes UI that doesn't exist. Releases can silently fail to reach the updater. | Update the three docs. SCHEMA.md should describe the "next = highest + 1" rule instead of a hardcoded number. | S |
| CLEAN-015 | Low | `src-tauri/src/monitoring.rs:434-443`, `lib.rs:1159`; `CLAUDE.md:318`; `README.md:36` | `AlertEngine.rules` is a plain `Arc<Mutex<Vec<AlertRule>>>` built empty. `add_alert_rule` only calls `state.add_rule(rule)`, and no migration creates an `alert_rules` table. CLAUDE.md lists an `alert_rules` table, and README advertises configurable alerts. AGENTS.md:1881 ("Alert Persistence - Alerts currently in-memory only") is the only accurate statement. | Rules a user configures disappear on restart, and the docs say otherwise. This is a functional gap that belongs to Phase 3 and is recorded here as doc drift. | Short term: document it. Real fix (Phase 3): persist rules in a new migration. | S (doc) / M (code) |
| CLEAN-016 | Info | `index.html:5`, `public/tauri.svg`, `public/vite.svg`, `src/assets/react.svg` | The favicon is the Vite template's `/vite.svg`. `tauri.svg` and `react.svg` are referenced nowhere (grep). | Template leftovers. The window and tab icon is the Vite logo. | Point the favicon at the Quasar logo and delete the three template SVGs. | S |
| CLEAN-017 | Info | `src-tauri/.cargo/config.toml:1-3` | The comment says the file "Prevents CARGO_TARGET_DIR … from redirecting the build". Cargo gives the `CARGO_TARGET_DIR` env var **precedence** over `build.target-dir` in config. The real guard is `scripts/tauri-dev.js:31`, which overwrites the env var. | A misleading comment. A direct `cargo tauri dev` with a stale `CARGO_TARGET_DIR` is still redirected. | Correct the comment. | S |
| CLEAN-018 | Info | `postcss.config.js:4`, `package.json` devDeps `autoprefixer` | With Tailwind v4's `@tailwindcss/postcss`, vendor prefixing is built in, and Tailwind's v4 upgrade guide says to remove `autoprefixer`. | A redundant dev dependency and PostCSS pass. | Remove it after confirming the CSS output diff is empty (needs a build, which is out of scope here). | S |
| CLEAN-019 | Info | remote branches (below) | 5 remote branches are merged or superseded: `bolt/react-derived-state-…`, `bolt/optimize-date-formatting-…`, `palette/alertfeed-a11y-…`, `palette/improve-search-a11y-…`, `claude/nifty-pasteur-36u236`. 1 is unmerged with no open PR: `claude/google-jules-connector-v8hhwq`. | Branch clutter. The Jules connector branch adds a `.mcp.json` that reads `JULES_API_KEY`, so it needs an owner decision rather than silent deletion. | Delete the 5. Decide on the connector branch. Enable "auto-delete head branches" in repo settings. | S |
| CLEAN-020 | Info | repo root (11 files, ~130 KB) | 11 historical reports and plans sit at the root (`CODEBASE_AUDIT_2026-02-01.md`, `IMPLEMENTATION_SUMMARY_…`, `MONITORING_*COMPLETE.md` ×4, `MVP_COMPLETION_ROADMAP.md`, `code-review-projecttitan-*.md`, `migration-options-comparison-*.md`, `CODEBASE_AUDIT_REPORT.md`, plus `docs/AUDIT_PLAN.md`). All are dated February 2026 and describe states that have since changed. | Root clutter. Hard to tell current docs from history. | ARCHIVE or DELETE per the inventory. | S |

## Inventory

Classes: **KEEP** · **UPDATE** (keep, but content is stale) · **MERGE into X** · **ARCHIVE** (move to `docs/archive/`) · **DELETE** (recoverable from git history; this clone is shallow, but full history is on GitHub).

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

**Classification counts (rows):** KEEP 23 · UPDATE 16 · MERGE 4 · ARCHIVE 12 · DELETE 25 = 80 rows covering all 144 tracked non-source files (+ `audit/`).

## Proposed target doc structure

```
CLAUDE.md               ≤15 KB, current state only: session start, commands, repo map, conventions,
                        security invariants (condensed items 7-14), constraints. No dated sections.
AGENTS.md               3-line stub: "Agent instructions live in CLAUDE.md" (Codex/Jules read this name).
                        (Don't use a symlink: breaks on Windows without core.symlinks.)
README.md               What it is, verified features, install/dev/build, links. No changelog.
CHANGELOG.md            NEW: dated entries lifted from README "Recent Updates" + AGENTS.md headings (1-3 lines each + PR/commit link).
SECURITY.md             Unchanged (disclosure + accepted advisories).
docs/ARCHITECTURE.md    NEW: IPC surface (generated list), events, managed state, vault locking model
                        (credential_gate → inner lock order), scan claim pattern, ssh_connect phases, DB access rule.
docs/SECURITY_MODEL.md  NEW: from conductor security-model.md, params corrected to code.
docs/SCHEMA.md          Updated table list + "next migration = highest+1" rule.
docs/CORE_WORKFLOWS.md  Minor fixes.
docs/RELEASE_SIGNING.md Draft-release step; key generated outside repo.
docs/style/             typescript.md (fixed export rule), general.md, html-css.md.
docs/archive/           Feb audits, roadmap, migration decision, AUDIT_PLAN, product vision, design_concept.pdf,
                        automation-removal-2026-02.md.
.claude/agents, .claude/agent-memory   Keep (Claude-specific), fixed per CLEAN-005.
.jules/                 Keep (bot journals), single lowercase dir.
```

Removed from the tree: GEMINI.md, `.codex/`, `.windsurf/`, `conductor/` (after moves), `_archived/` (after one-page summary), the root report files, `test-workflow-*.json`, `test_salt_api.rs`, and the template SVGs.

### AGENTS.md: what to keep and where

| AGENTS.md section (line) | Keep? | Destination |
|---|---|---|
| Credential-Access Gate, EDW-15 (:10) | Design yes, narrative no | `docs/ARCHITECTURE.md` "Vault concurrency" (gate→inner lock order, drop `CredentialAccess` before network phase). CLAUDE.md item 14 already has the rule. |
| Dependabot #1 triage (:50), RUSTSEC-2023-0071 (:433) | Already in SECURITY.md | CHANGELOG one-liners |
| Tailscale integration (:73) | Behavior yes | `docs/CORE_WORKFLOWS.md` §7 already covers it; CHANGELOG |
| Deferred-audit status reconciliation (:188), Planned (:407) | No (superseded) | CHANGELOG one-liner |
| PR backlog cleanup + scan TOCTOU (:269) | Invariant yes; also the lesson "a bot commit reverted a fix mid-review: re-verify bot pushes" | Invariant: CLAUDE.md item 11 (already). Pattern: `docs/ARCHITECTURE.md`. Incident: CHANGELOG. |
| Vault lockout persistence (:490) | Invariant yes | CLAUDE.md item 10 (already); ARCHITECTURE |
| Code signing & updater (:541) | Yes | `docs/RELEASE_SIGNING.md` (already covers it) |
| Full bug audit Aug 22 (:618) | Rules yes: `db::open_connection()` only, `getErrorMessage` | CLAUDE.md item 8 (already); CHANGELOG |
| SSH diagnostics / credential save / protocol parity Aug 21 (:691) | Rules yes: camelCase `invoke` keys, `has_*` flags | CLAUDE.md testing patterns (already) |
| Test coverage Mar 6 (:739), Light theme (:793), Feb 12-18 entries (:818-1199) | Mostly no | CHANGELOG one-liners; the cron 6-field note (:893) is already in CORE_WORKFLOWS |
| Jan 30 – Feb 3 sections (:1200-1841) | No | archive (describe removed or rewritten code) |
| Architecture Notes (:1842), Known Issues (:1872), Testing Notes (:1892), File Structure (:1935), Perf (:1978) | No: stale (`AutomationState`, `workflow-*` events, `automation.rs`, "21 compiler warnings") | Replace with generated `docs/ARCHITECTURE.md` |
| Development Commands (:1915), Agent Guidelines (:1965) | No: `npm run tauri:dev` doesn't exist; "update this file" is harmful | none (CLAUDE.md has the correct commands) |
| Cursor Cloud instructions (:1991) | Linux system-deps apt list only | README "Prerequisites (Linux)" (the list is broader than ci.yml's, so reconcile) |

### CLAUDE.md fix list

1. Replace the IPC command list (:217-273) with the 73 real commands, grouped the same way and ideally generated from `generate_handler!`. Remove the 21 fake names listed in CLEAN-002.
2. Repository layout: delete `src/db.ts` (:105). Change "55+" to "43 components" (:100). Change "12 numbered" (:138) to "13 files: 001, 003–014 (no 002, intentional)". Add `src/hooks/useViewVisibility.tsx` and `useUpdater.ts` if listing hooks.
3. Tech stack (:163-178): TypeScript 7, Vite 8, aes-gcm 0.11, secrecy 0.10 (`SecretString`), sysinfo 0.39, cron 0.17, zeroize 1.9. russh 0.62 / russh-sftp 2.4 / Vitest 4 / RTL 16 / React 19 / xterm 6 / Tailwind 4 are correct.
4. Database (:303-320): migration count; remove `scheduled_task_runs`, `system_metrics_history` and `alert_rules`; add `metrics_history`, `alert_history` and `monitoring_host_credential`; note that alert rules are in-memory (CLEAN-015).
5. Adding migrations (:322-327): "next is 015" or better "highest + 1". Replace "`ADD COLUMN IF NOT EXISTS`" with "SQLite has no `ADD COLUMN IF NOT EXISTS`; use an `up_with_hook` column-exists check like 011/012 (`lib.rs:117-121`)".
6. Conventions: resolve "Named exports only" (:417) against reality (43 files default-export). Either relax the rule or plan a codemod. Change `secrecy::Secret<String>` (:444) to `SecretString`. Point the style-guide source (:414) at the new `docs/style/`.
7. Component patterns and Security note 3 (:455, :463): `filterType="password_only"` becomes `allowedTypes={['ssh']}` (`RemoteManager.tsx:367`).
8. CI section (:487): clippy runs with `--all-targets` in ci.yml.
9. Remove the dated sections (:35-94 "Recent Work" / "Previous Work") and the dated "as of" qualifiers. Move them to CHANGELOG.md. Keep Security Notes 7-14 as terse invariants (rule + regression test name) and drop the incident narratives.
10. The test inventory table (:348-398) is accurate (every listed test file exists), but it is about 6 KB of detail. Keep only "Key testing patterns" and move the table to `docs/` or drop it.
11. Session Start (:5-33) is user workflow policy and stays as-is.

## Remote branches (list only; nothing was deleted)

Source: `git ls-remote origin`. Dates come from the local objects or, marked (API), from the GitHub `get_commit` API.

| Branch | Head | Last commit | Status | Recommendation |
|---|---|---|---|---|
| `master` | 97dd84a | 2026-09-24 15:40 -0500 | default | keep |
| `claude/stoic-einstein-j4ajfe` | 6665448 | 2026-09-24 20:45 UTC | 3 ahead of master; current session | keep (active) |
| `claude/quasar-audit-cleanup-3srz1j` | 2cab112 | 2026-09-24 15:36 UTC | 2 ahead of master; audit branch | keep until the audit lands |
| `bolt/react-derived-state-optimizations-136666266663708391` | 30bd7da | 2026-09-24 06:33 UTC | **merged** (ancestor of master; PR #64) | delete |
| `claude/nifty-pasteur-36u236` | a94081f | 2026-09-17 16:06 UTC (API) | PR #42 head; squash-merged as 31f6fa9 "(#42)" on master | delete |
| `bolt/optimize-date-formatting-13184887323198778578` | b353335 | 2026-09-23 06:24 UTC (API) | PR #56 closed unmerged; superseded by #58 | delete |
| `palette/alertfeed-a11y-4334004861991957031` | e1cf703 | 2026-09-23 05:20 UTC (API) | PR #55 closed unmerged; superseded by #58 | delete |
| `palette/improve-search-a11y-7011989054922102331` | dd6c8ae | 2026-09-22 22:24 UTC (API) | PR #54 closed unmerged; superseded by #58 | delete |
| `claude/google-jules-connector-v8hhwq` | 4e3a0de | 2026-09-24 05:17 UTC (API) | **unmerged; no PR** among the 20 most recent. Adds a Jules MCP server and `.mcp.json` reading `JULES_API_KEY` | owner decision |

The merged-PR branches for #57-#63 and #65-#67 are already gone from the remote. Enabling auto-delete would make that the default.

## Checked and clean

- **No secrets in tracked non-source files.** I read `.claude/settings.local.json`, both workflows (secrets only via `${{ secrets.* }}`), `tauri.conf.json` (public minisign key only, as intended), the test-workflow JSONs (`"password": null`, host `localhost`), and `.cargo/*.toml`.
- **No large or generated artifacts committed.** The largest tracked files are `Cargo.lock` (181 KB), `package-lock.json` (142 KB), AGENTS.md (130 KB) and `design_concept.pdf` (459 KB). No `dist/`, `target/`, coverage HTML, `.db` files or `.DS_Store` are tracked.
- **`_archived/` isn't referenced by the build or tests.** tsconfig `include: ["src"]`, there are no Cargo paths into it, and it has 0 `*.test.*` files, so vitest's default glob won't pick anything up.
- **All migrations are registered** (`lib.rs:104-125`), and a test applies them to fresh and current DBs (`lib.rs:2209`).
- **Every test file named in CLAUDE.md's inventory table exists** (scripted check, 0 missing).
- **`scripts/tauri-dev.js` matches its docs.** It forwards arguments (`npm run tauri build` works), sets `CARGO_TARGET_DIR`, and has the Windows `npx.cmd`/shell handling.
- **SECURITY.md is consistent** with `src-tauri/.cargo/audit.toml` and ci.yml's `cargo audit` step.
- **RELEASE_SIGNING.md's secret names match `release.yml`** exactly (TAURI_SIGNING_*, WINDOWS_CERTIFICATE*, APPLE_*).
- **README claims confirmed true:** 13-port scan (`scanner.rs:404-406`), router/printer/workstation classification (`scanner.rs:152-163`), reverse DNS (`scanner.rs:135`), and "master key wiped on lock" (`vault.rs:432` sets `None`; `MasterKey` zeroizes on `Drop`, `vault.rs:83-91`).
- **Recon correction:** recon §3.3 says `alerts-recovered` has "no listener". That is wrong. It is listened to in `dashboard/SystemHealthWidget.tsx:89` and `dashboard/AlertFeed.tsx:110`, so CORE_WORKFLOWS' "shown in the Recent Activity feed" is accurate.

## UNVERIFIED / needs follow-up

- **`release.yml` step `if: runner.os == 'Windows' && secrets.WINDOWS_CERTIFICATE != ''`.** GitHub documents that `secrets` can't be referenced directly in `if:` conditionals. If Actions rejects this, the release workflow fails validation. Not verified (no Actions run available); handed to Phase 5.
- **Whether Jules and Codex actually load AGENTS.md here.** That is the convention, and `google-labs-jules[bot]` commits are present, but no run log confirms it. CLEAN-003's severity assumes they do.
- **Status of PR #42.** Inferred as squash-merged from master commit 31f6fa9's "(#42)" suffix. The PR record itself wasn't fetched: it's older than the 20-PR page retrieved.
- **Whether `claude/google-jules-connector-v8hhwq` has a closed or older PR** outside the 20 most recent. None appears in #48-#67.
- **The autoprefixer removal (CLEAN-018)** needs a build diff to confirm, and builds are out of scope for this read-only phase.
- **`GEMINI.md` has no unique content.** Based on headings and the :15-40 excerpt only, per the no-full-load rule.

## Severity counts

| Severity | Count |
|---|---|
| Critical | 0 |
| High | 0 |
| Medium | 3 |
| Low | 12 |
| Info | 5 |
| **Total** | **20** |

| Inventory class | Rows |
|---|---|
| KEEP | 23 |
| UPDATE | 16 |
| MERGE | 4 |
| ARCHIVE | 12 |
| DELETE | 25 |

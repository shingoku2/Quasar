# Phase 1 — Baseline

Date: 2026-09-24 · Base commit: `75a6ca8` (= master `fa85cfe` + the CLAUDE.md session-start commit + `audit/00-recon.md`).
Linear: EDW-18. Nothing outside `audit/` was modified. `git status` after all runs shows only untracked `audit/` paths
(`audit/raw/git-status-after.txt`).

Raw output for every command is in `audit/raw/<name>.txt`, and `audit/raw/_status.txt` has the exit code for each.
**Subagents: use these files. Do not re-run builds or tests.**

## Summary

| Check | Result | Pre-existing failure? | Raw file |
|---|---|---|---|
| `npm ci` | ✅ exit 0, 196 packages, 0 vulns | — | `npm-ci.txt` |
| `npx tsc --noEmit` | ✅ exit 0, no errors | — | `tsc.txt` |
| `npm test -- --coverage` | ✅ **49 files / 351 tests passed** (after the provider install, see note 2) | Yes: the coverage run can't start on a clean checkout (note 2) | `vitest-coverage.txt`, `vitest-coverage-retry.txt` |
| FE coverage (v8) | Statements **70.64%**, Branches **66.13%**, Functions **65.63%**, Lines **72.83%** | — | `audit/coverage-fe/coverage-summary.json` |
| `npm run build` | ✅ exit 0. Main chunk **1,313.95 kB** (379.76 kB gzip), over Vite's 500 kB warning; terminal chunk 337.69 kB | Warning only | `npm-build.txt` |
| `cargo build` | ✅ exit 0, no warnings | — | `cargo-build.txt` |
| `cargo clippy --all-targets -- -D warnings` | ✅ exit 0, **0 warnings** | — | `cargo-clippy.txt` |
| `cargo test` | ✅ **135 passed**, 0 failed, 1 ignored (lib 131 + `tests/vault_salt_fix_test.rs` 4); 2 doctests ignored | — | `cargo-test.txt` |
| RS coverage (`cargo llvm-cov`) | Lines **52.60%**, Regions 52.01%, Functions 36.00% | — | `cargo-llvm-cov.txt` |
| `npm audit` | ✅ **0** vulnerabilities (271 deps: 55 prod / 210 dev) | — | `npm-audit.txt` |
| `npm outdated` | 18 outdated: 1 major, 10 minor, 7 patch | exit 1 is normal when anything is outdated | `npm-outdated.txt` |
| `cargo audit` | ✅ **0 unignored vulns**. 1 ignored (RUSTSEC-2023-0071, `src-tauri/.cargo/audit.toml`), 6 unmaintained, 1 unsound, 1 yanked. Yanked check **incomplete** (note 4) | — | `cargo-audit.txt` (JSON), `cargo-audit-human.txt` |
| `cargo outdated --root-deps-only` | 13 outdated: 5 semver-incompatible, 8 compatible | — | `cargo-outdated.txt` |
| `cargo tree -d` | 108 duplicate-version entries across 47 crate names | — | `cargo-tree-dups.txt` |
| `cargo deny check` | ❌ exit 5: **advisories FAILED, licenses FAILED**, bans ok, sources ok (note 5) | Yes: no `deny.toml` exists in the repo | `cargo-deny-full.txt` (`cargo-deny.txt` is a truncated earlier run) |

**Bottom line:** no pre-existing build, lint, type or test failures. The only red results are environment/tooling gaps
(notes 1–2) and `cargo deny`, which has never been configured for this repo.

## Toolchain used

node 24.15.0 · npm 12.0.2 · rustc/cargo 1.98.1 · clippy 0.1.98 · tauri-cli 2.11.4 (`raw/toolchain.txt`).

## Notes on things that didn't run cleanly

1. **The container's default toolchain can't build this repo.** It shipped node 22.22.2 (`.nvmrc`/`engines` want 24.15.0) and
   rustc 1.94.1 (`Cargo.toml` `rust-version = "1.95"`). node 24.15.0 was installed to `/opt/node24`, npm was upgraded to 12.0.2,
   and `rustup update` took rust to 1.98.1. See `00-recon.md` §1 for the CI-side mismatch: CI runs npm 11.12.1.
2. **`npm test -- --coverage` fails on a clean checkout.** `@vitest/coverage-v8` isn't in `devDependencies`, so the first run
   exits 1 with `MISSING DEPENDENCY Cannot find dependency '@vitest/coverage-v8'`. It was installed with
   `npm i --no-save @vitest/coverage-v8@4.1.11` (matches vitest 4.1.11), and the lockfile and package.json are unchanged. That
   package is now in `node_modules` without being in the lockfile. That's harmless for read-only phases, but `npm ci` will
   remove it. → Finding for Phase 5 (TEST/CI): coverage can't be produced from the declared dependencies.
3. **Tools installed with `cargo install`** (not previously present): cargo-audit 0.22.2, cargo-outdated 0.19.0,
   cargo-llvm-cov 0.9.1, cargo-deny 0.20.2 (`raw/cargo-install-tools.txt`).
4. **cargo-audit's yanked check hit 338 `503 Service Unavailable` errors** from the registry index, so the yanked list is
   incomplete. cargo-audit still reported `chacha20 0.10.1` as yanked, and cargo-deny separately reports `wnaf 0.14.0` as
   well. The advisory DB fetch succeeded, so the **vulnerability results are complete**; only the yanked results aren't.
5. **cargo-deny with no `deny.toml`** uses its defaults:
   - `licenses FAILED` = **670 `rejected`** errors. With no allow-list configured, every license is rejected. That's a config
     gap, not a real license finding. There's also 1 `no-license-field` warning (`ollama-rs 0.3.6` has no license in its
     manifest), which is worth a real look in Phase 5.
   - `advisories FAILED` = 1 `vulnerability` (**RUSTSEC-2023-0071, Marvin Attack, `rsa 0.10.0-rc.18`** via russh/ssh-key/p521)
     + 6 `unmaintained`. cargo-deny doesn't read `.cargo/audit.toml`, so the accepted-risk ignore doesn't apply here.
   - 60 `duplicate` warnings, 2 `yanked` warnings (chacha20, wnaf).
   - The first run (`cargo-deny.txt`) was piped through `tail -200` and recorded `exit=0` from `tail`, not from cargo-deny. It was
     re-run in full (`cargo-deny-full.txt`, real exit 5).

## Coverage hot spots (input for Phase 5)

Rust line coverage by file (`cargo-llvm-cov.txt`). Anything under 40% is **bold**:

| File | Lines | | File | Lines |
|---|---|---|---|---|
| **ai.rs** | **0.00%** | | scanner.rs | 73.85% |
| **discovery.rs** | **0.00%** | | tailscale.rs | 74.22% |
| **ssh_auth.rs** | **0.00%** | | monitoring.rs | 77.48% |
| **ssh_tunnel.rs** | **0.00%** | | ssh_connect.rs | 81.74% |
| **sftp.rs** | **0.53%** | | vault/credentials.rs | 84.58% |
| **launcher.rs** | **17.14%** | | vault.rs | 85.19% |
| **ssh.rs** | **20.74%** | | db.rs | 88.89% |
| **lib.rs** (all 73 commands) | **21.51%** | | crypto.rs | 91.73% |
| **health.rs** | **31.62%** | | host_tracker.rs | 95.05% |
| **ssh_exec.rs** | **31.40%** | | errors.rs | 97.87% |
| **ssh_pool.rs** | **39.88%** | | validation.rs | 99.38% |
| vault/audit.rs | 51.28% | | | |
| scheduler.rs | 52.48% | | | |
| vault/ssh_keys.rs | 67.77% | | | |

The **SSH auth path (0%)**, **the IPC command layer (21.5%)** and **the scheduler (52%)** are the security-relevant gaps.
The ignored Rust test is `ssh_pool::tests::bench_session_reuse_against_live_host` (needs `QUASAR_TEST_SSH_HOST`).

Weakest frontend files (`vitest-coverage-retry.txt`): `useSshHostKeyVerification.ts` 69.23% lines (lines 129–183 are
uncovered, which is the host-key approval path), `VaultProvider.tsx` 70.12%, `CredentialManager.tsx` 72.84%,
`VaultSettings.tsx` 84.72%. Per-file totals are in `coverage-fe/coverage-summary.json`.

## Dependencies at a glance (triage is Phase 5)

**npm** (`npm-outdated.txt`): the one major is `vitest` 4.1.11 → 5.0.1. The minors include react/react-dom 19.2.8 → 19.3.0,
vite 8.2.2 → 8.3.0, `@tauri-apps/plugin-updater` 2.10.1 → 2.12.0 and lucide-react 1.33 → 1.48. The rest are patches.

**cargo** (`cargo-outdated.txt`):
- semver-incompatible: `argon2` 0.5.3 → 0.6.0 (vault KDF, so it needs a hash-compat check), `russh` 0.62.7 → 0.63.3,
  `russh-sftp` 2.4.0 → 3.0.0, `dns-lookup` 3 → 4, `mdns-sd` 0.20 → 0.21.
- compatible: tauri 2.11.5 → 2.11.6, plugin-dialog/opener patches, **plugin-updater 2.10.1 → 2.12.0**, uuid, rand, log,
  surge-ping.

**Tauri JS ↔ Rust plugin alignment is currently in lockstep:** dialog 2.7.2/2.7.2, opener 2.5.4/2.5.4, updater
2.10.1/2.10.1. The upgrade batches have to keep it that way.

**Warnings:** 6 unmaintained advisories (`proc-macro-error`, five `unic-*` crates), 1 unsound advisory (`glib 0.18.5`,
RUSTSEC-2024-0429) and 2 yanked crates (`chacha20`, `wnaf`). All are transitive. Phase 5 decides which are reachable.

## Doc drift spotted while baselining (input for Phase 6)

CLAUDE.md's Tech Stack table is out of date against what actually got built: Vite is **8.2.2** (CLAUDE.md says 7), `secrecy`
is **0.10.3** (CLAUDE.md says 0.8) and `sysinfo` is **0.39.6** (CLAUDE.md says 0.33). CLAUDE.md's CI section says
`cargo clippy -- -D warnings`. This baseline also ran `--all-targets` and it's still clean.

## Artifacts committed vs. left local

- Committed: this file, `audit/raw/*.txt` and `audit/coverage-fe/coverage-summary.json`.
- Left untracked (about 5 MB of generated HTML): `audit/coverage-fe/` HTML and `audit/coverage-rs/html/`. Regenerate with the
  commands at the top of `raw/vitest-coverage-retry.txt` and `raw/cargo-llvm-cov.txt`.

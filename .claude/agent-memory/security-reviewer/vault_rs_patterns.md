---
name: vault-rs-patterns
description: Locking/concurrency patterns and known gotchas in src-tauri/src/vault.rs, esp. around change_master_password and the changing_password flag
metadata:
  type: project
---

## VaultState locking model (as of 2026-08-22)

`VaultState.inner` is a `tokio::sync::RwLock<VaultStateInner>`. `MasterKey { key: [u8;32] }` has
`Drop` that zeroizes; `master_key: Option<MasterKey>` is the single source of truth for
"vault unlocked". `changing_password: bool` is a separate flag inside the same inner struct.

**Important: `changing_password` is only respected by `check_auto_lock()`.** It is a guard
against *auto-lock* firing mid password-change — nothing else. `lock_vault()` does NOT check
it and will unconditionally set `master_key = None` even while a password change is in flight.
This is intentional and fine as long as `change_master_password` (below) respects an explicit
lock rather than overwriting it.

## change_master_password lock-drop window (fixed 2026-08-22)

`change_master_password` only holds the write lock briefly at the start (snapshot `old_key`/
`db_path`, check `changing_password` isn't already set, set it) and briefly at the end (reset
`changing_password`, install `new_master_key`), doing the expensive Argon2id + re-encryption
in `spawn_blocking` with the lock released. This fixed the "auto-lock / any concurrent vault
read stalls behind a multi-second password change" problem, but a first pass at the refactor
introduced a real regression, caught by a security review before it shipped and fixed in the
same session:

1. **`lock_vault()` race (was CRITICAL, now fixed).** Because `lock_vault()` isn't gated by
   `changing_password`, a caller can lock the vault *during* the in-flight password change.
   The fix: at the reacquire-and-install step, `change_master_password` now checks
   `inner.master_key.is_some()` before installing the new key — an explicit lock taken during
   the window is respected (vault stays locked), rather than silently reverted. The new
   password is already persisted in the DB either way, so `unlock_vault` works correctly
   afterward regardless of which branch ran. **Confirmed applied** — see
   `change_master_password`'s tail (the `if inner.master_key.is_some() { ... }` guard right
   before the final `new_master_key.zeroize()`), and the regression test
   `vault::tests::test_lock_vault_during_password_change_stays_locked` (spawns the password
   change, yields until it's inside the `spawn_blocking` window, calls `lock_vault()`, asserts
   the vault stays locked and the new password already works). If you touch this function
   again, preserve that guard — don't reintroduce an unconditional `inner.master_key = Some(...)`
   at the end.

2. **Torn-state window (MEDIUM, accepted trade-off, not fixed — narrow and self-healing).**
   Between the SQLite `tx.commit()` inside `spawn_blocking` (DB now has credentials
   re-encrypted with the new key) and the outer function reacquiring the write lock to swap
   `inner.master_key`, `get_master_key()` still returns the OLD key. Any concurrent credential
   decrypt in that narrow window gets an AEAD auth failure against the now-new ciphertext. This
   didn't exist under the old hold-the-lock-the-whole-time design (concurrent callers just
   blocked until everything was consistent). There's a comment at the reacquire site in
   `vault.rs` documenting this. Flag if the window grows (e.g. more work added between
   `tx.commit()` and the final lock reacquire) — that would make the transient-failure window
   more likely to actually hit in practice.

3. **Not a regression, pre-existing:** `old_key`/`new_master_key` as plain `[u8;32]` (not
   `Zeroizing<[u8;32]>`) at the point they're extracted/returned across the
   async-fn/spawn_blocking boundary — Copy-type transient copies aren't zeroized on drop, only
   the final `MasterKey` struct and the explicit trailing `.zeroize()` calls are. Same in both
   the old and new version of this function, so don't flag as new — but worth a standalone
   hardening pass someday.

Second concurrent `change_master_password` calls ARE correctly blocked: the check-then-set of
`changing_password` happens under one single write-lock acquisition (no TOCTOU there).
Similarly `old_key` itself can't go stale mid-function — it's read once as a `Copy` snapshot
before the lock is dropped, so later mutations to `inner.master_key` (by `lock_vault`, etc.)
can't change the value already captured in the local `old_key` variable.

`initialize_vault` had the same class of TOCTOU (separate `is_initialized()` read-lock check
before a separate write-lock acquisition) — also fixed 2026-08-22 by moving the check inside
the same write-lock acquisition as the write itself.

## Argon2id params baseline (confirmed in this file)

`Params::new(47104, 2, 1, Some(32))` — 47104 KiB (46 MiB) memory, 2 iterations, 1 parallelism,
32-byte output. Used identically in `initialize_vault`, `unlock_vault`, and
`change_master_password`. Matches OWASP baseline per the checklist (>= 47 MiB, >= 2 iter).
Treat any PR that lowers these numbers as a HIGH/CRITICAL finding.

See also [[security-reviewer-memory-index]].

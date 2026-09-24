pub mod audit;
pub mod credentials;
pub mod kdf;
pub mod ssh_keys;

use crate::crypto;
use crate::db;
use rusqlite::{OptionalExtension, TransactionBehavior};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};
use tokio::time::{Duration, Instant};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

pub use audit::{AuditLogEntry, AuditLogFilter, AuditLogManager};
pub use credentials::{CredentialFrontendView, CredentialManager, CredentialSummary};
pub use ssh_keys::{HostKeyVerificationResult, SshHostKey, SshKeyManager, TrustStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSettings {
    pub auto_lock_timeout_minutes: u64,
    pub vault_initialized: bool,
}

/// Accepted auto-lock timeout range, in minutes (1 minute to 24 hours). Enforced here, not
/// just in the Settings UI, so an IPC caller can't disable auto-lock (IPC-006).
pub const AUTO_LOCK_MINUTES_RANGE: std::ops::RangeInclusive<u64> = 1..=1440;
const DEFAULT_AUTO_LOCK_MINUTES: u64 = 15;

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            auto_lock_timeout_minutes: DEFAULT_AUTO_LOCK_MINUTES,
            vault_initialized: false,
        }
    }
}

#[derive(Clone)]
pub struct VaultState {
    inner: Arc<RwLock<VaultStateInner>>,
    /// Serializes credential operations with master-password rekeying. Every
    /// operation that encrypts, decrypts or deletes a stored credential holds a
    /// shared guard for as long as it uses the master key (see `credential_access`);
    /// `change_master_password` takes it exclusively first thing, before it marks
    /// itself in progress or reads the old key, and holds it until the new key is
    /// installed. Without this, a credential write
    /// landing mid-rotation is encrypted with the old key after (or while) the
    /// re-encryption pass runs, and is undecryptable under the new key.
    ///
    /// Lock order: `credential_gate` before `inner`, never the reverse.
    credential_gate: Arc<RwLock<()>>,
    /// True while `change_master_password` runs; suspends auto-lock. An atomic outside
    /// `inner` so a drop guard can reset it even if the rotation future is cancelled.
    changing_password: Arc<AtomicBool>,
}

/// Resets `changing_password` when dropped: on success, error, or a cancelled future.
struct RotationInProgress(Arc<AtomicBool>);

impl Drop for RotationInProgress {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// The master key plus a shared hold on the credential gate. While this is alive, a
/// master-password change stays queued on the gate (it hasn't read the old key or
/// set `changing_password` yet), so the key stays valid for every credential read or
/// write made with it. Drop it as soon as the credential work is
/// done (not after a slow network operation that merely uses the decrypted result).
pub struct CredentialAccess {
    key: Zeroizing<[u8; 32]>,
    _gate: OwnedRwLockReadGuard<()>,
}

impl CredentialAccess {
    pub fn key(&self) -> &[u8; 32] {
        &self.key
    }
}

struct VaultStateInner {
    master_key: Option<MasterKey>,
    settings: VaultSettings,
    last_activity: Option<Instant>,
    failed_attempts: u32,
    lockout_until: Option<Instant>,
    db_path: String,
}

struct MasterKey {
    key: [u8; 32],
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

/// How the master password is verified, as stored in `vault_settings` (see vault/kdf.rs).
enum StoredKdf {
    /// `kdf_version = 2`: HKDF-separated verifier; the encryption key is never stored.
    V2 { salt_b64: String, verifier_hex: String },
    /// No `kdf_version` row: legacy PHC hash, which equals the encryption key (RSEC-001).
    /// Migrated to v2 on the next successful unlock.
    Legacy { salt_b64: String, phc: String },
}

impl VaultState {
    pub fn new(db_path: String) -> Self {
        Self {
            inner: Arc::new(RwLock::new(VaultStateInner {
                master_key: None,
                settings: VaultSettings::default(),
                last_activity: None,
                failed_attempts: 0,
                lockout_until: None,
                db_path,
            })),
            credential_gate: Arc::new(RwLock::new(())),
            changing_password: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn is_locked(&self) -> bool {
        let inner = self.inner.read().await;
        inner.master_key.is_none()
    }

    pub async fn is_initialized(&self) -> Result<bool, String> {
        let inner = self.inner.read().await;
        let conn = db::open_connection(&inner.db_path)?;

        let result: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT value FROM vault_settings WHERE key = 'vault_initialized'",
            [],
            |row| row.get(0),
        );

        Ok(matches!(result, Ok(val) if val == "true"))
    }

    pub async fn initialize_vault(&self, master_password: SecretString) -> Result<(), String> {
        // Validate password strength
        if master_password.expose_secret().len() < 12 {
            return Err("Master password must be at least 12 characters".to_string());
        }

        let mut inner = self.inner.write().await;
        let conn = db::open_connection(&inner.db_path)?;

        // Check-then-act under the same write-lock acquisition used for the write
        // below, so two concurrent initialize_vault calls (e.g. a double-submit)
        // can't both pass this check before either has written the salt/hash.
        let already_initialized: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT value FROM vault_settings WHERE key = 'vault_initialized'",
            [],
            |row| row.get(0),
        );
        if matches!(already_initialized, Ok(val) if val == "true") {
            return Err("Vault is already initialized".to_string());
        }

        // v2 key derivation (see vault/kdf.rs): one Argon2id pass, then HKDF into an
        // encryption key (memory only) and a verifier (stored). Never store anything the
        // encryption key can be read out of (audit RSEC-001).
        let salt = crate::crypto::generate_salt()?;
        let keys = kdf::derive_v2(master_password.expose_secret().as_bytes(), salt.as_str())?;
        Self::write_v2_kdf_state(&conn, salt.as_str(), &keys.verifier)?;

        conn.execute(
            "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["vault_initialized", "true", chrono::Utc::now().timestamp()],
        )
        .map_err(|e| format!("Failed to mark vault as initialized: {}", e))?;

        conn.execute(
            "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["auto_lock_timeout", "15", chrono::Utc::now().timestamp()],
        )
        .map_err(|e| format!("Failed to store auto-lock timeout: {}", e))?;

        inner.settings.vault_initialized = true;

        inner.master_key = Some(MasterKey { key: *keys.enc_key });
        inner.last_activity = Some(Instant::now());

        // Log audit event
        Self::log_audit_event(
            &conn,
            "vault_initialize",
            None,
            None,
            "vault",
            "initialize",
            "success",
            None,
        )?;

        Ok(())
    }

    /// Loads any lockout state persisted in `vault_settings` and folds it into
    /// the in-memory tracking. `VaultStateInner` starts fresh on every process
    /// launch, so without this an attacker with local access to the database
    /// file could bypass the escalating lockout below entirely — just by
    /// relaunching the app every 4 guesses to reset the in-memory counter.
    /// Only ever raises `inner`'s state (the DB is strictly more up to date
    /// than a fresh process), so this is safe to call unconditionally.
    fn load_persisted_lockout(conn: &rusqlite::Connection, inner: &mut VaultStateInner) {
        let persisted_attempts: u32 = conn
            .query_row(
                "SELECT value FROM vault_settings WHERE key = 'lockout_failed_attempts'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if persisted_attempts > inner.failed_attempts {
            inner.failed_attempts = persisted_attempts;
        }

        let persisted_until_unix: Option<i64> = conn
            .query_row(
                "SELECT value FROM vault_settings WHERE key = 'lockout_until_unix'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok());

        if let Some(until_unix) = persisted_until_unix {
            let remaining_secs = until_unix - chrono::Utc::now().timestamp();
            if remaining_secs > 0 {
                let candidate = Instant::now() + Duration::from_secs(remaining_secs as u64);
                if inner.lockout_until.map(|c| candidate > c).unwrap_or(true) {
                    inner.lockout_until = Some(candidate);
                }
            }
        }
    }

    /// Persists lockout state so it survives an app restart. `lockout_until`
    /// is converted from the process-local monotonic clock to a wall-clock
    /// unix timestamp, since `Instant` has no meaning across process runs.
    fn persist_lockout(
        conn: &rusqlite::Connection,
        failed_attempts: u32,
        lockout_until: Option<Instant>,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["lockout_failed_attempts", failed_attempts.to_string(), now],
        )
        .map_err(|e| format!("Failed to persist lockout state: {}", e))?;

        match lockout_until {
            Some(instant) => {
                let remaining = instant.checked_duration_since(Instant::now()).unwrap_or_default();
                let until_unix = now + remaining.as_secs() as i64;
                conn.execute(
                    "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                    rusqlite::params!["lockout_until_unix", until_unix.to_string(), now],
                )
                .map_err(|e| format!("Failed to persist lockout deadline: {}", e))?;
            }
            None => {
                conn.execute("DELETE FROM vault_settings WHERE key = 'lockout_until_unix'", [])
                    .map_err(|e| format!("Failed to clear lockout deadline: {}", e))?;
            }
        }

        Ok(())
    }

    pub async fn unlock_vault(&self, master_password: SecretString) -> Result<(), String> {
        // Exclusive credential gate (before `inner`, the usual lock order): unlocking a
        // legacy v1 vault migrates it, rewriting every credential, so no other credential
        // operation or password change may interleave with it.
        let _credential_gate = self.credential_gate.clone().write_owned().await;
        let mut inner = self.inner.write().await;
        let conn = db::open_connection(&inner.db_path)?;
        Self::load_persisted_lockout(&conn, &mut inner);

        // Check if locked out
        if let Some(lockout_until) = inner.lockout_until {
            if Instant::now() < lockout_until {
                let remaining = (lockout_until - Instant::now()).as_secs();
                return Err(format!(
                    "Vault is locked out. Try again in {} seconds",
                    remaining
                ));
            } else {
                // Clear the expired lockout but keep failed_attempts so the policy
                // escalates (5 → 5 min, 10 → 15 min, 15 → 60 min). Resetting the
                // counter here would pin every lockout at the 5-minute tier forever.
                // The counter is cleared on a successful unlock.
                inner.lockout_until = None;
                Self::persist_lockout(&conn, inner.failed_attempts, None)?;
            }
        }

        let stored = Self::load_stored_kdf(&conn)?;
        let password_bytes = master_password.expose_secret().as_bytes();
        let verified_key = Self::check_password(&stored, password_bytes)?;

        if verified_key.is_none() {
            inner.failed_attempts += 1;

            // Implement lockout policy
            let lockout_message = if inner.failed_attempts >= 15 {
                inner.lockout_until = Some(Instant::now() + Duration::from_secs(3600));
                Some("Too many failed attempts. Vault locked for 60 minutes".to_string())
            } else if inner.failed_attempts >= 10 {
                inner.lockout_until = Some(Instant::now() + Duration::from_secs(900));
                Some("Too many failed attempts. Vault locked for 15 minutes".to_string())
            } else if inner.failed_attempts >= 5 {
                inner.lockout_until = Some(Instant::now() + Duration::from_secs(300));
                Some("Too many failed attempts. Vault locked for 5 minutes".to_string())
            } else {
                None
            };

            Self::persist_lockout(&conn, inner.failed_attempts, inner.lockout_until)?;
            let details = format!(
                "failed attempt {}{}",
                inner.failed_attempts,
                if lockout_message.is_some() { "; lockout started" } else { "" }
            );
            if let Err(e) = Self::log_audit_event(
                &conn, "vault_unlock", None, None, "vault", "unlock", "failure", Some(&details),
            ) {
                log::warn!("Failed to audit failed unlock: {}", e);
            }

            return Err(lockout_message.unwrap_or_else(|| {
                format!(
                    "Invalid master password. {} attempts remaining before lockout",
                    5u32.saturating_sub(inner.failed_attempts)
                )
            }));
        }

        let key = verified_key.ok_or("Invalid master password")?;
        let key = match stored {
            StoredKdf::V2 { .. } => {
                if conn
                    .query_row("SELECT 1 FROM vault_settings WHERE key = 'kdf_scrub_pending'", [], |_| Ok(()))
                    .optional()
                    .ok()
                    .flatten()
                    .is_some()
                {
                    Self::scrub_or_mark_pending(&conn);
                }
                key
            }
            StoredKdf::Legacy { ref salt_b64, .. } => {
                // RSEC-001: this vault's stored hash *is* its encryption key. Migrate it to v2
                // now, while we hold the password. On failure, stay on the legacy key (which
                // still decrypts everything) and retry at the next unlock rather than locking
                // the user out of their credentials.
                match Self::migrate_legacy_vault(&inner.db_path, password_bytes, &key, salt_b64) {
                    Ok(new_key) => new_key,
                    Err(e) => {
                        log::error!("Vault KDF migration failed; still on the legacy format: {}", e);
                        let _ = Self::log_audit_event(
                            &conn, "vault_kdf_migration", None, None, "vault", "migrate", "failure", None,
                        );
                        key
                    }
                }
            }
        };

        inner.master_key = Some(MasterKey { key: *key });
        // The in-memory settings start at defaults on every launch; without this the
        // saved timeout was ignored after a restart (RUST-003).
        inner.settings.auto_lock_timeout_minutes = Self::load_auto_lock_minutes(&conn);
        inner.last_activity = Some(Instant::now());
        inner.failed_attempts = 0;
        inner.lockout_until = None;
        Self::persist_lockout(&conn, 0, None)?;

        // Log audit event
        Self::log_audit_event(
            &conn,
            "vault_unlock",
            None,
            None,
            "vault",
            "unlock",
            "success",
            None,
        )?;

        Ok(())
    }

    pub async fn lock_vault(&self) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        inner.master_key = None;
        inner.last_activity = None;

        let conn = db::open_connection(&inner.db_path)?;

        Self::log_audit_event(
            &conn,
            "vault_lock",
            None,
            None,
            "vault",
            "lock",
            "success",
            None,
        )?;

        Ok(())
    }

    /// For replacing the whole database file (`import_database`): waits for in-flight
    /// credential operations, blocks new ones and any password change for as long as the
    /// returned guard lives, and locks the vault, whose in-memory key belongs to the
    /// database being replaced (RSEC-004). The next unlock derives the imported vault's key.
    pub async fn lock_for_database_replacement(&self) -> Result<OwnedRwLockWriteGuard<()>, String> {
        let gate = self.credential_gate.clone().write_owned().await;
        let mut inner = self.inner.write().await;
        inner.master_key = None;
        inner.last_activity = None;
        if let Ok(conn) = db::open_connection(&inner.db_path) {
            let _ = Self::log_audit_event(
                &conn, "vault_lock", None, None, "vault", "lock", "success", Some("database import"),
            );
        }
        Ok(gate)
    }

    pub async fn check_auto_lock(&self) -> Result<bool, String> {
        let mut inner = self.inner.write().await;

        if inner.master_key.is_none() || self.changing_password.load(Ordering::SeqCst) {
            return Ok(false);
        }

        if let Some(last_activity) = inner.last_activity {
            let timeout =
                Duration::from_secs(inner.settings.auto_lock_timeout_minutes.saturating_mul(60));
            if Instant::now().duration_since(last_activity) > timeout {
                inner.master_key = None;
                inner.last_activity = None;
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub async fn update_activity(&self) {
        let mut inner = self.inner.write().await;
        if inner.master_key.is_some() {
            inner.last_activity = Some(Instant::now());
        }
    }

    /// Test/rotation-internals only; credential code uses `credential_access()`.
    pub(crate) async fn get_master_key(&self) -> Result<[u8; 32], String> {
        self.update_activity().await;
        self.current_key().await
    }

    async fn current_key(&self) -> Result<[u8; 32], String> {
        let inner = self.inner.read().await;
        inner
            .master_key
            .as_ref()
            .map(|mk| mk.key)
            .ok_or_else(|| "Vault is locked".to_string())
    }

    /// Returns the master key together with a shared credential-gate guard. Use this
    /// (not `get_master_key`) for any credential operation: it waits for an in-flight
    /// master-password change to finish, then pins the key until the guard is dropped.
    /// Counts as user activity (resets the auto-lock timer).
    pub async fn credential_access(&self) -> Result<CredentialAccess, String> {
        let gate = self.credential_gate.clone().read_owned().await;
        let key = Zeroizing::new(self.get_master_key().await?);
        Ok(CredentialAccess { key, _gate: gate })
    }

    /// Like `credential_access`, but for background work (monitoring polls, scheduled
    /// tasks) that must **not** reset the auto-lock timer: otherwise a 30 s dashboard poll
    /// keeps an unattended vault unlocked forever (RSEC-002).
    pub async fn credential_access_background(&self) -> Result<CredentialAccess, String> {
        let gate = self.credential_gate.clone().read_owned().await;
        let key = Zeroizing::new(self.current_key().await?);
        Ok(CredentialAccess { key, _gate: gate })
    }

    /// Shared credential-gate guard for credential operations that don't need the key
    /// (deletes), so they can't interleave with the rekey transaction either.
    pub async fn credential_gate(&self) -> OwnedRwLockReadGuard<()> {
        self.credential_gate.clone().read_owned().await
    }

    pub async fn get_settings(&self) -> VaultSettings {
        let (db_path, in_memory) = {
            let inner = self.inner.read().await;
            (inner.db_path.clone(), inner.settings.clone())
        };
        // Load authoritative vault_initialized and auto_lock from DB so Settings page
        // shows correct state after app restart (in-memory settings default to false until init).
        let conn = match db::open_connection(&db_path) {
            Ok(c) => c,
            Err(_) => return in_memory,
        };
        let vault_initialized = conn
            .query_row(
                "SELECT value FROM vault_settings WHERE key = 'vault_initialized'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|v| v == "true")
            .unwrap_or(false);
        VaultSettings {
            vault_initialized,
            auto_lock_timeout_minutes: Self::load_auto_lock_minutes(&conn),
        }
    }

    pub async fn change_master_password(
        &self,
        current_password: SecretString,
        new_password: SecretString,
    ) -> Result<(), String> {
        // Wait for in-flight credential operations to drain, then block new ones until
        // the new key is installed below (the guard lives to the end of this function).
        // Taken before `inner`, matching the lock order credential operations use, and
        // before `changing_password` is set, so if this future is dropped while queued
        // here (the one wait whose length depends on other operations) the flag is never
        // left stuck at true.
        // A second concurrent rotation queues here too; by the time it gets the gate
        // the first has finished, so it fails verification against the new hash.
        let _credential_gate = self.credential_gate.clone().write_owned().await;

        let (old_key, db_path) = {
            let inner = self.inner.read().await;


            // The vault MUST be unlocked: re-encrypting stored credentials requires the
            // current master key. Proceeding while locked would rewrite the password hash
            // and salt without re-encrypting, permanently orphaning every credential.
            let old_key = match inner.master_key.as_ref().map(|mk| mk.key) {
                Some(key) => key,
                None => {
                    return Err("Vault must be unlocked to change the master password".to_string())
                }
            };

            (old_key, inner.db_path.clone())
        };

        // Suspend auto-lock for the rotation. It's safe to drop `inner` now: other vault
        // reads (e.g. an in-flight SSH credential lookup) proceed with the old key instead
        // of stalling for the whole re-encryption pass, and the new key is swapped in
        // atomically below. The guard clears the flag on every exit path, including this
        // future being dropped mid-rotation (RSEC-016); it's dropped after the new key is
        // installed, so auto-lock can't fire in between.
        self.changing_password.store(true, Ordering::SeqCst);
        let _rotation = RotationInProgress(self.changing_password.clone());

        // Perform heavy crypto and DB operations in a blocking thread, without holding
        // the vault lock.
        let result = tauri::async_runtime::spawn_blocking(move || {
            // Use single connection for entire operation to prevent connection leak
            let mut conn = db::open_connection(&db_path)?;

            let stored = Self::load_stored_kdf(&conn)?;
            let current_key = Self::check_password(&stored, current_password.expose_secret().as_bytes())?
                .ok_or_else(|| "Invalid current password".to_string())?;
            // The in-memory key must be the one this database's credentials are encrypted
            // under. If it isn't (e.g. the database was replaced while unlocked), re-encrypting
            // would fail or orphan credentials, so refuse and make the user re-unlock first.
            if *current_key != old_key {
                return Err("Vault key does not match the database; lock and unlock the vault, then retry".to_string());
            }

            let new_salt = crate::crypto::generate_salt()?;
            let new_keys = kdf::derive_v2(new_password.expose_secret().as_bytes(), new_salt.as_str())?;

            {
                // IMMEDIATE: take SQLite's write lock up front so a concurrent writer can't
                // invalidate this transaction's read snapshot mid-rotation.
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|e| format!("Failed to begin transaction: {}", e))?;

                Self::reencrypt_all_credentials(&tx, &db_path, &old_key, &new_keys.enc_key, false)?;

                // Validate that the new key round-trips through the same AEAD encrypt/decrypt
                // path used above, before committing. This is deliberately done in-memory rather
                // than by reading a credential back through a fresh `db::open_connection()` call:
                // that connection wouldn't see this transaction's uncommitted writes.
                let (probe_ct, probe_nonce, probe_tag) =
                    crypto::encrypt(b"vault-reencryption-validation", &new_keys.enc_key)?;
                let probe_plaintext = crypto::decrypt(&probe_ct, &new_keys.enc_key, &probe_nonce, &probe_tag)
                    .map_err(|e| format!("Re-encryption validation failed: {}", e))?;
                if probe_plaintext != b"vault-reencryption-validation" {
                    return Err("Re-encryption validation failed: round-trip mismatch".to_string());
                }

                Self::write_v2_kdf_state(&tx, new_salt.as_str(), &new_keys.verifier)?;

                // Commit transaction (automatically rolls back on drop if not committed)
                tx.commit()
                    .map_err(|e| format!("Failed to commit transaction: {}", e))?;
            }

            // Committed: from here on nothing may fail the rotation, or `inner` would keep the
            // old key while the database uses the new one.
            if let Err(e) = Self::log_audit_event(&conn, "password_change", None, None, "vault", "update", "success", None) {
                log::warn!("Failed to audit password change: {}", e);
            }
            if matches!(stored, StoredKdf::Legacy { .. }) {
                // Still-legacy vault (its migration failed at unlock): the deleted hash was
                // the old key, so scrub it like the migration does.
                Self::scrub_or_mark_pending(&conn);
            }

            Ok::<[u8; 32], String>(*new_keys.enc_key) // copy out before Zeroizing drops
        }).await
            .map_err(|e| format!("Task failed: {}", e))
            .and_then(|inner_result| inner_result);

        // The DB transaction above already committed credentials/hash/salt under the
        // new key, but `inner.master_key` isn't swapped until we reacquire the lock
        // here. Credential operations can't observe that gap: they go through
        // `credential_access()`, which is blocked on `_credential_gate` until this
        // function returns. (A bare `get_master_key()` caller could, which is why
        // credential code must not use it.)
        let mut inner = self.inner.write().await;

        if let Err(e) = &result {
            if let Ok(conn) = db::open_connection(&inner.db_path) {
                let _ = Self::log_audit_event(
                    &conn, "password_change", None, None, "vault", "update", "failure", Some(e.as_str()),
                );
            }
        }
        let mut new_master_key = result?;

        // The vault lock was dropped for the (multi-second) re-encryption work above,
        // so it's possible lock_vault() ran and completed in that window. Respect an
        // explicit lock instead of silently reviving it: the new password is already
        // persisted, so the next unlock_vault() call will derive the right key from it.
        if inner.master_key.is_some() {
            inner.master_key = Some(MasterKey {
                key: new_master_key,
            });
            inner.last_activity = Some(Instant::now());
        }
        new_master_key.zeroize();

        Ok(())
    }

    /// Only the auto-lock timeout is caller-settable; `vault_initialized` is derived state
    /// and is ignored here (it used to be copied from the webview's struct wholesale).
    pub async fn update_settings(&self, settings: VaultSettings) -> Result<(), String> {
        let minutes = settings.auto_lock_timeout_minutes;
        if !AUTO_LOCK_MINUTES_RANGE.contains(&minutes) {
            return Err(format!(
                "Auto-lock timeout must be between {} and {} minutes",
                AUTO_LOCK_MINUTES_RANGE.start(),
                AUTO_LOCK_MINUTES_RANGE.end()
            ));
        }
        let mut inner = self.inner.write().await;
        let conn = db::open_connection(&inner.db_path)?;

        conn.execute(
            "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES ('auto_lock_timeout', ?1, ?2)",
            rusqlite::params![minutes.to_string(), chrono::Utc::now().timestamp()],
        )
        .map_err(|e| format!("Failed to update auto-lock timeout: {}", e))?;

        inner.settings.auto_lock_timeout_minutes = minutes;
        Ok(())
    }

    /// The persisted auto-lock timeout, clamped to the valid range (default 15).
    fn load_auto_lock_minutes(conn: &rusqlite::Connection) -> u64 {
        conn.query_row(
            "SELECT value FROM vault_settings WHERE key = 'auto_lock_timeout'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|m| AUTO_LOCK_MINUTES_RANGE.contains(m))
        .unwrap_or(DEFAULT_AUTO_LOCK_MINUTES)
    }

    /// Reads which key-derivation format this vault is stored in.
    fn load_stored_kdf(conn: &rusqlite::Connection) -> Result<StoredKdf, String> {
        let get = |key: &str| -> Result<Option<String>, String> {
            conn.query_row(
                "SELECT value FROM vault_settings WHERE key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| format!("Failed to read vault settings: {}", e))
        };
        let salt_b64 = get("salt")?;
        if get("kdf_version")?.as_deref() == Some(kdf::KDF_VERSION_V2) {
            let verifier_hex = get("master_password_verifier")?.ok_or("Vault not initialized")?;
            Ok(StoredKdf::V2 { salt_b64: salt_b64.ok_or("Salt not found")?, verifier_hex })
        } else {
            let phc = get("master_password_hash")?.ok_or("Vault not initialized")?;
            Ok(StoredKdf::Legacy { salt_b64: salt_b64.ok_or("Salt not found")?, phc })
        }
    }

    /// Checks `password` against the stored state. Returns the key the vault's credentials
    /// are currently encrypted under (v2 encryption key, or the v1 legacy key), or `None`
    /// for a wrong password.
    fn check_password(
        stored: &StoredKdf,
        password: &[u8],
    ) -> Result<Option<Zeroizing<[u8; 32]>>, String> {
        match stored {
            StoredKdf::V2 { salt_b64, verifier_hex } => {
                let keys = kdf::derive_v2(password, salt_b64)?;
                Ok(kdf::verifier_matches(&keys.verifier, verifier_hex).then_some(keys.enc_key))
            }
            StoredKdf::Legacy { salt_b64, phc } => {
                if kdf::verify_legacy(password, phc)? {
                    Ok(Some(kdf::derive_ikm(password, salt_b64)?))
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Stores v2 state (salt, verifier, version) and deletes any legacy key-equivalent hash.
    fn write_v2_kdf_state(
        conn: &rusqlite::Connection,
        salt_b64: &str,
        verifier: &[u8; 32],
    ) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp();
        for (key, value) in [
            ("salt", salt_b64.to_string()),
            ("master_password_verifier", kdf::encode_hex(verifier)),
            ("kdf_version", kdf::KDF_VERSION_V2.to_string()),
        ] {
            conn.execute(
                "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![key, value, now],
            )
            .map_err(|e| format!("Failed to store {}: {}", key, e))?;
        }
        conn.execute("DELETE FROM vault_settings WHERE key = 'master_password_hash'", [])
            .map_err(|e| format!("Failed to remove legacy password hash: {}", e))?;
        Ok(())
    }

    /// Re-encrypts every credential from `old_key` to `new_key` in place (ids preserved, so
    /// scheduled_tasks/monitoring references stay valid). With `skip_undecryptable`, rows that
    /// don't decrypt under `old_key` are left untouched and their ids returned; otherwise the
    /// first such row aborts the whole operation.
    fn reencrypt_all_credentials(
        tx: &rusqlite::Transaction,
        db_path: &str,
        old_key: &[u8; 32],
        new_key: &[u8; 32],
        skip_undecryptable: bool,
    ) -> Result<Vec<String>, String> {
        let credential_manager = credentials::CredentialManager::new(db_path.to_string());
        let summaries = credential_manager.list_credentials_conn(tx)?;
        if !skip_undecryptable {
            // Name every credential that can't be carried over, instead of failing on the
            // first one with a generic error the user can't act on (RSEC-012).
            let mut undecryptable = Vec::new();
            for summary in &summaries {
                if !Self::password_blob_decrypts(tx, &summary.id, old_key)? {
                    undecryptable.push(format!("{} ({})", summary.name, summary.id));
                }
            }
            if !undecryptable.is_empty() {
                return Err(format!(
                    "{} credential(s) can't be decrypted with the current key and would be lost by a password change: {}. Delete or re-create them, then retry.",
                    undecryptable.len(),
                    undecryptable.join(", ")
                ));
            }
        }
        let mut skipped = Vec::new();
        for summary in summaries {
            // Only an authentication failure under `old_key` marks a row as an orphan
            // (encrypted under some other key). Any other problem (malformed columns, a failed
            // write) aborts, so a row that *is* readable is never left behind under a key
            // that's about to become underivable.
            if skip_undecryptable && !Self::password_blob_decrypts(tx, &summary.id, old_key)? {
                log::warn!("Credential {} does not decrypt under the vault key; left as is", summary.id);
                skipped.push(summary.id);
                continue;
            }
            let credential = credential_manager.get_credential_conn(tx, old_key, &summary.id)?;
            credential_manager.reencrypt_credential_tx(
                tx,
                new_key,
                &summary.id,
                credential.created_at,
                credential.name,
                credential.username,
                credential.password,
                credential.credential_type,
                credential.host,
                credential.port,
                credential.metadata,
                credential.key_path,
                credential.private_key,
                credential.key_passphrase,
            )?;
        }
        Ok(skipped)
    }

    /// `Ok(false)` only when the stored password ciphertext fails AES-GCM authentication
    /// under `key`; malformed rows are an `Err`.
    fn password_blob_decrypts(
        conn: &rusqlite::Connection,
        credential_id: &str,
        key: &[u8; 32],
    ) -> Result<bool, String> {
        let (ciphertext, nonce, tag): (Vec<u8>, Vec<u8>, Vec<u8>) = conn
            .query_row(
                "SELECT encrypted_password, nonce, tag FROM credentials WHERE id = ?1",
                [credential_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|e| format!("Failed to read credential {}: {}", credential_id, e))?;
        let nonce: [u8; 12] = nonce
            .try_into()
            .map_err(|_| format!("Credential {} has a malformed nonce", credential_id))?;
        let tag: [u8; 16] = tag
            .try_into()
            .map_err(|_| format!("Credential {} has a malformed auth tag", credential_id))?;
        Ok(crypto::decrypt(&ciphertext, key, &nonce, &tag).is_ok())
    }

    /// VACUUM (drops free pages, where superseded hashes linger) and a truncating WAL
    /// checkpoint. True only if both fully completed; `wal_checkpoint` reports "busy" in
    /// its result row rather than as an error.
    fn scrub_database(conn: &rusqlite::Connection) -> bool {
        let vacuumed = match conn.execute_batch("VACUUM;") {
            Ok(()) => true,
            Err(e) => {
                log::warn!("VACUUM failed: {}", e);
                false
            }
        };
        let checkpointed = match conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get::<_, i64>(0)) {
            Ok(0) => true,
            Ok(_) => {
                log::warn!("WAL checkpoint was blocked by another connection");
                false
            }
            Err(e) => {
                log::warn!("WAL checkpoint failed: {}", e);
                false
            }
        };
        vacuumed && checkpointed
    }

    /// Scrubs, or records `kdf_scrub_pending` so `unlock_vault` retries until it succeeds.
    fn scrub_or_mark_pending(conn: &rusqlite::Connection) {
        let result = if Self::scrub_database(conn) {
            conn.execute("DELETE FROM vault_settings WHERE key = 'kdf_scrub_pending'", [])
        } else {
            conn.execute(
                "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES ('kdf_scrub_pending', '1', ?1)",
                [chrono::Utc::now().timestamp()],
            )
        };
        if let Err(e) = result {
            log::warn!("Failed to record database scrub state: {}", e);
        }
    }

    /// `import_database` leaves a full copy of the previous database at `<db>.bak`. If that
    /// copy is a v1 vault, its `master_password_hash` *is* its key; remove it (best effort).
    /// The copy's credentials stay encrypted under that key, now only recoverable with the
    /// password.
    fn strip_legacy_hash_from_backup(db_path: &str) {
        let backup_path = format!("{}.bak", db_path);
        if !std::path::Path::new(&backup_path).exists() {
            return;
        }
        let result = db::open_connection(&backup_path).and_then(|conn| {
            conn.execute("DELETE FROM vault_settings WHERE key = 'master_password_hash'", [])
                .map_err(|e| e.to_string())?;
            if !Self::scrub_database(&conn) {
                return Err("scrub incomplete".to_string());
            }
            Ok(())
        });
        if let Err(e) = result {
            log::warn!("Could not remove the legacy key hash from {}: {}", backup_path, e);
        }
    }

    /// Migrates a v1 (legacy) vault to v2 under a **fresh salt**: a new salt means the new
    /// key can't be derived from the old stored hash, so old copies of the database (backups,
    /// exports, `quasar.db.bak`) don't leak it. Those copies still expose their own
    /// (legacy) ciphertexts, which no migration can fix. Returns the new encryption key.
    /// Caller holds the credential gate exclusively.
    fn migrate_legacy_vault(
        db_path: &str,
        password: &[u8],
        legacy_key: &[u8; 32],
        legacy_salt_b64: &str,
    ) -> Result<Zeroizing<[u8; 32]>, String> {
        let new_salt = crate::crypto::generate_salt()?;
        let new_keys = kdf::derive_v2(password, new_salt.as_str())?;

        let mut conn = db::open_connection(db_path)?;
        let skipped = {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|e| format!("Failed to begin migration transaction: {}", e))?;
            let skipped =
                Self::reencrypt_all_credentials(&tx, db_path, legacy_key, &new_keys.enc_key, true)?;
            Self::write_v2_kdf_state(&tx, new_salt.as_str(), &new_keys.verifier)?;
            if !skipped.is_empty() {
                // Rows left under their old encryption keep the salt they were written with, so
                // they stay recoverable with the password (a salt isn't secret).
                tx.execute(
                    "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES ('legacy_salt', ?1, ?2)",
                    rusqlite::params![legacy_salt_b64, chrono::Utc::now().timestamp()],
                )
                .map_err(|e| format!("Failed to keep legacy salt: {}", e))?;
            }
            tx.commit()
                .map_err(|e| format!("Failed to commit migration: {}", e))?;
            skipped
        };

        // Committed: the database is now v2 under the new key. Nothing below may fail the
        // migration, or the caller would fall back to a key the database no longer uses.
        let details = (!skipped.is_empty())
            .then(|| format!("{} credential(s) left undecryptable: {}", skipped.len(), skipped.join(",")));
        if let Err(e) = Self::log_audit_event(
            &conn, "vault_kdf_migration", None, None, "vault", "migrate", "success", details.as_deref(),
        ) {
            log::warn!("Failed to audit vault migration: {}", e);
        }
        // The deleted legacy hash (== the old key) can survive in free pages and the WAL.
        Self::scrub_or_mark_pending(&conn);
        Self::strip_legacy_hash_from_backup(db_path);

        Ok(new_keys.enc_key)
    }

    fn log_audit_event(
        conn: &rusqlite::Connection,
        event_type: &str,
        resource_id: Option<&str>,
        resource_type: Option<&str>,
        resource_type_default: &str,
        action: &str,
        result: &str,
        details: Option<&str>,
    ) -> Result<(), String> {
        let id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO security_audit_log (id, timestamp, event_type, resource_id, resource_type, action, result, details)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                id,
                timestamp,
                event_type,
                resource_id,
                resource_type.unwrap_or(resource_type_default),
                action,
                result,
                details
            ],
        ).map_err(|e| format!("Failed to log audit event: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::password_hash::PasswordHash;

    fn setup_test_db() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        // Create a unique temporary file for each test. A nanosecond timestamp
        // alone isn't a reliable uniqueness guarantee (clock resolution can be
        // coarser than 1ns, and concurrent test threads can race), so a
        // per-process counter is appended to make collisions impossible.
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_path = format!("test_vault_{}_{}.db", timestamp, seq);

        let conn = rusqlite::Connection::open(&db_path).expect("Failed to open test database");

        // Create vault_settings table
        conn.execute(
            "CREATE TABLE vault_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )
        .expect("Failed to create vault_settings table");

        // Create audit log table
        conn.execute(
            "CREATE TABLE security_audit_log (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                resource_id TEXT,
                resource_type TEXT,
                action TEXT NOT NULL,
                result TEXT NOT NULL,
                details TEXT
            )",
            [],
        )
        .expect("Failed to create audit log table");

        // Create credentials table so change_master_password's re-encryption pass
        // (which lists/reads/rewrites every credential) has somewhere to operate,
        // even when a test doesn't add any credentials itself.
        conn.execute(
            "CREATE TABLE credentials (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                username TEXT NOT NULL,
                encrypted_password BLOB NOT NULL,
                nonce BLOB NOT NULL,
                tag BLOB NOT NULL,
                credential_type TEXT NOT NULL DEFAULT 'password',
                host TEXT,
                port INTEGER,
                metadata TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_used_at INTEGER
            )",
            [],
        )
        .expect("Failed to create credentials table");
        conn.execute_batch(
            "ALTER TABLE credentials ADD COLUMN key_path TEXT;
             ALTER TABLE credentials ADD COLUMN encrypted_private_key BLOB;
             ALTER TABLE credentials ADD COLUMN private_key_nonce BLOB;
             ALTER TABLE credentials ADD COLUMN private_key_tag BLOB;
             ALTER TABLE credentials ADD COLUMN encrypted_key_passphrase BLOB;
             ALTER TABLE credentials ADD COLUMN key_passphrase_nonce BLOB;
             ALTER TABLE credentials ADD COLUMN key_passphrase_tag BLOB;",
        )
        .expect("Failed to add key columns");

        db_path
    }

    fn cleanup_test_db(db_path: &str) {
        if let Err(e) = std::fs::remove_file(db_path) {
            eprintln!("Warning: Failed to cleanup test DB {}: {}", db_path, e);
        }
    }

    /// RSEC-001 regression: nothing stored in `vault_settings` may equal (or encode) the
    /// AES master key. Before the fix, the stored Argon2 PHC string's hash field *was* the key.
    #[tokio::test]
    async fn test_stored_vault_settings_never_contain_master_key() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault.initialize_vault(SecretString::from("TestPassword123!")).await.unwrap();
        assert_settings_do_not_contain_key(&db_path, &vault.get_master_key().await.unwrap());

        vault
            .change_master_password(
                SecretString::from("TestPassword123!"),
                SecretString::from("AnotherPassword456!"),
            )
            .await
            .unwrap();
        assert_settings_do_not_contain_key(&db_path, &vault.get_master_key().await.unwrap());
        cleanup_test_db(&db_path);
    }

    fn assert_settings_do_not_contain_key(db_path: &str, key: &[u8; 32]) {
        let conn = rusqlite::Connection::open(db_path).unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM vault_settings").unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        let key_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
        for (k, v) in &rows {
            assert!(!v.contains(&key_hex), "vault_settings.{} contains the master key (hex)", k);
            if let Ok(ph) = PasswordHash::new(v) {
                if let Some(h) = ph.hash {
                    assert_ne!(h.as_bytes(), &key[..], "vault_settings.{} PHC hash field is the master key", k);
                }
            }
        }
    }

    /// Builds a v1 (pre-RSEC-001-fix) vault exactly as the old `initialize_vault` did:
    /// PHC hash + salt, no `kdf_version`. Returns (legacy key, stored PHC string).
    fn write_legacy_vault(db_path: &str, password: &str) -> ([u8; 32], String) {
        use argon2::password_hash::PasswordHasher;
        let salt = crate::crypto::generate_salt().unwrap();
        let phc = kdf::argon2()
            .unwrap()
            .hash_password(password.as_bytes(), &salt)
            .unwrap()
            .to_string();
        let legacy_key = *kdf::derive_ikm(password.as_bytes(), salt.as_str()).unwrap();
        let conn = rusqlite::Connection::open(db_path).unwrap();
        for (k, v) in [
            ("master_password_hash", phc.as_str()),
            ("salt", salt.as_str()),
            ("vault_initialized", "true"),
        ] {
            conn.execute(
                "INSERT INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, 0)",
                rusqlite::params![k, v],
            )
            .unwrap();
        }
        (legacy_key, phc)
    }

    fn file_contains(path: &str, needle: &[u8]) -> bool {
        std::fs::read(path)
            .map(|bytes| bytes.windows(needle.len()).any(|w| w == needle))
            .unwrap_or(false)
    }

    fn vault_setting(db_path: &str, key: &str) -> Option<String> {
        rusqlite::Connection::open(db_path)
            .unwrap()
            .query_row("SELECT value FROM vault_settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .unwrap()
    }

    /// RSEC-001 migration: unlocking a legacy vault re-encrypts everything under a v2 key
    /// derived from a fresh salt, deletes the key-equivalent hash, scrubs it from the file,
    /// and leaves rows that never decrypted (e.g. pre-EDW-15 orphans) untouched.
    #[tokio::test]
    async fn test_legacy_vault_is_migrated_to_v2_on_unlock() {
        let db_path = setup_test_db();
        let password = "LegacyPassword123!";
        let (legacy_key, phc) = write_legacy_vault(&db_path, password);
        let legacy_salt = vault_setting(&db_path, "salt").unwrap();
        let hash_field = PasswordHash::new(&phc).unwrap().hash.unwrap().to_string();

        let creds = credentials::CredentialManager::new(db_path.clone());
        let good_id = creds
            .add_credential(&legacy_key, "srv".into(), "root".into(), "remote-secret".into(),
                "password".into(), None, None, None, None, Some("PEM-DATA".into()), None)
            .unwrap();
        let orphan_id = creds
            .add_credential(&[7u8; 32], "orphan".into(), "x".into(), "unrecoverable".into(),
                "password".into(), None, None, None, None, None, None)
            .unwrap();
        let orphan_ct_before: Vec<u8> = rusqlite::Connection::open(&db_path).unwrap()
            .query_row("SELECT encrypted_password FROM credentials WHERE id = ?1", [&orphan_id], |r| r.get(0))
            .unwrap();
        assert!(
            file_contains(&db_path, hash_field.as_bytes())
                || file_contains(&format!("{}-wal", db_path), hash_field.as_bytes()),
            "fixture sanity: the legacy key-equivalent hash should be on disk before migration"
        );
        // Free pages are where superseded hashes (e.g. from earlier password changes) linger;
        // VACUUM is what removes them. Create some so the test can tell whether it ran.
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE junk (b BLOB); INSERT INTO junk VALUES (zeroblob(262144)); DROP TABLE junk;",
            )
            .unwrap();
            let free: i64 = conn.query_row("PRAGMA freelist_count", [], |r| r.get(0)).unwrap();
            assert!(free > 0, "fixture sanity: expected free pages before migration");
        }

        let vault = VaultState::new(db_path.clone());
        vault.unlock_vault(SecretString::from(password)).await.unwrap();
        let new_key = vault.get_master_key().await.unwrap();

        assert_ne!(new_key, legacy_key);
        assert_eq!(vault_setting(&db_path, "kdf_version").as_deref(), Some(kdf::KDF_VERSION_V2));
        assert_eq!(vault_setting(&db_path, "master_password_hash"), None);
        assert_ne!(vault_setting(&db_path, "salt").unwrap(), legacy_salt, "migration must use a fresh salt");
        assert_settings_do_not_contain_key(&db_path, &new_key);

        let migrated = creds.get_credential(&new_key, &good_id).unwrap();
        assert_eq!(migrated.password, "remote-secret");
        assert_eq!(migrated.private_key.as_deref(), Some("PEM-DATA"));
        assert!(creds.get_credential(&legacy_key, &good_id).is_err());

        let orphan_ct_after: Vec<u8> = rusqlite::Connection::open(&db_path).unwrap()
            .query_row("SELECT encrypted_password FROM credentials WHERE id = ?1", [&orphan_id], |r| r.get(0))
            .unwrap();
        assert_eq!(orphan_ct_before, orphan_ct_after, "undecryptable rows are left as they were");

        let audit_details: Option<String> = rusqlite::Connection::open(&db_path).unwrap()
            .query_row(
                "SELECT details FROM security_audit_log WHERE event_type = 'vault_kdf_migration' AND result = 'success'",
                [], |r| r.get(0))
            .unwrap();
        assert!(audit_details.unwrap_or_default().contains(&orphan_id));

        let free_after: i64 = rusqlite::Connection::open(&db_path).unwrap()
            .query_row("PRAGMA freelist_count", [], |r| r.get(0))
            .unwrap();
        assert_eq!(free_after, 0, "migration must VACUUM so no free page keeps an old hash");
        assert!(!file_contains(&db_path, hash_field.as_bytes()), "legacy hash must be scrubbed from the DB file");
        assert!(!file_contains(&format!("{}-wal", db_path), hash_field.as_bytes()), "legacy hash must be scrubbed from the WAL");

        // The migrated vault unlocks through the v2 path with the same password, and only that password.
        vault.lock_vault().await.unwrap();
        assert!(vault.unlock_vault(SecretString::from("WrongPassword123!")).await.is_err());
        vault.unlock_vault(SecretString::from(password)).await.unwrap();
        assert_eq!(vault.get_master_key().await.unwrap(), new_key);

        cleanup_test_db(&db_path);
    }

    fn raw_password_decrypts(db_path: &str, id: &str, key: &[u8; 32]) -> bool {
        let conn = rusqlite::Connection::open(db_path).unwrap();
        VaultState::password_blob_decrypts(&conn, id, key).unwrap()
    }

    /// Review blocker: once the migration has committed, a later failure (here the audit
    /// insert) must not make unlock fall back to the legacy key, which the database no
    /// longer uses and whose salt is gone.
    #[tokio::test]
    async fn test_migration_post_commit_failure_still_installs_new_key() {
        let db_path = setup_test_db();
        let password = "LegacyPassword123!";
        let (legacy_key, _) = write_legacy_vault(&db_path, password);
        // No credentials: reading one inside the migration also writes an audit row, which
        // would fail the migration *before* commit. This isolates the post-commit failure.
        rusqlite::Connection::open(&db_path).unwrap()
            .execute_batch("DROP TABLE security_audit_log;")
            .unwrap();

        let vault = VaultState::new(db_path.clone());
        // The final "vault_unlock" audit insert fails too; what matters is the installed key.
        let _ = vault.unlock_vault(SecretString::from(password)).await;
        let key = vault.get_master_key().await.unwrap();

        assert_eq!(vault_setting(&db_path, "kdf_version").as_deref(), Some(kdf::KDF_VERSION_V2));
        let expected = kdf::derive_v2(password.as_bytes(), &vault_setting(&db_path, "salt").unwrap()).unwrap();
        assert_ne!(key, legacy_key, "must not fall back to the legacy key after the migration committed");
        assert_eq!(key, *expected.enc_key);
        cleanup_test_db(&db_path);
    }

    /// Same rule for a password change: after commit, an audit failure mustn't leave the old key installed.
    #[tokio::test]
    async fn test_password_change_post_commit_failure_still_installs_new_key() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault.initialize_vault(SecretString::from("TestPassword123!")).await.unwrap();
        let old_key = vault.get_master_key().await.unwrap();
        rusqlite::Connection::open(&db_path).unwrap()
            .execute_batch("DROP TABLE security_audit_log;")
            .unwrap();

        vault
            .change_master_password(SecretString::from("TestPassword123!"), SecretString::from("AnotherPassword456!"))
            .await
            .unwrap();

        let key = vault.get_master_key().await.unwrap();
        let expected = kdf::derive_v2(b"AnotherPassword456!", &vault_setting(&db_path, "salt").unwrap()).unwrap();
        assert_ne!(key, old_key);
        assert_eq!(key, *expected.enc_key);
        cleanup_test_db(&db_path);
    }

    /// Review finding 2: only AEAD failures are skipped. A row that decrypts but can't be
    /// read for another reason (here a port outside u16) aborts the migration instead of
    /// being stranded under a key whose salt is about to be replaced.
    #[tokio::test]
    async fn test_migration_aborts_on_non_decryption_errors() {
        let db_path = setup_test_db();
        let password = "LegacyPassword123!";
        let (legacy_key, phc) = write_legacy_vault(&db_path, password);
        let id = credentials::CredentialManager::new(db_path.clone())
            .add_credential(&legacy_key, "srv".into(), "root".into(), "s".into(),
                "password".into(), None, None, None, None, None, None)
            .unwrap();
        rusqlite::Connection::open(&db_path).unwrap()
            .execute("UPDATE credentials SET port = 70000 WHERE id = ?1", [&id])
            .unwrap();

        let vault = VaultState::new(db_path.clone());
        vault.unlock_vault(SecretString::from(password)).await.unwrap();

        assert_eq!(vault.get_master_key().await.unwrap(), legacy_key);
        assert_eq!(vault_setting(&db_path, "master_password_hash"), Some(phc));
        assert!(raw_password_decrypts(&db_path, &id, &legacy_key));
        cleanup_test_db(&db_path);
    }

    /// Orphans left behind by the migration keep the salt they need; `.bak` loses its
    /// key-equivalent hash.
    #[tokio::test]
    async fn test_migration_keeps_legacy_salt_for_orphans_and_strips_backup_hash() {
        let db_path = setup_test_db();
        let password = "LegacyPassword123!";
        let (_, phc) = write_legacy_vault(&db_path, password);
        let legacy_salt = vault_setting(&db_path, "salt").unwrap();
        credentials::CredentialManager::new(db_path.clone())
            .add_credential(&[9u8; 32], "orphan".into(), "x".into(), "y".into(),
                "password".into(), None, None, None, None, None, None)
            .unwrap();
        let backup = format!("{}.bak", db_path);
        std::fs::copy(&db_path, &backup).unwrap();
        let hash_field = PasswordHash::new(&phc).unwrap().hash.unwrap().to_string();
        assert!(file_contains(&backup, hash_field.as_bytes()), "fixture sanity");

        let vault = VaultState::new(db_path.clone());
        vault.unlock_vault(SecretString::from(password)).await.unwrap();

        assert_eq!(vault_setting(&db_path, "legacy_salt"), Some(legacy_salt));
        assert_eq!(vault_setting(&backup, "master_password_hash"), None);
        assert!(!file_contains(&backup, hash_field.as_bytes()));
        let _ = std::fs::remove_file(&backup);
        let _ = std::fs::remove_file(format!("{}-wal", backup));
        let _ = std::fs::remove_file(format!("{}-shm", backup));
        cleanup_test_db(&db_path);
    }

    /// A legacy unlock rewrites every credential, so it must wait for in-flight credential
    /// operations (which hold the gate shared) instead of interleaving with them.
    #[tokio::test]
    async fn test_legacy_unlock_waits_for_credential_gate() {
        let db_path = setup_test_db();
        let password = "LegacyPassword123!";
        write_legacy_vault(&db_path, password);
        let vault = VaultState::new(db_path.clone());

        let in_flight = vault.credential_gate().await;
        let unlocking = {
            let vault = vault.clone();
            tokio::spawn(async move { vault.unlock_vault(SecretString::from(password)).await })
        };
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        assert!(!unlocking.is_finished(), "unlock must queue behind a held credential gate");
        assert!(vault.is_locked().await);

        drop(in_flight);
        unlocking.await.unwrap().unwrap();
        assert_eq!(vault_setting(&db_path, "kdf_version").as_deref(), Some(kdf::KDF_VERSION_V2));
        cleanup_test_db(&db_path);
    }

    /// RSEC-002: background credential use (monitoring polls, scheduled runs) must not reset
    /// the auto-lock timer; user-initiated use must.
    #[tokio::test]
    async fn test_background_access_does_not_count_as_activity() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault.initialize_vault(SecretString::from("TestPassword123!")).await.unwrap();
        let past = Instant::now() - Duration::from_secs(600);
        vault.inner.write().await.last_activity = Some(past);

        drop(vault.credential_access_background().await.unwrap());
        assert_eq!(vault.inner.read().await.last_activity, Some(past));

        drop(vault.credential_access().await.unwrap());
        assert!(vault.inner.read().await.last_activity.unwrap() > past);
        cleanup_test_db(&db_path);
    }

    /// RUST-003: the saved timeout is what auto-lock uses after a restart.
    #[tokio::test]
    async fn test_saved_auto_lock_timeout_is_loaded_on_unlock() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault.initialize_vault(SecretString::from("TestPassword123!")).await.unwrap();
        vault
            .update_settings(VaultSettings { auto_lock_timeout_minutes: 5, vault_initialized: true })
            .await
            .unwrap();

        let restarted = VaultState::new(db_path.clone());
        assert_eq!(restarted.inner.read().await.settings.auto_lock_timeout_minutes, 15);
        restarted.unlock_vault(SecretString::from("TestPassword123!")).await.unwrap();
        assert_eq!(restarted.inner.read().await.settings.auto_lock_timeout_minutes, 5);
        cleanup_test_db(&db_path);
    }

    /// IPC-006: out-of-range timeouts are rejected server-side (0 locked within 30 s; a huge
    /// value overflowed `* 60`), and `vault_initialized` can't be set by the caller.
    #[tokio::test]
    async fn test_update_settings_validates_timeout_and_ignores_derived_fields() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault.initialize_vault(SecretString::from("TestPassword123!")).await.unwrap();
        for bad in [0, 1441, u64::MAX] {
            assert!(vault
                .update_settings(VaultSettings { auto_lock_timeout_minutes: bad, vault_initialized: true })
                .await
                .is_err());
        }
        vault
            .update_settings(VaultSettings { auto_lock_timeout_minutes: 30, vault_initialized: false })
            .await
            .unwrap();
        assert!(vault.inner.read().await.settings.vault_initialized);
        assert_eq!(vault.get_settings().await.auto_lock_timeout_minutes, 30);
        cleanup_test_db(&db_path);
    }

    /// RSEC-016: a rotation future dropped mid-flight must not leave auto-lock disabled.
    #[tokio::test]
    async fn test_cancelled_password_change_clears_rotation_flag() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault.initialize_vault(SecretString::from("TestPassword123!")).await.unwrap();
        let rotation = {
            let vault = vault.clone();
            tokio::spawn(async move {
                vault
                    .change_master_password(
                        SecretString::from("TestPassword123!"),
                        SecretString::from("AnotherPassword456!"),
                    )
                    .await
            })
        };
        for _ in 0..1000 {
            if vault.changing_password.load(Ordering::SeqCst) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(vault.changing_password.load(Ordering::SeqCst), "rotation should be in flight");
        rotation.abort();
        let _ = rotation.await;
        assert!(!vault.changing_password.load(Ordering::SeqCst));
        cleanup_test_db(&db_path);
    }

    /// RSEC-012: a password change names the credentials it can't carry over.
    #[tokio::test]
    async fn test_password_change_lists_undecryptable_credentials() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault.initialize_vault(SecretString::from("TestPassword123!")).await.unwrap();
        let orphan_id = credentials::CredentialManager::new(db_path.clone())
            .add_credential(&[5u8; 32], "lost-cred".into(), "u".into(), "p".into(),
                "password".into(), None, None, None, None, None, None)
            .unwrap();
        let err = vault
            .change_master_password(SecretString::from("TestPassword123!"), SecretString::from("AnotherPassword456!"))
            .await
            .unwrap_err();
        assert!(err.contains("can't be decrypted with the current key"), "{}", err);
        assert!(err.contains("lost-cred") && err.contains(&orphan_id), "{}", err);
        cleanup_test_db(&db_path);
    }

    /// RSEC-011: failed unlocks are audited.
    #[tokio::test]
    async fn test_failed_unlock_is_audited() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault.initialize_vault(SecretString::from("TestPassword123!")).await.unwrap();
        vault.lock_vault().await.unwrap();
        assert!(vault.unlock_vault(SecretString::from("WrongPassword123!")).await.is_err());
        let failures: i64 = rusqlite::Connection::open(&db_path).unwrap()
            .query_row(
                "SELECT COUNT(*) FROM security_audit_log WHERE event_type = 'vault_unlock' AND result = 'failure'",
                [], |r| r.get(0))
            .unwrap();
        assert_eq!(failures, 1);
        cleanup_test_db(&db_path);
    }

    /// TEST-008: the gate must stay exclusively held until the new key is installed. The
    /// EDW-15 test can't catch a rotation that releases the gate right after commit.
    #[tokio::test]
    async fn test_gate_held_until_new_key_installed() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault.initialize_vault(SecretString::from("TestPassword123!")).await.unwrap();
        let initial_verifier = vault_setting(&db_path, "master_password_verifier");

        let rotation = {
            let vault = vault.clone();
            tokio::spawn(async move {
                vault
                    .change_master_password(
                        SecretString::from("TestPassword123!"),
                        SecretString::from("AnotherPassword456!"),
                    )
                    .await
            })
        };
        for _ in 0..1000 {
            if vault.changing_password.load(Ordering::SeqCst) {
                break;
            }
            tokio::task::yield_now().await;
        }
        // Hold `inner` so the rotation can't install the new key, then wait for its commit.
        let held = vault.inner.read().await;
        let mut committed = false;
        for _ in 0..3000 {
            if vault_setting(&db_path, "master_password_verifier") != initial_verifier {
                committed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(committed, "rotation should have committed");
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        assert!(
            vault.credential_gate.try_read().is_err(),
            "gate must stay exclusive until the new key is in memory"
        );
        drop(held);
        rotation.await.unwrap().unwrap();
        let access = vault.credential_access().await.unwrap();
        let expected = kdf::derive_v2(b"AnotherPassword456!", &vault_setting(&db_path, "salt").unwrap()).unwrap();
        assert_eq!(access.key(), &*expected.enc_key);
        drop(access);
        cleanup_test_db(&db_path);
    }

    /// If the migration can't complete, the unlock still succeeds on the legacy key (the
    /// user keeps access to their credentials) and nothing is half-migrated.
    #[tokio::test]
    async fn test_failed_legacy_migration_keeps_vault_usable_and_unchanged() {
        let db_path = setup_test_db();
        let password = "LegacyPassword123!";
        let (legacy_key, phc) = write_legacy_vault(&db_path, password);
        rusqlite::Connection::open(&db_path).unwrap()
            .execute_batch("DROP TABLE credentials;")
            .unwrap();

        let vault = VaultState::new(db_path.clone());
        vault.unlock_vault(SecretString::from(password)).await.unwrap();

        assert_eq!(vault.get_master_key().await.unwrap(), legacy_key);
        assert_eq!(vault_setting(&db_path, "master_password_hash"), Some(phc));
        assert_eq!(vault_setting(&db_path, "kdf_version"), None);
        let failures: i64 = rusqlite::Connection::open(&db_path).unwrap()
            .query_row(
                "SELECT COUNT(*) FROM security_audit_log WHERE event_type = 'vault_kdf_migration' AND result = 'failure'",
                [], |r| r.get(0))
            .unwrap();
        assert_eq!(failures, 1);

        cleanup_test_db(&db_path);
    }

    /// A password change must not re-encrypt when the in-memory key isn't the one the
    /// database was written with (e.g. the DB file was replaced while unlocked): doing so
    /// would orphan every credential.
    #[tokio::test]
    async fn test_change_master_password_refuses_when_key_does_not_match_database() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault.initialize_vault(SecretString::from("TestPassword123!")).await.unwrap();

        // Simulate a different vault's settings landing under the unlocked vault.
        let other_salt = crate::crypto::generate_salt().unwrap();
        let other = kdf::derive_v2(b"OtherVaultPass123!", other_salt.as_str()).unwrap();
        VaultState::write_v2_kdf_state(
            &rusqlite::Connection::open(&db_path).unwrap(),
            other_salt.as_str(),
            &other.verifier,
        )
        .unwrap();

        let err = vault
            .change_master_password(
                SecretString::from("OtherVaultPass123!"),
                SecretString::from("BrandNewPassword789!"),
            )
            .await
            .unwrap_err();
        assert!(err.contains("does not match"), "unexpected error: {}", err);
        assert_eq!(vault_setting(&db_path, "salt").as_deref(), Some(other_salt.as_str()));

        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_vault_initialization() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());

        // Initially should not be initialized
        assert!(!vault.is_initialized().await.unwrap());

        // Initialize vault
        let result = vault
            .initialize_vault(SecretString::from("TestPassword123!"))
            .await;
        assert!(result.is_ok(), "Failed to initialize vault: {:?}", result);

        // Should now be initialized
        assert!(vault.is_initialized().await.unwrap());

        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_vault_unlock_lock() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());

        vault
            .initialize_vault(SecretString::from("TestPassword123!"))
            .await
            .unwrap();

        // Vault is unlocked after initialization
        assert!(!vault.is_locked().await);

        // Lock vault
        vault.lock_vault().await.unwrap();
        assert!(vault.is_locked().await);

        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_change_master_password_rejected_while_locked() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());

        vault
            .initialize_vault(SecretString::from("TestPassword123!"))
            .await
            .unwrap();
        vault.lock_vault().await.unwrap();

        // Changing the password while locked cannot re-encrypt stored credentials.
        // It must fail rather than silently orphan them behind a new key.
        let result = vault
            .change_master_password(
                SecretString::from("TestPassword123!"),
                SecretString::from("NewPassword456!"),
            )
            .await;
        assert!(result.is_err(), "locked vault must reject password change");

        // The stored hash must be untouched, so the original password still unlocks.
        vault
            .unlock_vault(SecretString::from("TestPassword123!"))
            .await
            .expect("original password must still work after the rejected change");

        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_lock_vault_during_password_change_stays_locked() {
        // Regression test: change_master_password() drops the vault's write lock
        // for the duration of its spawn_blocking re-encryption work, so an explicit
        // lock_vault() call can land in that window. It must not be silently undone
        // when the password change finishes installing the new key.
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());

        vault
            .initialize_vault(SecretString::from("TestPassword123!"))
            .await
            .unwrap();
        assert!(!vault.is_locked().await);

        let vault_clone = vault.clone();
        let change_handle = tokio::spawn(async move {
            vault_clone
                .change_master_password(
                    SecretString::from("TestPassword123!"),
                    SecretString::from("NewPassword456!"),
                )
                .await
        });

        // Give the password-change task a chance to run past its initial lock
        // acquisition (which drops the write lock before the blocking Argon2id /
        // re-encryption work) and reach its `.await` point on that blocking task.
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // An explicit lock during that window must win: finishing the password
        // change must not silently re-unlock the vault.
        vault.lock_vault().await.unwrap();

        let change_result = change_handle.await.unwrap();
        assert!(
            change_result.is_ok(),
            "password change should still succeed: {:?}",
            change_result
        );

        assert!(
            vault.is_locked().await,
            "explicit lock must not be reverted by a concurrently-finishing password change"
        );

        // The new password must already be persisted even though we stayed locked.
        vault
            .unlock_vault(SecretString::from("NewPassword456!"))
            .await
            .expect("new password must work after the interleaved lock");

        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_change_master_password_preserves_credential_ids() {
        // Regression test: change_master_password() used to re-encrypt every credential
        // via delete + re-insert, which minted a brand-new UUID for each row. That
        // silently orphaned any foreign key pointing at the old id (scheduled_tasks.credential_id,
        // monitoring_host_credential.credential_id). A password change must rewrite each
        // credential's encrypted material in place, keeping its id (and created_at) stable.
        use crate::vault::credentials::CredentialManager;

        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());

        vault
            .initialize_vault(SecretString::from("TestPassword123!"))
            .await
            .unwrap();

        let master_key = vault.get_master_key().await.unwrap();
        let credential_manager = CredentialManager::new(db_path.clone());
        let id = credential_manager
            .add_credential(
                &master_key,
                "Test Host".to_string(),
                "root".to_string(),
                "hunter2".to_string(),
                "password".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .expect("failed to add credential");

        let before = credential_manager
            .get_credential(&master_key, &id)
            .expect("credential should exist before password change");

        vault
            .change_master_password(
                SecretString::from("TestPassword123!"),
                SecretString::from("NewPassword456!"),
            )
            .await
            .expect("password change should succeed");

        let new_master_key = vault.get_master_key().await.unwrap();
        let after = credential_manager
            .get_credential(&new_master_key, &id)
            .expect("credential must still be reachable under its original id after password change");

        assert_eq!(after.id, before.id);
        assert_eq!(after.created_at, before.created_at);
        assert_eq!(after.password, "hunter2");

        let summaries = credential_manager
            .list_credentials()
            .expect("failed to list credentials");
        assert_eq!(summaries.len(), 1, "password change must not duplicate credentials");
        assert_eq!(summaries[0].id, id);

        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_credential_write_waits_for_master_password_change() {
        // Regression test (EDW-15): credential writes used to ignore an in-flight
        // change_master_password(). One landing during the re-encryption pass or the
        // post-commit key-swap window was encrypted with the old key and became
        // undecryptable under the new one. Writes now queue behind the rotation via
        // credential_access(), and must come out encrypted under the new key.
        use crate::vault::credentials::CredentialManager;

        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        vault
            .initialize_vault(SecretString::from("TestPassword123!"))
            .await
            .unwrap();
        let old_key = vault.get_master_key().await.unwrap();

        // An in-flight credential operation holds the gate, so the rotation below
        // parks waiting for exclusive access before it touches any vault state.
        let in_flight = vault.credential_access().await.unwrap();

        let rekey = tokio::spawn({
            let vault = vault.clone();
            async move {
                vault
                    .change_master_password(
                        SecretString::from("TestPassword123!"),
                        SecretString::from("NewPassword456!"),
                    )
                    .await
            }
        });
        // The rotation is queued once the gate can't be read-acquired any more: tokio's
        // RwLock is fair, so a waiting writer blocks new readers.
        for _ in 0..1000 {
            if vault.credential_gate.try_read().is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            vault.credential_gate.try_read().is_err(),
            "the rotation must queue on the credential gate"
        );
        assert!(
            !vault.changing_password.load(Ordering::SeqCst),
            "a rotation queued on the gate must not have marked itself in progress yet"
        );

        // A credential write issued mid-rotation.
        let writer = tokio::spawn({
            let vault = vault.clone();
            let db_path = db_path.clone();
            async move {
                let access = vault.credential_access().await?;
                let id = CredentialManager::new(db_path).add_credential(
                    access.key(),
                    "Mid-rotation".to_string(),
                    "root".to_string(),
                    "hunter2".to_string(),
                    "password".to_string(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )?;
                Ok::<_, String>((id, *access.key()))
            }
        });
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        assert!(
            !writer.is_finished(),
            "a credential write must not proceed while a password change is pending"
        );

        drop(in_flight);
        rekey
            .await
            .unwrap()
            .expect("password change should succeed");
        let (id, write_key) = writer
            .await
            .unwrap()
            .expect("credential write should succeed");

        let new_key = vault.get_master_key().await.unwrap();
        assert_ne!(new_key, old_key);
        assert_eq!(
            write_key, new_key,
            "queued write must use the post-rotation key"
        );
        let cred = CredentialManager::new(db_path.clone())
            .get_credential(&new_key, &id)
            .expect("credential written mid-rotation must decrypt under the new key");
        assert_eq!(cred.password, "hunter2");

        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_invalid_password() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());

        vault
            .initialize_vault(SecretString::from("TestPassword123!"))
            .await
            .unwrap();

        // Lock the vault first
        vault.lock_vault().await.unwrap();
        assert!(vault.is_locked().await);

        // Try to unlock with wrong password
        let result = vault
            .unlock_vault(SecretString::from("WrongPassword"))
            .await;
        assert!(result.is_err());
        assert!(vault.is_locked().await);

        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_lockout_persists_across_vault_state_restart() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());

        vault
            .initialize_vault(SecretString::from("TestPassword123!"))
            .await
            .unwrap();
        vault.lock_vault().await.unwrap();

        // 5 failed attempts trips the first lockout tier.
        for _ in 0..5 {
            let _ = vault
                .unlock_vault(SecretString::from("WrongPassword"))
                .await;
        }

        // A brand-new VaultState (simulating an app restart, which resets all
        // in-memory tracking) pointed at the same database must still honor
        // the lockout — even with the *correct* password.
        let restarted = VaultState::new(db_path.clone());
        let result = restarted
            .unlock_vault(SecretString::from("TestPassword123!"))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("locked out"));

        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_successful_unlock_clears_persisted_lockout() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());

        vault
            .initialize_vault(SecretString::from("TestPassword123!"))
            .await
            .unwrap();
        vault.lock_vault().await.unwrap();

        // A few failed attempts (short of a lockout tier), then a success.
        for _ in 0..3 {
            let _ = vault
                .unlock_vault(SecretString::from("WrongPassword"))
                .await;
        }
        vault
            .unlock_vault(SecretString::from("TestPassword123!"))
            .await
            .unwrap();
        vault.lock_vault().await.unwrap();

        // A fresh VaultState should see no residual lockout from those
        // earlier failed attempts — the successful unlock must have cleared
        // the persisted state, not just the in-memory counter.
        let restarted = VaultState::new(db_path.clone());
        let result = restarted
            .unlock_vault(SecretString::from("TestPassword123!"))
            .await;
        assert!(result.is_ok());

        cleanup_test_db(&db_path);
    }
}

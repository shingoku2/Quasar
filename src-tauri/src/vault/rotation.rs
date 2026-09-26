//! Re-keying the vault: master-password change (invariants 2-4) and the legacy v1 -> v2
//! migration on unlock (RSEC-001, invariant 17), which share the re-encryption code.

use super::*;

impl VaultState {
    /// Changes the master password. The rotation runs as its own task, which this awaits:
    /// if the caller is dropped, the rotation still finishes. Cancelling it between the DB
    /// commit and installing the new key left memory on the old key while the database used
    /// the new one, so credentials written next were orphaned (P7-4 review).
    pub async fn change_master_password(
        &self,
        current_password: SecretString,
        new_password: SecretString,
    ) -> Result<(), String> {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            this.rotate_master_password(current_password, new_password).await
        })
        .await
        .map_err(|e| format!("Task failed: {}", e))?
    }

    pub(super) async fn rotate_master_password(
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

    /// Stores v2 state (salt, verifier, version) and deletes any legacy key-equivalent hash.
    pub(super) fn write_v2_kdf_state(
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
    pub(super) fn reencrypt_all_credentials(
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

    /// Whether a credential row's secrets were encrypted under `key`. Judged on the first
    /// encrypted field the row has: the password, else the private key, else the key
    /// passphrase. A key-only credential has a NULL password triple (migration 009; switching
    /// a credential to `ssh_key` clears it), so reading only the password made one such row
    /// block every password change and every v1→v2 migration (PR #68 review). A row with no
    /// encrypted field has nothing to carry over, so it counts as decryptable. `Ok(false)`
    /// only when that field fails AES-GCM authentication; a partial or mis-sized triple is an
    /// `Err`, never "absent".
    pub(super) fn password_blob_decrypts(
        conn: &rusqlite::Connection,
        credential_id: &str,
        key: &[u8; 32],
    ) -> Result<bool, String> {
        type Blob = Option<Vec<u8>>;
        let triples: [(Blob, Blob, Blob); 3] = conn
            .query_row(
                "SELECT encrypted_password, nonce, tag,
                        encrypted_private_key, private_key_nonce, private_key_tag,
                        encrypted_key_passphrase, key_passphrase_nonce, key_passphrase_tag
                 FROM credentials WHERE id = ?1",
                [credential_id],
                |r| {
                    Ok([
                        (r.get(0)?, r.get(1)?, r.get(2)?),
                        (r.get(3)?, r.get(4)?, r.get(5)?),
                        (r.get(6)?, r.get(7)?, r.get(8)?),
                    ])
                },
            )
            .map_err(|e| format!("Failed to read credential {}: {}", credential_id, e))?;
        for triple in triples {
            let (ciphertext, nonce, tag) = match triple {
                (None, None, None) => continue,
                (Some(c), Some(n), Some(t)) => (c, n, t),
                _ => return Err(format!("Credential {} has an incomplete encrypted field", credential_id)),
            };
            let nonce: [u8; 12] = nonce
                .try_into()
                .map_err(|_| format!("Credential {} has a malformed nonce", credential_id))?;
            let tag: [u8; 16] = tag
                .try_into()
                .map_err(|_| format!("Credential {} has a malformed auth tag", credential_id))?;
            return Ok(crypto::decrypt(&ciphertext, key, &nonce, &tag).is_ok());
        }
        Ok(true)
    }

    /// VACUUM (drops free pages, where superseded hashes linger) and a truncating WAL
    /// checkpoint. True only if both fully completed; `wal_checkpoint` reports "busy" in
    /// its result row rather than as an error.
    pub(super) fn scrub_database(conn: &rusqlite::Connection) -> bool {
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
    pub(super) fn scrub_or_mark_pending(conn: &rusqlite::Connection) {
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

    /// `import_database` leaves a full copy of the previous database at `<db>.bak`: the vault
    /// that was replaced, which may be a different vault from this one. If it's a v1 vault,
    /// its `master_password_hash` *is* its key. Best effort, never fails the caller:
    /// - same password (the same vault): migrate the copy to v2 too, so it stays unlockable
    ///   and stops exposing its key;
    /// - a different password: leave it untouched. Deleting the hash (what this used to do)
    ///   made the recovery copy impossible to unlock after an import (PR #68 review). It's a
    ///   pre-migration copy, which CLAUDE.md invariant 17 says to treat as plaintext; it
    ///   migrates itself when imported and unlocked with its own password.
    pub(super) fn migrate_backup_if_same_vault(db_path: &str, password: &[u8]) {
        let backup_path = format!("{}.bak", db_path);
        if !std::path::Path::new(&backup_path).exists() {
            return;
        }
        let stored = match db::open_connection(&backup_path).and_then(|conn| Self::load_stored_kdf(&conn)) {
            Ok(stored) => stored,
            Err(e) => {
                log::warn!("Could not read vault settings from {}: {}", backup_path, e);
                return;
            }
        };
        let StoredKdf::Legacy { salt_b64, .. } = &stored else {
            return; // already v2: nothing key-equivalent is stored
        };
        match Self::check_password(&stored, password) {
            Ok(Some(legacy_key)) => {
                if let Err(e) = Self::migrate_legacy_vault_at(&backup_path, password, &legacy_key, salt_b64, false) {
                    log::warn!("Could not migrate the legacy backup {}: {}", backup_path, e);
                }
            }
            Ok(None) => log::warn!(
                "{} is a legacy vault with a different password; left as is (treat it as plaintext)",
                backup_path
            ),
            Err(e) => log::warn!("Could not check the legacy backup {}: {}", backup_path, e),
        }
    }

    /// Migrates a v1 (legacy) vault to v2 under a **fresh salt**: a new salt means the new
    /// key can't be derived from the old stored hash, so old copies of the database (backups,
    /// exports, `quasar.db.bak`) don't leak it. Those copies still expose their own
    /// (legacy) ciphertexts, which no migration can fix. Returns the new encryption key.
    /// Caller holds the credential gate exclusively.
    pub(super) fn migrate_legacy_vault(
        db_path: &str,
        password: &[u8],
        legacy_key: &[u8; 32],
        legacy_salt_b64: &str,
    ) -> Result<Zeroizing<[u8; 32]>, String> {
        Self::migrate_legacy_vault_at(db_path, password, legacy_key, legacy_salt_b64, true)
    }

    /// `migrate_legacy_vault`, optionally also handling `<db_path>.bak` (off when migrating
    /// the backup itself).
    pub(super) fn migrate_legacy_vault_at(
        db_path: &str,
        password: &[u8],
        legacy_key: &[u8; 32],
        legacy_salt_b64: &str,
        migrate_backup: bool,
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
        if migrate_backup {
            Self::migrate_backup_if_same_vault(db_path, password);
        }

        Ok(new_keys.enc_key)
    }
}

pub mod credentials;
pub mod ssh_keys;
pub mod audit;

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use zeroize::Zeroize;
use secrecy::{Secret, ExposeSecret};
use argon2::{
    password_hash::{PasswordHash, PasswordVerifier, SaltString, PasswordHasher, rand_core::OsRng},
    Argon2, Params, Version,
};
use password_hash::Salt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::db;

pub use credentials::{Credential, CredentialSummary, CredentialManager};
pub use ssh_keys::{SshKeyManager, SshHostKey, TrustStatus, HostKeyVerificationResult};
pub use audit::{AuditLogManager, AuditLogEntry, AuditLogFilter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSettings {
    pub auto_lock_timeout_minutes: u64,
    pub require_password_on_credential_use: bool,
    pub vault_initialized: bool,
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            auto_lock_timeout_minutes: 15,
            require_password_on_credential_use: false,
            vault_initialized: false,
        }
    }
}

#[derive(Clone)]
pub struct VaultState {
    inner: Arc<RwLock<VaultStateInner>>,
}

struct VaultStateInner {
    master_key: Option<MasterKey>,
    settings: VaultSettings,
    last_activity: Option<Instant>,
    failed_attempts: u32,
    lockout_until: Option<Instant>,
    db_path: String,
    changing_password: bool,
}

struct MasterKey {
    key: [u8; 32],
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
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
                changing_password: false,
            })),
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

    pub async fn initialize_vault(&self, master_password: Secret<String>) -> Result<(), String> {
        if self.is_initialized().await? {
            return Err("Vault is already initialized".to_string());
        }

        // Validate password strength
        if master_password.expose_secret().len() < 12 {
            return Err("Master password must be at least 12 characters".to_string());
        }

        let mut inner = self.inner.write().await;
        let conn = db::open_connection(&inner.db_path)?;

        // Generate salt for key derivation
        let salt = SaltString::generate(&mut password_hash::rand_core::OsRng);
        
        // OWASP-recommended Argon2id params for key derivation (47 MiB memory, 2 iterations, 1 parallelism)
        let params = Params::new(47104, 2, 1, Some(32))
            .map_err(|e| format!("Failed to create Argon2 params: {}", e))?;
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
        
        // Hash master password for verification
        let password_hash = argon2
            .hash_password(master_password.expose_secret().as_bytes(), &salt)
            .map_err(|e| format!("Failed to hash password: {}", e))?
            .to_string();

        // Store vault settings (use INSERT OR REPLACE to handle existing keys)
        conn.execute(
            "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["master_password_hash", password_hash, chrono::Utc::now().timestamp()],
        ).map_err(|e| format!("Failed to store password hash: {}", e))?;

        conn.execute(
            "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["salt", salt.as_str(), chrono::Utc::now().timestamp()],
        ).map_err(|e| format!("Failed to store salt: {}", e))?;

        conn.execute(
            "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["vault_initialized", "true", chrono::Utc::now().timestamp()],
        ).map_err(|e| format!("Failed to mark vault as initialized: {}", e))?;

        conn.execute(
            "INSERT OR REPLACE INTO vault_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["auto_lock_timeout", "15", chrono::Utc::now().timestamp()],
        ).map_err(|e| format!("Failed to store auto-lock timeout: {}", e))?;

        inner.settings.vault_initialized = true;

        // Derive master key and unlock vault immediately after initialization
        let salt_string = SaltString::from_b64(salt.as_str())
            .map_err(|e| format!("Failed to parse salt: {}", e))?;

        let salt_decoded = Salt::from_b64(salt_string.as_str())
            .map_err(|e| format!("Failed to decode salt: {}", e))?;

        let mut master_key = [0u8; 32];
        let mut salt_bytes = [0u8; 64];
        let salt_decoded_bytes = salt_decoded.decode_b64(&mut salt_bytes)
            .map_err(|e| format!("Failed to decode salt bytes: {}", e))?;

        argon2.hash_password_into(master_password.expose_secret().as_bytes(), salt_decoded_bytes, &mut master_key)
            .map_err(|e| format!("Failed to derive key: {}", e))?;

        inner.master_key = Some(MasterKey { key: master_key });
        inner.last_activity = Some(Instant::now());

        // Log audit event
        Self::log_audit_event(&conn, "vault_initialize", None, None, "vault", "initialize", "success", None)?;

        Ok(())
    }

    pub async fn unlock_vault(&self, master_password: Secret<String>) -> Result<(), String> {
        let mut inner = self.inner.write().await;

        // Check if locked out
        if let Some(lockout_until) = inner.lockout_until {
            if Instant::now() < lockout_until {
                let remaining = (lockout_until - Instant::now()).as_secs();
                return Err(format!("Vault is locked out. Try again in {} seconds", remaining));
            } else {
                inner.lockout_until = None;
                inner.failed_attempts = 0;
            }
        }

        let conn = db::open_connection(&inner.db_path)?;

        // Get stored password hash
        let stored_hash: String = conn.query_row(
            "SELECT value FROM vault_settings WHERE key = 'master_password_hash'",
            [],
            |row| row.get(0),
        ).map_err(|_| "Vault not initialized")?;

        // Get salt
        let salt_str: String = conn.query_row(
            "SELECT value FROM vault_settings WHERE key = 'salt'",
            [],
            |row| row.get(0),
        ).map_err(|_| "Salt not found")?;

        // Verify password using OWASP-recommended params
        let params = Params::new(47104, 2, 1, Some(32))
            .map_err(|e| format!("Failed to create Argon2 params: {}", e))?;
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
        
        let parsed_hash = PasswordHash::new(&stored_hash)
            .map_err(|e| format!("Failed to parse password hash: {}", e))?;
        
        if argon2.verify_password(master_password.expose_secret().as_bytes(), &parsed_hash).is_err() {
            inner.failed_attempts += 1;
            
            // Implement lockout policy
            if inner.failed_attempts >= 15 {
                inner.lockout_until = Some(Instant::now() + Duration::from_secs(3600));
                return Err("Too many failed attempts. Vault locked for 60 minutes".to_string());
            } else if inner.failed_attempts >= 10 {
                inner.lockout_until = Some(Instant::now() + Duration::from_secs(900));
                return Err("Too many failed attempts. Vault locked for 15 minutes".to_string());
            } else if inner.failed_attempts >= 5 {
                inner.lockout_until = Some(Instant::now() + Duration::from_secs(300));
                return Err("Too many failed attempts. Vault locked for 5 minutes".to_string());
            }
            
            return Err(format!("Invalid master password. {} attempts remaining before lockout", 5u32.saturating_sub(inner.failed_attempts)));
        }

        // Derive master key from password using hash_password_into for direct key derivation
        let salt_string = SaltString::from_b64(&salt_str)
            .map_err(|e| format!("Failed to parse salt: {}", e))?;
        
        // Decode the base64 salt to raw bytes
        let salt = Salt::from_b64(salt_string.as_str())
            .map_err(|e| format!("Failed to decode salt: {}", e))?;
        
        let mut master_key = [0u8; 32];
        // FIX: Decode base64 salt to raw bytes - binary data must not be converted through UTF-8
        // Per NIST SP 800-132: salt is arbitrary binary data, not text
        let mut salt_bytes = [0u8; 64]; // Max salt length
        let salt_decoded = salt.decode_b64(&mut salt_bytes)
            .map_err(|e| format!("Failed to decode salt bytes: {}", e))?;
        argon2.hash_password_into(master_password.expose_secret().as_bytes(), salt_decoded, &mut master_key)
            .map_err(|e| format!("Failed to derive key: {}", e))?;

        inner.master_key = Some(MasterKey { key: master_key });
        inner.last_activity = Some(Instant::now());
        inner.failed_attempts = 0;
        inner.lockout_until = None;

        // Log audit event
        Self::log_audit_event(&conn, "vault_unlock", None, None, "vault", "unlock", "success", None)?;

        Ok(())
    }

    pub async fn lock_vault(&self) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        inner.master_key = None;
        inner.last_activity = None;

        let conn = db::open_connection(&inner.db_path)?;
        
        Self::log_audit_event(&conn, "vault_lock", None, None, "vault", "lock", "success", None)?;

        Ok(())
    }

    pub async fn check_auto_lock(&self) -> Result<bool, String> {
        let mut inner = self.inner.write().await;
        
        if inner.master_key.is_none() || inner.changing_password {
            return Ok(false);
        }

        if let Some(last_activity) = inner.last_activity {
            let timeout = Duration::from_secs(inner.settings.auto_lock_timeout_minutes * 60);
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

    pub async fn get_master_key(&self) -> Result<[u8; 32], String> {
        self.update_activity().await;
        let inner = self.inner.read().await;
        inner.master_key.as_ref()
            .map(|mk| mk.key)
            .ok_or_else(|| "Vault is locked".to_string())
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
        let auto_lock_timeout_minutes: u64 = conn
            .query_row(
                "SELECT value FROM vault_settings WHERE key = 'auto_lock_timeout'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(15);
        VaultSettings {
            vault_initialized,
            auto_lock_timeout_minutes,
            require_password_on_credential_use: in_memory.require_password_on_credential_use,
        }
    }

    pub async fn change_master_password(&self, current_password: &str, new_password: &str) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        
        // Set flag to prevent auto-lock during password change
        inner.changing_password = true;
        
        let db_path = inner.db_path.clone();
        let master_key_option = inner.master_key.as_ref().map(|mk| mk.key);
        let current_password = current_password.to_string();
        let new_password = new_password.to_string();

        // Perform heavy crypto and DB operations in a blocking thread
        let result = tauri::async_runtime::spawn_blocking(move || {
            // FIX: Use single connection for entire operation to prevent connection leak
            let mut conn = db::open_connection(&db_path)?;
            
            let stored_hash: String = conn.query_row(
                "SELECT value FROM vault_settings WHERE key = 'master_password_hash'",
                [],
                |row| row.get(0)
            ).map_err(|_| "Vault not initialized".to_string())?;
            
            // Use OWASP-recommended Argon2 params
            let params = Params::new(47104, 2, 1, Some(32))
                .map_err(|e| format!("Failed to create Argon2 params: {}", e))?;
            let argon2 = Argon2::new(
                argon2::Algorithm::Argon2id,
                Version::V0x13,
                params,
            );
            
            let parsed_hash = PasswordHash::new(&stored_hash)
                .map_err(|e| format!("Invalid password hash: {}", e))?;
            
            argon2
                .verify_password(current_password.as_bytes(), &parsed_hash)
                .map_err(|_| "Invalid current password".to_string())?;
            
            // Generate new hash and salt
            let new_salt = SaltString::generate(&mut OsRng);
            
            let password_hash = argon2.hash_password(new_password.as_bytes(), &new_salt)
                .map_err(|e| format!("Failed to hash password: {}", e))?
                .to_string();
            
            // Derive new master key using hash_password_into with proper salt bytes
            let mut new_master_key = [0u8; 32];
            // FIX: Decode base64 salt to raw bytes (not UTF-8 string)
            let new_salt_decoded = Salt::from_b64(new_salt.as_str())
                .map_err(|e| format!("Failed to parse salt: {}", e))?;
            let mut new_salt_bytes = [0u8; 64]; // Max salt length
            let new_salt_raw = new_salt_decoded.decode_b64(&mut new_salt_bytes)
                .map_err(|e| format!("Failed to decode salt bytes: {}", e))?;
            argon2.hash_password_into(new_password.as_bytes(), new_salt_raw, &mut new_master_key)
                .map_err(|e| format!("Failed to derive key: {}", e))?;
            
            // Re-encrypt all credentials with new key using transaction for safety
            if let Some(old_key) = master_key_option {
                let credential_manager = credentials::CredentialManager::new(db_path.clone());
                let summaries = credential_manager.list_credentials()?;
                
                // FIX: Reuse existing connection for transaction (no second connection)
                let tx = conn.transaction()
                    .map_err(|e| format!("Failed to begin transaction: {}", e))?;
                
                // Collect all re-encrypted credentials first
                let mut re_encrypted_credentials = Vec::new();
                for summary in &summaries {
                    let credential = credential_manager.get_credential(&old_key, &summary.id)?;
                    re_encrypted_credentials.push((
                        summary.id.clone(),
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
                    ));
                }
                
                // Now delete and re-add within transaction
                let mut first_credential_id: Option<String> = None;
                for (id, name, username, password, cred_type, host, port, metadata, key_path, private_key, key_passphrase) in re_encrypted_credentials {
                    credential_manager.delete_credential_tx(&tx, &id)?;
                    let new_id = credential_manager.add_credential_tx(
                        &tx,
                        &new_master_key,
                        name,
                        username,
                        password,
                        cred_type,
                        host,
                        port,
                        metadata,
                        key_path,
                        private_key,
                        key_passphrase,
                    )?;
                    
                    // Store first credential ID for validation
                    if first_credential_id.is_none() {
                        first_credential_id = Some(new_id);
                    }
                }
                
                // Validate re-encryption by attempting to decrypt first credential with new key
                if let Some(cred_id) = first_credential_id {
                    // This will fail if encryption/decryption doesn't work with new key
                    let _ = credential_manager.get_credential(&new_master_key, &cred_id)
                        .map_err(|e| format!("Re-encryption validation failed: {}", e))?;
                }
                
                // Update stored hash and salt within same transaction
                let now = chrono::Utc::now().timestamp();
                tx.execute(
                    "UPDATE vault_settings SET value = ?1, updated_at = ?2 WHERE key = 'master_password_hash'",
                    rusqlite::params![password_hash, now],
                ).map_err(|e| format!("Failed to update password hash: {}", e))?;
                
                tx.execute(
                    "UPDATE vault_settings SET value = ?1, updated_at = ?2 WHERE key = 'salt'",
                    rusqlite::params![new_salt.as_str(), now],
                ).map_err(|e| format!("Failed to update salt: {}", e))?;
                
                // Commit transaction (automatically rolls back on drop if not committed)
                tx.commit()
                    .map_err(|e| format!("Failed to commit transaction: {}", e))?;
            } else {
                // No credentials to re-encrypt, just update password
                let now = chrono::Utc::now().timestamp();
                conn.execute(
                    "UPDATE vault_settings SET value = ?1, updated_at = ?2 WHERE key = 'master_password_hash'",
                    rusqlite::params![password_hash, now],
                ).map_err(|e| format!("Failed to update password hash: {}", e))?;
                
                conn.execute(
                    "UPDATE vault_settings SET value = ?1, updated_at = ?2 WHERE key = 'salt'",
                    rusqlite::params![new_salt.as_str(), now],
                ).map_err(|e| format!("Failed to update salt: {}", e))?;
            }
            
            Self::log_audit_event(&conn, "password_change", None, None, "vault", "update", "success", None)?;
            
            Ok::<[u8; 32], String>(new_master_key)
        }).await
            .map_err(|e| format!("Task failed: {}", e))
            .and_then(|inner_result| inner_result);
        
        // CRITICAL: Always reset changing_password on ALL code paths (success, error, panic).
        // If this flag stays true, auto-lock is permanently disabled (security vulnerability).
        inner.changing_password = false;
        
        // Now propagate the error (flag is already reset)
        let new_master_key = result?;
        
        // Update master key in memory
        inner.master_key = Some(MasterKey { key: new_master_key });
        inner.last_activity = Some(Instant::now());
        
        Ok(())
    }

    pub async fn update_settings(&self, settings: VaultSettings) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        let conn = db::open_connection(&inner.db_path)?;

        conn.execute(
            "UPDATE vault_settings SET value = ?1, updated_at = ?2 WHERE key = 'auto_lock_timeout'",
            rusqlite::params![settings.auto_lock_timeout_minutes.to_string(), chrono::Utc::now().timestamp()],
        ).map_err(|e| format!("Failed to update auto-lock timeout: {}", e))?;

        inner.settings = settings;
        Ok(())
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

    fn setup_test_db() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        // Create a unique temporary file for each test
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let db_path = format!("test_vault_{}.db", timestamp);
        
        let conn = rusqlite::Connection::open(&db_path).expect("Failed to open test database");
        
        // Create vault_settings table
        conn.execute(
            "CREATE TABLE vault_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        ).expect("Failed to create vault_settings table");
        
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
        ).expect("Failed to create audit log table");
        
        db_path
    }
    
    fn cleanup_test_db(db_path: &str) {
        if let Err(e) = std::fs::remove_file(db_path) {
            eprintln!("Warning: Failed to cleanup test DB {}: {}", db_path, e);
        }
    }

    #[tokio::test]
    async fn test_vault_initialization() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        
        // Initially should not be initialized
        assert!(!vault.is_initialized().await.unwrap());
        
        // Initialize vault
        let result = vault.initialize_vault(Secret::new("TestPassword123!".to_string())).await;
        assert!(result.is_ok(), "Failed to initialize vault: {:?}", result);
        
        // Should now be initialized
        assert!(vault.is_initialized().await.unwrap());
        
        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_vault_unlock_lock() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        
        vault.initialize_vault(Secret::new("TestPassword123!".to_string())).await.unwrap();
        
        // Vault is unlocked after initialization
        assert!(!vault.is_locked().await);
        
        // Lock vault
        vault.lock_vault().await.unwrap();
        assert!(vault.is_locked().await);
        
        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_invalid_password() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        
        vault.initialize_vault(Secret::new("TestPassword123!".to_string())).await.unwrap();
        
        // Lock the vault first
        vault.lock_vault().await.unwrap();
        assert!(vault.is_locked().await);
        
        // Try to unlock with wrong password
        let result = vault.unlock_vault(Secret::new("WrongPassword".to_string())).await;
        assert!(result.is_err());
        assert!(vault.is_locked().await);
        
        cleanup_test_db(&db_path);
    }
}

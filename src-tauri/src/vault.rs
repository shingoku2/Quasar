pub mod credentials;
pub mod ssh_keys;
pub mod audit;

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use zeroize::Zeroize;
use argon2::{
    password_hash::{PasswordHash, PasswordVerifier, SaltString, PasswordHasher, rand_core::OsRng},
    Argon2, Params, Version,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
            })),
        }
    }

    pub async fn is_locked(&self) -> bool {
        let inner = self.inner.read().await;
        inner.master_key.is_none()
    }

    pub async fn is_initialized(&self) -> Result<bool, String> {
        let inner = self.inner.read().await;
        let conn = rusqlite::Connection::open(&inner.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;
        
        let result: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT value FROM vault_settings WHERE key = 'vault_initialized'",
            [],
            |row| row.get(0),
        );

        Ok(result.is_ok() && result.unwrap() == "true")
    }

    pub async fn initialize_vault(&self, master_password: String) -> Result<(), String> {
        if self.is_initialized().await? {
            return Err("Vault is already initialized".to_string());
        }

        // Validate password strength
        if master_password.len() < 12 {
            return Err("Master password must be at least 12 characters".to_string());
        }

        let mut inner = self.inner.write().await;
        let conn = rusqlite::Connection::open(&inner.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        // Generate salt for key derivation
        let salt = SaltString::generate(&mut password_hash::rand_core::OsRng);
        
        // Hash master password for verification (using default Argon2 params)
        let password_hash = Argon2::default()
            .hash_password(master_password.as_bytes(), &salt)
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

        Ok(())
    }

    pub async fn unlock_vault(&self, master_password: String) -> Result<(), String> {
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

        let conn = rusqlite::Connection::open(&inner.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

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

        // Verify password
        let parsed_hash = PasswordHash::new(&stored_hash)
            .map_err(|e| format!("Failed to parse password hash: {}", e))?;
        
        if Argon2::default().verify_password(master_password.as_bytes(), &parsed_hash).is_err() {
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
            
            return Err(format!("Invalid master password. {} attempts remaining", 5 - inner.failed_attempts.min(5)));
        }

        // Derive master key from password
        let salt = SaltString::from_b64(&salt_str)
            .map_err(|e| format!("Failed to parse salt: {}", e))?;
        
        // Use custom Argon2 params for key derivation (64 MB memory, 3 iterations)
        let params = Params::new(65536, 3, 4, Some(32))
            .map_err(|e| format!("Failed to create Argon2 params: {}", e))?;
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
        
        let password_hash = argon2.hash_password(master_password.as_bytes(), &salt)
            .map_err(|e| format!("Failed to derive key: {}", e))?;
        
        let hash_bytes = password_hash.hash.ok_or("No hash output")?;
        let mut master_key = [0u8; 32];
        master_key.copy_from_slice(hash_bytes.as_bytes());

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

        let conn = rusqlite::Connection::open(&inner.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;
        
        Self::log_audit_event(&conn, "vault_lock", None, None, "vault", "lock", "success", None)?;

        Ok(())
    }

    pub async fn check_auto_lock(&self) -> Result<bool, String> {
        let mut inner = self.inner.write().await;
        
        if inner.master_key.is_none() {
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
        let inner = self.inner.read().await;
        inner.settings.clone()
    }

    pub async fn change_master_password(&self, current_password: &str, new_password: &str) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        
        // Verify current password
        let conn = rusqlite::Connection::open(&inner.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;
        
        let stored_hash: String = conn.query_row(
            "SELECT value FROM vault_settings WHERE key = 'master_password_hash'",
            [],
            |row| row.get(0)
        ).map_err(|_| "Vault not initialized".to_string())?;
        
        let parsed_hash = PasswordHash::new(&stored_hash)
            .map_err(|e| format!("Invalid password hash: {}", e))?;
        
        Argon2::default()
            .verify_password(current_password.as_bytes(), &parsed_hash)
            .map_err(|_| "Invalid current password".to_string())?;
        
        // Generate new hash
        let salt = SaltString::generate(&mut OsRng);
        let params = Params::new(65536, 3, 4, Some(32))
            .map_err(|e| format!("Failed to create Argon2 params: {}", e))?;
        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            Version::V0x13,
            params,
        );
        
        let password_hash = argon2.hash_password(new_password.as_bytes(), &salt)
            .map_err(|e| format!("Failed to hash password: {}", e))?
            .to_string();
        
        // Derive new master key
        let mut new_master_key = [0u8; 32];
        argon2.hash_password_into(new_password.as_bytes(), salt.as_str().as_bytes(), &mut new_master_key)
            .map_err(|e| format!("Failed to derive key: {}", e))?;
        
        // Re-encrypt all credentials with new key using transaction for safety
        if let Some(ref old_key) = inner.master_key {
            let credential_manager = credentials::CredentialManager::new(inner.db_path.clone());
            let summaries = credential_manager.list_credentials()?;
            
            // Start transaction - need mutable connection
            let mut conn_mut = rusqlite::Connection::open(&inner.db_path)
                .map_err(|e| format!("Failed to open database: {}", e))?;
            let tx = conn_mut.transaction()
                .map_err(|e| format!("Failed to begin transaction: {}", e))?;
            
            // Collect all re-encrypted credentials first
            let mut re_encrypted_credentials = Vec::new();
            for summary in &summaries {
                let credential = credential_manager.get_credential(&old_key.key, &summary.id)?;
                re_encrypted_credentials.push((
                    summary.id.clone(),
                    credential.name,
                    credential.username,
                    credential.password,
                    credential.credential_type,
                    credential.metadata,
                ));
            }
            
            // Now delete and re-add within transaction
            for (id, name, username, password, cred_type, metadata) in re_encrypted_credentials {
                credential_manager.delete_credential(&id)?;
                credential_manager.add_credential(
                    &new_master_key,
                    name,
                    username,
                    password,
                    cred_type,
                    metadata,
                )?;
            }
            
            // Update stored hash and salt within same transaction
            let now = chrono::Utc::now().timestamp();
            tx.execute(
                "UPDATE vault_settings SET value = ?1, updated_at = ?2 WHERE key = 'master_password_hash'",
                rusqlite::params![password_hash, now],
            ).map_err(|e| format!("Failed to update password hash: {}", e))?;
            
            tx.execute(
                "UPDATE vault_settings SET value = ?1, updated_at = ?2 WHERE key = 'salt'",
                rusqlite::params![salt.as_str(), now],
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
                rusqlite::params![salt.as_str(), now],
            ).map_err(|e| format!("Failed to update salt: {}", e))?;
        }
        
        // Update master key in memory
        inner.master_key = Some(MasterKey { key: new_master_key });
        inner.last_activity = Some(Instant::now());
        
        Self::log_audit_event(&conn, "password_change", None, None, "vault", "update", "success", None)?;
        
        Ok(())
    }

    pub async fn update_settings(&self, settings: VaultSettings) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        let conn = rusqlite::Connection::open(&inner.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

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
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn test_vault_initialization() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        
        // Initially should not be initialized
        assert!(!vault.is_initialized().await.unwrap());
        
        // Initialize vault
        let result = vault.initialize_vault("TestPassword123!".to_string()).await;
        assert!(result.is_ok(), "Failed to initialize vault: {:?}", result);
        
        // Should now be initialized
        assert!(vault.is_initialized().await.unwrap());
        
        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_vault_unlock_lock() {
        let db_path = setup_test_db();
        let vault = VaultState::new(db_path.clone());
        
        vault.initialize_vault("TestPassword123!".to_string()).await.unwrap();
        
        // Should be locked initially
        assert!(vault.is_locked().await);
        
        // Unlock with correct password
        let result = vault.unlock_vault("TestPassword123!".to_string()).await;
        assert!(result.is_ok(), "Failed to unlock vault: {:?}", result);
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
        
        vault.initialize_vault("TestPassword123!".to_string()).await.unwrap();
        
        // Try to unlock with wrong password
        let result = vault.unlock_vault("WrongPassword".to_string()).await;
        assert!(result.is_err());
        assert!(vault.is_locked().await);
        
        cleanup_test_db(&db_path);
    }
}

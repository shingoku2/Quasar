use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::db;
use crate::crypto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: String,
    pub name: String,
    pub username: String,
    /// Password for password-based auth; empty when credential_type is ssh_key.
    pub password: String,
    pub credential_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub metadata: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_used_at: Option<i64>,
    /// Path to private key file (e.g. ~/.ssh/id_ed25519). Used when credential_type is ssh_key.
    pub key_path: Option<String>,
    /// Decrypted private key PEM. Set when key is stored in DB (encrypted_private_key).
    pub private_key: Option<String>,
    /// Passphrase for decrypting the private key (file or stored key).
    pub key_passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: String,
    pub name: String,
    pub username: String,
    pub credential_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_used_at: Option<i64>,
}

impl From<Credential> for CredentialSummary {
    fn from(cred: Credential) -> Self {
        Self {
            id: cred.id,
            name: cred.name,
            username: cred.username,
            credential_type: cred.credential_type,
            host: cred.host,
            port: cred.port,
            created_at: cred.created_at,
            updated_at: cred.updated_at,
            last_used_at: cred.last_used_at,
        }
    }
}

pub struct CredentialManager {
    db_path: String,
}

fn decrypt_optional_blob(
    master_key: &[u8; 32],
    ciphertext: Option<Vec<u8>>,
    nonce_vec: Option<Vec<u8>>,
    tag_vec: Option<Vec<u8>>,
) -> Result<Option<String>, rusqlite::Error> {
    let (Some(ciphertext), Some(nonce_vec), Some(tag_vec)) = (ciphertext, nonce_vec, tag_vec) else {
        return Ok(None);
    };
    if nonce_vec.len() != 12 || tag_vec.len() != 16 {
        return Ok(None);
    }
    let mut nonce = [0u8; 12];
    let mut tag = [0u8; 16];
    nonce.copy_from_slice(&nonce_vec);
    tag.copy_from_slice(&tag_vec);
    let decrypted = crypto::decrypt(&ciphertext, master_key, &nonce, &tag)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))?;
    let s = String::from_utf8(decrypted)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(Some(s))
}

impl CredentialManager {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    pub fn add_credential(
        &self,
        master_key: &[u8; 32],
        name: String,
        username: String,
        password: String,
        credential_type: String,
        host: Option<String>,
        port: Option<u16>,
        metadata: Option<String>,
        key_path: Option<String>,
        private_key: Option<String>,
        key_passphrase: Option<String>,
    ) -> Result<String, String> {
        let conn = db::open_connection(&self.db_path)?;

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        // Encrypt password (use empty string for ssh_key when no password)
        let password_bytes = password.as_bytes();
        let (ciphertext, nonce, tag) = crypto::encrypt(password_bytes, master_key)?;

        let (encrypted_key, key_nonce, key_tag) = match &private_key {
            Some(pk) => {
                let (c, n, t) = crypto::encrypt(pk.as_bytes(), master_key)?;
                (Some(c), Some(n.to_vec()), Some(t.to_vec()))
            }
            None => (None, None, None),
        };
        let (encrypted_kp, kp_nonce, kp_tag) = match &key_passphrase {
            Some(kp) => {
                let (c, n, t) = crypto::encrypt(kp.as_bytes(), master_key)?;
                (Some(c), Some(n.to_vec()), Some(t.to_vec()))
            }
            None => (None, None, None),
        };

        conn.execute(
            "INSERT INTO credentials (id, name, username, encrypted_password, nonce, tag, credential_type, host, port, metadata, created_at, updated_at,
             key_path, encrypted_private_key, private_key_nonce, private_key_tag, encrypted_key_passphrase, key_passphrase_nonce, key_passphrase_tag)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            rusqlite::params![
                id,
                name,
                username,
                ciphertext,
                nonce.to_vec(),
                tag.to_vec(),
                credential_type,
                host,
                port,
                metadata,
                now,
                now,
                key_path,
                encrypted_key,
                key_nonce,
                key_tag,
                encrypted_kp,
                kp_nonce,
                kp_tag,
            ],
        ).map_err(|e| format!("Failed to insert credential: {}", e))?;

        // Log audit event
        Self::log_audit_event(
            &conn,
            "credential_create",
            Some(&id),
            Some("credential"),
            "create",
            "success",
            None,
        )?;

        Ok(id)
    }

    pub fn get_credential(
        &self,
        master_key: &[u8; 32],
        credential_id: &str,
    ) -> Result<Credential, String> {
        let conn = db::open_connection(&self.db_path)?;

        let mut stmt = conn.prepare(
            "SELECT id, name, username, encrypted_password, nonce, tag, credential_type, host, port, metadata, created_at, updated_at, last_used_at,
                    key_path, encrypted_private_key, private_key_nonce, private_key_tag, encrypted_key_passphrase, key_passphrase_nonce, key_passphrase_tag
             FROM credentials WHERE id = ?1"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let credential = stmt.query_row([credential_id], |row| {
            let encrypted_password: Vec<u8> = row.get(3)?;
            let nonce_vec: Vec<u8> = row.get(4)?;
            let tag_vec: Vec<u8> = row.get(5)?;

            if nonce_vec.len() != 12 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid nonce length: expected 12, got {}", nonce_vec.len()),
                ))));
            }

            if tag_vec.len() != 16 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid auth tag length: expected 16, got {}", tag_vec.len()),
                ))));
            }

            let mut nonce = [0u8; 12];
            let mut tag = [0u8; 16];
            nonce.copy_from_slice(&nonce_vec);
            tag.copy_from_slice(&tag_vec);

            // Decrypt password
            let decrypted = crypto::decrypt(&encrypted_password, master_key, &nonce, &tag)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))?;

            let password = String::from_utf8(decrypted)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            // Optional SSH key fields (columns 13..20; may be NULL if migration 008 not run or password-only cred)
            let key_path: Option<String> = row.get(13).ok().flatten();
            let private_key = decrypt_optional_blob(master_key, row.get(14).ok().flatten(), row.get(15).ok().flatten(), row.get(16).ok().flatten())?;
            let key_passphrase = decrypt_optional_blob(master_key, row.get(17).ok().flatten(), row.get(18).ok().flatten(), row.get(19).ok().flatten())?;

            Ok(Credential {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                password,
                credential_type: row.get(6)?,
                host: row.get(7)?,
                port: row.get(8)?,
                metadata: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
                last_used_at: row.get(12)?,
                key_path,
                private_key,
                key_passphrase,
            })
        }).map_err(|e| format!("Failed to get credential: {}", e))?;

        // Update last_used_at
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE credentials SET last_used_at = ?1 WHERE id = ?2",
            rusqlite::params![now, credential_id],
        ).map_err(|e| format!("Failed to update last_used_at: {}", e))?;

        // Log audit event
        Self::log_audit_event(
            &conn,
            "credential_access",
            Some(credential_id),
            Some("credential"),
            "read",
            "success",
            None,
        )?;

        Ok(credential)
    }

    pub fn list_credentials(&self) -> Result<Vec<CredentialSummary>, String> {
        let conn = db::open_connection(&self.db_path)?;

        let mut stmt = conn.prepare(
            "SELECT id, name, username, credential_type, host, port, created_at, updated_at, last_used_at
             FROM credentials ORDER BY name ASC"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let credentials = stmt.query_map([], |row| {
            Ok(CredentialSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                credential_type: row.get(3)?,
                host: row.get(4)?,
                port: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                last_used_at: row.get(8)?,
            })
        }).map_err(|e| format!("Failed to query credentials: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect credentials: {}", e))?;

        Ok(credentials)
    }

    pub fn update_credential(
        &self,
        master_key: &[u8; 32],
        credential_id: &str,
        name: Option<String>,
        username: Option<String>,
        password: Option<String>,
        metadata: Option<String>,
        key_path: Option<String>,
        private_key: Option<String>,
        key_passphrase: Option<String>,
    ) -> Result<(), String> {
        let conn = db::open_connection(&self.db_path)?;

        let now = chrono::Utc::now().timestamp();

        if let Some(n) = name {
            conn.execute(
                "UPDATE credentials SET name = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![n, now, credential_id],
            ).map_err(|e| format!("Failed to update credential name: {}", e))?;
        }

        if let Some(u) = username {
            conn.execute(
                "UPDATE credentials SET username = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![u, now, credential_id],
            ).map_err(|e| format!("Failed to update credential username: {}", e))?;
        }

        if let Some(p) = password {
            let password_bytes = p.as_bytes();
            let (ciphertext, nonce, tag) = crypto::encrypt(password_bytes, master_key)?;
            conn.execute(
                "UPDATE credentials SET encrypted_password = ?1, nonce = ?2, tag = ?3, updated_at = ?4 WHERE id = ?5",
                rusqlite::params![ciphertext, nonce.to_vec(), tag.to_vec(), now, credential_id],
            ).map_err(|e| format!("Failed to update credential password: {}", e))?;
        }

        if let Some(m) = metadata {
            conn.execute(
                "UPDATE credentials SET metadata = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![m, now, credential_id],
            ).map_err(|e| format!("Failed to update credential metadata: {}", e))?;
        }

        if let Some(kp) = key_path {
            conn.execute(
                "UPDATE credentials SET key_path = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![if kp.is_empty() { None::<String> } else { Some(kp) }, now, credential_id],
            ).map_err(|e| format!("Failed to update credential key_path: {}", e))?;
        }

        if let Some(pk) = private_key {
            if pk.is_empty() {
                conn.execute(
                    "UPDATE credentials SET encrypted_private_key = NULL, private_key_nonce = NULL, private_key_tag = NULL, updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![now, credential_id],
                ).map_err(|e| format!("Failed to clear credential private key: {}", e))?;
            } else {
                let (ciphertext, nonce, tag) = crypto::encrypt(pk.as_bytes(), master_key)?;
                conn.execute(
                    "UPDATE credentials SET encrypted_private_key = ?1, private_key_nonce = ?2, private_key_tag = ?3, updated_at = ?4 WHERE id = ?5",
                    rusqlite::params![ciphertext, nonce.to_vec(), tag.to_vec(), now, credential_id],
                ).map_err(|e| format!("Failed to update credential private key: {}", e))?;
            }
        }

        if let Some(kp) = key_passphrase {
            if kp.is_empty() {
                conn.execute(
                    "UPDATE credentials SET encrypted_key_passphrase = NULL, key_passphrase_nonce = NULL, key_passphrase_tag = NULL, updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![now, credential_id],
                ).map_err(|e| format!("Failed to clear credential key passphrase: {}", e))?;
            } else {
                let (ciphertext, nonce, tag) = crypto::encrypt(kp.as_bytes(), master_key)?;
                conn.execute(
                    "UPDATE credentials SET encrypted_key_passphrase = ?1, key_passphrase_nonce = ?2, key_passphrase_tag = ?3, updated_at = ?4 WHERE id = ?5",
                    rusqlite::params![ciphertext, nonce.to_vec(), tag.to_vec(), now, credential_id],
                ).map_err(|e| format!("Failed to update credential key passphrase: {}", e))?;
            }
        }

        // Log audit event
        Self::log_audit_event(
            &conn,
            "credential_update",
            Some(credential_id),
            Some("credential"),
            "write",
            "success",
            None,
        )?;

        Ok(())
    }

    pub fn delete_credential(&self, credential_id: &str) -> Result<(), String> {
        let conn = db::open_connection(&self.db_path)?;

        conn.execute(
            "DELETE FROM credentials WHERE id = ?1",
            [credential_id],
        ).map_err(|e| format!("Failed to delete credential: {}", e))?;

        // Log audit event
        Self::log_audit_event(
            &conn,
            "credential_delete",
            Some(credential_id),
            Some("credential"),
            "delete",
            "success",
            None,
        )?;

        Ok(())
    }

    pub fn delete_credential_tx(&self, tx: &rusqlite::Transaction, credential_id: &str) -> Result<(), String> {
        tx.execute(
            "DELETE FROM credentials WHERE id = ?1",
            [credential_id],
        ).map_err(|e| format!("Failed to delete credential: {}", e))?;

        // Log audit event
        Self::log_audit_event_tx(
            tx,
            "credential_delete",
            Some(credential_id),
            Some("credential"),
            "delete",
            "success",
            None,
        )?;

        Ok(())
    }

    pub fn add_credential_tx(
        &self,
        tx: &rusqlite::Transaction,
        master_key: &[u8; 32],
        name: String,
        username: String,
        password: String,
        credential_type: String,
        host: Option<String>,
        port: Option<u16>,
        metadata: Option<String>,
        key_path: Option<String>,
        private_key: Option<String>,
        key_passphrase: Option<String>,
    ) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        let password_bytes = password.as_bytes();
        let (ciphertext, nonce, tag) = crypto::encrypt(password_bytes, master_key)?;

        let (encrypted_key, key_nonce, key_tag) = match &private_key {
            Some(pk) => {
                let (c, n, t) = crypto::encrypt(pk.as_bytes(), master_key)?;
                (Some(c), Some(n.to_vec()), Some(t.to_vec()))
            }
            None => (None, None, None),
        };
        let (encrypted_kp, kp_nonce, kp_tag) = match &key_passphrase {
            Some(kp) => {
                let (c, n, t) = crypto::encrypt(kp.as_bytes(), master_key)?;
                (Some(c), Some(n.to_vec()), Some(t.to_vec()))
            }
            None => (None, None, None),
        };

        tx.execute(
            "INSERT INTO credentials (id, name, username, encrypted_password, nonce, tag, credential_type, host, port, metadata, created_at, updated_at,
             key_path, encrypted_private_key, private_key_nonce, private_key_tag, encrypted_key_passphrase, key_passphrase_nonce, key_passphrase_tag)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            rusqlite::params![
                id,
                name,
                username,
                ciphertext,
                nonce.to_vec(),
                tag.to_vec(),
                credential_type,
                host,
                port,
                metadata,
                now,
                now,
                key_path,
                encrypted_key,
                key_nonce,
                key_tag,
                encrypted_kp,
                kp_nonce,
                kp_tag,
            ],
        ).map_err(|e| format!("Failed to insert credential: {}", e))?;

        // Log audit event
        Self::log_audit_event_tx(
            tx,
            "credential_create",
            Some(&id),
            Some("credential"),
            "create",
            "success",
            None,
        )?;

        Ok(id)
    }

    pub fn search_credentials(&self, query: &str) -> Result<Vec<CredentialSummary>, String> {
        let conn = db::open_connection(&self.db_path)?;

        let search_pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            "SELECT id, name, username, credential_type, host, port, created_at, updated_at, last_used_at
             FROM credentials 
             WHERE name LIKE ?1 OR username LIKE ?1 OR credential_type LIKE ?1
             ORDER BY name ASC"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let credentials = stmt.query_map([&search_pattern], |row| {
            Ok(CredentialSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                credential_type: row.get(3)?,
                host: row.get(4)?,
                port: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                last_used_at: row.get(8)?,
            })
        }).map_err(|e| format!("Failed to query credentials: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect credentials: {}", e))?;

        Ok(credentials)
    }

    fn log_audit_event(
        conn: &Connection,
        event_type: &str,
        resource_id: Option<&str>,
        resource_type: Option<&str>,
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
                resource_type,
                action,
                result,
                details
            ],
        ).map_err(|e| format!("Failed to log audit event: {}", e))?;

        Ok(())
    }

    fn log_audit_event_tx(
        tx: &rusqlite::Transaction,
        event_type: &str,
        resource_id: Option<&str>,
        resource_type: Option<&str>,
        action: &str,
        result: &str,
        details: Option<&str>,
    ) -> Result<(), String> {
        let id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();

        tx.execute(
            "INSERT INTO security_audit_log (id, timestamp, event_type, resource_id, resource_type, action, result, details)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                id,
                timestamp,
                event_type,
                resource_id,
                resource_type,
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

    fn setup_test_db() -> (String, [u8; 32]) {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let db_path = format!("test_credentials_{}.db", timestamp);

        let conn = Connection::open(&db_path).expect("Failed to open test database");

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
        ).expect("Failed to create credentials table");
        // Migration 008 columns so get_credential SELECT works
        conn.execute_batch(
            "ALTER TABLE credentials ADD COLUMN key_path TEXT;
             ALTER TABLE credentials ADD COLUMN encrypted_private_key BLOB;
             ALTER TABLE credentials ADD COLUMN private_key_nonce BLOB;
             ALTER TABLE credentials ADD COLUMN private_key_tag BLOB;
             ALTER TABLE credentials ADD COLUMN encrypted_key_passphrase BLOB;
             ALTER TABLE credentials ADD COLUMN key_passphrase_nonce BLOB;
             ALTER TABLE credentials ADD COLUMN key_passphrase_tag BLOB;",
        ).expect("Failed to add key columns");

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

        let master_key = [42u8; 32]; // Test key
        (db_path, master_key)
    }

    fn cleanup_test_db(db_path: &str) {
        if let Err(e) = std::fs::remove_file(db_path) {
            eprintln!("Warning: Failed to cleanup test DB {}: {}", db_path, e);
        }
    }

    #[test]
    fn test_add_and_get_credential() {
        let (db_path, master_key) = setup_test_db();
        let manager = CredentialManager::new(db_path.clone());

        let cred_id = manager.add_credential(
            &master_key,
            "Test Credential".to_string(),
            "testuser".to_string(),
            "SecurePassword123!".to_string(),
            "password".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        ).expect("Failed to add credential");

        let credential = manager.get_credential(&master_key, &cred_id)
            .expect("Failed to get credential");

        assert_eq!(credential.name, "Test Credential");
        assert_eq!(credential.username, "testuser");
        assert_eq!(credential.password, "SecurePassword123!");
        assert_eq!(credential.credential_type, "password");

        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_list_credentials() {
        let (db_path, master_key) = setup_test_db();
        let manager = CredentialManager::new(db_path.clone());

        manager.add_credential(&master_key, "Cred 1".to_string(), "user1".to_string(), "pass1".to_string(), "password".to_string(), None, None, None, None, None, None).unwrap();
        manager.add_credential(&master_key, "Cred 2".to_string(), "user2".to_string(), "pass2".to_string(), "password".to_string(), None, None, None, None, None, None).unwrap();

        let credentials = manager.list_credentials().expect("Failed to list credentials");

        assert_eq!(credentials.len(), 2);
        assert_eq!(credentials[0].name, "Cred 1");
        assert_eq!(credentials[1].name, "Cred 2");

        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_update_credential() {
        let (db_path, master_key) = setup_test_db();
        let manager = CredentialManager::new(db_path.clone());

        let cred_id = manager.add_credential(&master_key, "Original".to_string(), "user".to_string(), "pass".to_string(), "password".to_string(), None, None, None, None, None, None).unwrap();

        manager.update_credential(
            &master_key,
            &cred_id,
            Some("Updated".to_string()),
            Some("newuser".to_string()),
            Some("newpass".to_string()),
            None,
            None,
            None,
            None,
        ).expect("Failed to update credential");

        let credential = manager.get_credential(&master_key, &cred_id).unwrap();

        assert_eq!(credential.name, "Updated");
        assert_eq!(credential.username, "newuser");
        assert_eq!(credential.password, "newpass");

        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_delete_credential() {
        let (db_path, master_key) = setup_test_db();
        let manager = CredentialManager::new(db_path.clone());

        let cred_id = manager.add_credential(&master_key, "To Delete".to_string(), "user".to_string(), "pass".to_string(), "password".to_string(), None, None, None, None, None, None).unwrap();

        manager.delete_credential(&cred_id).expect("Failed to delete credential");

        let result = manager.get_credential(&master_key, &cred_id);
        assert!(result.is_err());

        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_search_credentials() {
        let (db_path, master_key) = setup_test_db();
        let manager = CredentialManager::new(db_path.clone());

        manager.add_credential(&master_key, "GitHub Account".to_string(), "user1".to_string(), "pass1".to_string(), "password".to_string(), None, None, None, None, None, None).unwrap();
        manager.add_credential(&master_key, "GitLab Account".to_string(), "user2".to_string(), "pass2".to_string(), "password".to_string(), None, None, None, None, None, None).unwrap();
        manager.add_credential(&master_key, "AWS Console".to_string(), "user3".to_string(), "pass3".to_string(), "password".to_string(), None, None, None, None, None, None).unwrap();

        let results = manager.search_credentials("Git").expect("Failed to search credentials");

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|c| c.name == "GitHub Account"));
        assert!(results.iter().any(|c| c.name == "GitLab Account"));

        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_get_credential_invalid_nonce_length_returns_error() {
        let (db_path, master_key) = setup_test_db();
        let manager = CredentialManager::new(db_path.clone());

        let conn = Connection::open(&db_path).expect("Failed to open test database");
        conn.execute(
            "INSERT INTO credentials (id, name, username, encrypted_password, nonce, tag, credential_type, host, port, metadata, created_at, updated_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                "malformed-nonce",
                "Bad Cred",
                "user",
                vec![1u8, 2, 3],
                vec![1u8, 2, 3], // invalid nonce length (must be 12)
                vec![0u8; 16],
                "password",
                Option::<String>::None,
                Option::<u16>::None,
                Option::<String>::None,
                1i64,
                1i64,
                Option::<i64>::None,
            ],
        ).expect("Failed to insert malformed credential");

        let result = manager.get_credential(&master_key, "malformed-nonce");
        assert!(result.is_err(), "Malformed nonce must return an error instead of panicking");

        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_get_credential_invalid_tag_length_returns_error() {
        let (db_path, master_key) = setup_test_db();
        let manager = CredentialManager::new(db_path.clone());

        let conn = Connection::open(&db_path).expect("Failed to open test database");
        conn.execute(
            "INSERT INTO credentials (id, name, username, encrypted_password, nonce, tag, credential_type, host, port, metadata, created_at, updated_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                "malformed-tag",
                "Bad Cred 2",
                "user",
                vec![1u8, 2, 3],
                vec![0u8; 12],
                vec![1u8, 2, 3], // invalid tag length (must be 16)
                "password",
                Option::<String>::None,
                Option::<u16>::None,
                Option::<String>::None,
                1i64,
                1i64,
                Option::<i64>::None,
            ],
        ).expect("Failed to insert malformed credential");

        let result = manager.get_credential(&master_key, "malformed-tag");
        assert!(result.is_err(), "Malformed auth tag must return an error instead of panicking");

        cleanup_test_db(&db_path);
    }
}

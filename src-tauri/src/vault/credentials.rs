use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: String,
    pub name: String,
    pub username: String,
    pub password: String,
    pub credential_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub metadata: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_used_at: Option<i64>,
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
        metadata: Option<String>,
    ) -> Result<String, String> {
        let conn = Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        // Encrypt password
        let password_bytes = password.as_bytes();
        let (ciphertext, nonce, tag) = crypto::encrypt(password_bytes, master_key)?;

        conn.execute(
            "INSERT INTO credentials_new (id, name, username, encrypted_password, nonce, tag, credential_type, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                id,
                name,
                username,
                ciphertext,
                nonce.to_vec(),
                tag.to_vec(),
                credential_type,
                metadata,
                now,
                now
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
        let conn = Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let mut stmt = conn.prepare(
            "SELECT id, name, username, encrypted_password, nonce, tag, credential_type, metadata, created_at, updated_at, last_used_at
             FROM credentials_new WHERE id = ?1"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let credential = stmt.query_row([credential_id], |row| {
            let encrypted_password: Vec<u8> = row.get(3)?;
            let nonce_vec: Vec<u8> = row.get(4)?;
            let tag_vec: Vec<u8> = row.get(5)?;

            let mut nonce = [0u8; 12];
            let mut tag = [0u8; 16];
            nonce.copy_from_slice(&nonce_vec);
            tag.copy_from_slice(&tag_vec);

            // Decrypt password
            let decrypted = crypto::decrypt(&encrypted_password, master_key, &nonce, &tag)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))?;

            let password = String::from_utf8(decrypted)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            let metadata: Option<String> = row.get(7)?;
            let (host, port) = if let Some(ref meta) = metadata {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(meta) {
                    let host = parsed.get("host").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let port = parsed.get("port").and_then(|v| v.as_u64()).map(|p| p as u16);
                    (host, port)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            Ok(Credential {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                password,
                credential_type: row.get(6)?,
                host,
                port,
                metadata,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
                last_used_at: row.get(10)?,
            })
        }).map_err(|e| format!("Failed to get credential: {}", e))?;

        // Update last_used_at
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE credentials_new SET last_used_at = ?1 WHERE id = ?2",
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
        let conn = Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let mut stmt = conn.prepare(
            "SELECT id, name, username, credential_type, metadata, created_at, updated_at, last_used_at
             FROM credentials_new ORDER BY name ASC"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let credentials = stmt.query_map([], |row| {
            let metadata: Option<String> = row.get(4)?;
            let (host, port) = if let Some(ref meta) = metadata {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(meta) {
                    let host = parsed.get("host").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let port = parsed.get("port").and_then(|v| v.as_u64()).map(|p| p as u16);
                    (host, port)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            Ok(CredentialSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                credential_type: row.get(3)?,
                host,
                port,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                last_used_at: row.get(7)?,
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
    ) -> Result<(), String> {
        let conn = Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let now = chrono::Utc::now().timestamp();

        // Build dynamic update query
        let mut updates = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(n) = name {
            updates.push("name = ?");
            params.push(Box::new(n));
        }

        if let Some(u) = username {
            updates.push("username = ?");
            params.push(Box::new(u));
        }

        if let Some(p) = password {
            // Encrypt new password
            let password_bytes = p.as_bytes();
            let (ciphertext, nonce, tag) = crypto::encrypt(password_bytes, master_key)?;

            updates.push("encrypted_password = ?");
            updates.push("nonce = ?");
            updates.push("tag = ?");
            params.push(Box::new(ciphertext));
            params.push(Box::new(nonce.to_vec()));
            params.push(Box::new(tag.to_vec()));
        }

        if let Some(m) = metadata {
            updates.push("metadata = ?");
            params.push(Box::new(m));
        }

        if updates.is_empty() {
            return Ok(());
        }

        updates.push("updated_at = ?");
        params.push(Box::new(now));

        let query = format!(
            "UPDATE credentials_new SET {} WHERE id = ?",
            updates.join(", ")
        );

        params.push(Box::new(credential_id.to_string()));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&query, params_refs.as_slice())
            .map_err(|e| format!("Failed to update credential: {}", e))?;

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
        let conn = Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        conn.execute(
            "DELETE FROM credentials_new WHERE id = ?1",
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
            "DELETE FROM credentials_new WHERE id = ?1",
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
        metadata: Option<String>,
    ) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        // Encrypt password
        let password_bytes = password.as_bytes();
        let (ciphertext, nonce, tag) = crypto::encrypt(password_bytes, master_key)?;

        tx.execute(
            "INSERT INTO credentials_new (id, name, username, encrypted_password, nonce, tag, credential_type, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                id,
                name,
                username,
                ciphertext,
                nonce.to_vec(),
                tag.to_vec(),
                credential_type,
                metadata,
                now,
                now
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
        let conn = Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let search_pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            "SELECT id, name, username, credential_type, metadata, created_at, updated_at, last_used_at
             FROM credentials_new 
             WHERE name LIKE ?1 OR username LIKE ?1 OR credential_type LIKE ?1
             ORDER BY name ASC"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let credentials = stmt.query_map([&search_pattern], |row| {
            let metadata: Option<String> = row.get(4)?;
            let (host, port) = if let Some(ref meta) = metadata {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(meta) {
                    let host = parsed.get("host").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let port = parsed.get("port").and_then(|v| v.as_u64()).map(|p| p as u16);
                    (host, port)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            Ok(CredentialSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                credential_type: row.get(3)?,
                host,
                port,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                last_used_at: row.get(7)?,
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
            "CREATE TABLE credentials_new (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                username TEXT NOT NULL,
                encrypted_password BLOB NOT NULL,
                nonce BLOB NOT NULL,
                tag BLOB NOT NULL,
                credential_type TEXT NOT NULL DEFAULT 'password',
                metadata TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_used_at INTEGER
            )",
            [],
        ).expect("Failed to create credentials table");

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
        let _ = std::fs::remove_file(db_path);
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

        manager.add_credential(&master_key, "Cred 1".to_string(), "user1".to_string(), "pass1".to_string(), "password".to_string(), None).unwrap();
        manager.add_credential(&master_key, "Cred 2".to_string(), "user2".to_string(), "pass2".to_string(), "password".to_string(), None).unwrap();

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

        let cred_id = manager.add_credential(&master_key, "Original".to_string(), "user".to_string(), "pass".to_string(), "password".to_string(), None).unwrap();

        manager.update_credential(
            &master_key,
            &cred_id,
            Some("Updated".to_string()),
            Some("newuser".to_string()),
            Some("newpass".to_string()),
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

        let cred_id = manager.add_credential(&master_key, "To Delete".to_string(), "user".to_string(), "pass".to_string(), "password".to_string(), None).unwrap();

        manager.delete_credential(&cred_id).expect("Failed to delete credential");

        let result = manager.get_credential(&master_key, &cred_id);
        assert!(result.is_err());

        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_search_credentials() {
        let (db_path, master_key) = setup_test_db();
        let manager = CredentialManager::new(db_path.clone());

        manager.add_credential(&master_key, "GitHub Account".to_string(), "user1".to_string(), "pass1".to_string(), "password".to_string(), None).unwrap();
        manager.add_credential(&master_key, "GitLab Account".to_string(), "user2".to_string(), "pass2".to_string(), "password".to_string(), None).unwrap();
        manager.add_credential(&master_key, "AWS Console".to_string(), "user3".to_string(), "pass3".to_string(), "password".to_string(), None).unwrap();

        let results = manager.search_credentials("Git").expect("Failed to search credentials");

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|c| c.name == "GitHub Account"));
        assert!(results.iter().any(|c| c.name == "GitLab Account"));

        cleanup_test_db(&db_path);
    }
}

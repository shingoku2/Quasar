use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrustStatus {
    #[serde(rename = "trusted")]
    Trusted,
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(rename = "changed")]
    Changed,
    #[serde(rename = "rejected")]
    Rejected,
}

impl TrustStatus {
    fn to_string(&self) -> &'static str {
        match self {
            TrustStatus::Trusted => "trusted",
            TrustStatus::Unknown => "unknown",
            TrustStatus::Changed => "changed",
            TrustStatus::Rejected => "rejected",
        }
    }

    fn from_string(s: &str) -> Self {
        match s {
            "trusted" => TrustStatus::Trusted,
            "unknown" => TrustStatus::Unknown,
            "changed" => TrustStatus::Changed,
            "rejected" => TrustStatus::Rejected,
            _ => TrustStatus::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshHostKey {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint: String,
    pub public_key: Vec<u8>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub trust_status: TrustStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostKeyVerificationResult {
    pub allowed: bool,
    pub status: TrustStatus,
    pub fingerprint: String,
    pub message: String,
}

pub struct SshKeyManager {
    conn: Arc<Mutex<Connection>>,
}

impl SshKeyManager {
    pub fn new(db_path: String) -> Result<Self, String> {
        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;
        
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Verify a server's host key by fingerprint
    pub async fn verify_host_key_by_fingerprint(
        &self,
        host: &str,
        port: u16,
        fingerprint: &str,
        _key_type: &str,
    ) -> Result<HostKeyVerificationResult, String> {
        // Query database in a scope to ensure MutexGuard is dropped before await
        let existing = {
            let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
            
            // Check if we have a known key for this host
            conn.query_row(
                "SELECT id, host, port, key_type, fingerprint, public_key, 
                        first_seen_at, last_seen_at, trust_status 
                 FROM ssh_known_hosts 
                 WHERE host = ?1 AND port = ?2",
                params![host, port],
                |row| {
                    Ok(SshHostKey {
                        id: row.get(0)?,
                        host: row.get(1)?,
                        port: row.get(2)?,
                        key_type: row.get(3)?,
                        fingerprint: row.get(4)?,
                        public_key: row.get(5)?,
                        first_seen_at: row.get(6)?,
                        last_seen_at: row.get(7)?,
                        trust_status: TrustStatus::from_string(&row.get::<_, String>(8)?),
                    })
                },
            )
            .ok()
        }; // MutexGuard dropped here

        let result = match existing {
            None => {
                // New host - return Unknown status
                HostKeyVerificationResult {
                    allowed: false,
                    status: TrustStatus::Unknown,
                    fingerprint: fingerprint.to_string(),
                    message: format!(
                        "Unknown host key for {}:{}. Fingerprint: {}",
                        host, port, fingerprint
                    ),
                }
            }
            Some(known) => {
                if known.fingerprint == fingerprint {
                    // Key matches - check trust status
                    match known.trust_status {
                        TrustStatus::Trusted => {
                            HostKeyVerificationResult {
                                allowed: true,
                                status: TrustStatus::Trusted,
                                fingerprint: fingerprint.to_string(),
                                message: format!("Trusted host key for {}:{}", host, port),
                            }
                        }
                        TrustStatus::Rejected => {
                            HostKeyVerificationResult {
                                allowed: false,
                                status: TrustStatus::Rejected,
                                fingerprint: fingerprint.to_string(),
                                message: format!("Rejected host key for {}:{}", host, port),
                            }
                        }
                        _ => {
                            HostKeyVerificationResult {
                                allowed: false,
                                status: known.trust_status.clone(),
                                fingerprint: fingerprint.to_string(),
                                message: format!(
                                    "Host key for {}:{} requires confirmation",
                                    host, port
                                ),
                            }
                        }
                    }
                } else {
                    // Key has changed! Possible MITM attack
                    HostKeyVerificationResult {
                        allowed: false,
                        status: TrustStatus::Changed,
                        fingerprint: fingerprint.to_string(),
                        message: format!(
                            "WARNING: Host key for {}:{} has changed!\nOld: {}\nNew: {}\nThis could indicate a man-in-the-middle attack!",
                            host, port, known.fingerprint, fingerprint
                        ),
                    }
                }
            }
        };

        // Update last_seen_at if trusted (lock already dropped)
        if result.allowed && matches!(result.status, TrustStatus::Trusted) {
            let _ = self.update_last_seen(host, port).await;
        }

        Ok(result)
    }

    /// Add or update a host key with trust status
    pub async fn trust_host_key(
        &self,
        host: &str,
        port: u16,
        fingerprint: &str,
        key_type: &str,
        key_bytes: Vec<u8>,
        trust_status: TrustStatus,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp();
        
        {
            let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

            // Check if entry exists
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM ssh_known_hosts WHERE host = ?1 AND port = ?2",
                    params![host, port],
                    |_| Ok(true),
                )
                .unwrap_or(false);

            if exists {
            // Update existing entry
            conn.execute(
                "UPDATE ssh_known_hosts 
                 SET key_type = ?1, fingerprint = ?2, public_key = ?3, 
                     last_seen_at = ?4, trust_status = ?5
                 WHERE host = ?6 AND port = ?7",
                params![
                    key_type,
                    fingerprint,
                    key_bytes,
                    now,
                    trust_status.to_string(),
                    host,
                    port
                ],
            )
            .map_err(|e| format!("Failed to update host key: {}", e))?;
        } else {
            // Insert new entry
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO ssh_known_hosts 
                 (id, host, port, key_type, fingerprint, public_key, 
                  first_seen_at, last_seen_at, trust_status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    id,
                    host,
                    port,
                    key_type,
                    fingerprint,
                    key_bytes,
                    now,
                    now,
                    trust_status.to_string()
                ],
            )
            .map_err(|e| format!("Failed to insert host key: {}", e))?;
            }
        } // MutexGuard dropped here

        Ok(())
    }

    async fn update_last_seen(&self, host: &str, port: u16) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp();
        
        {
            let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

            conn.execute(
                "UPDATE ssh_known_hosts SET last_seen_at = ?1 WHERE host = ?2 AND port = ?3",
                params![now, host, port],
            )
            .map_err(|e| format!("Failed to update last_seen_at: {}", e))?;
        } // MutexGuard dropped here

        Ok(())
    }

    pub async fn get_known_hosts(&self) -> Result<Vec<SshHostKey>, String> {
        {
            let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

            let mut stmt = conn
                .prepare(
                    "SELECT id, host, port, key_type, fingerprint, public_key, 
                            first_seen_at, last_seen_at, trust_status 
                     FROM ssh_known_hosts 
                     ORDER BY last_seen_at DESC",
                )
                .map_err(|e| format!("Failed to prepare statement: {}", e))?;

            let hosts: Vec<SshHostKey> = stmt.query_map([], |row| {
                    Ok(SshHostKey {
                        id: row.get(0)?,
                        host: row.get(1)?,
                        port: row.get(2)?,
                        key_type: row.get(3)?,
                        fingerprint: row.get(4)?,
                        public_key: row.get(5)?,
                        first_seen_at: row.get(6)?,
                        last_seen_at: row.get(7)?,
                        trust_status: TrustStatus::from_string(&row.get::<_, String>(8)?),
                    })
                })
                .map_err(|e| format!("Failed to query known hosts: {}", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to collect results: {}", e))?;

            Ok(hosts)
        } // MutexGuard dropped here
    }

    pub async fn remove_host_key(&self, host: &str, port: u16) -> Result<(), String> {
        {
            let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

            conn.execute(
                "DELETE FROM ssh_known_hosts WHERE host = ?1 AND port = ?2",
                params![host, port],
            )
            .map_err(|e| format!("Failed to remove host key: {}", e))?;
        } // MutexGuard dropped here

        Ok(())
    }

    pub async fn update_trust_status(
        &self,
        host: &str,
        port: u16,
        trust_status: TrustStatus,
    ) -> Result<(), String> {
        {
            let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

            conn.execute(
                "UPDATE ssh_known_hosts SET trust_status = ?1 WHERE host = ?2 AND port = ?3",
                params![trust_status.to_string(), host, port],
            )
            .map_err(|e| format!("Failed to update trust status: {}", e))?;
        } // MutexGuard dropped here

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_manager() -> SshKeyManager {
        let db_path = ":memory:".to_string();
        
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(include_str!("../../migrations/003_security_vault.sql"))
            .unwrap();
        drop(conn);

        SshKeyManager::new(db_path).unwrap()
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let _manager = create_test_manager().await;
    }

    #[tokio::test]
    async fn test_unknown_host() {
        let manager = create_test_manager().await;
        
        let result = manager
            .verify_host_key_by_fingerprint("example.com", 22, "SHA256:abc123", "ssh-ed25519")
            .await
            .unwrap();
        
        assert!(!result.allowed);
        assert!(matches!(result.status, TrustStatus::Unknown));
    }

    #[tokio::test]
    async fn test_trust_and_verify() {
        let manager = create_test_manager().await;
        
        manager
            .trust_host_key(
                "example.com",
                22,
                "SHA256:abc123",
                "ssh-ed25519",
                vec![1, 2, 3],
                TrustStatus::Trusted,
            )
            .await
            .unwrap();

        let result = manager
            .verify_host_key_by_fingerprint("example.com", 22, "SHA256:abc123", "ssh-ed25519")
            .await
            .unwrap();
        
        assert!(result.allowed);
        assert!(matches!(result.status, TrustStatus::Trusted));
    }

    #[tokio::test]
    async fn test_key_changed_detection() {
        let manager = create_test_manager().await;
        
        manager
            .trust_host_key(
                "example.com",
                22,
                "SHA256:abc123",
                "ssh-ed25519",
                vec![1, 2, 3],
                TrustStatus::Trusted,
            )
            .await
            .unwrap();

        let result = manager
            .verify_host_key_by_fingerprint("example.com", 22, "SHA256:different", "ssh-ed25519")
            .await
            .unwrap();
        
        assert!(!result.allowed);
        assert!(matches!(result.status, TrustStatus::Changed));
    }
}

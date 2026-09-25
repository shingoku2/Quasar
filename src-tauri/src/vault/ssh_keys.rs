use crate::db;
use rusqlite::OptionalExtension;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use subtle::ConstantTimeEq;

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
    /// The previously stored fingerprint when `status` is `Changed` (FE-010: the UI used to
    /// show the whole prose `message`, new fingerprint included, as the "old" one).
    #[serde(default)]
    pub old_fingerprint: Option<String>,
}

pub struct SshKeyManager {
    conn: Arc<Mutex<Connection>>,
}

/// Wire encoding of the host key a server presented, which its fingerprint is taken over.
/// A host certificate is pinned by the key it certifies: Quasar doesn't trust host CAs, and
/// that key is the one the handshake proved possession of, so the pin means the same thing
/// either way (russh 0.63 started passing certificates here).
pub fn presented_key_bytes(presented: &russh::keys::PublicKeyOrCertificate) -> Vec<u8> {
    use russh::keys::ssh_encoding::Encode;
    use russh::keys::{PublicKeyBase64, PublicKeyOrCertificate};
    match presented {
        PublicKeyOrCertificate::PublicKey { key, .. } => key.public_key_bytes(),
        PublicKeyOrCertificate::Certificate(cert) => cert.public_key().encode_vec().unwrap_or_default(),
    }
}

impl SshKeyManager {
    /// Host-key decision for connections that can't prompt the user (one-shot exec, the
    /// pooled sessions behind monitoring and scheduled tasks, SFTP): only a key already
    /// stored as trusted for this host *and port* is accepted. Unknown, changed and rejected
    /// keys, and any lookup error, refuse the connection.
    pub async fn check_non_interactive(
        &self,
        host: &str,
        port: u16,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, russh::Error> {
        let fingerprint = crate::crypto::ssh_host_key_fingerprint(&presented_key_bytes(server_public_key));
        match self
            .verify_host_key_by_fingerprint(host, port, &fingerprint, "ssh-key")
            .await
        {
            Ok(result) if result.allowed => Ok(true),
            _ => Err(russh::Error::Disconnect),
        }
    }

    pub fn new(db_path: String) -> Result<Self, String> {
        let conn = db::open_connection(&db_path)?;

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
                 WHERE host = ?1 COLLATE NOCASE AND port = ?2",
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
            .optional()
            // A lookup error must fail closed. It used to read as "no known key", so a DB
            // problem showed a first-use prompt and could hide a changed key (RUST-013).
            .map_err(|e| format!("Failed to look up known host key: {}", e))?
        }; // MutexGuard dropped here

        // The same host (any letter case) with a different trusted key on another port. A
        // port switch must not turn a pinned host into a first-seen one: that let a
        // compromised webview skip the changed-key confirmation (P7-4 review).
        let other_port_key: Option<(String, u16)> = if existing.is_none() {
            let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
            let mut stmt = conn
                .prepare(
                    "SELECT fingerprint, port FROM ssh_known_hosts
                     WHERE host = ?1 COLLATE NOCASE AND port != ?2 AND trust_status = 'trusted'",
                )
                .map_err(|e| format!("Failed to look up known host key: {}", e))?;
            let keys = stmt
                .query_map(params![host, port], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u16>(1)?)))
                .map_err(|e| format!("Failed to look up known host key: {}", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to look up known host key: {}", e))?;
            // Any trusted key for this host that differs from the presented one makes it
            // Changed. The same key on every other trusted port (one server, several ports) is
            // a plain first use. Matching just *one* of them isn't enough: with key A on 22 and
            // B on 2222, a new port presenting A differs from B (PR #68 review).
            keys.into_iter()
                .find(|(fp, _)| !bool::from(fp.as_bytes().ct_eq(fingerprint.as_bytes())))
        } else {
            None
        };

        let result = match existing {
            None if other_port_key.is_some() => {
                let (old, old_port) = other_port_key.unwrap_or_default();
                HostKeyVerificationResult {
                    allowed: false,
                    status: TrustStatus::Changed,
                    fingerprint: fingerprint.to_string(),
                    message: format!(
                        "WARNING: {} is trusted with a different key on port {}.\nTrusted: {}\nPresented on port {}: {}\nThis could indicate a man-in-the-middle attack!",
                        host, old_port, old, port, fingerprint
                    ),
                    old_fingerprint: Some(old),
                }
            }
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
                    old_fingerprint: None,
                }
            }
            Some(known) => {
                if known
                    .fingerprint
                    .as_bytes()
                    .ct_eq(fingerprint.as_bytes())
                    .into()
                {
                    // Key matches - check trust status
                    match known.trust_status {
                        TrustStatus::Trusted => HostKeyVerificationResult {
                            allowed: true,
                            status: TrustStatus::Trusted,
                            fingerprint: fingerprint.to_string(),
                            message: format!("Trusted host key for {}:{}", host, port),
                            old_fingerprint: None,
                        },
                        TrustStatus::Rejected => HostKeyVerificationResult {
                            allowed: false,
                            status: TrustStatus::Rejected,
                            fingerprint: fingerprint.to_string(),
                            message: format!("Rejected host key for {}:{}", host, port),
                            old_fingerprint: None,
                        },
                        _ => HostKeyVerificationResult {
                            allowed: false,
                            status: known.trust_status.clone(),
                            fingerprint: fingerprint.to_string(),
                            message: format!(
                                "Host key for {}:{} requires confirmation",
                                host, port
                            ),
                            old_fingerprint: None,
                        },
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
                        old_fingerprint: Some(known.fingerprint.clone()),
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
                    "SELECT 1 FROM ssh_known_hosts WHERE host = ?1 COLLATE NOCASE AND port = ?2",
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
                 WHERE host = ?6 COLLATE NOCASE AND port = ?7",
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
                "UPDATE ssh_known_hosts SET last_seen_at = ?1 WHERE host = ?2 COLLATE NOCASE AND port = ?3",
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

            let hosts: Vec<SshHostKey> = stmt
                .query_map([], |row| {
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
                "DELETE FROM ssh_known_hosts WHERE host = ?1 COLLATE NOCASE AND port = ?2",
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
                "UPDATE ssh_known_hosts SET trust_status = ?1 WHERE host = ?2 COLLATE NOCASE AND port = ?3",
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

    async fn create_test_manager() -> (SshKeyManager, String) {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        // A nanosecond timestamp alone isn't a reliable uniqueness guarantee
        // (clock resolution can be coarser than 1ns, and concurrent test threads
        // can race), so a per-process counter is appended to rule out collisions.
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_path = format!("test_ssh_keys_{}_{}.db", timestamp, seq);

        let conn = Connection::open(&db_path).unwrap();
        // Create only the ssh_known_hosts table needed for these tests
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS ssh_known_hosts (
                id TEXT PRIMARY KEY,
                host TEXT NOT NULL,
                port INTEGER NOT NULL DEFAULT 22,
                key_type TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                public_key BLOB NOT NULL,
                first_seen_at INTEGER NOT NULL,
                last_seen_at INTEGER NOT NULL,
                trust_status TEXT NOT NULL DEFAULT 'trusted',
                UNIQUE(host, port)
            );
            CREATE INDEX IF NOT EXISTS idx_known_hosts_lookup ON ssh_known_hosts(host, port);",
        )
        .unwrap();
        drop(conn);

        let manager = SshKeyManager::new(db_path.clone()).unwrap();
        (manager, db_path)
    }

    fn cleanup_test_db(db_path: &str) {
        if let Err(e) = std::fs::remove_file(db_path) {
            eprintln!("Warning: Failed to cleanup test DB {}: {}", db_path, e);
        }
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let (_manager, db_path) = create_test_manager().await;
        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_unknown_host() {
        let (manager, db_path) = create_test_manager().await;

        let result = manager
            .verify_host_key_by_fingerprint("example.com", 22, "SHA256:abc123", "ssh-ed25519")
            .await
            .unwrap();

        assert!(!result.allowed);
        assert!(matches!(result.status, TrustStatus::Unknown));
        cleanup_test_db(&db_path);
    }

    /// RUST-013 / TEST-003: a lookup error fails closed instead of reading as "unknown host".
    #[tokio::test]
    async fn test_lookup_error_fails_closed() {
        let (manager, db_path) = create_test_manager().await;
        manager.conn.lock().unwrap().execute_batch("DROP TABLE ssh_known_hosts;").unwrap();
        assert!(manager
            .verify_host_key_by_fingerprint("h", 22, "SHA256:x", "ssh-key")
            .await
            .is_err());
        let _ = std::fs::remove_file(&db_path);
    }

    /// P7-4 review: letter case and a different port can't turn a pinned host into a
    /// first-seen one (which skipped the native changed-key confirmation).
    #[tokio::test]
    async fn test_case_and_port_changes_keep_the_pin() {
        let (manager, db_path) = create_test_manager().await;
        manager.trust_host_key("db1", 22, "SHA256:a", "ssh-key", vec![1], TrustStatus::Trusted).await.unwrap();
        let upper = manager.verify_host_key_by_fingerprint("DB1", 22, "SHA256:a", "ssh-key").await.unwrap();
        assert!(upper.allowed, "host names match case-insensitively");
        let upper_changed = manager.verify_host_key_by_fingerprint("DB1", 22, "SHA256:b", "ssh-key").await.unwrap();
        assert_eq!(upper_changed.old_fingerprint.as_deref(), Some("SHA256:a"));
        let other_port = manager.verify_host_key_by_fingerprint("db1", 2222, "SHA256:b", "ssh-key").await.unwrap();
        assert!(!other_port.allowed);
        assert!(matches!(other_port.status, TrustStatus::Changed));
        assert_eq!(other_port.old_fingerprint.as_deref(), Some("SHA256:a"));
        // One server on two ports with the same key is still a plain first use.
        let same_key = manager.verify_host_key_by_fingerprint("db1", 2222, "SHA256:a", "ssh-key").await.unwrap();
        assert!(matches!(same_key.status, TrustStatus::Unknown));
        assert_eq!(same_key.old_fingerprint, None);
        // PR #68 review: with different keys trusted on two ports, a new port presenting one
        // of them still differs from the other, so it's Changed, not a first use.
        manager.trust_host_key("db1", 2222, "SHA256:b", "ssh-key", vec![2], TrustStatus::Trusted).await.unwrap();
        let matches_one = manager.verify_host_key_by_fingerprint("db1", 3333, "SHA256:a", "ssh-key").await.unwrap();
        assert!(!matches_one.allowed);
        assert!(matches!(matches_one.status, TrustStatus::Changed));
        assert_eq!(matches_one.old_fingerprint.as_deref(), Some("SHA256:b"));
        let _ = std::fs::remove_file(&db_path);
    }

    /// D7 (russh 0.63): a plain key must fingerprint exactly as before the upgrade, or every
    /// stored pin would read as a changed key; a host certificate pins to the key it carries.
    #[test]
    fn presented_key_bytes_are_stable_and_certificates_pin_their_key() {
        use russh::keys::ssh_key::certificate::{Builder, CertType};
        use russh::keys::{PublicKeyBase64, PublicKeyOrCertificate};
        let host = russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
        let ca = russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
        let plain: PublicKeyOrCertificate = host.public_key().clone().into();
        assert_eq!(presented_key_bytes(&plain), host.public_key().public_key_bytes());

        let mut builder = Builder::new([7u8; 16], host.public_key().key_data().clone(), 0, u64::MAX).unwrap();
        builder.cert_type(CertType::Host).unwrap();
        builder.key_id("test-host").unwrap();
        builder.all_principals_valid().unwrap();
        let cert = builder.sign(&ca).unwrap();
        let certified: PublicKeyOrCertificate = cert.into();
        assert_eq!(presented_key_bytes(&certified), host.public_key().public_key_bytes());
    }

    /// TEST-003: table test of the non-interactive decision (exec, pool, SFTP).
    #[tokio::test]
    async fn test_non_interactive_accepts_only_a_trusted_key_for_that_port() {
        use russh::keys::PublicKeyBase64;
        let (manager, db_path) = create_test_manager().await;
        let key = |_: ()| {
            russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone()
        };
        let (trusted, other, rejected) = (key(()), key(()), key(()));
        let fp = |k: &russh::keys::PublicKey| crate::crypto::ssh_host_key_fingerprint(&k.public_key_bytes());
        manager.trust_host_key("h", 22, &fp(&trusted), "ssh-key", vec![1], TrustStatus::Trusted).await.unwrap();
        manager.trust_host_key("r", 22, &fp(&rejected), "ssh-key", vec![2], TrustStatus::Rejected).await.unwrap();

        let cases: [(&str, u16, &russh::keys::PublicKey, bool); 5] = [
            ("h", 22, &trusted, true),     // stored and trusted
            ("h", 2222, &trusted, false),  // same key, other port
            ("h", 22, &other, false),      // changed key
            ("new", 22, &other, false),    // never seen: no TOFU without a prompt
            ("r", 22, &rejected, false),   // explicitly rejected
        ];
        for (host, port, k, allowed) in cases {
            let decision = manager.check_non_interactive(host, port, &k.clone().into()).await;
            assert_eq!(decision.is_ok(), allowed, "{}:{}", host, port);
        }
        manager.conn.lock().unwrap().execute_batch("DROP TABLE ssh_known_hosts;").unwrap();
        assert!(manager.check_non_interactive("h", 22, &trusted.clone().into()).await.is_err(), "lookup error fails closed");
        let _ = std::fs::remove_file(&db_path);
    }

    /// TEST-003: a rejected key is not allowed; ports are isolated; a changed key reports the
    /// old fingerprint on its own (FE-010).
    #[tokio::test]
    async fn test_rejected_port_isolation_and_old_fingerprint() {
        let (manager, db_path) = create_test_manager().await;
        manager
            .trust_host_key("h", 22, "SHA256:old", "ssh-key", vec![1], TrustStatus::Trusted)
            .await
            .unwrap();
        let other_port = manager.verify_host_key_by_fingerprint("h", 2222, "SHA256:old", "ssh-key").await.unwrap();
        assert!(!other_port.allowed);
        let changed = manager.verify_host_key_by_fingerprint("h", 22, "SHA256:new", "ssh-key").await.unwrap();
        assert!(!changed.allowed);
        assert_eq!(changed.old_fingerprint.as_deref(), Some("SHA256:old"));
        manager
            .trust_host_key("r", 22, "SHA256:r", "ssh-key", vec![2], TrustStatus::Rejected)
            .await
            .unwrap();
        let rejected = manager.verify_host_key_by_fingerprint("r", 22, "SHA256:r", "ssh-key").await.unwrap();
        assert!(!rejected.allowed);
        assert!(matches!(rejected.status, TrustStatus::Rejected));
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_trust_and_verify() {
        let (manager, db_path) = create_test_manager().await;

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
        cleanup_test_db(&db_path);
    }

    #[tokio::test]
    async fn test_key_changed_detection() {
        let (manager, db_path) = create_test_manager().await;

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
        cleanup_test_db(&db_path);
    }
}

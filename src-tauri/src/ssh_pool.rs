//! Reuse of authenticated SSH sessions for non-interactive commands.
//!
//! Every one-shot command used to pay a full TCP connect, key exchange and
//! authentication. Measured against a LAN host that handshake was ~290 ms of a
//! ~550 ms metrics probe, and monitoring repeats the probe for every host on a
//! 30 second cycle. Sessions are therefore cached per (host, port, user, auth)
//! and reused; russh multiplexes channels, so a cached session can serve
//! several concurrent commands.
//!
//! Security notes:
//! - The host key is verified when a session is established, not per command
//!   (the same trade-off OpenSSH's ControlMaster makes). `MAX_LIFETIME` bounds
//!   how long a session may run before it is rebuilt and the key re-verified.
//! - The cache key includes a hash of the authentication material, so changed
//!   or rotated credentials never reuse a session authenticated with the old
//!   secret. Only the hash is retained, never the secret itself.

use crate::ssh_exec::ExecClient;
use russh::client::Handle;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::Mutex as AsyncMutex;

/// Sessions unused for longer than this are closed. Kept comfortably above the
/// 30 second monitoring cycle so consecutive polls reuse one session, and below
/// the idle timeout typical SSH servers enforce.
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Hard cap on session age, so host keys are re-verified periodically.
const MAX_LIFETIME: Duration = Duration::from_secs(1800);

/// Identifies a reusable session. Two requests share a session only if they
/// target the same endpoint *and* present the same authentication material.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PoolKey {
    host: String,
    port: u16,
    username: String,
    /// SHA-256 over the auth material. The secrets themselves are never stored.
    auth_fingerprint: [u8; 32],
}

/// Everything needed to establish a session, and to decide whether an existing
/// one may be reused.
pub struct ConnectParams<'a> {
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub password: Option<&'a str>,
    pub key_path: Option<&'a str>,
    pub private_key: Option<&'a str>,
    pub key_passphrase: Option<&'a str>,
    pub timeout_secs: u64,
}

impl ConnectParams<'_> {
    pub fn pool_key(&self) -> PoolKey {
        let mut hasher = Sha256::new();
        // Length-prefix each component so that distinct field values cannot be
        // concatenated into the same digest.
        for part in [
            self.password,
            self.key_path,
            self.private_key,
            self.key_passphrase,
        ] {
            match part {
                Some(value) => {
                    hasher.update((value.len() as u64).to_le_bytes());
                    hasher.update(value.as_bytes());
                }
                None => hasher.update(u64::MAX.to_le_bytes()),
            }
        }
        let mut auth_fingerprint = [0u8; 32];
        auth_fingerprint.copy_from_slice(&hasher.finalize());

        PoolKey {
            host: self.host.to_string(),
            port: self.port,
            username: self.username.to_string(),
            auth_fingerprint,
        }
    }
}

struct PooledSession {
    handle: Arc<Handle<ExecClient>>,
    established: Instant,
    last_used: Instant,
}

impl PooledSession {
    /// A session is only handed out if it is still connected and within both
    /// the idle and absolute lifetime bounds.
    fn is_usable(&self) -> bool {
        !self.handle.is_closed()
            && self.last_used.elapsed() < IDLE_TIMEOUT
            && self.established.elapsed() < MAX_LIFETIME
    }
}

/// A session borrowed from the pool.
pub struct Lease {
    pub handle: Arc<Handle<ExecClient>>,
    /// True when this came from the cache. Only a reused session may be
    /// silently retried, because a freshly built one failing is a real error.
    pub reused: bool,
}

/// One slot per endpoint. The async mutex serialises connection setup for a
/// single host without blocking other hosts.
type Slot = Arc<AsyncMutex<Option<PooledSession>>>;

#[derive(Default)]
pub struct SshConnectionPool {
    slots: Mutex<HashMap<PoolKey, Slot>>,
}

impl SshConnectionPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// The map lock is held only long enough to clone the slot handle.
    fn slot(&self, key: &PoolKey) -> Slot {
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        slots.entry(key.clone()).or_default().clone()
    }

    /// Return a usable session, establishing one only if necessary.
    pub async fn acquire(
        &self,
        app_handle: &AppHandle,
        params: &ConnectParams<'_>,
    ) -> Result<Lease, String> {
        let key = params.pool_key();
        let slot = self.slot(&key);
        let mut guard = slot.lock().await;

        if let Some(session) = guard.as_ref() {
            if session.is_usable() {
                let handle = session.handle.clone();
                if let Some(session) = guard.as_mut() {
                    session.last_used = Instant::now();
                }
                return Ok(Lease {
                    handle,
                    reused: true,
                });
            }
            // Stale or dead: drop it before building a replacement.
            if let Some(session) = guard.take() {
                let _ = session
                    .handle
                    .disconnect(russh::Disconnect::ByApplication, "", "en")
                    .await;
            }
        }

        let handle = Arc::new(connect_and_authenticate(app_handle, params).await?);
        *guard = Some(PooledSession {
            handle: handle.clone(),
            established: Instant::now(),
            last_used: Instant::now(),
        });

        Ok(Lease {
            handle,
            reused: false,
        })
    }

    /// Drop a cached session, e.g. after discovering it is no longer usable.
    pub async fn invalidate(&self, params: &ConnectParams<'_>) {
        let slot = self.slot(&params.pool_key());
        let mut guard = slot.lock().await;
        if let Some(session) = guard.take() {
            let _ = session
                .handle
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
        }
    }

    /// Close sessions that have gone idle or exceeded their lifetime.
    ///
    /// Slots currently in use are skipped rather than waited on; they will be
    /// caught by a later sweep.
    pub async fn sweep_idle(&self) -> usize {
        let slots: Vec<(PoolKey, Slot)> = {
            let slots = self
                .slots
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            slots.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        };

        let mut closed = 0;
        let mut empty_keys = Vec::new();

        for (key, slot) in slots {
            let Ok(mut guard) = slot.try_lock() else {
                continue; // busy: leave it for the next sweep
            };
            match guard.as_ref() {
                Some(session) if !session.is_usable() => {
                    if let Some(session) = guard.take() {
                        let _ = session
                            .handle
                            .disconnect(russh::Disconnect::ByApplication, "", "en")
                            .await;
                        closed += 1;
                    }
                    empty_keys.push(key);
                }
                None => empty_keys.push(key),
                _ => {}
            }
        }

        // Forget slots holding nothing so the map does not grow with every host
        // that was ever contacted.
        if !empty_keys.is_empty() {
            let mut slots = self
                .slots
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for key in empty_keys {
                // Only remove if still unused, so a concurrent acquire is not lost.
                if let Some(slot) = slots.get(&key) {
                    if Arc::strong_count(slot) == 1 {
                        slots.remove(&key);
                    }
                }
            }
        }

        closed
    }

    /// Number of cached slots. Test/diagnostic helper.
    #[cfg(test)]
    pub fn slot_count(&self) -> usize {
        self.slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

/// Open a TCP connection, verify the host key and authenticate.
async fn connect_and_authenticate(
    app_handle: &AppHandle,
    params: &ConnectParams<'_>,
) -> Result<Handle<ExecClient>, String> {
    let config = Arc::new(russh::client::Config::default());
    let handler = ExecClient::new(app_handle.clone(), params.host.to_string(), params.port);
    let mut session = crate::ssh_connect::connect_with_diagnostics(
        config,
        params.host,
        params.port,
        handler,
        Duration::from_secs(params.timeout_secs),
    )
    .await?;

    crate::ssh_auth::authenticate(
        &mut session,
        params.username,
        params.password,
        params.key_path,
        params.private_key,
        params.key_passphrase,
    )
    .await
    .map_err(|e| format!("Authentication error: {}", e))?;

    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params<'a>(
        host: &'a str,
        username: &'a str,
        password: Option<&'a str>,
    ) -> ConnectParams<'a> {
        ConnectParams {
            host,
            port: 22,
            username,
            password,
            key_path: None,
            private_key: None,
            key_passphrase: None,
            timeout_secs: 10,
        }
    }

    #[test]
    fn test_same_endpoint_and_auth_share_a_key() {
        let a = params("10.0.0.1", "edward", Some("hunter2")).pool_key();
        let b = params("10.0.0.1", "edward", Some("hunter2")).pool_key();
        assert_eq!(a, b);
    }

    #[test]
    fn test_changed_password_produces_a_different_key() {
        // A rotated or revoked secret must never reuse the old session.
        let old = params("10.0.0.1", "edward", Some("hunter2")).pool_key();
        let new = params("10.0.0.1", "edward", Some("hunter3")).pool_key();
        assert_ne!(old, new);
    }

    #[test]
    fn test_absent_and_empty_credentials_are_distinct() {
        let none = params("10.0.0.1", "edward", None).pool_key();
        let empty = params("10.0.0.1", "edward", Some("")).pool_key();
        assert_ne!(
            none, empty,
            "no credential must not collide with an empty one"
        );
    }

    #[test]
    fn test_different_host_user_or_port_do_not_share() {
        let base = params("10.0.0.1", "edward", Some("pw")).pool_key();
        assert_ne!(base, params("10.0.0.2", "edward", Some("pw")).pool_key());
        assert_ne!(base, params("10.0.0.1", "root", Some("pw")).pool_key());

        let mut other_port = params("10.0.0.1", "edward", Some("pw"));
        other_port.port = 2222;
        assert_ne!(base, other_port.pool_key());
    }

    #[test]
    fn test_key_material_fields_are_not_interchangeable() {
        // Length prefixing keeps distinct fields from hashing to the same digest.
        let mut as_path = params("h", "u", None);
        as_path.key_path = Some("secret");
        let mut as_pem = params("h", "u", None);
        as_pem.private_key = Some("secret");
        assert_ne!(as_path.pool_key(), as_pem.pool_key());
    }

    #[test]
    fn test_pool_key_does_not_retain_the_secret() {
        let key = params("10.0.0.1", "edward", Some("super-secret-password")).pool_key();
        let rendered = format!("{:?}", key);
        assert!(
            !rendered.contains("super-secret-password"),
            "the pool key must not carry the plaintext secret"
        );
    }

    /// Test-only handler. Accepts any host key because this benchmark targets a
    /// host the operator names explicitly; the production path verifies keys via
    /// `ExecClient`.
    struct BenchHandler;

    impl russh::client::Handler for BenchHandler {
        type Error = russh::Error;
        async fn check_server_key(
            &mut self,
            _key: &russh::keys::PublicKey,
        ) -> Result<bool, Self::Error> {
            Ok(true)
        }
    }

    async fn bench_connect(
        host: &str,
        port: u16,
        user: &str,
        key_pem: &str,
    ) -> Result<Handle<BenchHandler>, String> {
        let config = Arc::new(russh::client::Config::default());
        let mut session = russh::client::connect(config, (host, port), BenchHandler)
            .await
            .map_err(|e| e.to_string())?;
        let key = russh::keys::decode_secret_key(key_pem, None).map_err(|e| e.to_string())?;
        let alg = session.best_supported_rsa_hash().await.ok().flatten().flatten();
        let res = session
            .authenticate_publickey(user, russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), alg))
            .await
            .map_err(|e| e.to_string())?;
        if !matches!(res, russh::client::AuthResult::Success) {
            return Err("auth failed".to_string());
        }
        Ok(session)
    }

    async fn bench_run(session: &Handle<BenchHandler>, cmd: &str) -> Result<(), String> {
        let mut ch = session
            .channel_open_session()
            .await
            .map_err(|e| e.to_string())?;
        ch.exec(true, cmd.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        while let Some(msg) = ch.wait().await {
            if matches!(msg, russh::ChannelMsg::Eof) {
                break;
            }
        }
        Ok(())
    }

    /// Measures the saving this pool exists to capture, against a real server.
    ///
    /// Run with:
    ///   QUASAR_TEST_SSH_HOST=<ip> QUASAR_TEST_SSH_USER=<user> \
    ///   cargo test ssh_pool::tests::bench -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "requires a live SSH host; set QUASAR_TEST_SSH_HOST and QUASAR_TEST_SSH_USER"]
    async fn bench_session_reuse_against_live_host() {
        let (Ok(host), Ok(user)) = (
            std::env::var("QUASAR_TEST_SSH_HOST"),
            std::env::var("QUASAR_TEST_SSH_USER"),
        ) else {
            eprintln!("skipped: QUASAR_TEST_SSH_HOST / QUASAR_TEST_SSH_USER not set");
            return;
        };
        let port: u16 = std::env::var("QUASAR_TEST_SSH_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(22);
        let key_file = std::env::var("QUASAR_TEST_SSH_KEY").unwrap_or_else(|_| {
            format!(
                "{}/.ssh/id_ed25519",
                std::env::var("USERPROFILE")
                    .or_else(|_| std::env::var("HOME"))
                    .unwrap_or_default()
            )
        });
        let key_pem = std::fs::read_to_string(&key_file)
            .unwrap_or_else(|e| panic!("cannot read key {key_file}: {e}"));

        const ROUNDS: usize = 5;
        let cmd = "cat /proc/loadavg";

        // Old behaviour: connect + authenticate for every command.
        let start = Instant::now();
        for _ in 0..ROUNDS {
            let s = bench_connect(&host, port, &user, &key_pem).await.unwrap();
            bench_run(&s, cmd).await.unwrap();
            let _ = s
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
        }
        let fresh = start.elapsed();

        // Pooled behaviour: one handshake, then reuse for every command.
        let start = Instant::now();
        let s = bench_connect(&host, port, &user, &key_pem).await.unwrap();
        for _ in 0..ROUNDS {
            bench_run(&s, cmd).await.unwrap();
        }
        let pooled = start.elapsed();
        let _ = s
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;

        println!("fresh connection each time : {fresh:?} for {ROUNDS} commands");
        println!("reused session             : {pooled:?} for {ROUNDS} commands");
        println!(
            "speedup                    : {:.1}x",
            fresh.as_secs_f64() / pooled.as_secs_f64()
        );

        assert!(
            pooled < fresh,
            "reusing a session must beat reconnecting: {pooled:?} vs {fresh:?}"
        );
    }

    #[tokio::test]
    async fn test_sweep_discards_slots_with_no_session() {
        let pool = SshConnectionPool::new();
        // Touch a slot without ever establishing a session.
        let _ = pool.slot(&params("10.0.0.1", "edward", None).pool_key());
        assert_eq!(pool.slot_count(), 1);

        pool.sweep_idle().await;
        assert_eq!(
            pool.slot_count(),
            0,
            "empty slots must not accumulate for every host ever contacted"
        );
    }
}

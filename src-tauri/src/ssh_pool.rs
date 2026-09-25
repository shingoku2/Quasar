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
use russh::client::{Handle, Handler};
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

struct PooledSession<H: Handler> {
    handle: Arc<Handle<H>>,
    /// `MAX_LIFETIME` after the session was established.
    expires_at: Instant,
    last_used: Instant,
}

impl<H: Handler> PooledSession<H> {
    /// A session is only handed out if it is still connected and within both
    /// the idle and absolute lifetime bounds.
    fn is_usable(&self) -> bool {
        !self.handle.is_closed()
            && self.last_used.elapsed() < IDLE_TIMEOUT
            && Instant::now() < self.expires_at
    }

    /// True while a `Lease` still holds the handle, i.e. a command may be running on it.
    fn is_leased(&self) -> bool {
        Arc::strong_count(&self.handle) > 1
    }

    /// Takes a retired session out of service. A leased one is only dropped from the pool,
    /// not disconnected: its command finishes and the connection closes when the lease
    /// drops the last handle. Disconnecting it killed commands mid-run (audit RUST-007).
    async fn retire(self) -> bool {
        if self.is_leased() {
            return false;
        }
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        true
    }
}

/// A session borrowed from the pool.
pub struct Lease<H: Handler = ExecClient> {
    pub handle: Arc<Handle<H>>,
    /// True when this came from the cache. Only a reused session may be
    /// silently retried, because a freshly built one failing is a real error.
    pub reused: bool,
}

/// One slot per endpoint. The async mutex serialises connection setup for a
/// single host without blocking other hosts.
type Slot<H> = Arc<AsyncMutex<Option<PooledSession<H>>>>;

pub struct SshConnectionPool<H: Handler = ExecClient> {
    slots: Mutex<HashMap<PoolKey, Slot<H>>>,
}

impl SshConnectionPool<ExecClient> {
    pub fn new() -> Self {
        Self::empty()
    }

    /// Return a usable session, establishing one only if necessary.
    pub async fn acquire(
        &self,
        app_handle: &AppHandle,
        params: &ConnectParams<'_>,
    ) -> Result<Lease, String> {
        self.acquire_with(params.pool_key(), || connect_and_authenticate(app_handle, params))
            .await
    }
}

impl<H: Handler> SshConnectionPool<H> {
    fn empty() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
        }
    }

    /// The map lock is held only long enough to clone the slot handle.
    fn slot(&self, key: &PoolKey) -> Slot<H> {
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        slots.entry(key.clone()).or_default().clone()
    }

    /// Return a usable session for `key`, calling `connect` only if necessary.
    async fn acquire_with<F, Fut>(&self, key: PoolKey, connect: F) -> Result<Lease<H>, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Handle<H>, String>>,
    {
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
                session.retire().await;
            }
        }

        let handle = Arc::new(connect().await?);
        *guard = Some(PooledSession {
            handle: handle.clone(),
            expires_at: Instant::now() + MAX_LIFETIME,
            last_used: Instant::now(),
        });

        Ok(Lease {
            handle,
            reused: false,
        })
    }

    /// Drop the cached session that `failed` belongs to, after a command on it failed.
    /// Only that session: another task may already have replaced it with a fresh one, which
    /// stays. It's retired, not disconnected, so commands other leases are still running on
    /// it (e.g. when one hit the server's MaxSessions limit) aren't killed; it closes once
    /// the last lease drops it (PR #68 review; RUST-007).
    pub async fn invalidate(&self, params: &ConnectParams<'_>, failed: &Arc<Handle<H>>) {
        self.invalidate_key(&params.pool_key(), failed).await;
    }

    async fn invalidate_key(&self, key: &PoolKey, failed: &Arc<Handle<H>>) {
        let slot = self.slot(key);
        let mut guard = slot.lock().await;
        if guard.as_ref().is_some_and(|s| Arc::ptr_eq(&s.handle, failed)) {
            if let Some(session) = guard.take() {
                session.retire().await;
            }
        }
    }

    /// Close sessions that have gone idle or exceeded their lifetime.
    ///
    /// Slots currently in use are skipped rather than waited on; they will be
    /// caught by a later sweep.
    pub async fn sweep_idle(&self) -> usize {
        let slots: Vec<(PoolKey, Slot<H>)> = {
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
                // A leased session past its lifetime is still running a command: leave it
                // for a later sweep (acquire won't hand it out, since it isn't usable).
                Some(session) if !session.is_usable() && !session.is_leased() => {
                    if let Some(session) = guard.take() {
                        if session.retire().await {
                            closed += 1;
                        }
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

    /// Expires every cached session now. Test helper for lifetime expiry.
    #[cfg(test)]
    async fn expire_sessions(&self) {
        let slots: Vec<Slot<H>> = self
            .slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .cloned()
            .collect();
        for slot in slots {
            if let Some(session) = slot.lock().await.as_mut() {
                session.expires_at = Instant::now();
            }
        }
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

    struct AcceptAnyKey;

    impl russh::client::Handler for AcceptAnyKey {
        type Error = russh::Error;
        async fn check_server_key(
            &mut self,
            _key: &russh::keys::PublicKeyOrCertificate,
        ) -> Result<bool, Self::Error> {
            Ok(true)
        }
    }

    async fn connect_test(port: u16) -> Result<Handle<AcceptAnyKey>, String> {
        let mut session = crate::ssh_connect::connect_with_diagnostics(
            Arc::new(russh::client::Config::default()),
            "127.0.0.1",
            port,
            AcceptAnyKey,
            Duration::from_secs(10),
        )
        .await?;
        crate::ssh_auth::authenticate(&mut session, "u", None, None, None, None).await?;
        Ok(session)
    }

    /// RUST-007: a session past its lifetime must not be disconnected while a lease is
    /// still using it; it's closed once the lease is gone.
    #[tokio::test]
    async fn expired_sessions_are_not_closed_under_an_active_lease() {
        let policy = crate::ssh_test_server::Policy { accept_none: true, ..Default::default() };
        let (port, _) = crate::ssh_test_server::spawn(policy).await;
        let pool: SshConnectionPool<AcceptAnyKey> = SshConnectionPool::empty();
        let key = params("127.0.0.1", "u", None).pool_key();

        let lease = pool.acquire_with(key.clone(), || connect_test(port)).await.unwrap();
        pool.expire_sessions().await;

        assert_eq!(pool.sweep_idle().await, 0, "leased session must survive the sweep");
        // A new acquire replaces the expired session without killing the leased one.
        let fresh = pool.acquire_with(key.clone(), || connect_test(port)).await.unwrap();
        assert!(!fresh.reused);
        assert!(!lease.handle.is_closed());
        assert!(lease.handle.channel_open_session().await.is_ok(), "leased session still works");
        drop(lease);
        drop(fresh);

        pool.expire_sessions().await;
        assert_eq!(pool.sweep_idle().await, 1, "unleased expired session is closed");
    }

    /// PR #68 review: a failed command invalidates only its own session, and never closes
    /// it under other leases still running commands on it; a replacement another task
    /// already made is kept.
    #[tokio::test]
    async fn invalidate_keeps_other_leases_and_newer_sessions() {
        let policy = crate::ssh_test_server::Policy { accept_none: true, ..Default::default() };
        let (port, _) = crate::ssh_test_server::spawn(policy).await;
        let pool: SshConnectionPool<AcceptAnyKey> = SshConnectionPool::empty();
        let key = params("127.0.0.1", "u", None).pool_key();

        let first = pool.acquire_with(key.clone(), || connect_test(port)).await.unwrap();
        let second = pool.acquire_with(key.clone(), || connect_test(port)).await.unwrap();
        assert!(second.reused && Arc::ptr_eq(&first.handle, &second.handle));

        pool.invalidate_key(&key, &first.handle).await;
        assert!(!second.handle.is_closed(), "the other lease's session must stay open");
        assert!(second.handle.channel_open_session().await.is_ok());

        let replacement = pool.acquire_with(key.clone(), || connect_test(port)).await.unwrap();
        assert!(!replacement.reused, "the invalidated session left the pool");
        // A late invalidate for the old session must not evict the replacement.
        pool.invalidate_key(&key, &first.handle).await;
        let again = pool.acquire_with(key.clone(), || connect_test(port)).await.unwrap();
        assert!(again.reused && Arc::ptr_eq(&again.handle, &replacement.handle));
    }

    /// Test-only handler. Accepts any host key because this benchmark targets a
    /// host the operator names explicitly; the production path verifies keys via
    /// `ExecClient`.
    struct BenchHandler;

    impl russh::client::Handler for BenchHandler {
        type Error = russh::Error;
        async fn check_server_key(
            &mut self,
            _key: &russh::keys::PublicKeyOrCertificate,
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

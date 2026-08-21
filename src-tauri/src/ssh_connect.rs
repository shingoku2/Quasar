//! Shared SSH connection establishment with phase-aware error reporting.
//!
//! Splits connection setup into three phases — DNS resolution, TCP connect,
//! and SSH handshake — so a failure names the exact phase (and address) that
//! failed instead of collapsing everything into one opaque "Connection timed
//! out". Each resolved address gets its own TCP attempt, so a host whose
//! first DNS record is unreachable (e.g. a dead IPv6 route) no longer starves
//! a working address under a single shared timer.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;

/// Upper bound for resolving a hostname to socket addresses.
const DNS_TIMEOUT: Duration = Duration::from_secs(5);

/// Connect to `host:port` and complete the SSH handshake.
///
/// `timeout` bounds each phase separately (every TCP attempt and the
/// handshake), not all phases combined.
pub async fn connect_with_diagnostics<H>(
    config: Arc<russh::client::Config>,
    host: &str,
    port: u16,
    handler: H,
    timeout: Duration,
) -> Result<russh::client::Handle<H>, String>
where
    H: russh::client::Handler + Send + 'static,
    H::Error: std::fmt::Display,
{
    let addrs: Vec<SocketAddr> =
        match tokio::time::timeout(DNS_TIMEOUT, tokio::net::lookup_host((host, port))).await {
            Ok(Ok(addrs)) => addrs.collect(),
            Ok(Err(e)) => return Err(format!("DNS lookup for '{}' failed: {}", host, e)),
            Err(_) => {
                return Err(format!(
                    "DNS lookup for '{}' timed out after {}s",
                    host,
                    DNS_TIMEOUT.as_secs()
                ))
            }
        };
    if addrs.is_empty() {
        return Err(format!("DNS lookup for '{}' returned no addresses", host));
    }

    let mut attempts: Vec<String> = Vec::new();
    let mut connected: Option<(TcpStream, SocketAddr)> = None;
    for addr in addrs {
        match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                connected = Some((stream, addr));
                break;
            }
            Ok(Err(e)) => attempts.push(format!("{}: {}", addr, e)),
            Err(_) => attempts.push(format!(
                "{}: no response after {}s (host down, or port filtered by a firewall)",
                addr,
                timeout.as_secs()
            )),
        }
    }
    let (stream, addr) = match connected {
        Some(ok) => ok,
        None => {
            return Err(format!(
                "TCP connection to {}:{} failed — {}",
                host,
                port,
                attempts.join("; ")
            ))
        }
    };

    if config.nodelay {
        // Best-effort, mirroring russh::client::connect().
        let _ = stream.set_nodelay(true);
    }

    match tokio::time::timeout(
        timeout,
        russh::client::connect_stream(config, stream, handler),
    )
    .await
    {
        Ok(Ok(handle)) => Ok(handle),
        Ok(Err(e)) => Err(format!("SSH handshake with {} failed: {}", addr, e)),
        Err(_) => Err(format!(
            "SSH handshake with {} timed out after {}s (the port is open but did not complete an SSH key exchange)",
            addr,
            timeout.as_secs()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHandler;

    impl russh::client::Handler for TestHandler {
        type Error = russh::Error;

        async fn check_server_key(
            &mut self,
            _server_public_key: &russh::keys::PublicKey,
        ) -> Result<bool, Self::Error> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn tcp_refusal_is_reported_as_tcp_phase_failure() {
        // Bind then drop a listener to obtain a port that is almost certainly closed.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let err = connect_with_diagnostics(
            Arc::new(russh::client::Config::default()),
            "127.0.0.1",
            port,
            TestHandler,
            Duration::from_secs(2),
        )
        .await
        .err()
        .expect("connecting to a closed port must fail");

        assert!(
            err.contains("TCP connection to 127.0.0.1"),
            "expected TCP-phase error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn silent_server_is_reported_as_handshake_failure() {
        // A listener that accepts but never speaks SSH: the TCP phase succeeds
        // and the handshake phase must be the one that times out.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let hold = tokio::spawn(async move {
            let _conn = listener.accept().await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        let err = connect_with_diagnostics(
            Arc::new(russh::client::Config::default()),
            "127.0.0.1",
            port,
            TestHandler,
            Duration::from_secs(1),
        )
        .await
        .err()
        .expect("connecting to a silent server must fail");
        hold.abort();

        assert!(
            err.contains("SSH handshake with"),
            "expected handshake-phase error, got: {}",
            err
        );
    }
}

//! In-process SSH server for tests (audit TEST-003/TEST-004): records every authentication
//! attempt and accepts according to a policy, so connect/auth behaviour can be tested
//! against real russh handshakes on 127.0.0.1.

use russh::server::{Auth, ChannelOpenHandle, Handler, Msg, Session};
use russh::{Channel, ChannelId};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Default)]
pub struct Policy {
    pub accept_none: bool,
    pub password: Option<String>,
    pub accept_publickey: bool,
    /// Reply to any `exec` request. Sent like OpenSSH does: output, EOF, then the exit
    /// status, then close (the status arrives *after* EOF).
    pub exec: Option<ExecReply>,
}

#[derive(Clone, Default)]
pub struct ExecReply {
    pub output: Vec<u8>,
    pub exit_status: u32,
    /// Send EOF and the exit status but never close the channel (some embedded servers
    /// wait for the client to close).
    pub hold_open: bool,
}

#[derive(Clone)]
pub struct TestServer {
    pub policy: Policy,
    pub attempts: Arc<Mutex<Vec<String>>>,
}

impl Handler for TestServer {
    type Error = russh::Error;

    async fn auth_none(&mut self, user: &str) -> Result<Auth, Self::Error> {
        self.attempts.lock().unwrap().push(format!("none:{}", user));
        Ok(if self.policy.accept_none { Auth::Accept } else { Auth::reject() })
    }

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        self.attempts.lock().unwrap().push(format!("password:{}", user));
        Ok(if self.policy.password.as_deref() == Some(password) { Auth::Accept } else { Auth::reject() })
    }

    async fn auth_publickey_offered(
        &mut self,
        _user: &str,
        _public_key: &russh::keys::PublicKey,
    ) -> Result<Auth, Self::Error> {
        Ok(if self.policy.accept_publickey { Auth::Accept } else { Auth::reject() })
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        _public_key: &russh::keys::PublicKey,
    ) -> Result<Auth, Self::Error> {
        self.attempts.lock().unwrap().push(format!("publickey:{}", user));
        Ok(if self.policy.accept_publickey { Auth::Accept } else { Auth::reject() })
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        _data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let Some(reply) = self.policy.exec.clone() else {
            session.channel_failure(channel)?;
            return Ok(());
        };
        session.channel_success(channel)?;
        for chunk in reply.output.chunks(32 * 1024) {
            session.data(channel, chunk.to_vec())?;
        }
        session.eof(channel)?;
        session.exit_status_request(channel, reply.exit_status)?;
        if !reply.hold_open {
            session.close(channel)?;
        }
        Ok(())
    }
}

/// Starts a server on 127.0.0.1 that serves connections until the test ends. Returns the
/// port and the shared log of authentication attempts.
pub async fn spawn(policy: Policy) -> (u16, Arc<Mutex<Vec<String>>>) {
    let (port, attempts, _) = spawn_killable(policy).await;
    (port, attempts)
}

/// Ends every open server-side session, like a server restart.
#[derive(Clone)]
pub struct Killer(Arc<Mutex<Vec<russh::server::Handle>>>);

impl Killer {
    pub async fn kill_connections(&self) {
        let handles: Vec<_> = self.0.lock().unwrap().drain(..).collect();
        for handle in handles {
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "test kill".into(), "en".into())
                .await;
        }
    }
}

pub async fn spawn_killable(policy: Policy) -> (u16, Arc<Mutex<Vec<String>>>, Killer) {
    let config = Arc::new(russh::server::Config {
        auth_rejection_time: Duration::from_millis(0),
        auth_rejection_time_initial: Some(Duration::from_millis(0)),
        keys: vec![russh::keys::PrivateKey::random(
            &mut rand::rng(),
            russh::keys::Algorithm::Ed25519,
        )
        .unwrap()],
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let killer = Killer(Arc::new(Mutex::new(Vec::new())));
    let handler = TestServer { policy, attempts: attempts.clone() };
    let tasks = killer.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let config = config.clone();
            let handler = handler.clone();
            if let Ok(session) = russh::server::run_stream(config, stream, handler).await {
                tasks.0.lock().unwrap().push(session.handle());
                tokio::spawn(async move {
                    let _ = session.await;
                });
            }
        }
    });
    (port, attempts, killer)
}

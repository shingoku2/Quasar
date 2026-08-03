use crate::crypto;
use crate::vault::SshKeyManager;
use russh::keys::PublicKeyBase64;
use russh::*;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Simple SSH client for command execution (non-interactive)
#[derive(Clone)]
pub struct ExecClient {
    app_handle: AppHandle,
    host: String,
    port: u16,
}

impl ExecClient {
    pub fn new(app_handle: AppHandle, host: String, port: u16) -> Self {
        Self {
            app_handle,
            host,
            port,
        }
    }
}

impl client::Handler for ExecClient {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Get SSH key manager from app state
        let ssh_key_manager = self.app_handle.state::<SshKeyManager>();

        // Compute SHA256 fingerprint using shared utility (RFC 4253 §6.6)
        let key_bytes = server_public_key.public_key_bytes();
        let fingerprint = crypto::ssh_host_key_fingerprint(&key_bytes);
        let key_type = "ssh-key";

        // Verify host key
        match ssh_key_manager
            .verify_host_key_by_fingerprint(&self.host, self.port, &fingerprint, key_type)
            .await
        {
            Ok(result) => {
                if result.allowed {
                    Ok(true)
                } else {
                    // Reject connection for non-interactive execution if not already trusted
                    // Note: In non-interactive mode we can't easily prompt the user,
                    // so we only allow already trusted hosts.
                    Err(russh::Error::Disconnect)
                }
            }
            Err(_) => Err(russh::Error::Disconnect),
        }
    }
}

/// Why a command attempt failed, so the caller can tell whether retrying is safe.
enum RunFailure {
    /// The channel could not be opened, so the command was never delivered.
    /// Retrying cannot double-execute anything.
    ChannelOpen(String),
    /// The command was (or may have been) delivered. Never retried.
    AfterDelivery(String),
}

impl RunFailure {
    fn into_message(self) -> String {
        match self {
            RunFailure::ChannelOpen(m) | RunFailure::AfterDelivery(m) => m,
        }
    }

    /// Whether the attempt may be transparently retried.
    ///
    /// Only a *reused* session failing at channel-open qualifies: the command
    /// was provably never delivered, and the likely cause is a cached session
    /// that died since it was last checked. Anything past delivery must not be
    /// retried — a scheduled task could otherwise run its side effects twice.
    fn is_retryable(&self, reused: bool) -> bool {
        reused && matches!(self, RunFailure::ChannelOpen(_))
    }
}

/// Run one command over an already-authenticated session.
async fn run_command(
    session: &russh::client::Handle<ExecClient>,
    command: &str,
    timeout_secs: u64,
) -> Result<String, RunFailure> {
    let mut channel = session.channel_open_session().await.map_err(|e| {
        RunFailure::ChannelOpen(format!("Failed to open channel: {}", e))
    })?;

    channel.exec(true, command.as_bytes()).await.map_err(|e| {
        // exec may have reached the server, so this is not retryable.
        RunFailure::AfterDelivery(format!("Failed to execute command: {}", e))
    })?;

    let mut output = Vec::new();
    let mut exit_code: Option<u32> = None;
    let wait_result = tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        loop {
            match channel.wait().await {
                Some(russh::ChannelMsg::Data { ref data }) => {
                    output.extend_from_slice(data);
                }
                Some(russh::ChannelMsg::ExtendedData { ref data, .. }) => {
                    output.extend_from_slice(data);
                }
                Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                    exit_code = Some(exit_status);
                }
                Some(russh::ChannelMsg::Eof) => break,
                None => break,
                _ => {}
            }
        }
    })
    .await;

    if wait_result.is_err() {
        return Err(RunFailure::AfterDelivery(
            "Command execution timed out".to_string(),
        ));
    }

    if let Some(code) = exit_code {
        if code != 0 {
            return Err(RunFailure::AfterDelivery(format!(
                "Command exited with status {}",
                code
            )));
        }
    }

    String::from_utf8(output)
        .map_err(|e| RunFailure::AfterDelivery(format!("Invalid UTF-8 in output: {}", e)))
}

/// Execute a single SSH command and return output.
/// Supports password and/or SSH key auth (key_path or private_key PEM string).
///
/// When a connection pool is registered on the app, the authenticated session is
/// reused across calls; otherwise a one-shot connection is made and closed.
pub async fn execute_ssh_command(
    app_handle: AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    key_path: Option<&str>,
    private_key: Option<&str>,
    key_passphrase: Option<&str>,
    command: &str,
    timeout_secs: u64,
) -> Result<String, String> {
    if app_handle
        .try_state::<crate::ssh_pool::SshConnectionPool>()
        .is_some()
    {
        return execute_pooled(
            app_handle,
            host,
            port,
            username,
            password,
            key_path,
            private_key,
            key_passphrase,
            command,
            timeout_secs,
        )
        .await;
    }

    execute_one_shot(
        app_handle,
        host,
        port,
        username,
        password,
        key_path,
        private_key,
        key_passphrase,
        command,
        timeout_secs,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn execute_pooled(
    app_handle: AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    key_path: Option<&str>,
    private_key: Option<&str>,
    key_passphrase: Option<&str>,
    command: &str,
    timeout_secs: u64,
) -> Result<String, String> {
    use crate::ssh_pool::{ConnectParams, SshConnectionPool};

    let params = ConnectParams {
        host,
        port,
        username,
        password: if password.is_empty() {
            None
        } else {
            Some(password)
        },
        key_path,
        private_key,
        key_passphrase,
        timeout_secs,
    };

    let pool = app_handle.state::<SshConnectionPool>();
    let lease = pool.acquire(&app_handle, &params).await?;

    match run_command(&lease.handle, command, timeout_secs).await {
        Ok(output) => Ok(output),
        Err(failure) if failure.is_retryable(lease.reused) => {
            // A cached session can die between the liveness check and use.
            log::debug!("Pooled SSH session for {host}:{port} was unusable; reconnecting");
            pool.invalidate(&params).await;
            let lease = pool.acquire(&app_handle, &params).await?;
            run_command(&lease.handle, command, timeout_secs)
                .await
                .map_err(RunFailure::into_message)
        }
        Err(other) => Err(other.into_message()),
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_one_shot(
    app_handle: AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    key_path: Option<&str>,
    private_key: Option<&str>,
    key_passphrase: Option<&str>,
    command: &str,
    timeout_secs: u64,
) -> Result<String, String> {
    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = ExecClient {
        app_handle,
        host: host.to_string(),
        port,
    };

    let addr = format!("{}:{}", host, port);

    // Connect with timeout
    let mut session = match tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        russh::client::connect(config, addr, sh),
    )
    .await
    {
        Ok(res) => res.map_err(|e| format!("Connection failed: {}", e))?,
        Err(_) => return Err("Connection timed out".to_string()),
    };

    let result = async {
        crate::ssh_auth::authenticate(
            &mut session,
            username,
            if password.is_empty() {
                None
            } else {
                Some(password)
            },
            key_path,
            private_key,
            key_passphrase,
        )
        .await
        .map_err(|e| format!("Authentication error: {}", e))?;

        run_command(&session, command, timeout_secs)
            .await
            .map_err(RunFailure::into_message)
    }
    .await;

    // Explicitly disconnect SSH session regardless of result
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await;

    result
}

/// Separates the metric fields inside the single combined probe command below.
/// Chosen to be something no shell command in the probe can emit on its own.
const METRIC_FIELD_DELIMITER: &str = "__QUASAR_FIELD__";

/// Total budget for connecting and running the combined metrics probe.
const METRICS_TIMEOUT_SECS: u64 = 15;

/// Split combined probe output into its individual metric fields.
///
/// The probe emits the delimiter between every field, so a well-formed response
/// yields one entry per probe. A field belonging to a command that failed on the
/// remote host comes back empty and simply parses to `None`.
fn split_metric_fields(output: &str) -> Vec<&str> {
    output
        .split(METRIC_FIELD_DELIMITER)
        .map(|field| field.trim())
        .collect()
}

/// Get system metrics via SSH (password or key auth).
///
/// The five probes are sent as one compound command over a single channel rather
/// than five separate `exec` channels: opening a channel costs a round trip, and
/// this runs for every monitored host on every poll.
pub async fn get_system_metrics(
    app_handle: AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    key_path: Option<&str>,
    private_key: Option<&str>,
    key_passphrase: Option<&str>,
) -> Result<crate::health::HealthMetrics, String> {
    let commands = [
        // CPU usage (1 second average)
        "top -bn1 | grep 'Cpu(s)' | sed 's/.*, *\\([0-9.]*\\)%* id.*/\\1/' | awk '{print 100 - $1}'",
        // Memory info
        "free -m | awk 'NR==2{print $3,$2}'",
        // Disk usage for root
        "df -BG / | awk 'NR==2{print $3,$2}' | sed 's/G//g'",
        // Uptime in seconds
        "cat /proc/uptime | awk '{print int($1)}'",
        // Load average
        "cat /proc/loadavg | awk '{print $1,$2,$3}'",
    ];

    // Join with `;` so a probe that is unsupported on the remote host (for example
    // /proc on a BSD box) leaves its own field empty instead of aborting the rest.
    // The trailing `true` keeps the compound exit status zero for the same reason.
    let script = format!(
        "{}; true",
        commands
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(&format!("; echo {}; ", METRIC_FIELD_DELIMITER))
    );

    let raw_output = execute_ssh_command(
        app_handle,
        host,
        port,
        username,
        password.unwrap_or(""),
        key_path,
        private_key,
        key_passphrase,
        &script,
        METRICS_TIMEOUT_SECS,
    )
    .await?;

    Ok(parse_metric_fields(&split_metric_fields(&raw_output)))
}

/// Turn the split probe fields into `HealthMetrics`.
///
/// Every field is optional: a probe the remote host could not satisfy yields
/// `None` for that metric rather than failing the whole reading.
fn parse_metric_fields(outputs: &[&str]) -> crate::health::HealthMetrics {
    /// Parse a "used total" pair, as emitted by the memory and disk probes.
    fn parse_pair(field: Option<&&str>) -> (Option<u64>, Option<u64>) {
        field
            .and_then(|s| {
                let parts: Vec<&str> = s.split_whitespace().collect();
                if parts.len() >= 2 {
                    Some((parts[0].parse::<u64>().ok()?, parts[1].parse::<u64>().ok()?))
                } else {
                    None
                }
            })
            .map_or((None, None), |(used, total)| (Some(used), Some(total)))
    }

    let cpu_percent = outputs.first().and_then(|s| s.trim().parse::<f32>().ok());
    let (memory_used_mb, memory_total_mb) = parse_pair(outputs.get(1));
    let (disk_used_gb, disk_total_gb) = parse_pair(outputs.get(2));
    let uptime_seconds = outputs.get(3).and_then(|s| s.trim().parse::<u64>().ok());

    let load_average = outputs.get(4).and_then(|s| {
        let parts: Vec<f32> = s
            .split_whitespace()
            .filter_map(|p| p.parse::<f32>().ok())
            .collect();
        if parts.len() >= 3 {
            Some(parts)
        } else {
            None
        }
    });

    crate::health::HealthMetrics {
        cpu_percent,
        memory_used_mb,
        memory_total_mb,
        disk_used_gb,
        disk_total_gb,
        uptime_seconds,
        load_average,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_command_structure() {
        // This test just validates the function is available and type-checks.
        let _ = execute_ssh_command;
    }

    #[test]
    fn test_only_undelivered_commands_on_a_reused_session_are_retried() {
        let channel_open = RunFailure::ChannelOpen("dead session".to_string());
        let after_delivery = RunFailure::AfterDelivery("exited with status 1".to_string());

        // The one safe case: a cached session died before the command was sent.
        assert!(channel_open.is_retryable(true));

        // A freshly built session failing is a genuine error, not a stale handle.
        assert!(!channel_open.is_retryable(false));

        // Never retry once the command may have reached the server — a scheduled
        // task would otherwise run its side effects twice.
        assert!(!after_delivery.is_retryable(true));
        assert!(!after_delivery.is_retryable(false));
    }

    /// Builds a probe response the way the remote shell would emit it.
    fn joined(fields: &[&str]) -> String {
        fields.join(&format!("\n{}\n", METRIC_FIELD_DELIMITER))
    }

    #[test]
    fn test_split_metric_fields_returns_one_entry_per_probe() {
        let output = joined(&["12.5", "2048 8192", "40 500", "86400", "0.5 0.6 0.7"]);
        let fields = split_metric_fields(&output);

        assert_eq!(fields.len(), 5);
        assert_eq!(fields[0], "12.5");
        assert_eq!(fields[1], "2048 8192");
        assert_eq!(fields[4], "0.5 0.6 0.7");
    }

    #[test]
    fn test_split_metric_fields_keeps_positions_when_a_probe_produces_nothing() {
        // A probe that is unsupported on the remote host emits no output, but the
        // delimiters still land, so later fields must not shift into its slot.
        let output = joined(&["12.5", "", "40 500", "", "0.5 0.6 0.7"]);
        let fields = split_metric_fields(&output);

        assert_eq!(fields.len(), 5);
        assert_eq!(fields[1], "", "missing memory probe must stay empty");
        assert_eq!(fields[2], "40 500", "disk must not shift into memory's slot");
        assert_eq!(fields[3], "");
        assert_eq!(fields[4], "0.5 0.6 0.7");
    }

    #[test]
    fn test_split_metric_fields_trims_surrounding_whitespace() {
        let output = format!(
            "  12.5 \n{}\n  2048 8192  \n",
            METRIC_FIELD_DELIMITER
        );
        let fields = split_metric_fields(&output);

        assert_eq!(fields[0], "12.5");
        assert_eq!(fields[1], "2048 8192");
    }

    /// Verbatim output captured from a real Linux 6.17 host (Ubuntu, 16 GB RAM,
    /// 119 GB root) running the combined probe over SSH. Guards the end-to-end
    /// contract between the probe command and the parser.
    const REAL_HOST_OUTPUT: &str = "3.3\n\
         __QUASAR_FIELD__\n\
         3192 15909\n\
         __QUASAR_FIELD__\n\
         45 119\n\
         __QUASAR_FIELD__\n\
         944918\n\
         __QUASAR_FIELD__\n\
         0.87 0.84 0.76\n";

    #[test]
    fn test_parses_real_host_output() {
        let metrics = parse_metric_fields(&split_metric_fields(REAL_HOST_OUTPUT));

        assert_eq!(metrics.cpu_percent, Some(3.3));
        assert_eq!(metrics.memory_used_mb, Some(3192));
        assert_eq!(metrics.memory_total_mb, Some(15909));
        assert_eq!(metrics.disk_used_gb, Some(45));
        assert_eq!(metrics.disk_total_gb, Some(119));
        assert_eq!(metrics.uptime_seconds, Some(944918));
        assert_eq!(metrics.load_average, Some(vec![0.87, 0.84, 0.76]));
    }

    #[test]
    fn test_parse_metric_fields_tolerates_unsupported_probes() {
        // A host without /proc still reports cpu, memory and disk.
        let output = "3.3\n__QUASAR_FIELD__\n3192 15909\n__QUASAR_FIELD__\n\
                      45 119\n__QUASAR_FIELD__\n\n__QUASAR_FIELD__\n";
        let metrics = parse_metric_fields(&split_metric_fields(output));

        assert_eq!(metrics.cpu_percent, Some(3.3));
        assert_eq!(metrics.memory_used_mb, Some(3192));
        assert_eq!(metrics.disk_used_gb, Some(45));
        assert_eq!(metrics.uptime_seconds, None);
        assert_eq!(metrics.load_average, None);
    }

    #[test]
    fn test_parse_metric_fields_on_no_output_yields_all_none() {
        let metrics = parse_metric_fields(&split_metric_fields(""));

        assert!(metrics.cpu_percent.is_none());
        assert!(metrics.memory_used_mb.is_none());
        assert!(metrics.disk_total_gb.is_none());
        assert!(metrics.load_average.is_none());
    }

    #[test]
    fn test_split_metric_fields_on_empty_output() {
        // A host that returned nothing at all yields a single empty field rather
        // than panicking; every metric then parses to None.
        let fields = split_metric_fields("");
        assert_eq!(fields, vec![""]);
    }
}

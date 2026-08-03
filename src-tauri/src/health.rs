use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    pub cpu_percent: Option<f32>,
    pub memory_used_mb: Option<u64>,
    pub memory_total_mb: Option<u64>,
    pub disk_used_gb: Option<u64>,
    pub disk_total_gb: Option<u64>,
    pub uptime_seconds: Option<u64>,
    pub load_average: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthCheckResult {
    pub host: String,
    pub reachable: bool,
    pub latency_ms: Option<u32>,
    pub metrics: Option<HealthMetrics>,
    pub error: Option<String>,
}

/// Check if a host is reachable via ping
pub async fn check_ping(host: &str) -> Result<u32, String> {
    use surge_ping::{Client, Config, PingIdentifier, PingSequence};

    let addr: std::net::IpAddr = host.parse().map_err(|_| "Invalid IP address".to_string())?;

    let client = Client::new(&Config::default()).map_err(|e| e.to_string())?;
    let payload = vec![0; 56];
    let ident = PingIdentifier(rand::random::<u16>());
    let seq = PingSequence(0);

    let mut pinger = client.pinger(addr, ident).await;

    match timeout(Duration::from_secs(2), pinger.ping(seq, &payload)).await {
        Ok(Ok((_, dur))) => Ok(dur.as_millis() as u32),
        Ok(Err(e)) => Err(format!("Ping error: {}", e)),
        Err(_) => Err("Ping timeout".to_string()),
    }
}

use tauri::AppHandle;

/// Check health via SSH connection and command execution.
/// When `skip_ping` is true, the caller has already verified reachability; only SSH metrics are run (avoids double ping).
/// Auth: password and/or key (key_path or private_key PEM with optional key_passphrase).
pub async fn check_ssh_health(
    app_handle: AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    skip_ping: bool,
    key_path: Option<&str>,
    private_key: Option<&str>,
    key_passphrase: Option<&str>,
) -> HealthCheckResult {
    let latency_ms = if skip_ping {
        None
    } else {
        let ping_result = check_ping(host).await;
        match ping_result {
            Ok(ms) => Some(ms),
            Err(_) => {
                return HealthCheckResult {
                    host: host.to_string(),
                    reachable: false,
                    latency_ms: None,
                    metrics: None,
                    error: Some("Host unreachable".to_string()),
                };
            }
        }
    };

    let has_auth = password.is_some() || key_path.is_some() || private_key.is_some();
    let metrics = if has_auth {
        crate::ssh_exec::get_system_metrics(
            app_handle,
            host,
            port,
            username,
            password,
            key_path,
            private_key,
            key_passphrase,
        )
        .await
        .ok()
    } else {
        None
    };

    HealthCheckResult {
        host: host.to_string(),
        reachable: true,
        latency_ms,
        metrics,
        error: None,
    }
}

/// Quick pre-flight check without credentials
pub async fn preflight_check(host: &str) -> HealthCheckResult {
    match check_ping(host).await {
        Ok(latency_ms) => HealthCheckResult {
            host: host.to_string(),
            reachable: true,
            latency_ms: Some(latency_ms),
            metrics: None,
            error: None,
        },
        Err(e) => HealthCheckResult {
            host: host.to_string(),
            reachable: false,
            latency_ms: None,
            metrics: None,
            error: Some(e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_metrics_serialization() {
        let metrics = HealthMetrics {
            cpu_percent: Some(45.5),
            memory_used_mb: Some(4096),
            memory_total_mb: Some(8192),
            disk_used_gb: Some(100),
            disk_total_gb: Some(500),
            uptime_seconds: Some(86400),
            load_average: Some(vec![0.5, 0.6, 0.7]),
        };

        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("cpu_percent"));
        assert!(json.contains("45.5"));
    }

    #[test]
    fn test_health_check_result_unreachable() {
        let result = HealthCheckResult {
            host: "192.168.255.255".to_string(),
            reachable: false,
            latency_ms: None,
            metrics: None,
            error: Some("Host unreachable".to_string()),
        };

        assert!(!result.reachable);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_health_check_result_reachable() {
        let result = HealthCheckResult {
            host: "192.168.1.1".to_string(),
            reachable: true,
            latency_ms: Some(10),
            metrics: None,
            error: None,
        };

        assert!(result.reachable);
        assert_eq!(result.latency_ms, Some(10));
        assert!(result.error.is_none());
    }
}

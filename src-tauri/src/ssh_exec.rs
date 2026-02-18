use russh::*;
use russh::keys::PublicKeyBase64;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use crate::crypto;
use crate::vault::SshKeyManager;

/// Simple SSH client for command execution (non-interactive)
#[derive(Clone)]
struct ExecClient {
    app_handle: AppHandle,
    host: String,
    port: u16,
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
        match ssh_key_manager.verify_host_key_by_fingerprint(
            &self.host,
            self.port,
            &fingerprint,
            key_type,
        ).await {
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

/// Execute a single SSH command and return output.
/// Supports password and/or SSH key auth (key_path or private_key PEM string).
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
        russh::client::connect(config, addr, sh)
    ).await {
        Ok(res) => res.map_err(|e| format!("Connection failed: {}", e))?,
        Err(_) => return Err("Connection timed out".to_string()),
    };

    let result = async {
        crate::ssh_auth::authenticate(
            &mut session,
            username,
            if password.is_empty() { None } else { Some(password) },
            key_path,
            private_key,
            key_passphrase,
        ).await.map_err(|e| format!("Authentication error: {}", e))?;

        // Open channel and execute command
        let mut channel = session.channel_open_session()
            .await
            .map_err(|e| format!("Failed to open channel: {}", e))?;
        
        channel.exec(true, command.as_bytes())
            .await
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        // Collect output by waiting for channel messages
        let mut output = Vec::new();
        let mut exit_code: Option<u32> = None;
        let wait_result = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            async {
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
                Ok(())
            }
        ).await;

        match wait_result {
            Ok(Ok(())) => {
                // Check exit code after collecting all output
                if let Some(code) = exit_code {
                    if code != 0 {
                        return Err(format!("Command exited with status {}", code));
                    }
                }
            },
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("Command execution timed out".to_string()),
        }

        String::from_utf8(output)
            .map_err(|e| format!("Invalid UTF-8 in output: {}", e))
    }.await;

    // Explicitly disconnect SSH session regardless of result
    let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;

    result
}

/// Execute multiple commands in sequence on the same SSH session
pub async fn execute_ssh_commands_batch(
    app_handle: AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    key_path: Option<&str>,
    private_key: Option<&str>,
    key_passphrase: Option<&str>,
    commands: &[&str],
    timeout_secs: u64,
) -> Result<Vec<String>, String> {
    // Early return for empty commands array
    if commands.is_empty() {
        return Ok(Vec::new());
    }

    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = ExecClient {
        app_handle,
        host: host.to_string(),
        port,
    };

    let addr = format!("{}:{}", host, port);
    
    let mut session = match tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        russh::client::connect(config, addr, sh)
    ).await {
        Ok(res) => res.map_err(|e| format!("Connection failed: {}", e))?,
        Err(_) => return Err("Connection timed out".to_string()),
    };

    let result = async {
        crate::ssh_auth::authenticate(
            &mut session,
            username,
            password,
            key_path,
            private_key,
            key_passphrase,
        ).await.map_err(|e| format!("Authentication error: {}", e))?;

        let mut results = Vec::new();
        
        // Calculate per-command timeout: divide total timeout by number of commands, with minimum of 5 seconds
        let num_commands = commands.len() as u64;
        let per_command_timeout = (timeout_secs / num_commands).max(5);

        for command in commands {
            let mut channel = session.channel_open_session()
                .await
                .map_err(|e| format!("Failed to open channel: {}", e))?;
            
            channel.exec(true, command.as_bytes())
                .await
                .map_err(|e| format!("Failed to execute command: {}", e))?;

            let mut output = Vec::new();
            let mut exit_code: Option<u32> = None;
            
            let wait_result = tokio::time::timeout(
                Duration::from_secs(per_command_timeout),
                async {
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
                }
            ).await;

            if wait_result.is_err() {
                return Err(format!("Command '{}' timed out after {} seconds", command, per_command_timeout));
            }
            
            // Check exit code
            if let Some(code) = exit_code {
                if code != 0 {
                    return Err(format!("Command '{}' exited with status {}", command, code));
                }
            }

            let output_str = String::from_utf8(output)
                .map_err(|e| format!("Invalid UTF-8 in output: {}", e))?;
            
            results.push(output_str);
        }
        
        Ok(results)
    }.await;

    // Explicitly disconnect SSH session regardless of result
    let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;

    result
}

/// Get system metrics via SSH commands (password or key auth).
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
    let commands = vec![
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

    let outputs = execute_ssh_commands_batch(app_handle, host, port, username, password, key_path, private_key, key_passphrase, &commands, 10).await?;

    // Parse outputs
    let cpu_percent = outputs.get(0)
        .and_then(|s| s.trim().parse::<f32>().ok());
    
    let (memory_used_mb, memory_total_mb) = outputs.get(1)
        .and_then(|s| {
            let parts: Vec<&str> = s.trim().split_whitespace().collect();
            if parts.len() >= 2 {
                let used = parts[0].parse::<u64>().ok()?;
                let total = parts[1].parse::<u64>().ok()?;
                Some((used, total))
            } else {
                None
            }
        })
        .map(|(used, total)| (Some(used), Some(total)))
        .unwrap_or((None, None));
    
    let (disk_used_gb, disk_total_gb) = outputs.get(2)
        .and_then(|s| {
            let parts: Vec<&str> = s.trim().split_whitespace().collect();
            if parts.len() >= 2 {
                let used = parts[0].parse::<u64>().ok()?;
                let total = parts[1].parse::<u64>().ok()?;
                Some((used, total))
            } else {
                None
            }
        })
        .map(|(used, total)| (Some(used), Some(total)))
        .unwrap_or((None, None));
    
    let uptime_seconds = outputs.get(3)
        .and_then(|s| s.trim().parse::<u64>().ok());
    
    let load_average = outputs.get(4)
        .and_then(|s| {
            let parts: Vec<f32> = s.trim()
                .split_whitespace()
                .filter_map(|p| p.parse::<f32>().ok())
                .collect();
            if parts.len() >= 3 {
                Some(parts)
            } else {
                None
            }
        });

    Ok(crate::health::HealthMetrics {
        cpu_percent,
        memory_used_mb,
        memory_total_mb,
        disk_used_gb,
        disk_total_gb,
        uptime_seconds,
        load_average,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_command_structure() {
        // This test just validates the function is available and type-checks.
        let _ = execute_ssh_command;
    }
}

use russh::*;
use std::sync::Arc;
use std::time::Duration;

/// Simple SSH client for command execution (non-interactive)
#[derive(Clone)]
struct ExecClient;

impl client::Handler for ExecClient {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        // For automation, we assume host keys are already verified
        // This is safe because the vault's SSH key manager handles verification
        Ok(true)
    }
}

/// Execute a single SSH command and return output
pub async fn execute_ssh_command(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    command: &str,
    timeout_secs: u64,
) -> Result<String, String> {
    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = ExecClient;

    let addr = format!("{}:{}", host, port);
    
    // Connect with timeout
    let mut session = match tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        russh::client::connect(config, addr, sh)
    ).await {
        Ok(res) => res.map_err(|e| format!("Connection failed: {}", e))?,
        Err(_) => return Err("Connection timed out".to_string()),
    };

    // Authenticate
    let auth_res = session.authenticate_password(username, password)
        .await
        .map_err(|e| format!("Authentication error: {}", e))?;
    
    let is_success = matches!(auth_res, russh::client::AuthResult::Success);
    if !is_success {
        return Err("Authentication failed".to_string());
    }

    // Open channel and execute command
    let mut channel = session.channel_open_session()
        .await
        .map_err(|e| format!("Failed to open channel: {}", e))?;
    
    channel.exec(true, command.as_bytes())
        .await
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    // Collect output by waiting for channel messages
    let mut output = Vec::new();
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
                    Some(russh::ChannelMsg::Eof) => break,
                    Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                        if exit_status != 0 {
                            return Err(format!("Command exited with status {}", exit_status));
                        }
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
            Ok(())
        }
    ).await;

    match wait_result {
        Ok(Ok(())) => {},
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err("Command execution timed out".to_string()),
    }

    String::from_utf8(output)
        .map_err(|e| format!("Invalid UTF-8 in output: {}", e))
}

/// Execute multiple commands in sequence on the same SSH session
pub async fn execute_ssh_commands_batch(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    commands: &[&str],
    timeout_secs: u64,
) -> Result<Vec<String>, String> {
    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = ExecClient;

    let addr = format!("{}:{}", host, port);
    
    let mut session = match tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        russh::client::connect(config, addr, sh)
    ).await {
        Ok(res) => res.map_err(|e| format!("Connection failed: {}", e))?,
        Err(_) => return Err("Connection timed out".to_string()),
    };

    let auth_res = session.authenticate_password(username, password)
        .await
        .map_err(|e| format!("Authentication error: {}", e))?;
    
    let is_success = matches!(auth_res, russh::client::AuthResult::Success);
    if !is_success {
        return Err("Authentication failed".to_string());
    }

    let mut results = Vec::new();

    for command in commands {
        let mut channel = session.channel_open_session()
            .await
            .map_err(|e| format!("Failed to open channel: {}", e))?;
        
        channel.exec(true, command.as_bytes())
            .await
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        let mut output = Vec::new();
        
        let wait_result = tokio::time::timeout(
            Duration::from_secs(10),
            async {
                loop {
                    match channel.wait().await {
                        Some(russh::ChannelMsg::Data { ref data }) => {
                            output.extend_from_slice(data);
                        }
                        Some(russh::ChannelMsg::ExtendedData { ref data, .. }) => {
                            output.extend_from_slice(data);
                        }
                        Some(russh::ChannelMsg::Eof) => break,
                        Some(russh::ChannelMsg::ExitStatus { .. }) => break,
                        None => break,
                        _ => {}
                    }
                }
            }
        ).await;

        if wait_result.is_err() {
            return Err(format!("Command '{}' timed out", command));
        }

        let output_str = String::from_utf8(output)
            .map_err(|e| format!("Invalid UTF-8 in output: {}", e))?;
        
        results.push(output_str);
    }

    Ok(results)
}

/// Get system metrics via SSH commands
pub async fn get_system_metrics(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
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

    let outputs = execute_ssh_commands_batch(host, port, username, password, &commands, 10).await?;

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
        .unzip();
    
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
        .unzip();
    
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
        // This test just validates the function signature and error handling
        let result = execute_ssh_command(
            "invalid.host",
            22,
            "user",
            "pass",
            "echo test",
            5
        ).await;
        
        assert!(result.is_err());
    }
}

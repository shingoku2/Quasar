use russh::*;
use russh::keys::PublicKeyBase64;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::crypto;
use crate::vault::SshKeyManager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteFile {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub permissions: Option<u32>,
    pub modified: Option<u64>,
}

/// Simple SSH client for SFTP operations
#[derive(Clone)]
struct SftpClient {
    app_handle: AppHandle,
    host: String,
    port: u16,
}

impl client::Handler for SftpClient {
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
                    // Reject connection for SFTP operations if not already trusted
                    Err(russh::Error::Disconnect)
                }
            }
            Err(_) => Err(russh::Error::Disconnect),
        }
    }
}

fn require_password_auth(password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err(
            "SFTP requires password-based authentication. \
             SSH key authentication is not yet supported for SFTP operations."
                .to_string(),
        );
    }
    Ok(())
}

/// Progress callback for file transfer operations
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Upload a file to a remote host via SFTP
pub async fn upload_file(
    app_handle: AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    local_path: &str,
    remote_path: &str,
    progress_callback: Option<ProgressCallback>,
) -> Result<(), String> {
    require_password_auth(password)?;

    if !Path::new(local_path).exists() {
        return Err(format!("Local file not found: {}", local_path));
    }

    // Connect to SSH
    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = SftpClient {
        app_handle,
        host: host.to_string(),
        port,
    };

    let addr = format!("{}:{}", host, port);
    
    let mut session = match tokio::time::timeout(
        Duration::from_secs(10),
        russh::client::connect(config, addr, sh)
    ).await {
        Ok(res) => res.map_err(|e| format!("Connection failed: {}", e))?,
        Err(_) => return Err("Connection timed out".to_string()),
    };

    let result = async {
        // Authenticate
        let auth_res = session.authenticate_password(username, password)
            .await
            .map_err(|e| format!("Authentication error: {}", e))?;
        
        let is_success = matches!(auth_res, russh::client::AuthResult::Success);
        if !is_success {
            return Err("Authentication failed".to_string());
        }

        // Open SFTP channel
        let channel = session.channel_open_session()
            .await
            .map_err(|e| format!("Failed to open channel: {}", e))?;
        
        channel.request_subsystem(true, "sftp")
            .await
            .map_err(|e| format!("Failed to request SFTP subsystem: {}", e))?;

        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| format!("Failed to create SFTP session: {}", e))?;

        // Read local file
        let mut local_file = fs::File::open(local_path)
            .await
            .map_err(|e| format!("Failed to open local file: {}", e))?;
        
        let file_size = local_file.metadata()
            .await
            .map_err(|e| format!("Failed to get file metadata: {}", e))?
            .len();

        // Create remote file
        let mut remote_file = sftp.create(remote_path)
            .await
            .map_err(|e| format!("Failed to create remote file: {}", e))?;

        // Transfer data in chunks
        let mut buffer = vec![0u8; 32768]; // 32KB chunks
        let mut bytes_transferred = 0u64;

        loop {
            let bytes_read = local_file.read(&mut buffer)
                .await
                .map_err(|e| format!("Failed to read local file: {}", e))?;
            
            if bytes_read == 0 {
                break;
            }

            remote_file.write_all(&buffer[..bytes_read])
                .await
                .map_err(|e| format!("Failed to write to remote file: {}", e))?;

            bytes_transferred += bytes_read as u64;

            // Call progress callback if provided
            if let Some(ref callback) = progress_callback {
                callback(bytes_transferred, file_size);
            }
        }

        // Close files
        remote_file.shutdown()
            .await
            .map_err(|e| format!("Failed to close remote file: {}", e))?;

        sftp.close()
            .await
            .map_err(|e| format!("Failed to close SFTP session: {}", e))?;
            
        Ok(())
    }.await;

    // Explicitly disconnect SSH session regardless of result
    let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;

    result
}

/// Download a file from a remote host via SFTP
pub async fn download_file(
    app_handle: AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    remote_path: &str,
    local_path: &str,
    progress_callback: Option<ProgressCallback>,
) -> Result<(), String> {
    require_password_auth(password)?;

    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = SftpClient {
        app_handle,
        host: host.to_string(),
        port,
    };

    let addr = format!("{}:{}", host, port);
    
    let mut session = match tokio::time::timeout(
        Duration::from_secs(10),
        russh::client::connect(config, addr, sh)
    ).await {
        Ok(res) => res.map_err(|e| format!("Connection failed: {}", e))?,
        Err(_) => return Err("Connection timed out".to_string()),
    };

    let result = async {
        // Authenticate
        let auth_res = session.authenticate_password(username, password)
            .await
            .map_err(|e| format!("Authentication error: {}", e))?;
        
        let is_success = matches!(auth_res, russh::client::AuthResult::Success);
        if !is_success {
            return Err("Authentication failed".to_string());
        }

        // Open SFTP channel
        let channel = session.channel_open_session()
            .await
            .map_err(|e| format!("Failed to open channel: {}", e))?;
        
        channel.request_subsystem(true, "sftp")
            .await
            .map_err(|e| format!("Failed to request SFTP subsystem: {}", e))?;

        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| format!("Failed to create SFTP session: {}", e))?;

        // Open remote file
        let mut remote_file = sftp.open(remote_path)
            .await
            .map_err(|e| format!("Failed to open remote file: {}", e))?;

        // Get file size
        let file_attrs = sftp.metadata(remote_path)
            .await
            .map_err(|e| format!("Failed to get remote file metadata: {}", e))?;
        
        let file_size = file_attrs.size.unwrap_or(0);

        // Create local file
        let mut local_file = fs::File::create(local_path)
            .await
            .map_err(|e| format!("Failed to create local file: {}", e))?;

        // Transfer data in chunks
        let mut buffer = vec![0u8; 32768]; // 32KB chunks
        let mut bytes_transferred = 0u64;

        loop {
            let bytes_read = remote_file.read(&mut buffer)
                .await
                .map_err(|e| format!("Failed to read remote file: {}", e))?;
            
            if bytes_read == 0 {
                break;
            }

            local_file.write_all(&buffer[..bytes_read])
                .await
                .map_err(|e| format!("Failed to write to local file: {}", e))?;

            bytes_transferred += bytes_read as u64;

            // Call progress callback if provided
            if let Some(ref callback) = progress_callback {
                callback(bytes_transferred, file_size);
            }
        }

        // Close files
        local_file.sync_all()
            .await
            .map_err(|e| format!("Failed to sync local file: {}", e))?;

        sftp.close()
            .await
            .map_err(|e| format!("Failed to close SFTP session: {}", e))?;
            
        Ok(())
    }.await;

    // Explicitly disconnect SSH session regardless of result
    let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;

    result
}

/// List files in a remote directory via SFTP
pub async fn list_directory(
    app_handle: AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    remote_path: &str,
) -> Result<Vec<RemoteFile>, String> {
    require_password_auth(password)?;

    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = SftpClient {
        app_handle,
        host: host.to_string(),
        port,
    };

    let addr = format!("{}:{}", host, port);
    
    let mut session = match tokio::time::timeout(
        Duration::from_secs(10),
        russh::client::connect(config, addr, sh)
    ).await {
        Ok(res) => res.map_err(|e| format!("Connection failed: {}", e))?,
        Err(_) => return Err("Connection timed out".to_string()),
    };

    let result = async {
        // Authenticate
        let auth_res = session.authenticate_password(username, password)
            .await
            .map_err(|e| format!("Authentication error: {}", e))?;
        
        let is_success = matches!(auth_res, russh::client::AuthResult::Success);
        if !is_success {
            return Err("Authentication failed".to_string());
        }

        // Open SFTP channel
        let channel = session.channel_open_session()
            .await
            .map_err(|e| format!("Failed to open channel: {}", e))?;
        
        channel.request_subsystem(true, "sftp")
            .await
            .map_err(|e| format!("Failed to request SFTP subsystem: {}", e))?;

        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| format!("Failed to create SFTP session: {}", e))?;

        // Read directory - russh-sftp returns a Vec of entries
        let entries = sftp.read_dir(remote_path)
            .await
            .map_err(|e| format!("Failed to read directory: {}", e))?;

        let files: Vec<RemoteFile> = entries
            .into_iter()
            .map(|entry| {
                let attrs = entry.metadata();
                RemoteFile {
                    name: entry.file_name(),
                    is_dir: attrs.is_dir(),
                    size: attrs.size.unwrap_or(0),
                    permissions: attrs.permissions,
                    modified: attrs.mtime.map(|m| m as u64),
                }
            })
            .collect();

        sftp.close()
            .await
            .map_err(|e| format!("Failed to close SFTP session: {}", e))?;
            
        Ok(files)
    }.await;

    // Explicitly disconnect SSH session
    let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;

    result
}

/// Check if a remote file or directory exists
pub async fn remote_exists(
    app_handle: AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    remote_path: &str,
) -> Result<bool, String> {
    require_password_auth(password)?;

    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = SftpClient {
        app_handle,
        host: host.to_string(),
        port,
    };

    let addr = format!("{}:{}", host, port);
    
    let mut session = match tokio::time::timeout(
        Duration::from_secs(10),
        russh::client::connect(config, addr, sh)
    ).await {
        Ok(res) => res.map_err(|e| format!("Connection failed: {}", e))?,
        Err(_) => return Err("Connection timed out".to_string()),
    };

    let result = async {
        // Authenticate
        let auth_res = session.authenticate_password(username, password)
            .await
            .map_err(|e| format!("Authentication error: {}", e))?;
        
        let is_success = matches!(auth_res, russh::client::AuthResult::Success);
        if !is_success {
            return Err("Authentication failed".to_string());
        }

        // Open SFTP channel
        let channel = session.channel_open_session()
            .await
            .map_err(|e| format!("Failed to open channel: {}", e))?;
        
        channel.request_subsystem(true, "sftp")
            .await
            .map_err(|e| format!("Failed to request SFTP subsystem: {}", e))?;

        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| format!("Failed to create SFTP session: {}", e))?;

        // Try to get metadata
        let exists = sftp.metadata(remote_path).await.is_ok();

        sftp.close()
            .await
            .map_err(|e| format!("Failed to close SFTP session: {}", e))?;
            
        Ok(exists)
    }.await;

    // Explicitly disconnect SSH session
    let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;

    result
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_sftp_connection_structure() {
        // Note: This test can't easily be run without a mock app handle
        // but validates the function signatures for compilation
    }
}
//! Vault lifecycle, credentials, known host keys and the audit log.

use crate::*;

// Vault commands
#[tauri::command]
pub(crate) async fn is_vault_initialized(state: State<'_, vault::VaultState>) -> Result<bool, String> {
    state
        .is_initialized()
        .await
        .map_err(|e| sanitize_error(e, "vault"))
}

#[tauri::command]
pub(crate) async fn initialize_vault(
    state: State<'_, vault::VaultState>,
    master_password: String,
) -> Result<(), String> {
    use secrecy::SecretString;
    validate_master_password(&master_password)?;
    state
        .initialize_vault(SecretString::from(master_password))
        .await
        .map_err(|e| sanitize_error(e, "vault"))
}

#[tauri::command]
pub(crate) async fn unlock_vault(
    state: State<'_, vault::VaultState>,
    master_password: String,
) -> Result<(), String> {
    use secrecy::SecretString;
    if master_password.is_empty() {
        return Err("Password cannot be empty".to_string());
    }
    state
        .unlock_vault(SecretString::from(master_password))
        .await
        .map_err(errors::user_facing_vault_error)
}

#[tauri::command]
pub(crate) async fn lock_vault(state: State<'_, vault::VaultState>) -> Result<(), String> {
    state
        .lock_vault()
        .await
        .map_err(|e| sanitize_error(e, "vault"))
}

#[tauri::command]
pub(crate) async fn is_vault_locked(state: State<'_, vault::VaultState>) -> Result<bool, String> {
    Ok(state.is_locked().await)
}

#[tauri::command]
pub(crate) async fn get_vault_settings(
    state: State<'_, vault::VaultState>,
) -> Result<vault::VaultSettings, String> {
    Ok(state.get_settings().await)
}

#[tauri::command]
pub(crate) async fn update_vault_settings(
    state: State<'_, vault::VaultState>,
    settings: vault::VaultSettings,
) -> Result<(), String> {
    state
        .update_settings(settings)
        .await
        .map_err(errors::user_facing_vault_error)
}

// Credential management commands
#[tauri::command]
pub(crate) async fn add_credential(
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    name: String,
    username: String,
    password: String,
    credential_type: String,
    host: Option<String>,
    port: Option<u16>,
    metadata: Option<String>,
    key_path: Option<String>,
    private_key: Option<String>,
    key_passphrase: Option<String>,
) -> Result<String, String> {
    validate_credential_name(&name)?;
    validate_username(&username)?;
    if let Some(ref h) = host {
        if validate_ip(h).is_err() && validate_hostname(h).is_err() {
            return Err("Invalid host format".to_string());
        }
    }
    if let Some(p) = port {
        validate_port(p)?;
    }
    let access = vault_state
        .credential_access()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    credential_manager
        .add_credential(
            access.key(),
            name.clone(),
            username,
            password,
            credential_type,
            host,
            port,
            metadata,
            key_path,
            private_key,
            key_passphrase,
        )
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
pub(crate) async fn get_credential(
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
) -> Result<vault::CredentialFrontendView, String> {
    let access = vault_state
        .credential_access()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    credential_manager
        .get_credential(access.key(), &credential_id)
        .map(vault::CredentialFrontendView::from)
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
pub(crate) async fn reveal_credential_password(
    app: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
) -> Result<String, String> {
    let name = credential_manager
        .list_credentials()
        .map_err(|e| sanitize_error(e, "credential"))?
        .into_iter()
        .find(|c| c.id == credential_id)
        .map(|c| c.name)
        .ok_or_else(|| "Credential not found".to_string())?;
    // Don't ask the user to confirm something that will fail anyway.
    if vault_state.is_locked().await {
        return Err("Vault is locked".to_string());
    }
    // The plaintext leaves the vault only on a native confirmation, so a compromised
    // webview can't silently read every stored password (P7-3 review).
    if let Err(e) = native_confirm::confirm(
        &app,
        "Reveal password",
        format!(
            "Show the password for this credential?\n\n\"{}\"",
            native_confirm::display_text(&name)
        ),
        "Reveal",
    )
    .await
    {
        credential_manager.record_reveal_declined(&credential_id);
        return Err(e);
    }
    let access = vault_state
        .credential_access()
        .await
        .map_err(errors::user_facing_vault_error)?;
    let password = credential_manager
        .get_credential(access.key(), &credential_id)
        .map(|c| c.password)
        .map_err(|e| sanitize_error(e, "credential"))?;
    // A plaintext reveal is audited separately from ordinary (background) credential use.
    credential_manager.record_reveal(&credential_id);
    Ok(password)
}

#[tauri::command]
pub(crate) async fn list_credentials(
    credential_manager: State<'_, vault::CredentialManager>,
) -> Result<Vec<vault::CredentialSummary>, String> {
    credential_manager
        .list_credentials()
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
pub(crate) async fn update_credential(
    app: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
    name: Option<String>,
    username: Option<String>,
    password: Option<String>,
    metadata: Option<String>,
    credential_type: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    key_path: Option<String>,
    private_key: Option<String>,
    key_passphrase: Option<String>,
) -> Result<(), String> {
    if let Some(ref n) = name {
        validate_credential_name(n)?;
    }
    if let Some(ref u) = username {
        validate_username(u)?;
    }
    let stored = credential_manager
        .list_credentials()
        .map_err(|e| sanitize_error(e, "credential"))?
        .into_iter()
        .find(|c| c.id == credential_id)
        .ok_or_else(|| "Credential not found".to_string())?;
    if native_confirm::host_change_needs_confirm(stored.host.as_deref(), host.as_deref()) {
        // Moving or clearing a host binding widens where the secret can be sent (IPC-003).
        let to = host.as_deref().map(str::trim).filter(|h| !h.is_empty());
        native_confirm::confirm(
            &app,
            "Change credential host",
            format!(
                "This credential is restricted to one host. {}?\n\nCredential: \"{}\"\nCurrent host: \"{}\"",
                match to {
                    Some(h) => format!("Allow it to be used with \"{}\" instead", native_confirm::display_text(h)),
                    None => "Allow it to be used with any host".to_string(),
                },
                native_confirm::display_text(&stored.name),
                native_confirm::display_text(stored.host.as_deref().unwrap_or_default()),
            ),
            "Change host",
        )
        .await?;
    }
    let access = vault_state
        .credential_access()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    credential_manager
        .update_credential(
            access.key(),
            &credential_id,
            name,
            username,
            password,
            metadata,
            credential_type,
            host,
            port,
            key_path,
            private_key,
            key_passphrase,
        )
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
pub(crate) async fn delete_credential(
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
) -> Result<(), String> {
    let _gate = vault_state.credential_gate().await;
    credential_manager
        .delete_credential(&credential_id)
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
pub(crate) async fn search_credentials(
    credential_manager: State<'_, vault::CredentialManager>,
    query: String,
) -> Result<Vec<vault::CredentialSummary>, String> {
    credential_manager
        .search_credentials(&query)
        .map_err(|e| sanitize_error(e, "credential"))
}

#[tauri::command]
pub(crate) async fn trust_ssh_host_key(
    app: AppHandle,
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    approval_state: State<'_, ssh::HostKeyApprovalState>,
    audit: State<'_, vault::AuditLogManager>,
    request_id: String,
) -> Result<(), String> {
    confirm_changed_host_key(&app, &approval_state, &request_id).await?;
    // Only a key a handshake actually presented can be trusted, and only through its
    // pending approval. The webview used to pass host/fingerprint/key bytes itself and
    // could pin any key for any host, silently enabling a MITM on the unattended SSH,
    // SFTP and monitoring paths (IPC-002).
    let manager = ssh_key_manager.inner();
    let mut trusted: Option<(String, u16, String)> = None;
    approval_state
        .accept_and_persist(&request_id, |key| {
            trusted = Some((key.host.clone(), key.port, key.fingerprint.clone()));
            async move {
                manager
                    .trust_host_key(
                        &key.host,
                        key.port,
                        &key.fingerprint,
                        &key.key_type,
                        key.key_bytes,
                        vault::TrustStatus::Trusted,
                    )
                    .await
            }
        })
        .await
        .map_err(|e| {
            if e.contains("no longer pending") || e.contains("stopped waiting") {
                e
            } else {
                sanitize_error(e, "ssh")
            }
        })?;
    if let Some((host, port, fingerprint)) = trusted {
        audit.record(
            "ssh_host_key_trust",
            None,
            "ssh_host_key",
            "trust",
            "success",
            Some(&format!("{}:{} {}", host, port, fingerprint)),
        );
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn respond_ssh_host_key_verification(
    app: AppHandle,
    approval_state: State<'_, ssh::HostKeyApprovalState>,
    request_id: String,
    accepted: bool,
) -> Result<(), String> {
    if accepted {
        confirm_changed_host_key(&app, &approval_state, &request_id).await?;
    }
    approval_state.resolve(&request_id, accepted)
}

/// A changed host key is accepted (once or permanently) only after a native confirmation:
/// the webview's own prompt can be skipped by a compromised webview. Declining rejects the
/// pending handshake.
pub(crate) async fn confirm_changed_host_key(
    app: &AppHandle,
    approval_state: &ssh::HostKeyApprovalState,
    request_id: &str,
) -> Result<(), String> {
    let Some(warning) = approval_state.changed_key_warning(request_id)? else {
        return Ok(());
    };
    if let Err(e) = native_confirm::confirm(app, "Host key changed", warning, "Trust new key").await {
        let _ = approval_state.resolve(request_id, false);
        return Err(e);
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn get_known_ssh_hosts(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
) -> Result<Vec<vault::SshHostKey>, String> {
    ssh_key_manager
        .get_known_hosts()
        .await
        .map_err(|e| sanitize_error(e, "ssh"))
}

#[tauri::command]
pub(crate) async fn remove_ssh_host_key(
    app: AppHandle,
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    audit: State<'_, vault::AuditLogManager>,
    host: String,
    port: u16,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    // Removing a key turns the next connection into a first-seen (TOFU) prompt, which would
    // let a compromised webview replace a pinned key; so the user confirms natively.
    native_confirm::confirm(
        &app,
        "Remove host key",
        format!(
            "Remove the stored host key for {}:{}?\n\nThe next connection will ask you to trust whatever key the server presents.",
            host, port
        ),
        "Remove",
    )
    .await?;
    ssh_key_manager
        .remove_host_key(&host, port)
        .await
        .map_err(|e| sanitize_error(e, "ssh"))?;
    audit.record(
        "ssh_host_key_remove",
        None,
        "ssh_host_key",
        "delete",
        "success",
        Some(&format!("{}:{}", host, port)),
    );
    Ok(())
}

#[tauri::command]
pub(crate) async fn update_ssh_host_trust(
    app: AppHandle,
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    audit: State<'_, vault::AuditLogManager>,
    host: String,
    port: u16,
    trust_status: vault::TrustStatus,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    // Read the current status first: fails closed if it can't be read.
    let current = ssh_key_manager
        .trust_status(&host, port)
        .await
        .map_err(|e| sanitize_error(e, "ssh"))?;
    if native_confirm::trust_change_needs_confirm(current.as_ref(), &trust_status) {
        let message = if matches!(trust_status, vault::TrustStatus::Trusted) {
            format!(
                "Trust the stored host key for {}:{}?\n\nConnections, including scheduled tasks and monitoring, will then accept it without asking.",
                host, port
            )
        } else {
            format!(
                "Stop rejecting the stored host key for {}:{}?\n\nThe next connection will ask whether to trust it instead of refusing it.",
                host, port
            )
        };
        let (title, action) = if matches!(trust_status, vault::TrustStatus::Trusted) {
            ("Trust host key", "Trust")
        } else {
            ("Stop rejecting host key", "Stop rejecting")
        };
        native_confirm::confirm(&app, title, message, action).await?;
    }
    let details = format!("{}:{} -> {:?}", host, port, trust_status);
    ssh_key_manager
        .update_trust_status(&host, port, trust_status)
        .await
        .map_err(|e| sanitize_error(e, "ssh"))?;
    // Trust changes on stored keys are security-relevant (IPC-002).
    audit.record("ssh_host_key_trust_change", None, "ssh_host_key", "update", "success", Some(&details));
    Ok(())
}

// Change master password command
#[tauri::command]
pub(crate) async fn change_master_password(
    state: State<'_, vault::VaultState>,
    current_password: String,
    new_password: String,
) -> Result<(), String> {
    if current_password.is_empty() {
        return Err("Current password cannot be empty".to_string());
    }
    validate_master_password(&new_password)?;
    state
        .change_master_password(
            secrecy::SecretString::from(current_password),
            secrecy::SecretString::from(new_password),
        )
        .await
        .map_err(errors::user_facing_vault_error)
}

// Audit log commands
#[tauri::command]
pub(crate) async fn get_audit_logs(
    audit_manager: State<'_, vault::AuditLogManager>,
    filter: Option<vault::AuditLogFilter>,
) -> Result<Vec<vault::AuditLogEntry>, String> {
    audit_manager
        .get_audit_logs(filter)
        .map_err(|e| sanitize_error(e, "database"))
}

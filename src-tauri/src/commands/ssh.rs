//! Interactive SSH sessions, tunnels and RDP launch, and the credential-to-login path they share.

use crate::*;

/// What an interactive SSH connection or a tunnel authenticates with.
pub(crate) struct SshLogin {
    pub(crate) username: String,
    pub(crate) password: Option<String>,
    pub(crate) key_path: Option<String>,
    pub(crate) private_key: Option<String>,
    pub(crate) key_passphrase: Option<String>,
}

/// The login for `host`: from the vault credential when one is named, otherwise the typed
/// username and password. A stored credential must be allowed for `host` and be an SSH type
/// (invariant 10); vault access is dropped before returning, so no network phase holds it.
pub(crate) async fn resolve_ssh_login(
    vault_state: &vault::VaultState,
    credential_manager: &vault::CredentialManager,
    credential_id: Option<String>,
    host: &str,
    typed_username: String,
    typed_password: Option<String>,
) -> Result<SshLogin, String> {
    let Some(cid) = credential_id else {
        return Ok(SshLogin {
            username: typed_username,
            password: typed_password,
            key_path: None,
            private_key: None,
            key_passphrase: None,
        });
    };
    let access = vault_state
        .credential_access()
        .await
        .map_err(|e| sanitize_error(e, "vault"))?;
    let cred = credential_manager
        .get_credential(access.key(), &cid)
        .map_err(|e| sanitize_error(e, "credential"))?;
    drop(access);
    if !vault::credentials::host_allowed(cred.host.as_deref(), host) {
        return Err(vault::credentials::host_mismatch_error(&cred.name, cred.host.as_deref()));
    }
    vault::credentials::check_type(&cred.name, &cred.credential_type, vault::credentials::CredentialUse::Ssh)?;
    Ok(SshLogin {
        username: cred.username,
        password: (!cred.password.is_empty()).then_some(cred.password),
        key_path: cred.key_path,
        private_key: cred.private_key,
        key_passphrase: cred.key_passphrase,
    })
}

#[tauri::command]
pub(crate) async fn connect_ssh(
    ssh_state: State<'_, ssh::SshState>,
    app: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    id: String,
    host: String,
    user: String,
    port: u16,
    password: Option<String>,
    credential_id: Option<String>,
) -> Result<(), String> {
    let SshLogin { username, password, key_path, private_key, key_passphrase } =
        resolve_ssh_login(&vault_state, &credential_manager, credential_id, &host, user, password).await?;
    ssh::connect_ssh(
        ssh_state,
        app,
        id,
        host,
        username,
        port,
        password,
        key_path,
        private_key,
        key_passphrase,
        false, // enable_agent_forwarding: can be added to frontend later
    )
    .await
}

#[tauri::command]
pub(crate) async fn start_ssh_tunnel(
    app: AppHandle,
    tunnel_state: State<'_, ssh_tunnel::TunnelState>,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    tunnel_id: String,
    ssh_host: String,
    ssh_port: u16,
    ssh_user: String,
    password: Option<String>,
    credential_id: Option<String>,
    local_port: u16,
    remote_host: String,
    remote_port: u16,
) -> Result<ssh_tunnel::TunnelInfo, String> {
    if validate_ip(&ssh_host).is_err() && validate_hostname(&ssh_host).is_err() {
        return Err("Invalid SSH host format".to_string());
    }
    validate_port(ssh_port)?;
    validate_port(local_port)?;
    if validate_ip(&remote_host).is_err() && validate_hostname(&remote_host).is_err() {
        return Err("Invalid remote host format".to_string());
    }
    validate_port(remote_port)?;
    let SshLogin { username, password, key_path, private_key, key_passphrase } =
        resolve_ssh_login(&vault_state, &credential_manager, credential_id, &ssh_host, ssh_user, password).await?;
    ssh_tunnel::start_tunnel(
        app,
        tunnel_state.inner(),
        tunnel_id,
        ssh_host,
        ssh_port,
        username,
        password,
        key_path,
        private_key,
        key_passphrase,
        local_port,
        remote_host,
        remote_port,
    )
    .await
    .map_err(|e| sanitize_error(e, "tunnel"))
}

#[tauri::command]
pub(crate) fn list_ssh_tunnels(
    tunnel_state: State<'_, ssh_tunnel::TunnelState>,
) -> Vec<ssh_tunnel::TunnelInfo> {
    tunnel_state.list()
}

#[tauri::command]
pub(crate) fn close_ssh_tunnel(
    tunnel_state: State<'_, ssh_tunnel::TunnelState>,
    tunnel_id: String,
) -> Result<(), String> {
    if tunnel_state.remove(&tunnel_id) {
        Ok(())
    } else {
        Err("Tunnel not found".to_string())
    }
}

#[tauri::command]
pub(crate) async fn connect_rdp(address: String) -> Result<(), String> {
    if validate_ip(&address).is_err() && validate_hostname(&address).is_err() {
        return Err("Invalid host address format".to_string());
    }
    launcher::launch_rdp(&address).map_err(|e| sanitize_error(e, "network"))
}

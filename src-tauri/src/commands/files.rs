//! Backend-opened file dialogs (local path grants) and SFTP transfers.

use crate::*;

// SFTP commands
/// Opens a native "open file" dialog from the backend and records the choice, so the path
/// can later be used for file I/O (IPC-001). `None` when the user cancels.
#[tauri::command]
pub(crate) async fn pick_local_file(
    app: AppHandle,
    grants: State<'_, local_paths::LocalPathGrants>,
    title: Option<String>,
    extensions: Option<Vec<String>>,
) -> Result<Option<String>, String> {
    let dialog_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut builder = file_dialog(&dialog_app, extensions);
        if let Some(title) = title {
            builder = builder.set_title(title);
        }
        builder.blocking_pick_file()
    })
    .await
    .map_err(|e| sanitize_error(e.to_string(), "dialog"))?;
    record_pick(&grants, picked, local_paths::Intent::Read)
}

/// Opens a native "save file" dialog from the backend and records the choice (IPC-001).
#[tauri::command]
pub(crate) async fn pick_save_location(
    app: AppHandle,
    grants: State<'_, local_paths::LocalPathGrants>,
    default_name: Option<String>,
    extensions: Option<Vec<String>>,
) -> Result<Option<String>, String> {
    let dialog_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut builder = file_dialog(&dialog_app, extensions);
        if let Some(name) = default_name {
            builder = builder.set_file_name(name);
        }
        builder.blocking_save_file()
    })
    .await
    .map_err(|e| sanitize_error(e.to_string(), "dialog"))?;
    record_pick(&grants, picked, local_paths::Intent::Write)
}

/// A file dialog parented to the main window, optionally filtered to `extensions`.
pub(crate) fn file_dialog(
    app: &AppHandle,
    extensions: Option<Vec<String>>,
) -> tauri_plugin_dialog::FileDialogBuilder<tauri::Wry> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app.dialog().file();
    if let Some(window) = app.get_webview_window("main") {
        builder = builder.set_parent(&window);
    }
    if let Some(exts) = extensions.filter(|e| !e.is_empty()) {
        let exts: Vec<&str> = exts.iter().map(String::as_str).collect();
        builder = builder.add_filter("Files", &exts);
    }
    builder
}

pub(crate) fn record_pick(
    grants: &local_paths::LocalPathGrants,
    picked: Option<tauri_plugin_dialog::FilePath>,
    intent: local_paths::Intent,
) -> Result<Option<String>, String> {
    let Some(picked) = picked else { return Ok(None) };
    let path = picked
        .into_path()
        .map_err(|e| sanitize_error(e.to_string(), "dialog"))?;
    let path_str = path
        .to_str()
        .ok_or_else(|| "Selected path is not valid UTF-8".to_string())?
        .to_string();
    grants.grant(path, intent);
    Ok(Some(path_str))
}

/// Resolves SFTP auth: a vault credential by id (decrypted here, never sent to the webview),
/// or a password the user typed. SFTP is password-only (see CLAUDE.md constraints).
pub(crate) async fn resolve_sftp_auth(
    vault_state: &vault::VaultState,
    credential_manager: &vault::CredentialManager,
    host: &str,
    username: String,
    password: Option<String>,
    credential_id: Option<String>,
) -> Result<(String, String), String> {
    match credential_id {
        Some(cid) => {
            let access = vault_state
                .credential_access()
                .await
                .map_err(errors::user_facing_vault_error)?;
            let cred = credential_manager
                .get_credential(access.key(), &cid)
                .map_err(|e| sanitize_error(e, "credential"))?;
            drop(access);
            if !vault::credentials::host_allowed(cred.host.as_deref(), host) {
                return Err(vault::credentials::host_mismatch_error(&cred.name, cred.host.as_deref()));
            }
            vault::credentials::check_type(&cred.name, &cred.credential_type, vault::credentials::CredentialUse::Sftp)?;
            if cred.password.is_empty() {
                return Err("SFTP needs a password credential".to_string());
            }
            Ok((cred.username, cred.password))
        }
        None => Ok((username, password.ok_or_else(|| "A password or a credential is required".to_string())?)),
    }
}

#[tauri::command]
pub(crate) async fn sftp_upload_file(
    app_handle: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    grants: State<'_, local_paths::LocalPathGrants>,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    credential_id: Option<String>,
    local_path: String,
    remote_path: String,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    let (username, password) = resolve_sftp_auth(
        &vault_state,
        &credential_manager,
        &host,
        username,
        password,
        credential_id,
    )
    .await?;
    grants.take(&local_path, local_paths::Intent::Read)?;
    validate_path(&local_path)?;
    validate_path(&remote_path)?;
    // The grant is used up even if the transfer fails: giving it back would let a
    // compromised webview replay the pick against another host (PR #68 review). The UI
    // opens the dialog again for every transfer.
    sftp::upload_file(
        app_handle,
        &host,
        port,
        &username,
        &password,
        &local_path,
        &remote_path,
        None,
    )
    .await
    .map_err(|e| sanitize_error(e, "sftp"))
}

#[tauri::command]
pub(crate) async fn sftp_download_file(
    app_handle: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    grants: State<'_, local_paths::LocalPathGrants>,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    credential_id: Option<String>,
    remote_path: String,
    local_path: String,
) -> Result<(), String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    let (username, password) = resolve_sftp_auth(
        &vault_state,
        &credential_manager,
        &host,
        username,
        password,
        credential_id,
    )
    .await?;
    grants.take(&local_path, local_paths::Intent::Write)?;
    validate_path(&remote_path)?;
    validate_path(&local_path)?;
    // Used up even on failure, as for uploads.
    sftp::download_file(
        app_handle,
        &host,
        port,
        &username,
        &password,
        &remote_path,
        &local_path,
        None,
    )
    .await
    .map_err(|e| sanitize_error(e, "sftp"))
}

#[tauri::command]
pub(crate) async fn sftp_list_directory(
    app_handle: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    credential_id: Option<String>,
    remote_path: String,
) -> Result<Vec<sftp::RemoteFile>, String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    let (username, password) = resolve_sftp_auth(
        &vault_state,
        &credential_manager,
        &host,
        username,
        password,
        credential_id,
    )
    .await?;
    sftp::list_directory(app_handle, &host, port, &username, &password, &remote_path)
        .await
        .map_err(|e| sanitize_error(e, "sftp"))
}

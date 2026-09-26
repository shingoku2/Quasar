//! App info, metrics cleanup, and database export/import.

use crate::*;

/// App info for Settings (About, Data tabs).
#[derive(serde::Serialize)]
pub(crate) struct AppInfo {
    pub app_data_dir: String,
    pub db_path: String,
    pub db_size_bytes: Option<u64>,
    pub version: String,
    pub platform: String,
    pub arch: String,
}

#[tauri::command]
pub(crate) fn get_app_info(app: AppHandle) -> Result<AppInfo, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let app_data_dir = app_dir
        .to_str()
        .ok_or_else(|| "Invalid app data path".to_string())?
        .to_string();
    let db_path = app_dir.join(DB_FILENAME);
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| "Invalid database path".to_string())?
        .to_string();
    let db_size_bytes =
        std::fs::metadata(&db_path)
            .ok()
            .and_then(|m| if m.is_file() { Some(m.len()) } else { None });
    let version = app.package_info().version.to_string();
    let platform = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    Ok(AppInfo {
        app_data_dir,
        db_path: db_path_str,
        db_size_bytes,
        version,
        platform,
        arch,
    })
}

#[tauri::command]
pub(crate) fn clear_metrics_data(app: AppHandle) -> Result<(), String> {
    let conn = app_db_connection(&app)?;
    conn.execute("DELETE FROM metrics_history", [])
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    conn.execute("DELETE FROM alert_history", [])
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    Ok(())
}

#[tauri::command]
pub(crate) fn export_database(
    app: AppHandle,
    grants: State<'_, local_paths::LocalPathGrants>,
    dest_path: String,
) -> Result<(), String> {
    validate_path(&dest_path)?;
    grants.take(&dest_path, local_paths::Intent::Write)?;
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let src = app_dir.join(DB_FILENAME);
    if !src.exists() {
        return Err("Database file not found".to_string());
    }
    // Use the rusqlite backup API instead of VACUUM INTO string interpolation,
    // which was vulnerable to SQL injection via a crafted dest_path.
    let src_conn =
        rusqlite::Connection::open(&src).map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let mut dst_conn = rusqlite::Connection::open(&dest_path)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let backup = rusqlite::backup::Backup::new(&src_conn, &mut dst_conn)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    backup
        .run_to_completion(100, std::time::Duration::from_millis(0), None)
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    // The export carries the encrypted vault; owner-only like the live DB (RSEC-009).
    db::restrict_to_owner(std::path::Path::new(&dest_path))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn import_database(
    app: AppHandle,
    vault_state: State<'_, vault::VaultState>,
    grants: State<'_, local_paths::LocalPathGrants>,
    source_path: String,
) -> Result<(), String> {
    validate_path(&source_path)?;
    grants.take(&source_path, local_paths::Intent::Read)?;
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| sanitize_error(e.to_string(), "database"))?;
    let was_locked = vault_state.is_locked().await;
    let alert_engine = app.state::<Arc<monitoring::AlertEngine>>();
    let result = import_database_at(&app_dir, &source_path, &vault_state, Some(alert_engine.as_ref())).await;
    // The frontend handles this event by showing the unlock dialog. Emit it whenever the
    // import locked the vault, including a backup/restore failure after the lock, so the UI
    // never keeps showing an unlocked vault the backend has locked.
    if !was_locked && vault_state.is_locked().await {
        let _ = app.emit("vault-auto-locked", ());
    }
    result?;
    log::warn!(
        "Database imported from '{}'. Vault locked; restart recommended so background tasks reload it.",
        source_path
    );
    Ok(())
}

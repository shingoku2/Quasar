mod crypto;
mod launcher;
mod discovery;
mod ai;
mod ssh;
mod ssh_exec;
mod sftp;
mod scanner;
mod health;
mod monitoring;
mod vault;

use tauri::{AppHandle, Manager, State, Emitter};
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use std::sync::Arc;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn hash_password(password: String) -> Result<String, String> {
    crypto::hash_password(&password)
}

#[tauri::command]
fn verify_password(password: String, hashed: String) -> Result<bool, String> {
    crypto::verify_password(&password, &hashed)
}

#[tauri::command]
async fn launch_ssh_external(address: String, username: Option<String>) -> Result<(), String> {
    launcher::launch_ssh(&address, username.as_deref())
}

#[tauri::command]
async fn connect_rdp(address: String) -> Result<(), String> {
    launcher::launch_rdp(&address)
}

#[tauri::command]
fn start_discovery(app: AppHandle) {
    discovery::start_mdns_discovery(app);
}

#[tauri::command]
async fn check_ai_status() -> bool {
    ai::check_ollama_status().await
}

#[tauri::command]
async fn list_ai_models() -> Result<Vec<String>, String> {
    ai::list_models().await
}

// Simple struct to receive messages from frontend
#[derive(serde::Deserialize)]
struct FrontendMessage {
    role: String,
    content: String,
}

#[tauri::command]
async fn send_ai_chat(app: AppHandle, model: String, messages: Vec<FrontendMessage>) -> Result<(), String> {
    let chat_messages = messages.into_iter().map(|m| {
        let role = match m.role.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "system" => MessageRole::System,
            _ => MessageRole::User,
        };
        ChatMessage::new(role, m.content)
    }).collect();

    ai::chat_request(app, model, chat_messages).await
}

#[tauri::command]
async fn scan_network(
    state: State<'_, Arc<scanner::ScannerState>>,
    app: AppHandle,
    cidr: String,
) -> Result<(), String> {
    let state = Arc::clone(&state);
    let app_for_progress = app.clone();
    let app_for_result = app.clone();
    
    tokio::spawn(async move {
        let on_progress = move |progress: scanner::ScanProgress| {
            let _ = app_for_progress.emit("scan_progress", progress);
        };
        
        let on_result = move |result: scanner::ScanResult| {
            let _ = app_for_result.emit("scan_result", result);
        };
        
        let _ = scanner::scan_network(state, cidr, on_progress, on_result).await;
    });
    
    Ok(())
}

#[tauri::command]
fn stop_scan(state: State<'_, Arc<scanner::ScannerState>>) {
    scanner::stop_scan(&state);
}

#[tauri::command]
fn get_scan_progress(state: State<'_, Arc<scanner::ScannerState>>) -> scanner::ScanProgress {
    scanner::get_scan_progress(&state)
}

#[tauri::command]
fn is_scanning(state: State<'_, Arc<scanner::ScannerState>>) -> bool {
    scanner::is_scanning(&state)
}

#[tauri::command]
async fn preflight_check(host: String) -> Result<health::HealthCheckResult, String> {
    Ok(health::preflight_check(&host).await)
}

#[tauri::command]
async fn check_host_health(
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
) -> Result<health::HealthCheckResult, String> {
    Ok(health::check_ssh_health(&host, port, &username, password.as_deref()).await)
}

#[tauri::command]
fn get_system_metrics() -> monitoring::SystemMetrics {
    let mut collector = monitoring::MetricsCollector::new();
    collector.collect()
}

#[tauri::command]
async fn get_metrics_history(
    start: u64,
    end: u64,
    host: Option<String>,
    app: AppHandle,
) -> Result<Vec<monitoring::SystemMetrics>, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_dir.join("titan.db");
    let db_path_str = db_path.to_str().ok_or("Invalid database path")?.to_string();
    
    let store = monitoring::MetricsStore::new(db_path_str, 30)?;
    store.get_metrics_range(start, end, &host.unwrap_or_else(|| "localhost".to_string()))
}

#[tauri::command]
async fn get_alert_history(
    start: u64,
    end: u64,
    app: AppHandle,
) -> Result<Vec<monitoring::Alert>, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_dir.join("titan.db");
    let db_path_str = db_path.to_str().ok_or("Invalid database path")?.to_string();
    
    let store = monitoring::MetricsStore::new(db_path_str, 30)?;
    store.get_alert_history(start, end)
}

#[tauri::command]
fn add_alert_rule(
    state: State<'_, Arc<monitoring::AlertEngine>>,
    rule: monitoring::AlertRule,
) {
    state.add_rule(rule);
}

#[tauri::command]
fn remove_alert_rule(
    state: State<'_, Arc<monitoring::AlertEngine>>,
    rule_id: String,
) {
    state.remove_rule(&rule_id);
}

#[tauri::command]
fn get_alert_rules(state: State<'_, Arc<monitoring::AlertEngine>>) -> Vec<monitoring::AlertRule> {
    state.get_rules()
}

// Vault commands
#[tauri::command]
async fn is_vault_initialized(state: State<'_, vault::VaultState>) -> Result<bool, String> {
    state.is_initialized().await
}

#[tauri::command]
async fn initialize_vault(state: State<'_, vault::VaultState>, master_password: String) -> Result<(), String> {
    state.initialize_vault(master_password).await
}

#[tauri::command]
async fn unlock_vault(state: State<'_, vault::VaultState>, master_password: String) -> Result<(), String> {
    state.unlock_vault(master_password).await
}

#[tauri::command]
async fn lock_vault(state: State<'_, vault::VaultState>) -> Result<(), String> {
    state.lock_vault().await
}

#[tauri::command]
async fn is_vault_locked(state: State<'_, vault::VaultState>) -> Result<bool, String> {
    Ok(state.is_locked().await)
}

#[tauri::command]
async fn get_vault_settings(state: State<'_, vault::VaultState>) -> Result<vault::VaultSettings, String> {
    Ok(state.get_settings().await)
}

#[tauri::command]
async fn update_vault_settings(state: State<'_, vault::VaultState>, settings: vault::VaultSettings) -> Result<(), String> {
    state.update_settings(settings).await
}

// Credential management commands
#[tauri::command]
async fn add_credential(
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    name: String,
    username: String,
    password: String,
    credential_type: String,
    metadata: Option<String>,
) -> Result<String, String> {
    let master_key = vault_state.get_master_key().await?;
    credential_manager.add_credential(&master_key, name, username, password, credential_type, metadata)
}

#[tauri::command]
async fn get_credential(
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
) -> Result<vault::Credential, String> {
    let master_key = vault_state.get_master_key().await?;
    credential_manager.get_credential(&master_key, &credential_id)
}

#[tauri::command]
async fn list_credentials(
    credential_manager: State<'_, vault::CredentialManager>,
) -> Result<Vec<vault::CredentialSummary>, String> {
    credential_manager.list_credentials()
}

#[tauri::command]
async fn update_credential(
    vault_state: State<'_, vault::VaultState>,
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
    name: Option<String>,
    username: Option<String>,
    password: Option<String>,
    metadata: Option<String>,
) -> Result<(), String> {
    let master_key = vault_state.get_master_key().await?;
    credential_manager.update_credential(&master_key, &credential_id, name, username, password, metadata)
}

#[tauri::command]
async fn delete_credential(
    credential_manager: State<'_, vault::CredentialManager>,
    credential_id: String,
) -> Result<(), String> {
    credential_manager.delete_credential(&credential_id)
}

#[tauri::command]
async fn search_credentials(
    credential_manager: State<'_, vault::CredentialManager>,
    query: String,
) -> Result<Vec<vault::CredentialSummary>, String> {
    credential_manager.search_credentials(&query)
}

#[tauri::command]
async fn verify_ssh_host_key(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    host: String,
    port: u16,
    fingerprint: String,
    key_type: String,
) -> Result<vault::HostKeyVerificationResult, String> {
    ssh_key_manager.verify_host_key_by_fingerprint(&host, port, &fingerprint, &key_type).await
}

#[tauri::command]
async fn trust_ssh_host_key(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    host: String,
    port: u16,
    fingerprint: String,
    key_type: String,
    key_bytes: Vec<u8>,
    trust_status: vault::TrustStatus,
) -> Result<(), String> {
    ssh_key_manager.trust_host_key(&host, port, &fingerprint, &key_type, key_bytes, trust_status).await
}

#[tauri::command]
async fn get_known_ssh_hosts(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
) -> Result<Vec<vault::SshHostKey>, String> {
    ssh_key_manager.get_known_hosts().await
}

#[tauri::command]
async fn remove_ssh_host_key(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    host: String,
    port: u16,
) -> Result<(), String> {
    ssh_key_manager.remove_host_key(&host, port).await
}

#[tauri::command]
async fn update_ssh_host_trust(
    ssh_key_manager: State<'_, vault::SshKeyManager>,
    host: String,
    port: u16,
    trust_status: vault::TrustStatus,
) -> Result<(), String> {
    ssh_key_manager.update_trust_status(&host, port, trust_status).await
}

// Change master password command
#[tauri::command]
async fn change_master_password(
    state: State<'_, vault::VaultState>,
    current_password: String,
    new_password: String,
) -> Result<(), String> {
    state.change_master_password(&current_password, &new_password).await
}

// Audit log commands
#[tauri::command]
async fn get_audit_logs(
    audit_manager: State<'_, vault::AuditLogManager>,
    filter: Option<vault::AuditLogFilter>,
) -> Result<Vec<vault::AuditLogEntry>, String> {
    audit_manager.get_audit_logs(filter)
}

#[tauri::command]
async fn get_audit_log_count(
    audit_manager: State<'_, vault::AuditLogManager>,
    filter: Option<vault::AuditLogFilter>,
) -> Result<i64, String> {
    audit_manager.get_audit_log_count(filter)
}

// SFTP commands
#[tauri::command]
async fn sftp_upload_file(
    host: String,
    port: u16,
    username: String,
    password: String,
    local_path: String,
    remote_path: String,
) -> Result<(), String> {
    sftp::upload_file(&host, port, &username, &password, &local_path, &remote_path, None).await
}

#[tauri::command]
async fn sftp_download_file(
    host: String,
    port: u16,
    username: String,
    password: String,
    remote_path: String,
    local_path: String,
) -> Result<(), String> {
    sftp::download_file(&host, port, &username, &password, &remote_path, &local_path, None).await
}

#[tauri::command]
async fn sftp_list_directory(
    host: String,
    port: u16,
    username: String,
    password: String,
    remote_path: String,
) -> Result<Vec<String>, String> {
    sftp::list_directory(&host, port, &username, &password, &remote_path).await
}

#[tauri::command]
async fn sftp_remote_exists(
    host: String,
    port: u16,
    username: String,
    password: String,
    remote_path: String,
) -> Result<bool, String> {
    sftp::remote_exists(&host, port, &username, &password, &remote_path).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Get database path
            let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data dir");
            let db_path = app_dir.join("titan.db");
            let db_path_str = db_path.to_str().expect("Invalid database path").to_string();
            
            app.manage(ssh::SshState::new());
            app.manage(Arc::new(scanner::ScannerState::new()));
            app.manage(Arc::new(monitoring::AlertEngine::new()));
            app.manage(vault::VaultState::new(db_path_str.clone()));
            app.manage(vault::CredentialManager::new(db_path_str.clone()));
            app.manage(vault::SshKeyManager::new(db_path_str.clone()).expect("Failed to create SSH key manager"));
            app.manage(vault::AuditLogManager::new(db_path_str.clone()));
            
            // Initialize database tables
            let conn = rusqlite::Connection::open(&db_path_str).expect("Failed to open database");
            conn.execute_batch(include_str!("../migrations/003_security_vault.sql"))
                .expect("Failed to run security vault migrations");
            conn.execute_batch(include_str!("../migrations/004_monitoring.sql"))
                .expect("Failed to run monitoring migrations");
            
            // Start the background monitoring task using Tauri's async runtime
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                monitoring::start_monitoring_task(app_handle, 5).await;
            });
            
            // Start auto-lock checker task
            let vault_state_clone = app.state::<vault::VaultState>().inner().clone();
            let app_handle_vault = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    match vault_state_clone.check_auto_lock().await {
                        Ok(locked) => {
                            if locked {
                                let _ = app_handle_vault.emit("vault-auto-locked", ());
                            }
                        }
                        Err(e) => {
                            eprintln!("Auto-lock check failed: {}", e);
                        }
                    }
                }
            });
            
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            hash_password,
            verify_password,
            get_metrics_history,
            get_alert_history,
            launch_ssh_external,
            connect_rdp,
            start_discovery,
            check_ai_status,
            list_ai_models,
            send_ai_chat,
            ssh::connect_ssh,
            ssh::write_ssh,
            ssh::resize_ssh,
            ssh::disconnect_ssh,
            scan_network,
            stop_scan,
            get_scan_progress,
            is_scanning,
            preflight_check,
            check_host_health,
            get_system_metrics,
            add_alert_rule,
            remove_alert_rule,
            get_alert_rules,
            is_vault_initialized,
            initialize_vault,
            unlock_vault,
            lock_vault,
            is_vault_locked,
            get_vault_settings,
            update_vault_settings,
            change_master_password,
            add_credential,
            get_credential,
            list_credentials,
            update_credential,
            delete_credential,
            search_credentials,
            verify_ssh_host_key,
            trust_ssh_host_key,
            get_known_ssh_hosts,
            remove_ssh_host_key,
            update_ssh_host_trust,
            get_audit_logs,
            get_audit_log_count,
            sftp_upload_file,
            sftp_download_file,
            sftp_list_directory,
            sftp_remote_exists
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        let result = greet("World");
        assert_eq!(result, "Hello, World! You've been greeted from Rust!");
    }
}

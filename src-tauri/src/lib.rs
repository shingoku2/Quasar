mod automation;
mod crypto;
mod launcher;
mod discovery;
mod ai;
mod ssh;
mod scanner;
mod health;
mod monitoring;

use tauri::{AppHandle, Manager, State, Emitter};
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use std::sync::Arc;

use crate::automation::{AutomationState, Workflow, ExecutionRecord, ExecutionStatus};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn hash_password(password: String) -> String {
    crypto::hash_password(&password)
}

#[tauri::command]
fn verify_password(password: String, hashed: String) -> bool {
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
async fn create_workflow(
    state: State<'_, Arc<automation::AutomationState>>,
    workflow: automation::Workflow,
) -> Result<String, String> {
    Ok(state.create_workflow(workflow).await)
}

#[tauri::command]
async fn get_workflow(
    state: State<'_, Arc<automation::AutomationState>>,
    id: String,
) -> Result<Option<automation::Workflow>, String> {
    Ok(state.get_workflow(&id).await)
}

#[tauri::command]
async fn list_workflows(
    state: State<'_, Arc<automation::AutomationState>>,
) -> Result<Vec<automation::Workflow>, String> {
    Ok(state.list_workflows().await)
}

#[tauri::command]
async fn delete_workflow(
    state: State<'_, Arc<automation::AutomationState>>,
    id: String,
) -> Result<bool, String> {
    Ok(state.delete_workflow(&id).await)
}

#[tauri::command]
async fn execute_workflow(
    state: State<'_, Arc<automation::AutomationState>>,
    app: AppHandle,
    workflow_id: String,
) -> Result<String, String> {
    let workflow = state.get_workflow(&workflow_id).await
        .ok_or_else(|| "Workflow not found".to_string())?;
    
    let exec_id = state.create_execution(&workflow_id).await;
    let exec_id_for_spawn = exec_id.clone();
    
    // Spawn execution in background
    let state_clone = Arc::clone(&state);
    tauri::async_runtime::spawn(async move {
        let engine = automation::engine::WorkflowEngine::new(app);
        let mut context = automation::ExecutionContext::new();
        
        let status = engine.execute_workflow(&workflow, &exec_id_for_spawn, &mut context).await;
        
        // Update execution record
        if let Some(mut record) = state_clone.get_execution(&exec_id_for_spawn).await {
            record.status = status.unwrap_or(automation::ExecutionStatus::Failed);
            record.completed_at = Some(chrono::Utc::now());
            state_clone.update_execution(record).await;
        }
    });
    
    Ok(exec_id)
}

#[tauri::command]
async fn get_execution_status(
    state: State<'_, Arc<automation::AutomationState>>,
    id: String,
) -> Result<Option<automation::ExecutionRecord>, String> {
    Ok(state.get_execution(&id).await)
}

#[tauri::command]
fn add_alert_rule(rule: monitoring::AlertRule) {
    // AlertEngine will be managed state in setup
}

#[tauri::command]
fn remove_alert_rule(rule_id: String) {
    // AlertEngine will be managed state in setup
}

#[tauri::command]
fn get_alert_rules() -> Vec<monitoring::AlertRule> {
    Vec::new()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(ssh::SshState::new());
            app.manage(Arc::new(scanner::ScannerState::new()));
            app.manage(Arc::new(automation::AutomationState::new()));
            
            // Start the background monitoring task using Tauri's async runtime
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                monitoring::start_monitoring_task(app_handle, 5000).await;
            });
            
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            hash_password,
            verify_password,
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
            create_workflow,
            get_workflow,
            list_workflows,
            delete_workflow,
            execute_workflow,
            get_execution_status
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

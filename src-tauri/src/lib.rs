mod crypto;
mod launcher;
mod discovery;
mod ai;
mod ssh;

use tauri::{AppHandle, Manager};
use ollama_rs::generation::chat::{ChatMessage, MessageRole};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(ssh::SshState::new());
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
            ssh::disconnect_ssh
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

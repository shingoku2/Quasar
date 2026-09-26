//! The optional local AI assistant (Ollama).

use crate::*;

#[tauri::command]
pub(crate) async fn check_ai_status() -> bool {
    ai::check_ollama_status().await
}

#[tauri::command]
pub(crate) async fn list_ai_models() -> Result<Vec<String>, String> {
    ai::list_models()
        .await
        .map_err(|e| sanitize_error(e, "network"))
}

// Simple struct to receive messages from frontend
#[derive(serde::Deserialize)]
pub(crate) struct FrontendMessage {
    role: String,
    content: String,
}

#[tauri::command]
pub(crate) async fn send_ai_chat(
    app: AppHandle,
    model: String,
    messages: Vec<FrontendMessage>,
) -> Result<(), String> {
    let chat_messages = messages
        .into_iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "user" => MessageRole::User,
                "assistant" => MessageRole::Assistant,
                "system" => MessageRole::System,
                _ => MessageRole::User,
            };
            ChatMessage::new(role, m.content)
        })
        .collect();

    ai::chat_request(app, model, chat_messages)
        .await
        .map_err(|e| sanitize_error(e, "network"))
}

use ollama_rs::Ollama;
use ollama_rs::generation::chat::{ChatMessage, ChatMessageResponseStream};
use ollama_rs::generation::chat::request::ChatMessageRequest;
use tokio_stream::StreamExt;
use tauri::{AppHandle, Emitter};

pub async fn check_ollama_status() -> bool {
    let ollama = Ollama::default();
    // A simple way to check connectivity is to list local models or generate a dummy request
    match ollama.list_local_models().await {
        Ok(_) => true,
        Err(_) => false,
    }
}

pub async fn list_models() -> Result<Vec<String>, String> {
    let ollama = Ollama::default();
    let models = ollama.list_local_models().await.map_err(|e| e.to_string())?;
    Ok(models.into_iter().map(|m| m.name).collect())
}

#[derive(Clone, serde::Serialize)]
struct ChatStreamPayload {
    content: String,
    done: bool,
}

pub async fn chat_request(
    app: AppHandle,
    model: String,
    messages: Vec<ChatMessage>,
) -> Result<(), String> {
    let ollama = Ollama::default();
    let request = ChatMessageRequest::new(model, messages);

    let mut stream: ChatMessageResponseStream = ollama
        .send_chat_messages_stream(request)
        .await
        .map_err(|e| e.to_string())?;

    while let Some(Ok(res)) = stream.next().await {
        if let Some(content) = Some(res.message) {
            let payload = ChatStreamPayload {
                content: content.content,
                done: false,
            };
            let _ = app.emit("ai-chat-response", payload);
        }
        
        if res.done {
             let payload = ChatStreamPayload {
                content: String::new(),
                done: true,
            };
            let _ = app.emit("ai-chat-response", payload);
        }
    }

    Ok(())
}

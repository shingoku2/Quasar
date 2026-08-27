use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::chat::{ChatMessage, ChatMessageResponseStream};
use ollama_rs::Ollama;
use tauri::{AppHandle, Emitter};
use tokio_stream::StreamExt;

pub async fn check_ollama_status() -> bool {
    let ollama = Ollama::default();
    // A simple way to check connectivity is to list local models or generate a dummy request
    ollama.list_local_models().await.is_ok()
}

pub async fn list_models() -> Result<Vec<String>, String> {
    let ollama = Ollama::default();
    let models = ollama
        .list_local_models()
        .await
        .map_err(|e| e.to_string())?;
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

    loop {
        match stream.next().await {
            Some(Ok(res)) => {
                let payload = ChatStreamPayload {
                    content: res.message.content,
                    done: false,
                };
                let _ = app.emit("ai-chat-response", payload);

                if res.done {
                    let payload = ChatStreamPayload {
                        content: String::new(),
                        done: true,
                    };
                    let _ = app.emit("ai-chat-response", payload);
                    break;
                }
            }
            Some(Err(())) => {
                log::error!("Ollama chat stream error");
                // Ensure the frontend's streaming UI always resolves, even on a mid-stream error.
                let payload = ChatStreamPayload {
                    content: String::new(),
                    done: true,
                };
                let _ = app.emit("ai-chat-response", payload);
                return Err("Ollama chat stream failed".to_string());
            }
            None => break,
        }
    }

    Ok(())
}

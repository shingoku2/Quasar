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

    let stream: ChatMessageResponseStream = ollama
        .send_chat_messages_stream(request)
        .await
        .map_err(|e| e.to_string())?;

    forward_chat_stream(stream.map(|r| r.map(|res| (res.message.content, res.done))), |payload| {
        let _ = app.emit("ai-chat-response", payload);
    })
    .await
}

/// Forwards `(content, done)` chunks to `emit` and always finishes with exactly one
/// `done: true` payload, however the stream ends. A stream that just ended without a
/// `done` chunk used to leave the chat stuck "thinking" (audit RUST-023).
async fn forward_chat_stream<S, F>(mut stream: S, mut emit: F) -> Result<(), String>
where
    S: tokio_stream::Stream<Item = Result<(String, bool), ()>> + Unpin,
    F: FnMut(ChatStreamPayload),
{
    let finish = |emit: &mut F| {
        emit(ChatStreamPayload {
            content: String::new(),
            done: true,
        })
    };
    loop {
        match stream.next().await {
            Some(Ok((content, done))) => {
                emit(ChatStreamPayload { content, done: false });
                if done {
                    finish(&mut emit);
                    return Ok(());
                }
            }
            Some(Err(())) => {
                log::error!("Ollama chat stream error");
                finish(&mut emit);
                return Err("Ollama chat stream failed".to_string());
            }
            None => {
                finish(&mut emit);
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn run(items: Vec<Result<(String, bool), ()>>) -> (Result<(), String>, Vec<(String, bool)>) {
        let mut seen = Vec::new();
        let result = forward_chat_stream(tokio_stream::iter(items), |p| seen.push((p.content, p.done))).await;
        (result, seen)
    }

    /// RUST-023: every ending emits exactly one final `done`.
    #[tokio::test]
    async fn every_stream_ending_emits_one_done() {
        let (ok, seen) = run(vec![Ok(("Hel".into(), false)), Ok(("lo".into(), true))]).await;
        assert!(ok.is_ok());
        assert_eq!(seen.iter().filter(|(_, d)| *d).count(), 1);
        assert_eq!(seen.last(), Some(&(String::new(), true)));

        // Ends without a done chunk: used to emit nothing final.
        let (ok, seen) = run(vec![Ok(("partial".into(), false))]).await;
        assert!(ok.is_ok());
        assert_eq!(seen, vec![("partial".to_string(), false), (String::new(), true)]);

        let (err, seen) = run(vec![Ok(("x".into(), false)), Err(())]).await;
        assert!(err.is_err());
        assert_eq!(seen.last(), Some(&(String::new(), true)));

        let (_, seen) = run(vec![]).await;
        assert_eq!(seen, vec![(String::new(), true)]);
    }
}

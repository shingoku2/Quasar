# Implementation Plan - Local AI Integration

## Phase 1: Backend Foundation (Rust) [checkpoint: 92ee089]
- [x] Task: Implement Ollama API Wrapper [1da9f4d]
    - [x] Create `src-tauri/src/ai.rs` module.
    - [x] Implement `check_ollama_status` to verify connection.
    - [x] Implement `list_models` to fetch available models.
    - [x] Implement `chat_request` with streaming support (using Tauri events).
- [x] Task: Conductor - User Manual Verification 'Backend Foundation' (Protocol in workflow.md) [manual]

## Phase 2: Frontend UI (React)
- [ ] Task: AI Assistant UI
    - [ ] Create `src/components/AIAssistant.tsx`.
    - [ ] Implement "Connection Status" indicator (checking Ollama).
    - [ ] Implement "Model Selector" dropdown.
    - [ ] Implement "Chat Interface" (Message list, Input area).
- [ ] Task: Integration
    - [ ] Add "AI Assistant" to the `Layout` sidebar.
    - [ ] Wire up the Chat UI to the Rust backend (streaming responses).
- [ ] Task: Conductor - User Manual Verification 'Frontend UI' (Protocol in workflow.md)

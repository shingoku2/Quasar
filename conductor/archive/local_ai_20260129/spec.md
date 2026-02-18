# Track: Integrate Local AI with Ollama

## Specification
<<<<<<< HEAD
This track implements the "Local AI Integration" phase of Quasar, enabling privacy-focused, offline AI assistance for sysadmins.
=======
This track implements the "Local AI Integration" phase of Project Titan, enabling privacy-focused, offline AI assistance for sysadmins.
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7

### Scope
- **Ollama Integration:** Detect and interact with a local Ollama instance.
- **Backend API:** Implement a Rust wrapper around the Ollama REST API (specifically `/api/chat` and `/api/tags`).
- **UI:** Create a dedicated "AI Assistant" tab with a chat interface.
- **Model Management:** Allow the user to select from available local models.

### Technical Constraints
- **Local Only:** No data must be sent to cloud LLM providers.
- **Performance:** UI must handle streaming responses efficiently without blocking the main thread.
- **Dependency:** Requires Ollama to be installed on the user's machine (we will guide, not bundle).

### User Experience
- User clicks "AI Assistant" in the sidebar.
- If Ollama is not detected, show a helpful "Install Ollama" guide.
- If detected, show a chat interface.
- User can select a model (e.g., `llama3`, `mistral`) from a dropdown.
- User types a query (e.g., "How do I check disk space on Linux?").
- AI responds in real-time (streaming).

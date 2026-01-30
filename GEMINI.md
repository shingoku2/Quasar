# Project Titan: Gemini Context

This file provides persistent context for the Gemini CLI agent to ensure a smooth transition between sessions.

## Current Project Status
- **Framework:** Tauri v2 + React + TypeScript + Tailwind CSS v4.
- **Status:** **Module B Complete**.
- **Last Action:** Successfully implemented the "Remote Connection Manager" with native embedded terminals (xterm.js + russh), tabbed navigation, split-view mode, and credential prompting for secure connections.

## Progress Summary (2026-01-30)
1.  **Terminal Foundation:** Integrated `xterm.js` and connected it to a Rust-based `russh` backend via async channels.
2.  **Session Management:** Built `SessionContainer` for dynamic tab management and split-view capability.
3.  **UI/UX:** Implemented `SessionToolbar` for real-time latency/bandwidth monitoring and `CredentialPrompt` for secure password entry.
4.  **Backend Refactor:** Optimized SSH connection handling to support window resizing and prevent stalls during heavy output.

## Environment Status
- **Rust/Cargo:** **Functional**. (v1.93.0). `russh` 0.57 integrated.
- **Node.js:** Functional.
- **Dependencies:** `xterm.js`, `lucide-react`, `russh`, `russh-keys`, `tokio` (full features).

## Next Steps / Pending Tasks
- **Module D: Automation Workflow Editor:** Implement the React Flow canvas for visual playbooks.
- **Active Scanning:** Integrate `nmap` sidecar for more aggressive network discovery.
- **Native RDP:** Currently using placeholders; integrate `ironrdp` for real RDP sessions.

## Resume Instructions
1.  **Launch App:** `cmd /c "set PATH=%USERPROFILE%\.cargo\bin;%PATH% && npm run tauri dev"` to see the new terminal manager.
2.  **New Track:** Run `/conductor:setup` or `/conductor:implement` to define and start the next module (e.g., Automation Canvas).

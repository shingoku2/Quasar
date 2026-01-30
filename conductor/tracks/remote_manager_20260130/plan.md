# Implementation Plan: Remote Connection Manager (Module B)

This plan outlines the steps to implement embedded SSH/RDP/VNC sessions with a hybrid tabbed and split-pane interface.

## Phase 1: Terminal Foundation & SSH Integration
Implement the core terminal emulator component and connect it to the Rust SSH backend.

- [x] Task: Create `TerminalComponent` using `xterm.js` and `lucide-react`. 778170c
- [x] Task: Implement Tauri commands for SSH session lifecycle (connect, write, resize, disconnect). 3adba54
- [x] Task: Connect `TerminalComponent` to the Rust `russh` backend via async streams. 6bf5d43
- [x] Task: Conductor - User Manual Verification 'Phase 1: Terminal Foundation' (Protocol in workflow.md) [checkpoint: 0406ca4]

## Phase 2: Hybrid Session Management UI
Build the tabbed and split-pane container to manage multiple active sessions.

- [x] Task: Implement `SessionContainer` with support for dynamic tabs. 02bc21f
- [~] Task: Add "Split View" capability to `SessionContainer` (side-by-side or grid layout).
- [ ] Task: Update `App.tsx` to route remote connection requests to the new `SessionContainer`.
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Session Management' (Protocol in workflow.md)

## Phase 3: Session Toolbar & Diagnostics
Add the interactive toolbar for active sessions and real-time health monitoring.

- [ ] Task: Implement `SessionToolbar` component with Clipboard Sync toggle.
- [ ] Task: Implement real-time latency and bandwidth tracking in the Rust backend.
- [ ] Task: Connect diagnostic data to the `SessionToolbar` UI using Tauri events.
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Toolbar & Diagnostics' (Protocol in workflow.md)

## Phase 4: Integration & System Launcher Migration
Migrate existing host management to use the new embedded manager instead of external launchers.

- [ ] Task: Update `HostList` and `Discovery` components to trigger embedded sessions.
- [ ] Task: Refactor `src-tauri/src/launcher.rs` to support internal session handling alongside external fallbacks.
- [ ] Task: Final end-to-end testing and performance verification.
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Final Integration' (Protocol in workflow.md)

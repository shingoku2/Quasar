# Specification: Remote Connection Manager (Module B)

## Overview
This track focuses on transitioning from external system-level launchers to a native, embedded remote access experience within Project Titan. It implements the "Remote Connection Manager" using a hybrid interface that supports both tabbed navigation and split-pane viewing for SSH, RDP, and VNC sessions.

## Functional Requirements
1.  **Embedded Terminals (SSH):**
    *   Integrate `xterm.js` to provide high-performance, native terminal emulation for SSH connections.
    *   Support full ANSI color schemes and keyboard interactions.
2.  **Remote Desktop Streams (RDP/VNC):**
    *   Implement a canvas-based container for `ironrdp` and `rust-vnc` streams.
    *   Initial version may include high-fidelity placeholders or basic rendering as the backend crates are integrated.
3.  **Hybrid Session UI:**
    *   **Tabbed Navigation:** Manage multiple sessions in a standard top-bar tabbed interface.
    *   **Split-Pane Mode:** Ability to "tile" or split the viewport to view multiple active sessions (e.g., two side-by-side terminals) simultaneously.
4.  **Session Toolbar:**
    *   **Clipboard Sync Toggle:** A persistent control to enable or disable shared clipboard between the host and remote system.
    *   **Connection Diagnostics:** Real-time display of session health metrics, including latency (ping) and data throughput.

## Non-Functional Requirements
*   **Performance:** Terminal latency should be < 50ms for local network connections.
*   **Security:** Credentials must be pulled directly from the encrypted vault and never exposed in the UI or logs.
*   **Stability:** A crash in one session (e.g., a terminal disconnect) must not affect other active tabs or the main application process.

## Acceptance Criteria
- [ ] Users can launch an SSH session that opens in a new tab within the application.
- [ ] Users can toggle between "Tabbed" and "Split" view modes for active sessions.
- [ ] The Session Toolbar displays real-time latency for active connections.
- [ ] Clipboard synchronization can be successfully toggled on/off.
- [ ] Disconnecting a session correctly cleans up UI resources and closes the specific tab/pane.

## Out of Scope
*   Advanced terminal features like session recording or multi-exec (simultaneous typing to multiple terminals) for this initial phase.
*   Complex RDP features like RemoteFX or Multi-monitor support.

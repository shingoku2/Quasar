# Project Titan: Gemini Context

This file provides persistent context for the Gemini CLI agent to ensure a smooth transition between sessions.

## Current Project Status
- **Framework:** Tauri v2 + React + TypeScript + Tailwind CSS v4.
- **Status:** **Monitoring & Alerts Track Complete**. Starting Automation Canvas.
- **Last Action:** Fixed real-time metrics display in DashboardView. Now beginning Automation Canvas track.

## Progress Summary (2026-01-30)

### Completed Tracks

1. **Module B: Remote Connection Manager** - COMPLETE
   - Integrated `xterm.js` with Rust-based `russh` backend
   - Built `SessionContainer` for dynamic tab management and split-view
   - Implemented `SessionToolbar` for real-time latency/bandwidth monitoring
   - Added `CredentialPrompt` for secure password entry

2. **Asset Discovery & Monitoring Track** - COMPLETE
   - **Phase 1:** LAN Scanner Backend (Rust)
     - ICMP ping scanner with concurrent execution (50 parallel)
     - TCP port scanner for common services (22, 80, 443, 445, 3389)
     - Tauri commands: `scan_network`, `stop_scan`, `get_scan_progress`
     - Real-time progress and result emitters
   - **Phase 2:** Discovery UI & Integration
     - `NetworkScanner` component with start/stop controls
     - Integrated discovery with mDNS and network scan tabs
     - Host suggestion dialog for newly discovered devices
   - **Phase 3:** Pre-flight Health Checks
     - `HealthCheckBadge` component for real-time host status
     - `PreflightDialog` for pre-connection health metrics
     - SSH-based health check infrastructure
   - **Phase 4:** Dashboard Integration
     - `DiscoveryWidget` for recent discoveries
     - `NetworkMapWidget` for visual network topology
     - Auto-refresh mechanism for health indicators

3. **Monitoring & Alerts Track** - COMPLETE
   - **Phase 1:** System Metrics Collection (Rust backend with sysinfo)
   - **Phase 2:** Alert Engine with threshold evaluation
   - **Phase 3:** AlertFeed component with real-time updates
   - **Phase 4:** AlertRules UI for managing alert configurations
   - Live metrics in DashboardView (CPU, Memory, Disk I/O, Network)

### Active Track

4. **Automation Canvas Track** - IN PROGRESS
   - Visual workflow builder with drag-and-drop nodes
   - Support for triggers (scheduled, webhook, manual)
   - Action nodes for SSH command execution, file transfer, notifications
   - Canvas.tsx component for building workflows
   - Workflow engine in Rust to execute automation flows
- **Rust/Cargo:** **Functional**. (v1.93.0). 
  - `russh` 0.57 - SSH client library
  - `surge-ping` 0.7 - ICMP ping for network scanning
  - `cidr-utils` 0.5 - CIDR notation parsing
  - `tokio` 1.49 - Async runtime with full features
- **Node.js:** Functional.
- **Dependencies:** `xterm.js`, `lucide-react`, `recharts`, `@tauri-apps/api`, `@tauri-apps/plugin-sql`

## Next Steps / Pending Tasks
- **Module D: Automation Workflow Editor:** Implement the React Flow canvas for visual playbooks.
- **Native RDP:** Currently using external launcher; integrate `ironrdp` for embedded RDP sessions.
- **Monitoring & Alerting:** Real-time system monitoring with configurable thresholds and notifications.

## Resume Instructions
1. **Launch App:** `npm run tauri dev` to see the dashboard with discovery widgets.
2. **New Track:** Run `/conductor:setup` or `/conductor:implement` to define and start the next module (e.g., Automation Canvas).

## Key Commands
- `cargo test --lib` - Run Rust unit tests
- `npm test` - Run React component tests
- `npm run tauri dev` - Start development server
- `cargo tauri build` - Build production release

## Project Structure
- `src-tauri/src/` - Rust backend (scanner, health, ssh, discovery modules)
- `src/components/` - React components (dashboard, discovery, health, session management)
- `conductor/tracks/` - Track documentation and implementation plans

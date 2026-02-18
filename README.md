# Quasar

A powerful Tauri-based remote infrastructure management application for monitoring, managing, and automating remote servers and infrastructure.

## Overview

Quasar provides a comprehensive desktop application for managing remote infrastructure with features including SSH/SFTP connectivity, real-time monitoring, workflow automation, and secure credential management.

## Features

### 🔐 Security & Credential Management
- **Encrypted Vault** - Master password-protected credential storage with AES-256-GCM encryption
- **Credential Manager** - Store and manage SSH, RDP, database, and API credentials
- **SSH Host Key Verification** - MITM attack prevention with known hosts management
- **Auto-lock** - Configurable vault timeout with automatic locking
- **Audit Logging** - Complete security event tracking and monitoring

### 🖥️ Remote Management
- **SSH Terminal** - Full-featured SSH terminal with xterm.js integration
- **SFTP File Transfer** - Upload and download files with progress tracking
- **Quick Connect** - One-click connection to saved hosts
- **Session Management** - Multiple concurrent SSH sessions
- **Real Command Execution** - Execute commands and scripts on remote hosts
- **Scheduled Tasks** - Cron-based automation: run **SSH commands** or **SFTP upload/download** on saved hosts; task types (SSH command, Upload file, Download file) with optional local/remote paths for file transfer; view last run status (success/failure, output); **Run now** for manual execution

### 📊 Monitoring & Health Checks
- **Real-time Metrics** - CPU, memory, disk usage, and system load monitoring
- **Health Checks** - Pre-flight ping and SSH validation before connections
- **System Monitor** - Live dashboard with resource utilization graphs
- **Alert System** - Configurable alerts for system thresholds
- **Historical Data** - Track metrics over time with SQLite storage

### 🔍 Network Discovery & Scanning
- **Network Scanner** - Comprehensive CIDR-based network discovery with 13-port scanning
- **Device Detection** - Automatic classification (server, router, printer, workstation)
- **Service Identification** - Detect running services (SSH, HTTP, MySQL, RDP, etc.)
- **Network Topology** - Interactive force-directed graph visualization
- **Host Tracking** - Persistent storage of discovered hosts with history
- **Hostname Resolution** - Automatic reverse DNS lookup for discovered hosts

### 🎨 Modern UI
- **React + TypeScript** - Type-safe frontend with modern React patterns
- **Tailwind CSS** - Beautiful, responsive design with shadcn/ui components
- **Dark Navy/Cyan Theme** - Professional dark theme with custom Quasar branding
- **Dashboard Layout** - Hero topology view with real-time metrics, active sessions, and system health cards
- **Status Bar** - Connection status and scan information footer
- **Real-time Updates** - Live data updates without page refreshes

## Architecture

### Frontend
- **Framework**: React 19 + TypeScript + Vite
- **UI Library**: Tailwind CSS + Lucide icons
- **Terminal**: xterm.js for SSH terminal emulation
- **State Management**: React Context + hooks
- **Testing**: Vitest + React Testing Library

### Backend
- **Runtime**: Tauri (Rust)
- **Database**: SQLite with rusqlite
- **SSH/SFTP**: russh + russh-sftp
- **Encryption**: AES-256-GCM with Argon2id key derivation
- **Security**: zeroize for secure memory clearing

### Database Schema
See `docs/SCHEMA.md` for the full consolidated schema. Main tables:
- `hosts` - Remote host configurations
- `credentials` - Encrypted credential storage
- `vault_settings` - Master password and vault configuration
- `ssh_known_hosts` - SSH host key fingerprints
- `security_audit_log` - Security event tracking
- `scheduled_tasks` - Cron tasks (SSH command or SFTP upload/download)
- Monitoring/alert and discovery tables (see migrations 004, 006, 007)

## Getting Started

### Prerequisites
- Node.js 18+ and npm
- Rust 1.70+ and Cargo
- Windows, macOS, or Linux

### Installation

1. Clone the repository:
```bash
git clone https://github.com/yourusername/Quasar.git
cd Quasar
```

2. Install frontend dependencies:
```bash
npm install
```

3. Install Rust dependencies (handled by Cargo):
```bash
cd src-tauri
cargo build
```

### Development

Run the application in development mode:
```bash
npm run tauri dev
```

This runs `scripts/tauri-dev.js`, which sets `CARGO_TARGET_DIR` to the project’s `src-tauri/target`, then starts `npx tauri dev`. The script checks that `npx` is available before spawning and prints clear errors if Node/npm are missing.

You get:
- Vite dev server for hot module reloading
- Tauri backend compile and run
- Application window
### Building

Build the application for production:
```bash
npm run tauri build
```

This creates platform-specific installers in `src-tauri/target/release/bundle/`.

## Usage

### First Launch
1. Initialize the vault with a strong master password
2. Add your first remote host (hostname, port, username)
3. Add credentials to the vault or use manual password entry
4. Connect to your remote host

### Managing Credentials
1. Navigate to Security → Credentials
2. Add credentials with name, type, username, and password
3. Optionally associate credentials with specific hosts
4. Use credentials for quick SSH connections

### Scheduled Tasks (Automation)
1. Navigate to **Automation** (sidebar)
2. Add a task: choose **Task type** (SSH command, Upload file, or Download file), name, cron schedule (6-field e.g. `0 0 9 * * *` for daily 9:00), host, optional credential
3. For SSH: enter the command. For Upload/Download: enter local path and remote path
4. Tasks run automatically when due (scheduler checks every 60s)
5. Use **Run now** to execute a task immediately and see output/error in the result panel
6. View last run status (Success / Failed) and output on each task card

### Monitoring
1. View real-time metrics on the Dashboard
2. Configure alerts in Monitoring → Alerts
3. View historical data and trends
4. Set up health checks for critical hosts

## Project Structure

```
Quasar/
├── src/                          # React frontend
│   ├── components/               # React components
│   │   ├── dashboard/           # Dashboard widgets
│   │   ├── vault/               # Security & credential components
│   │   └── ...                  # Other components
│   ├── hooks/                   # Custom React hooks
│   ├── lib/                     # Utility functions
│   └── main.tsx                 # Application entry point
├── src-tauri/                   # Rust backend
│   ├── src/
│   │   ├── vault/               # Credential vault module
│   │   ├── ssh_exec.rs          # SSH command execution
│   │   ├── sftp.rs              # SFTP file transfer
│   │   ├── scheduler.rs         # Cron-based scheduled tasks
│   │   ├── health.rs            # Health check system
│   │   └── lib.rs               # Main Tauri application
│   ├── migrations/              # SQLite database migrations
│   └── Cargo.toml               # Rust dependencies
├── conductor/                   # Project documentation
│   ├── tracks/                  # Feature development tracks
│   └── code_styleguides/        # Code style guidelines
└── package.json                 # Node.js dependencies
```

## Development Guidelines

See `conductor/code_styleguides/` for detailed coding standards:
- `general.md` - General development practices
- `javascript.md` - TypeScript/React guidelines
- `html-css.md` - UI/styling guidelines

## Security

- All credentials are encrypted at rest with AES-256-GCM
- Master password uses Argon2id key derivation (memory-hard)
- SSH host keys are verified to prevent MITM attacks
- Vault auto-locks after configurable timeout
- Complete audit trail of all security operations
- Master key is securely wiped from memory on vault lock

## Contributing

1. Create a feature branch from `main`
2. Follow the code style guidelines in `conductor/code_styleguides/`
3. Write tests for new features
4. Update documentation as needed
5. Submit a pull request

## Recent Updates

### February 18, 2026 - Documentation & Script Fixes
- ✅ **tauri-dev.js** — Verifies `npx` is available before spawn (Windows: `where npx`; Unix: `command -v npx`); improved error messages if spawn fails
- ✅ **cross-env** — Removed unused devDependency; env is set directly in the script
- ✅ **Codebase audit** — Fixes applied for monitoring/scanner Mutex poison recovery, Discovery/MonitoringView listener cleanup, AGENTS.md/MVP roadmap doc updates, CredentialManager error helper, SCHEMA.md migration note; see `CODEBASE_AUDIT_REPORT.md`

### February 18, 2026 - Workflow Scheduling & Task Results
- ✅ **Scheduled tasks (cron)** — Automation view: create/edit/delete tasks; **task types**: SSH command, SFTP upload, SFTP download (with local/remote paths); background scheduler runs due tasks every 60s; supports password and SSH key auth
- ✅ **Last run result** — Each task stores and displays last run time, status (Success / Failed), error message, and truncated output (migrations 010/011)
- ✅ **Run now** — Manual run from UI with result panel (success/failure, output/error)
- ✅ **Cron format** — UI uses 6-field (sec min hour day month dow), e.g. `0 0 9 * * *` for 9:00 daily; see `docs/CORE_WORKFLOWS.md`
- ✅ **Scheduler tests** — Integration tests for CRUD, set_run_result, load_enabled_tasks, load_task_by_id, output truncation (`cargo test scheduler::`)
- ✅ **Documentation** — `docs/CORE_WORKFLOWS.md` covers vault, SSH, scheduled tasks (including SFTP types), SFTP, monitoring, discovery

### February 18, 2026 - SSH Terminal & Dashboard Fixes
- ✅ **SSH terminal hang fixed** — Refactored to use `Channel::wait()` for receiving data so russh’s internal buffer is drained and window adjustments keep data flowing; consecutive/simultaneous commands no longer stall
- ✅ **Output batching** — Flush on `\r`/`\n` and 4 ms interval for responsive apt progress and line output
- ✅ **SSH packet size** — Set to 32 KB (≤ TCP max); removed russh packet-size errors
- ✅ **Dashboard System Health** — Hosts Online count from remote health; Vault Auto-lock from settings; removed non-functional three-dots menus
- ✅ **Real-time Metrics** — All attached disks shown (per-disk usage from backend)
- ✅ **Password inputs** — `autoComplete` on all password fields (console warning resolved)
- ✅ **tauri-dev.js** — Use shell on Windows to avoid spawn EINVAL

### February 16, 2026 - Credential Edit & Type-Switch Fixes
- ✅ **update_credential** supports SSH key credentials: `key_path`, `private_key`, `key_passphrase` sent and persisted; empty string clears fields
- ✅ **credential_type** column updated on edit so DB stays in sync with UI
- ✅ Clearing key fields when editing works (send empty string; backend clears to NULL)
- ✅ Switching type clears opposite auth: password→ssh_key clears password columns; ssh_key→password clears key columns
- ✅ **Migration 009:** `encrypted_password`, `nonce`, `tag` nullable; empty password now clears to NULL (no stale encrypted data)
- ✅ All vault credential tests passing

### February 12, 2026 - UI Refactor
- ✅ Complete visual overhaul to dark navy/cyan aesthetic
- ✅ Custom Quasar SVG logo and sidebar restyle with vault status badge
- ✅ Dashboard restructured: hero topology card + 4-card bottom row
- ✅ SystemHealthWidget supports metrics and summary variants
- ✅ Topology view restyled with new device color palette
- ✅ Status bar footer with connection indicator
- ✅ All 69 tests passing with updated mocks and assertions

### February 3, 2026 - Critical Bug Fixes
- ✅ Fixed React duplicate key warnings in NetworkScanner component
- ✅ Fixed database migration system for fresh installations
- ✅ Created initial schema migration (001) for base tables
- ✅ Re-enabled migration 005 for proper credentials consolidation
- ✅ Cleaned up unused imports to reduce compiler warnings

### February 2, 2026 - Network Scanner Enhancement
- ✅ Comprehensive network discovery with 13-port scanning
- ✅ Device type detection and service identification
- ✅ Interactive network topology visualization
- ✅ Persistent host tracking with history

## Documentation

- **README.md** (this file) — Overview, features, getting started, usage
- **AGENTS.md** — AI agent context, implementation history, and bug-fix log
- **docs/CORE_WORKFLOWS.md** — Core user workflows (vault, SSH, scheduled tasks including SFTP types, SFTP, monitoring, discovery)
- **docs/SCHEMA.md** — Database schema and migration notes
- **MVP_COMPLETION_ROADMAP.md** — Development roadmap and success criteria
- **CODEBASE_AUDIT_REPORT.md** — Audit findings and applied fixes
- **conductor/** — Product guidelines and feature tracks
- **MONITORING_COMPLETE.md** — Monitoring system documentation

## Tech Stack

**Frontend**: React, TypeScript, Vite, Tailwind CSS, shadcn/ui, xterm.js  
**Backend**: Rust, Tauri, russh, russh-sftp, rusqlite  
**Security**: AES-256-GCM, Argon2id, zeroize  
**Database**: SQLite

## License

[Your License Here]

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/)
- [Tauri Extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

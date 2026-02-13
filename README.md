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
- **Framework**: React 18 + TypeScript + Vite
- **UI Library**: shadcn/ui + Tailwind CSS + Lucide icons
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
- `hosts` - Remote host configurations
- `credentials` - Encrypted credential storage
- `vault_settings` - Master password and vault configuration
- `ssh_known_hosts` - SSH host key fingerprints
- `security_audit_log` - Security event tracking
- `monitoring_metrics` - System metrics history
- `monitoring_alerts` - Alert configurations
- `discovered_hosts` - Network scan results and host tracking
- `host_services` - Detected services per discovered host
- `workflows` - Automation workflow definitions
- `workflow_executions` - Workflow execution history

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

This will:
- Start the Vite dev server for hot module reloading
- Compile and run the Tauri backend
- Open the application window

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

### Creating Workflows
1. Navigate to Automation → Workflows
2. Create a new workflow with the visual builder
3. Add actions: SSH commands, file transfers, health checks
4. Add conditional logic based on command output
5. Save and execute the workflow

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

- `AGENTS.md` - AI agent context and implementation history
- `conductor/` - Product guidelines and feature tracks
- `MVP_COMPLETION_ROADMAP.md` - Development roadmap
- `MONITORING_COMPLETE.md` - Monitoring system documentation

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

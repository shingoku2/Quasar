# Technology Stack

## Core Architecture
- **Framework:** Tauri v2 (Desktop Application)
- **Architecture Pattern:** "Sidecar" pattern for isolating GPL/AGPL components and handling heavy tasks.

## Backend (Core Logic)
- **Language:** Rust
- **Key Crates:**
    - `tauri`: IPC and window management.
    - `ironrdp`: Native RDP implementation (Planned).
    - `russh`: Native SSH implementation (Planned).
    - `rust-vnc`: Native VNC implementation.
    - `argon2`: Password hashing (Argon2id).
    - `aes-gcm`: AES-256-GCM encryption.
    - `tauri-plugin-sql`: SQLite database interaction.
    - `mdns-sd`: Zero-config LAN discovery.
    - `ollama-rs`: Ollama REST API interaction.
    - `tokio-stream`: Async stream utilities for AI chat.

## Frontend (User Interface)
- Framework: React
- Language: TypeScript
- Styling: Tailwind CSS
- Component Library: Lucide React (Icons), Recharts (Data Viz)
- Utilities: clsx, tailwind-merge (Dynamic styling)
- Terminal Emulator: xterm.js

## Data & Storage
- **Database:** SQLite (WAL mode)
- **Schema Management:** Automated migrations on startup.
- **Encryption:** Sensitive fields (passwords, keys) encrypted at rest using libsodium.

## External Tools & Sidecars
- **AI/LLM:** Ollama (Local inference, optional sidecar)
- **Network Scanning:** nmap (Sidecar subprocess)
- **System Monitoring:** Glances (Sidecar agent)

## Infrastructure & Build
- **Build Tool:** Cargo (Rust), Vite (Frontend)
- **Package Manager:** npm / pnpm
- **CI/CD:** GitHub Actions
- **Distribution:** MSI (Windows), DMG (macOS), Deb/AppImage (Linux)

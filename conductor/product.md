# Initial Concept

Project Titan is a unified, open-source IT operations console designed for sysadmins. It aims to unify remote access (RDP, SSH, VNC), monitoring, automation, and local AI diagnostics into a single, high-performance desktop application built with Tauri and Rust. The core focus is on solving "tab explosion," providing transparent and secure remote access, and integrating local AI for log analysis and automation without cloud data leakage.

## Target Audience
- System Administrators and IT Operations Professionals
- DevOps and SRE (Site Reliability Engineering) Teams
- Network Engineers and Security Analysts
- Everyone from professionals to home lab geeks

## Core Goals (v0.1 MVP)
- **Unified Interface:** Solve "tab explosion" with a cross-platform tabbed interface for RDP, SSH, and VNC.
- **Automated Discovery:** Provide zero-config LAN discovery and automated asset inventory management.
- **Performance:** Deliver high performance (<2s startup) and low resource usage (<100MB RAM idle) using Tauri and Rust.
- **Security:** Implement a secure credential vault with Argon2id key derivation and libsodium encryption.
- **Sidecar Monitoring:** Enable pre-flight health checks (CPU/RAM) without full connection using sidecar agents.

## Key Features
- **Tabbed Connection Manager:** Manage RDP, SSH, VNC, and web sessions in a single window with shared context.
- **Asset Inventory:** Active inventory populated by auto-discovery (mDNS, nmap) and manual entry.
- **Local AI Integration:** (Planned for Phase 2) Local Ollama inference for log analysis and playbook generation, ensuring privacy.
- **Sidecar Architecture:** Isolate GPL/AGPL components to prevent license contamination and ensure stability.
- **Portable Distribution:** Single-binary distribution with no runtime dependencies.

## Success Metrics (v0.1)
- App startup time < 2 seconds.
- LAN scan (/24) < 10 seconds.
- RAM usage (idle) < 100 MB.
- 90% feature parity across Windows, macOS, and Linux.
# Implementation Plan - MVP Core

## Phase 1: Foundation & Scaffold [checkpoint: e384873]
- [x] Task: Initialize Tauri v2 project with React/TS/Tailwind
    - [x] Run `npm create tauri-app@latest`
    - [x] Configure Tailwind CSS
    - [x] Verify build and window launch [e539bb8]
- [x] Task: Database & Security Initialization [checkpoint: f7a7ff3]
    - [x] Implement SQLite schema for `hosts` and `credentials` [aae7d76]
    - [x] Integrate `libsodium` for master password hashing (Argon2id) [77fe29d]
    - [x] Implement secure credential vault (AES-256-GCM) [59496ed]
- [ ] Task: Conductor - User Manual Verification 'Foundation & Scaffold' (Protocol in workflow.md)

## Phase 2: Core UI & Navigation
- [x] Task: Implement Main Dashboard & Tabbed UI [ba23785]
    - [x] Create layout with sidebar and main tab area
    - [x] Implement tab management (Add/Remove/Switch)
- [x] Task: Host Management UI [e60eb1f]
    - [x] Create host list view with filtering
    - [x] Implement "Add Host" dialog
- [ ] Task: Conductor - User Manual Verification 'Core UI & Navigation' (Protocol in workflow.md)

## Phase 3: Connectivity & Discovery
- [ ] Task: System Protocol Launchers
    - [ ] Implement SSH launch via system terminal
    - [ ] Implement RDP launch via system `mstsc` (Windows) or equivalent
- [ ] Task: LAN Discovery Module
    - [ ] Integrate `nmap` sidecar for scanning
    - [ ] Implement passive discovery via `mdns-sd`
- [ ] Task: Conductor - User Manual Verification 'Connectivity & Discovery' (Protocol in workflow.md)

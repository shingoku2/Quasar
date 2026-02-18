# Track: Create Quasar MVP with Core Remote Access and Discovery

## Specification
This track implements the foundational elements of Quasar, as defined in the Phase 1 blueprint.

### Scope
- **Application Scaffold:** Tauri v2 project setup with Rust backend and React/TypeScript/Tailwind frontend.
- **Data Foundation:** SQLite database initialization with schema for hosts, credentials, and logs.
- **Security:** Implementation of the master password and credential vault using libsodium.
- **Unified UI:** Tabbed interface for managing multiple remote sessions.
- **Core Connectivity:** Ability to launch SSH and RDP sessions (system-level integration initially).
- **Discovery:** Basic LAN scanning using nmap/mDNS to populate the host inventory.

### Technical Constraints
- No cloud dependencies.
- No AI features in this track (reserved for Phase 2).
- Focus on stability and performance for local execution.

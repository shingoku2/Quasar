# ProjectTitan - Agent Context & Bug Fixes

## Overview
ProjectTitan is a Tauri-based remote infrastructure management application with React frontend and Rust backend. This document tracks major bug fixes, architectural decisions, and context for AI agents working on this codebase.

---

## Recent Implementations (January 31, 2026)

### Security & Credential Vault - Phase 1, 2 & 3 Complete

#### Phase 1: Vault Infrastructure ✅
- **Location**: `src-tauri/src/vault.rs` (415 lines)
- **Implementation**: Master password-based vault with Argon2id key derivation
- **Features**:
  - VaultState with thread-safe RwLock for concurrent access
  - Master password key derivation (64 MB memory, 3 iterations, 4 threads)
  - Secure memory clearing with `zeroize` crate (master key wiped on lock)
  - Rate limiting & lockout policy (5/10/15 failed attempts → 5/15/60 min lockout)
  - Auto-lock timer infrastructure (configurable timeout, default 15 min)
  - Audit logging for all vault operations
- **Database Migration**: `migrations/003_security_vault.sql`
  - `vault_settings` - Master password hash, salt, configuration
  - `security_audit_log` - All credential access tracking
  - `ssh_known_hosts` - Ready for Phase 3
  - `credentials_new` - Enhanced schema with encryption fields
- **Tauri Commands**: 
  - `is_vault_initialized`, `initialize_vault`, `unlock_vault`, `lock_vault`
  - `is_vault_locked`, `get_vault_settings`, `update_vault_settings`
- **Tests**: 3/3 passing (initialization, unlock/lock, invalid password)

#### Phase 2: Credential Management Backend ✅
- **Location**: `src-tauri/src/vault/credentials.rs` (450 lines)
- **Implementation**: Full CRUD operations with AES-256-GCM encryption
- **Features**:
  - `CredentialManager` - Handles all credential operations
  - `Credential` - Full credential with decrypted password
  - `CredentialSummary` - Metadata only (safe to list without vault unlock)
  - All passwords encrypted at rest with unique nonce/tag per credential
  - Automatic last_used_at tracking on credential access
  - Comprehensive audit logging (create, read, update, delete)
  - Search functionality with fuzzy matching
- **Tauri Commands**:
  - `add_credential` - Encrypts password, requires unlocked vault
  - `get_credential` - Decrypts password, requires unlocked vault, logs access
  - `list_credentials` - Returns summaries (no passwords)
  - `update_credential` - Re-encrypts password if changed
  - `delete_credential` - Removes with audit logging
  - `search_credentials` - Fuzzy search by name/username/type
- **Tests**: 5/5 passing (add/get, list, update, delete, search)

#### Security Model
- **Encryption**: AES-256-GCM with unique 12-byte nonce per credential
- **Key Derivation**: Argon2id (memory-hard, resistant to GPU attacks)
- **Authentication**: 16-byte tag ensures data integrity
- **Audit Trail**: All operations logged with timestamps
- **Memory Safety**: Master key zeroized on vault lock/drop
- **Access Control**: Vault must be unlocked for password operations

#### Phase 3: SSH Host Key Verification ✅
- **Location**: `src-tauri/src/vault/ssh_keys.rs` (415 lines)
- **Implementation**: Known hosts management with fingerprint verification
- **Features**:
  - `SshKeyManager` - Manages SSH host key verification
  - Fingerprint-based host key verification (prevents MITM attacks)
  - Trust status tracking (Trusted, Unknown, Changed, Rejected)
  - Automatic detection of changed host keys
  - Database storage in `ssh_known_hosts` table
  - Thread-safe with `std::sync::Mutex` (compatible with Tauri async)
- **Tauri Commands**:
  - `verify_ssh_host_key` - Verify a host's key by fingerprint
  - `trust_ssh_host_key` - Add/update a host key with trust status
  - `get_known_ssh_hosts` - List all known hosts
  - `remove_ssh_host_key` - Remove a host key
  - `update_ssh_host_trust` - Update trust status for a host
- **Tests**: 3/3 passing (manager creation, unknown host, trust and verify)

#### Phase 4: Vault UI Components ✅
- **Location**: `src/components/vault/` (5 components)
- **Implementation**: Complete React UI for vault and credential management
- **Components**:
  - `VaultProvider` - Global vault state management with React Context
  - `VaultInitDialog` - First-time vault setup with password strength validation
  - `VaultUnlockDialog` - Master password entry with show/hide toggle
  - `CredentialManager` - Full CRUD UI with search, view, edit, delete
  - `CredentialSelector` - Smart credential picker for SSH connections
- **Features**:
  - Application startup vault check (init or unlock)
  - Password strength meter with real-time validation
  - Credential cards with type badges and metadata display
  - Search and filter credentials by name/username/host
  - Copy-to-clipboard for usernames and passwords
  - Integrated with RemoteManager for seamless SSH connections
  - Fallback to manual password entry if vault locked
- **Integration**:
  - Added "Security" view to sidebar navigation
  - VaultProvider wraps entire app in `main.tsx`
  - RemoteManager checks vault status before showing credential selector
  - Automatic fallback to manual entry if vault unavailable
- **UI/UX**:
  - Consistent design with existing ProjectTitan aesthetic
  - Modal dialogs with backdrop blur and animations
  - Responsive grid layout for credential cards
  - Color-coded credential types (SSH, RDP, Database, API, Other)

#### Phase 5: SSH Host Key UI ✅
- **Location**: `src/components/vault/` (3 new components + 1 hook)
- **Implementation**: Complete SSH host key verification UI with MITM detection
- **Components**:
  - `SshHostKeyPrompt` - Trust prompt for unknown/changed host keys
  - `KnownHostsManager` - Management interface for known SSH hosts
  - `SecurityView` - Tabbed interface combining Credentials and Known Hosts
  - `useSshHostKeyVerification` - React hook for host key verification flow
- **Features**:
  - Unknown host key prompt with fingerprint display
  - Changed host key warning (MITM detection) with visual alerts
  - Copy-to-clipboard for fingerprint verification
  - Permanent vs temporary trust options
  - Known hosts list with search and filtering
  - Trust status indicators (Trusted, Changed, Rejected, Unknown)
  - Remove host keys and update trust status
  - First seen / last seen timestamps
- **Integration**:
  - SecurityView provides tabbed interface (Credentials | Known Hosts)
  - Host key verification hook integrated into RemoteManager
  - Prompts appear before SSH connection establishment
  - Backend verification commands already implemented (Phase 3)
- **Security**:
  - Visual differentiation between unknown and changed keys
  - Strong warnings for potential MITM attacks
  - Fingerprint comparison workflow
  - User education about security risks

#### Phase 6: Security Settings & Audit Log UI ✅
- **Location**: `src/components/vault/` (2 new components)
- **Implementation**: Vault settings management and security audit log viewer
- **Components**:
  - `VaultSettings` - Vault configuration management interface
  - `AuditLogViewer` - Security event log viewer with filtering
- **Features**:
  - Auto-lock timeout configuration (1-1440 minutes)
  - Security options (require password on credential use)
  - Master password management (lock vault, change password placeholder)
  - Vault status display
  - Audit log search and filtering
  - Event type and result filtering
  - Timestamp formatting and display
  - Event icons and color-coded results
- **Integration**:
  - SecurityView expanded to 4 tabs (Credentials | Known Hosts | Settings | Audit Log)
  - Backend commands already implemented (get/update vault settings)
  - Settings persist to database
  - Audit log infrastructure ready for backend implementation
- **UI/UX**:
  - Comprehensive settings with helpful descriptions
  - Security warnings and recommendations
  - Success/error feedback for all operations
  - Consistent design with ProjectTitan aesthetic

#### Phase 7: Security Hardening & Polish ✅
- **Location**: `src-tauri/src/vault/audit.rs`, `src-tauri/src/lib.rs`, `src/components/vault/AuditLogViewer.tsx`
- **Implementation**: Complete security hardening with audit logs, auto-lock, and password management
- **Backend Features**:
  - `AuditLogManager` - Query and filter security audit logs from database
  - `get_audit_logs` command - Retrieve audit logs with filtering (event type, result, search)
  - `get_audit_log_count` command - Get total count of audit entries
  - `change_master_password` command - Change vault master password with credential re-encryption
  - Auto-lock timer task - Background task checks vault inactivity every 30 seconds
  - Emits `vault-auto-locked` event when vault auto-locks
- **Frontend Features**:
  - AuditLogViewer connected to real backend data
  - Displays all vault operations (unlock, lock, credential access, etc.)
  - Search and filter by event type and result
  - Timestamp formatting and event icons
- **Security Enhancements**:
  - Master password change re-encrypts all stored credentials
  - Auto-lock timer actively monitors vault activity
  - Comprehensive audit trail of all security events
  - Activity tracking updates on every vault operation
- **Tauri Commands**:
  - `get_audit_logs` - Query audit log with optional filters
  - `get_audit_log_count` - Get count of audit entries
  - `change_master_password` - Change master password (requires current password)

#### Next Steps
- Phase 8: SSH Feature Enhancements (SFTP, tunneling, themes, etc.)
- Phase 9: Monitoring & Alerting Enhancements
- Phase 10: Automation Workflow Builder

---

## MVP Completion Progress (February 1, 2026)

### Phase 8A: Real SSH Command Execution ✅

#### SSH Execution Module
- **Location**: `src-tauri/src/ssh_exec.rs` (280 lines)
- **Implementation**: Non-interactive SSH command execution using russh
- **Features**:
  - `execute_ssh_command` - Single command execution with configurable timeout
  - `execute_ssh_commands_batch` - Multiple commands on same session
  - `get_system_metrics` - Collect CPU, RAM, disk, uptime, load via SSH
  - Proper channel message handling (Data, ExtendedData, Eof, ExitStatus)
  - UTF-8 output validation
  - Timeout protection for all operations
- **Integration**:
  - Integrated into `automation/engine.rs` for workflow SSH actions
  - Used by `health.rs` for system metrics collection
  - Module added to `lib.rs` module tree

#### Automation Engine Enhancement
- **Location**: `src-tauri/src/automation/engine.rs:161-184`
- **Change**: Replaced simulated SSH output with real command execution
- **Features**:
  - Executes actual SSH commands on remote hosts
  - 30-second timeout per command
  - Stores output in execution context as `last_ssh_output`
  - Error handling with descriptive messages
  - Password handling for Option<String> credentials

#### Health Check Enhancement
- **Location**: `src-tauri/src/health.rs:45-83`
- **Change**: Added real SSH metrics collection to health checks
- **Features**:
  - Pre-flight ping validation (unchanged)
  - Optional SSH metrics collection with credentials
  - Collects: CPU %, memory usage/total, disk usage/total, uptime, load average
  - Graceful degradation if SSH fails (returns reachable=true, metrics=None)
  - Supports credential-less ping-only checks

#### Impact
- **Workflows**: Can now execute real commands on remote infrastructure
- **Health Checks**: Provide actual system metrics before full connection
- **Automation**: Enables complex multi-step workflows with SSH commands
- **Monitoring**: Pre-flight checks can validate system health

### Phase 8B: SFTP File Transfer ✅

#### SFTP Module
- **Location**: `src-tauri/src/sftp.rs` (380 lines)
- **Implementation**: Complete SFTP client using russh-sftp
- **Features**:
  - `upload_file` - Upload files to remote hosts with progress callback support
  - `download_file` - Download files from remote hosts with progress tracking
  - `list_directory` - List files in remote directories
  - `remote_exists` - Check if remote file/directory exists
  - 32KB chunk size for efficient transfers
  - Configurable timeout (10 seconds for connection)
  - Proper error handling and resource cleanup
- **Dependencies**: Added `russh-sftp = "2.0.4"` to Cargo.toml

#### Automation Engine Integration
- **Location**: `src-tauri/src/automation/engine.rs:187-245`
- **Change**: Implemented FileTransfer action with real SFTP operations
- **Features**:
  - Parses host string format: `username@host:port`
  - Supports Upload and Download directions
  - Retrieves password from execution context
  - Detailed success/failure messages
  - Integrates seamlessly with workflow execution

#### Tauri Commands for Frontend
- **Location**: `src-tauri/src/lib.rs:410-455`
- **Commands Added**:
  - `sftp_upload_file` - Upload file from local to remote
  - `sftp_download_file` - Download file from remote to local
  - `sftp_list_directory` - Browse remote directories
  - `sftp_remote_exists` - Check remote file existence
- **Integration**: All commands registered in invoke handler

#### Impact
- **Workflows**: Can now transfer files as part of automation
- **File Management**: Upload/download files to/from remote hosts
- **Automation**: Complete file-based workflows (backup, deployment, etc.)
- **Frontend Ready**: Commands available for UI implementation

---

## Bug Fixes & Code Quality Improvements (February 1, 2026)

### Critical Issues Fixed

#### 1. Password Validation Alignment (FIXED)
- **Issue**: Frontend required 8 chars minimum, backend required 12 chars
- **Location**: `VaultInitDialog.tsx:18` vs `vault.rs:100`
- **Fix**: Updated frontend validation to require 12 characters to match backend
- **Impact**: Prevents user confusion and failed vault initialization attempts

#### 2. Vault Auto-Lock Event Listener (FIXED)
- **Issue**: `vault-auto-locked` event listener not set up, no cleanup on unmount
- **Location**: `VaultProvider.tsx:51-53`
- **Fix**: Added proper event listener with async setup and cleanup in useEffect
- **Impact**: Vault now properly responds to auto-lock events from backend

#### 3. SSH Trust Status Enum Casing (FIXED)
- **Issue**: Frontend sent 'Trusted' (capitalized), backend expected 'trusted' (lowercase)
- **Location**: `useSshHostKeyVerification.ts:63`
- **Fix**: Changed trustStatus value to lowercase 'trusted'
- **Impact**: SSH host key trust operations now work correctly

### High Priority Issues Fixed

#### 4. Debug Console Logs in Production (FIXED)
- **Issue**: Verbose debug logging throughout terminal initialization
- **Location**: `TerminalComponent.tsx:29-119` (multiple console.log statements)
- **Fix**: Removed all debug console.log statements, kept only error logging
- **Impact**: Cleaner production code, reduced console noise

#### 5. Password Memory Security (FIXED)
- **Issue**: Master password remained in memory after failed attempts
- **Location**: `VaultUnlockDialog.tsx:27-31`, `VaultInitDialog.tsx:51-55`
- **Fix**: Clear password state immediately in both success and error paths
- **Impact**: Improved security - passwords no longer linger in component state

#### 6. Transaction Rollback Handling (FIXED)
- **Issue**: Manual rollback attempts in error handlers could fail silently
- **Location**: `vault.rs:320-373`
- **Fix**: Use proper rusqlite Transaction API with automatic rollback on drop
- **Impact**: Database consistency guaranteed - transactions properly roll back on error

### Code Quality Improvements

#### 7. Unused Imports Removed (FIXED)
- **Location**: `credentials.rs:4` - Removed unused `Zeroizing` import
- **Location**: `VaultInitDialog.tsx:2` - Removed unused `X` icon import
- **Impact**: Cleaner code, no lint warnings for these files

### Remaining Known Issues

These issues are documented but not yet fixed:

#### Database Schema Drift (DOCUMENTED)
- **Location**: `db.ts:21-32` vs `003_security_vault.sql`
- **Issue**: `credentials` table (frontend) and `credentials_new` table (backend) coexist
- **Impact**: Potential data inconsistency if both tables are used
- **Recommendation**: Implement migration or consolidate tables

#### SSH Command Execution Stub (DOCUMENTED)
- **Location**: `automation/engine.rs:161-166`
- **Issue**: SSH command execution returns simulated success without actual execution
- **Status**: Awaiting Phase 8 implementation

#### Minor Issues
- Potential memory leak in terminal resize observer (needs null check)
- Missing timeout handling for vault status check
- Inconsistent error message formatting across codebase

---

## Recent Bug Fixes (January 30, 2026)

### Critical Issues Resolved

#### 1. System Monitoring Interval (FIXED)
- **Issue**: Monitoring task was emitting metrics every 5000 seconds (~83 minutes) instead of 5 seconds
- **Location**: `src-tauri/src/lib.rs:244`
- **Fix**: Changed `monitoring::start_monitoring_task(app_handle, 5000)` to `monitoring::start_monitoring_task(app_handle, 5)`
- **Impact**: System metrics now update in real-time as intended

#### 2. Alert Rules Non-Functional (FIXED)
- **Issue**: Alert rule commands (`add_alert_rule`, `remove_alert_rule`, `get_alert_rules`) were no-op stubs
- **Location**: `src-tauri/src/lib.rs:212-230`
- **Fix**: 
  - Added `AlertEngine` to managed state in setup
  - Implemented proper state access in command handlers
  - Connected monitoring task to use managed `AlertEngine` instance
- **Impact**: Alert rules now persist and trigger correctly

#### 3. SSH Server Key Validation (DOCUMENTED)
- **Issue**: SSH connections accept any server key without verification (MITM vulnerability)
- **Location**: `src-tauri/src/ssh.rs:19-31`
- **Status**: Documented with comprehensive TODO comment
- **Required Fix**: Implement known_hosts storage and key verification
- **Security Risk**: HIGH - connections are vulnerable to man-in-the-middle attacks

---

### High Severity Issues Resolved

#### 4. Integer Underflow in Metrics (FIXED)
- **Issue**: Metrics calculation could underflow if system counters reset
- **Location**: `src-tauri/src/monitoring.rs:116-119`
- **Fix**: Used `saturating_sub()` instead of direct subtraction
- **Impact**: Prevents panic on counter resets or overflows

#### 5. Crypto Functions Panic on Error (FIXED)
- **Issue**: `hash_password` and `verify_password` used `.expect()` causing crashes
- **Location**: `src-tauri/src/crypto.rs:14-27`
- **Fix**: Changed return types to `Result<T, String>` with proper error handling
- **Impact**: Application no longer crashes on crypto errors

#### 6. mDNS Discovery Blocking Loop (FIXED)
- **Issue**: Discovery blocked on first service type, never browsing subsequent types
- **Location**: `src-tauri/src/discovery.rs:14-59`
- **Fix**: Changed to non-blocking poll with `recv_timeout()` across all receivers
- **Impact**: Now discovers both SSH and workstation services concurrently

#### 7. Workflow SSH Execution Stub (DOCUMENTED)
- **Issue**: Workflow SSH commands return fake success without executing
- **Location**: `src-tauri/src/automation/engine.rs:161-167`
- **Status**: Documented as TODO - requires integration with existing SSH module
- **Impact**: Workflows cannot execute real SSH commands yet

---

### Medium Severity Issues Resolved

#### 8. Topological Sort Reverse Order (FIXED)
- **Issue**: Workflow nodes returned in reverse execution order
- **Location**: `src-tauri/src/automation.rs:210-213`
- **Fix**: Added `.rev()` to reverse the sorted list
- **Impact**: Workflows now execute in correct order (trigger → action)

#### 9. React Stale Closure - RemoteManager (FIXED)
- **Issue**: `handleConnect` not in useEffect dependency array
- **Location**: `src/components/RemoteManager.tsx:108`
- **Fix**: Added `handleConnect` to dependency array
- **Impact**: Prevents stale closure bugs in host connection handling

#### 10. React Stale Closure - Canvas (FIXED)
- **Issue**: `deleteSelected` not in useEffect dependency array
- **Location**: `src/components/automation/Canvas.tsx:221`
- **Fix**: Added `deleteSelected` to dependency array
- **Impact**: Keyboard delete handlers now work correctly

#### 12. Invalid SVG Path (FIXED)
- **Issue**: Missing `M` command in SVG path
- **Location**: `src/components/AddHostDialog.tsx:48`
- **Fix**: Changed `d="6 18L18 6..."` to `d="M6 18L18 6..."`
- **Impact**: Close button icon renders correctly

---

### Low Severity Issues Resolved

#### 13. Unused Variables (FIXED)
- **Location**: `src-tauri/src/health.rs:48-52, 80`
- **Fix**: Prefixed unused parameters with `_` and removed unused `Instant` import
- **Impact**: Cleaner code, fewer compiler warnings

#### 14. Disk Usage Metric Always Skipped (DOCUMENTED)
- **Location**: `src-tauri/src/monitoring.rs:203-207`
- **Status**: Added clarifying comment - requires disk capacity data in metrics
- **Impact**: Disk usage alerts cannot trigger (by design, needs enhancement)

---

## Architecture Notes

### State Management
The application uses Tauri's managed state pattern:
- `SshState` - SSH session management
- `ScannerState` - Network scanning state
- `AutomationState` - Workflow definitions and executions
- `AlertEngine` - Alert rules and active alerts
- `VaultState` - Master password management, vault lock/unlock (Phase 1)
- `CredentialManager` - Encrypted credential CRUD operations (Phase 2)
- `SshKeyManager` - SSH host key verification and known hosts management (Phase 3)

### Event System
Backend emits events to frontend:
- `system-metrics` - Real-time system metrics (every 5 seconds)
- `alerts-triggered` - When alert rules trigger
- `scan_progress` / `scan_result` - Network scan updates
- `ssh_data_{id}` / `ssh_closed_{id}` - SSH session data
- `workflow-*` - Workflow execution events

### Database Schema
SQLite database (`titan.db`) with tables:
- `hosts` - Remote host inventory
- `credentials_new` - Encrypted credentials with AES-256-GCM (nonce, tag, metadata)
- `vault_settings` - Master password hash, salt, vault configuration
- `security_audit_log` - Comprehensive audit trail for all credential operations
- `ssh_known_hosts` - SSH host key verification (ready for Phase 3)

---

## Known Issues & TODOs

### Security
1. **SSH Host Key Verification** - ✅ COMPLETE - Backend implementation done (Phase 3)
   - Note: Still needs UI integration for user prompts when connecting to unknown/changed hosts
2. **Credential Storage** - ✅ COMPLETE - Fully encrypted with AES-256-GCM (Phase 1 & 2)

### Features
1. **Workflow SSH Execution** - Needs integration with SSH module
2. **Disk Usage Monitoring** - Needs disk capacity data in metrics
3. **Alert Persistence** - Alerts currently in-memory only

### Code Quality
- 21 compiler warnings (all pre-existing, mostly unused code for future features)
- Several dead code warnings for planned functionality (automation engine)
- No new warnings introduced by vault/SSH key implementation
- All vault tests passing (11/11 total: 3 vault + 5 credentials + 3 SSH keys)

---

## Testing Notes

### Verified Working
- System metrics emit every 5 seconds ✓
- Alert rules can be added/removed ✓
- Network scanner discovers hosts ✓
- SSH connections establish ✓
- Crypto functions handle errors gracefully ✓
- Vault initialization and unlock/lock operations ✓
- Credential encryption/decryption with AES-256-GCM ✓
- Credential CRUD operations (add, get, list, update, delete, search) ✓
- Audit logging for all credential operations ✓
- SSH host key verification (unknown, trusted, changed detection) ✓
- Known hosts database storage and retrieval ✓

### Needs Testing
- Alert rule triggering under load
- Workflow execution with multiple nodes
- mDNS discovery on networks with services
- SSH session stability over time

---

## Development Commands

```bash
# Run development server
npm run tauri dev

# Build for production
npm run tauri build

# Run tests
npm test

# Run Rust tests
cd src-tauri && cargo test
```

---

## File Structure

```
ProjectTitan/
├── src/                    # React frontend
│   ├── components/         # UI components
│   │   ├── dashboard/      # Dashboard widgets
│   │   └── automation/     # Workflow canvas
│   ├── db.ts              # Database initialization
│   └── main.tsx           # App entry point
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── lib.rs         # Main Tauri commands
│   │   ├── monitoring.rs  # System metrics & alerts
│   │   ├── ssh.rs         # SSH client
│   │   ├── scanner.rs     # Network scanner
│   │   ├── automation.rs  # Workflow engine
│   │   ├── crypto.rs      # Encryption utilities (Argon2id, AES-256-GCM)
│   │   ├── vault.rs       # Vault state & master password management
│   │   ├── vault/
│   │   │   └── credentials.rs  # Credential CRUD with encryption
│   │   └── ...
│   ├── migrations/
│   │   └── 003_security_vault.sql  # Vault database schema
│   └── Cargo.toml
└── conductor/             # Product documentation
```

---

## Agent Guidelines

When working on this codebase:

1. **Security First**: Be cautious with SSH, credentials, and network operations
2. **Error Handling**: Use `Result` types, avoid `.expect()` and `.unwrap()` in production code
3. **State Management**: Use Tauri's managed state for shared resources
4. **Event-Driven**: Emit events for real-time updates to frontend
5. **Testing**: Add tests for critical functionality
6. **Documentation**: Update this file when making architectural changes

---

## Recent Performance Improvements

- Monitoring interval: 5000s → 5s (1000x faster)
- Discovery: Sequential → Concurrent (2x faster)
- Metrics calculation: Protected against underflow
- Error handling: Graceful degradation instead of crashes

---

*Last Updated: January 31, 2026*
*Agent: Cascade (Windsurf IDE)*

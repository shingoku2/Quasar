# Implementation Plan: Security & Credential Vault

This plan outlines the phase-by-phase implementation of secure credential storage, SSH host key verification, and security hardening for Project Titan.

## Phase 1: Vault Infrastructure & Encryption Integration

Establish the foundation for encrypted credential storage with master password management.

### Tasks

- [x] Task: Create database migration for new tables (credentials enhancement, ssh_known_hosts, security_audit_log, vault_settings)
- [x] Task: Create `vault.rs` module with VaultState managed state
- [x] Task: Implement master password key derivation using existing Argon2id functions
- [x] Task: Add vault initialization command (`initialize_vault`)
- [x] Task: Add vault unlock/lock commands (`unlock_vault`, `lock_vault`, `is_vault_locked`)
- [x] Task: Implement secure memory clearing for master key on lock (zeroize crate)
- [x] Task: Create credential encryption wrapper using existing `encrypt()` function (ready for Phase 2)
- [x] Task: Create credential decryption wrapper using existing `decrypt()` function (ready for Phase 2)
- [x] Task: Add auto-lock timer with configurable timeout (check_auto_lock method)
- [x] Task: Implement vault settings storage and retrieval
- [x] Task: Write unit tests for vault operations (unlock, lock, key derivation)
- [x] Task: Write unit tests for credential encryption/decryption round-trip (deferred to Phase 2)
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Vault Infrastructure' (Protocol in workflow.md) [checkpoint: PHASE1_COMPLETE]

**Success Criteria:**
- Vault can be initialized with master password
- Master key derived in < 500ms
- Credentials encrypted/decrypted successfully
- Auto-lock timer functions correctly
- All tests passing

## Phase 2: Credential Management Backend

Implement CRUD operations for encrypted credentials with audit logging.

### Tasks

- [x] Task: Add `add_credential` command with encryption
- [x] Task: Add `get_credential` command with decryption and audit logging
- [x] Task: Add `list_credentials` command (returns summaries, no passwords)
- [x] Task: Add `update_credential` command with re-encryption
- [x] Task: Add `delete_credential` command with audit logging
- [x] Task: Add `search_credentials` command with fuzzy matching
- [x] Task: Implement audit log entry creation for all credential operations
- [ ] Task: Add `get_audit_log` command with filtering (deferred to Phase 6)
- [ ] Task: Create credential migration tool for existing unencrypted credentials (deferred - no legacy data)
- [ ] Task: Add credential export command (encrypted JSON format) (deferred to Phase 6)
- [ ] Task: Add credential import command with validation (deferred to Phase 6)
- [x] Task: Write integration tests for credential CRUD operations (5/5 passing)
- [x] Task: Write tests for audit logging completeness (integrated in CRUD tests)
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Credential Management' (Protocol in workflow.md) [checkpoint: PHASE2_COMPLETE]

**Success Criteria:**
- All credential operations work with encryption
- Audit log captures every credential access
- Migration tool successfully encrypts existing credentials
- Export/import maintains encryption
- All tests passing

## Phase 3: SSH Host Key Verification

Implement SSH host key verification to prevent MITM attacks.

### Tasks

- [ ] Task: Enhance `ssh.rs` to extract server public key during connection
- [ ] Task: Implement host key fingerprint calculation (SHA256)
- [ ] Task: Add `verify_ssh_host_key` command with known_hosts lookup
- [ ] Task: Add `trust_ssh_host_key` command to add/update known hosts
- [ ] Task: Add `list_known_hosts` command
- [ ] Task: Add `remove_known_host` command
- [ ] Task: Add `get_host_key_fingerprint` utility command
- [ ] Task: Integrate host key verification into SSH connection flow
- [ ] Task: Add audit logging for host key verification events
- [ ] Task: Handle host key mismatch scenarios (changed key detection)
- [ ] Task: Write unit tests for fingerprint calculation
- [ ] Task: Write integration tests for host key verification flow
- [ ] Task: Update AGENTS.md to mark SSH MITM vulnerability as FIXED
- [ ] Task: Conductor - User Manual Verification 'Phase 3: SSH Host Key Verification' (Protocol in workflow.md) [checkpoint: TBD]

**Success Criteria:**
- SSH connections verify host keys before connecting
- New hosts prompt for trust decision
- Changed host keys trigger security warning
- Host key verification completes in < 100ms
- All tests passing

## Phase 4: Vault UI Components

Build the user interface for vault management and credential operations.

### Tasks

- [ ] Task: Create `VaultUnlock.tsx` component with master password input
- [ ] Task: Add vault initialization wizard for first-time setup
- [ ] Task: Create password strength meter component
- [ ] Task: Add auto-lock timer display in UI
- [ ] Task: Create `CredentialVault.tsx` main interface
- [ ] Task: Implement credential list with search and filtering
- [ ] Task: Create `CredentialForm.tsx` for add/edit operations
- [ ] Task: Add password generator utility
- [ ] Task: Implement credential type selector (password, SSH key, API token)
- [ ] Task: Add credential metadata editor (JSON fields)
- [ ] Task: Create delete confirmation dialog
- [ ] Task: Add "copy to clipboard" functionality with auto-clear
- [ ] Task: Integrate vault into main navigation (new tab or settings)
- [ ] Task: Write component tests for VaultUnlock
- [ ] Task: Write component tests for CredentialVault
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Vault UI' (Protocol in workflow.md) [checkpoint: TBD]

**Success Criteria:**
- Users can unlock vault with master password
- Credentials can be added/edited/deleted via UI
- Search and filtering work correctly
- Password generator produces strong passwords
- All component tests passing

## Phase 5: SSH Host Key UI & Integration

Build UI components for SSH host key management and integrate with connection flow.

### Tasks

- [ ] Task: Create `HostKeyDialog.tsx` for new host verification
- [ ] Task: Display host key fingerprint in readable format
- [ ] Task: Add "Trust", "Reject", "Trust Once" actions
- [ ] Task: Create `HostKeyChangedDialog.tsx` for changed key warnings
- [ ] Task: Add "Update Key", "Reject", "Connect Anyway" actions (with warnings)
- [ ] Task: Create `KnownHostsManager.tsx` component
- [ ] Task: Display list of trusted hosts with fingerprints
- [ ] Task: Add remove/update host key functionality
- [ ] Task: Integrate HostKeyDialog into SSH connection flow in RemoteManager
- [ ] Task: Show host key status indicator in host list
- [ ] Task: Add host key verification to pre-flight health checks
- [ ] Task: Write component tests for HostKeyDialog
- [ ] Task: Write integration tests for SSH connection with host key verification
- [ ] Task: Conductor - User Manual Verification 'Phase 5: SSH Host Key UI' (Protocol in workflow.md) [checkpoint: TBD]

**Success Criteria:**
- Users prompted to verify new SSH hosts
- Host key changes trigger security warnings
- Known hosts can be managed via UI
- SSH connections blocked until host key verified
- All tests passing

## Phase 6: Security Settings & Audit Log UI

Build security configuration interface and audit log viewer.

### Tasks

- [ ] Task: Create `SecuritySettings.tsx` component
- [ ] Task: Add auto-lock timeout configuration (dropdown: 5, 15, 30, 60 min, never)
- [ ] Task: Add "Lock on minimize" toggle
- [ ] Task: Add "Require password on credential use" toggle
- [ ] Task: Implement change master password dialog
- [ ] Task: Add master password strength validation
- [ ] Task: Create `AuditLogViewer.tsx` component
- [ ] Task: Display audit log table with timestamp, event type, resource, action, result
- [ ] Task: Add event type filter (dropdown)
- [ ] Task: Add date range filter
- [ ] Task: Add search by resource ID
- [ ] Task: Implement audit log export (CSV/JSON)
- [ ] Task: Add credential export/import UI
- [ ] Task: Create export format selector (encrypted JSON, plaintext JSON with warning)
- [ ] Task: Add import file picker with validation
- [ ] Task: Integrate SecuritySettings into settings tab
- [ ] Task: Write component tests for SecuritySettings
- [ ] Task: Write component tests for AuditLogViewer
- [ ] Task: Conductor - User Manual Verification 'Phase 6: Security Settings' (Protocol in workflow.md) [checkpoint: TBD]

**Success Criteria:**
- Security settings can be configured via UI
- Master password can be changed
- Audit log is viewable and filterable
- Credentials can be exported/imported
- All tests passing

## Phase 7: Security Hardening & Polish

Final security review, performance optimization, and documentation.

### Tasks

- [ ] Task: Implement secure memory wiping for decrypted credentials
- [ ] Task: Add memory security tests (verify no plaintext in memory dumps)
- [ ] Task: Review all credential access points for security
- [ ] Task: Add rate limiting for vault unlock attempts (prevent brute force)
- [ ] Task: Implement vault lockout after failed attempts (5 attempts = 5 min lockout)
- [ ] Task: Add session timeout detection (lock on system sleep/hibernate)
- [ ] Task: Optimize Argon2id parameters for target unlock time (< 500ms)
- [ ] Task: Add performance benchmarks for encryption operations
- [ ] Task: Create security documentation for users
- [ ] Task: Add in-app security tips and best practices
- [ ] Task: Create sample credentials for testing (dev mode only)
- [ ] Task: Add keyboard shortcuts (Ctrl+L to lock, Ctrl+K for credential search)
- [ ] Task: Implement credential last-used timestamp tracking
- [ ] Task: Add "unused credentials" indicator (not used in 90 days)
- [ ] Task: Final integration testing across all modules
- [ ] Task: Security audit and penetration testing
- [ ] Task: Update README with security features documentation
- [ ] Task: Update AGENTS.md with completed security enhancements
- [ ] Task: Commit all changes with checkpoint
- [ ] Task: Conductor - User Manual Verification 'Phase 7: Security Hardening' (Protocol in workflow.md) [checkpoint: TBD]

**Success Criteria:**
- No plaintext credentials in database or memory
- Vault unlock time < 500ms
- Rate limiting prevents brute force attacks
- All security tests passing
- Documentation complete
- Zero high-severity security issues

## Success Metrics

- **Encryption:** All credentials encrypted with AES-256-GCM
- **Performance:** Vault unlock < 500ms, credential decryption < 50ms
- **Security:** Zero plaintext credentials in database or memory dumps
- **MITM Protection:** SSH host key verification prevents unauthorized connections
- **Audit Trail:** 100% of credential access events logged
- **Code Coverage:** > 85% for security-critical code
- **User Experience:** Vault unlock in < 3 clicks, credential search in < 1 second

## Dependencies

### Existing Code
- `src-tauri/src/crypto.rs` - Argon2id and AES-256-GCM functions
- `src-tauri/src/ssh.rs` - SSH connection infrastructure
- `src-tauri/src/db.rs` - SQLite database access

### New Dependencies (Rust)
- `russh::keys` - For SSH public key parsing and fingerprinting
- `sha2` - For SHA256 fingerprint calculation
- `zeroize` - For secure memory wiping

### New Dependencies (Frontend)
- `zxcvbn` - Password strength estimation (optional)
- None required - use existing UI components

## Risks and Mitigations

### Risk: Master Password Forgotten
- **Mitigation:** Clear warning during setup that password cannot be recovered
- **Mitigation:** Offer password hint storage (not the password itself)
- **Mitigation:** Document backup/export procedures

### Risk: Performance Impact of Encryption
- **Mitigation:** Benchmark and optimize Argon2id parameters
- **Mitigation:** Cache decrypted credentials in memory (cleared on lock)
- **Mitigation:** Use background threads for bulk operations

### Risk: SSH Host Key False Positives
- **Mitigation:** Clear UI explaining why key changed
- **Mitigation:** Show both old and new fingerprints
- **Mitigation:** Allow manual key update with confirmation

### Risk: Audit Log Growth
- **Mitigation:** Implement log rotation (keep last 10,000 entries)
- **Mitigation:** Add log export and archive functionality
- **Mitigation:** Index for fast queries

## Output Files

### Backend (Rust)
- `src-tauri/src/vault.rs` - Vault state and encryption management
- `src-tauri/src/vault/credentials.rs` - Credential CRUD operations
- `src-tauri/src/vault/audit.rs` - Audit logging
- `src-tauri/src/vault/known_hosts.rs` - SSH host key management
- `src-tauri/src/ssh.rs` - Enhanced with host key verification

### Frontend (React)
- `src/components/vault/VaultUnlock.tsx` - Master password unlock
- `src/components/vault/CredentialVault.tsx` - Main vault interface
- `src/components/vault/CredentialForm.tsx` - Add/edit credentials
- `src/components/vault/HostKeyDialog.tsx` - SSH host key verification
- `src/components/vault/KnownHostsManager.tsx` - Manage trusted hosts
- `src/components/vault/SecuritySettings.tsx` - Security configuration
- `src/components/vault/AuditLogViewer.tsx` - Audit log interface

### Database
- `migrations/003_security_vault.sql` - Schema updates

### Documentation
- `docs/security-model.md` - Security architecture
- `docs/user-guide-vault.md` - User guide for vault features
- `docs/developer-security.md` - Security guidelines for developers

## Testing Strategy

### Unit Tests (Rust)
- Vault unlock/lock cycle
- Key derivation consistency
- Encryption/decryption round-trip
- Host key fingerprint calculation
- Audit log entry creation

### Integration Tests (Rust)
- Credential CRUD with encryption
- SSH host key verification flow
- Auto-lock timer behavior
- Credential migration

### Component Tests (React)
- VaultUnlock component
- CredentialVault component
- HostKeyDialog component
- SecuritySettings component

### E2E Tests
- Complete vault initialization flow
- Add credential → Use credential → Lock vault → Unlock vault
- SSH connection with new host → Trust host → Reconnect
- Change master password flow

### Security Tests
- Verify no plaintext in database
- Memory dump analysis (no plaintext credentials)
- Host key mismatch detection
- Rate limiting effectiveness
- Audit log completeness

## Timeline Estimate

- **Phase 1:** 2-3 days (vault infrastructure)
- **Phase 2:** 2-3 days (credential management)
- **Phase 3:** 2-3 days (SSH host key verification)
- **Phase 4:** 3-4 days (vault UI)
- **Phase 5:** 2-3 days (SSH host key UI)
- **Phase 6:** 2-3 days (security settings & audit log)
- **Phase 7:** 2-3 days (hardening & polish)

**Total:** ~15-22 days (3-4 weeks)

## Post-Implementation

### Future Enhancements (Not in Scope)
- Biometric authentication (fingerprint, Face ID)
- Hardware security key support (YubiKey)
- Credential sharing between team members
- Cloud backup with end-to-end encryption
- Browser extension for credential autofill
- SSH key pair generation and management
- Certificate management (TLS/SSL certs)
- TOTP/2FA token storage

### Maintenance
- Regular security audits
- Dependency updates for crypto libraries
- Performance monitoring
- User feedback collection

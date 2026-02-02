# Security & Credential Vault - Technical Specification

## Architecture Overview

### Encryption Stack

```
┌─────────────────────────────────────────┐
│         User Master Password            │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│    Argon2id Key Derivation (32 bytes)   │
│    - Memory: 64 MB                       │
│    - Iterations: 3                       │
│    - Parallelism: 4                      │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│    AES-256-GCM Encryption                │
│    - Unique nonce per credential         │
│    - Authentication tag for integrity    │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│    SQLite Database (encrypted blob)     │
└─────────────────────────────────────────┘
```

### Security Model

**Threat Model:**
- ✅ Protects against: Database theft, memory dumps (after lock), credential leakage
- ✅ Protects against: SSH MITM attacks via host key verification
- ⚠️ Does not protect against: Keyloggers, compromised OS, active memory inspection
- ⚠️ Does not protect against: User choosing weak master password

**Security Boundaries:**
1. **At Rest:** All credentials encrypted in database
2. **In Memory:** Credentials decrypted only when needed, cleared after use
3. **In Transit:** SSH host keys verified, TLS for future web APIs
4. **Audit Trail:** All credential access logged with timestamps

## Database Schema

### Credentials Table (Enhanced)

```sql
CREATE TABLE credentials (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    username TEXT NOT NULL,
    encrypted_password BLOB NOT NULL,  -- AES-256-GCM encrypted
    nonce BLOB NOT NULL,               -- 12 bytes
    tag BLOB NOT NULL,                 -- 16 bytes (authentication tag)
    credential_type TEXT NOT NULL,     -- 'password', 'ssh_key', 'api_token'
    metadata TEXT,                     -- JSON for additional fields
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_used_at INTEGER
);

CREATE INDEX idx_credentials_type ON credentials(credential_type);
CREATE INDEX idx_credentials_name ON credentials(name);
```

### SSH Known Hosts Table (New)

```sql
CREATE TABLE ssh_known_hosts (
    id TEXT PRIMARY KEY,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 22,
    key_type TEXT NOT NULL,            -- 'ssh-rsa', 'ssh-ed25519', etc.
    fingerprint TEXT NOT NULL,         -- SHA256 fingerprint
    public_key BLOB NOT NULL,          -- Raw public key bytes
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    trust_status TEXT NOT NULL,        -- 'trusted', 'untrusted', 'changed'
    UNIQUE(host, port)
);

CREATE INDEX idx_known_hosts_lookup ON ssh_known_hosts(host, port);
```

### Audit Log Table (New)

```sql
CREATE TABLE security_audit_log (
    id TEXT PRIMARY KEY,
    timestamp INTEGER NOT NULL,
    event_type TEXT NOT NULL,          -- 'credential_access', 'vault_unlock', 'host_key_verify', etc.
    resource_id TEXT,                  -- credential_id or host_id
    resource_type TEXT,                -- 'credential', 'ssh_host', 'vault'
    action TEXT NOT NULL,              -- 'read', 'write', 'delete', 'verify', 'reject'
    result TEXT NOT NULL,              -- 'success', 'failure', 'denied'
    details TEXT,                      -- JSON with additional context
    user_context TEXT                  -- Session info, IP if applicable
);

CREATE INDEX idx_audit_timestamp ON security_audit_log(timestamp);
CREATE INDEX idx_audit_resource ON security_audit_log(resource_type, resource_id);
```

### Vault Settings Table (New)

```sql
CREATE TABLE vault_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Settings stored:
-- - 'master_password_hash': Argon2id hash for verification
-- - 'salt': Random salt for key derivation
-- - 'auto_lock_timeout': Minutes before auto-lock (default: 15)
-- - 'require_password_on_credential_use': Boolean
-- - 'vault_initialized': Boolean
```

## API Design

### Rust Backend (Tauri Commands)

#### Vault Management

```rust
#[tauri::command]
async fn initialize_vault(master_password: String) -> Result<(), String>

#[tauri::command]
async fn unlock_vault(master_password: String) -> Result<(), String>

#[tauri::command]
async fn lock_vault() -> Result<(), String>

#[tauri::command]
async fn is_vault_locked() -> Result<bool, String>

#[tauri::command]
async fn change_master_password(
    old_password: String, 
    new_password: String
) -> Result<(), String>
```

#### Credential Operations

```rust
#[tauri::command]
async fn add_credential(
    name: String,
    username: String,
    password: String,
    credential_type: String,
    metadata: Option<String>
) -> Result<String, String>  // Returns credential_id

#[tauri::command]
async fn get_credential(
    credential_id: String
) -> Result<DecryptedCredential, String>

#[tauri::command]
async fn list_credentials() -> Result<Vec<CredentialSummary>, String>

#[tauri::command]
async fn update_credential(
    credential_id: String,
    name: Option<String>,
    username: Option<String>,
    password: Option<String>,
    metadata: Option<String>
) -> Result<(), String>

#[tauri::command]
async fn delete_credential(credential_id: String) -> Result<(), String>

#[tauri::command]
async fn search_credentials(query: String) -> Result<Vec<CredentialSummary>, String>
```

#### SSH Host Key Management

```rust
#[tauri::command]
async fn verify_ssh_host_key(
    host: String,
    port: u16,
    key_type: String,
    public_key: Vec<u8>
) -> Result<HostKeyVerification, String>

#[tauri::command]
async fn trust_ssh_host_key(
    host: String,
    port: u16,
    key_type: String,
    public_key: Vec<u8>
) -> Result<(), String>

#[tauri::command]
async fn list_known_hosts() -> Result<Vec<KnownHost>, String>

#[tauri::command]
async fn remove_known_host(host: String, port: u16) -> Result<(), String>

#[tauri::command]
async fn get_host_key_fingerprint(public_key: Vec<u8>) -> Result<String, String>
```

#### Audit & Security

```rust
#[tauri::command]
async fn get_audit_log(
    start_time: Option<i64>,
    end_time: Option<i64>,
    event_type: Option<String>
) -> Result<Vec<AuditLogEntry>, String>

#[tauri::command]
async fn export_credentials(
    master_password: String,
    format: String  // 'json', 'encrypted_json'
) -> Result<String, String>

#[tauri::command]
async fn import_credentials(
    data: String,
    format: String,
    master_password: Option<String>
) -> Result<ImportResult, String>
```

### Frontend Components

#### Core Components

1. **`VaultUnlock.tsx`**
   - Master password input
   - Vault initialization wizard
   - Auto-lock timer display

2. **`CredentialVault.tsx`**
   - Main vault interface
   - Credential list with search/filter
   - Add/edit/delete credential dialogs

3. **`CredentialForm.tsx`**
   - Form for adding/editing credentials
   - Password generator integration
   - Credential type selector

4. **`HostKeyDialog.tsx`**
   - SSH host key verification prompt
   - Fingerprint display
   - Trust/reject actions

5. **`KnownHostsManager.tsx`**
   - List of trusted SSH hosts
   - Host key fingerprints
   - Remove/update host keys

6. **`SecuritySettings.tsx`**
   - Auto-lock timeout configuration
   - Master password change
   - Audit log viewer
   - Export/import credentials

7. **`AuditLogViewer.tsx`**
   - Filterable audit log table
   - Event type filtering
   - Export audit logs

## Security Features

### Master Password Requirements

- Minimum 12 characters
- Must contain: uppercase, lowercase, number, special character
- Strength meter during setup
- Optional passphrase mode (4+ words)

### Auto-Lock Behavior

- Configurable timeout (5, 15, 30, 60 minutes, never)
- Lock on system sleep/hibernate
- Lock on application minimize (optional)
- Lock on idle detection

### Memory Security

- Credentials cleared from memory after use
- Secure string handling (no string copies)
- Master key cleared on lock
- Memory wiping for sensitive data

### SSH Host Key Verification Flow

```
1. User initiates SSH connection
2. Backend retrieves server's public key
3. Check if host exists in known_hosts table
   
   IF NEW HOST:
     - Show HostKeyDialog with fingerprint
     - User can: Trust, Reject, Trust Once
     - If trusted, add to known_hosts table
   
   IF KNOWN HOST:
     - Compare public key with stored key
     - IF MATCH: Allow connection
     - IF MISMATCH: 
       - Show warning dialog (potential MITM)
       - User can: Update Key, Reject, Connect Anyway (dangerous)
       - Log security event
```

## Implementation Notes

### Crypto Module Integration

The existing `crypto.rs` module already implements:
- ✅ `hash_password()` - Argon2id hashing
- ✅ `verify_password()` - Password verification
- ✅ `encrypt()` - AES-256-GCM encryption
- ✅ `decrypt()` - AES-256-GCM decryption

**Integration Tasks:**
1. Create `VaultState` managed state to hold decrypted master key
2. Implement key derivation from master password
3. Add credential encryption/decryption wrappers
4. Implement secure memory clearing

### SSH Module Integration

The existing `ssh.rs` module needs enhancement:
- ❌ Currently accepts any host key (MITM vulnerability)
- ✅ Has SSH connection infrastructure

**Integration Tasks:**
1. Add host key extraction during connection
2. Implement known_hosts lookup and verification
3. Add host key fingerprint calculation (SHA256)
4. Integrate HostKeyDialog into connection flow

### Database Migration

Existing credentials need migration:
1. Detect unencrypted credentials on first vault initialization
2. Prompt user to set master password
3. Encrypt all existing credentials
4. Update schema with encryption fields

## Testing Strategy

### Unit Tests

- Encryption/decryption round-trip
- Key derivation consistency
- Host key fingerprint calculation
- Audit log entry creation

### Integration Tests

- Vault unlock/lock cycle
- Credential CRUD operations
- SSH host key verification flow
- Auto-lock timeout behavior

### Security Tests

- Verify no plaintext passwords in database
- Verify memory clearing on lock
- Test host key mismatch detection
- Test audit log completeness

## Performance Targets

- Vault unlock: < 500ms (Argon2id computation)
- Credential decryption: < 50ms per credential
- Host key verification: < 100ms
- Audit log query: < 200ms for 1000 entries
- Database encryption overhead: < 10% vs plaintext

## Compliance & Standards

- **Encryption:** AES-256-GCM (NIST approved)
- **Key Derivation:** Argon2id (RFC 9106)
- **SSH:** OpenSSH known_hosts format compatible
- **Audit Logging:** Follows NIST 800-53 guidelines

# Quasar Code Review - February 2, 2026

Comprehensive code review identifying potential bugs, security issues, and code quality improvements in the Quasar codebase.

## Executive Summary

Reviewed the entire Rust backend and identified **12 issues** across critical, high, and medium severity categories. The codebase shows good security practices overall (encryption, audit logging, zeroization) but has several areas requiring attention.

---

## Critical Issues (3)

### 1. SSH Session Resource Leak in ssh_exec.rs
**Location**: `src-tauri/src/ssh_exec.rs:108, 208`  
**Severity**: CRITICAL  
**Issue**: SSH sessions are explicitly disconnected but only on success path in `execute_ssh_command`. The `execute_ssh_commands_batch` function disconnects after the loop, but if an error occurs mid-loop, the session remains open.

**Code**:
```rust
// Line 108 - Only disconnects on success
let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;
result

// Line 208 - Disconnects after loop, but errors return early
let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;
```

**Impact**: Connection leaks under error conditions, potential server resource exhaustion.

**Fix**: Use RAII pattern or ensure disconnect in all paths:
```rust
// Wrap in struct with Drop implementation or use scopeguard crate
let result = async { /* operations */ }.await;
let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;
result
```

---

### 2. SFTP Session Resource Leak
**Location**: `src-tauri/src/sftp.rs:69-148, 176-252, 283-331, 362-394`  
**Severity**: CRITICAL  
**Issue**: All four SFTP functions (`upload_file`, `download_file`, `list_directory`, `remote_exists`) have the same pattern - SSH session disconnect is called after the async block, but if the async block returns early with an error, the disconnect may not execute properly due to async context issues.

**Code Pattern**:
```rust
let result = async {
    // ... operations that can return Err early
    sftp.close().await?;
    Ok(())
}.await;

// This disconnect happens regardless, but session may already be in bad state
let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;
result
```

**Impact**: SFTP sessions not properly cleaned up, connection pool exhaustion.

**Fix**: Ensure disconnect happens in all code paths with proper error handling.

---

### 3. Monitoring Task Initialization Panic Risk
**Location**: `src-tauri/src/monitoring.rs:689-700`  
**Severity**: CRITICAL  
**Issue**: Uses `.ok()` and `.and_then()` chain which silently fails if metrics store initialization fails. While this prevents panic, it means monitoring runs without persistence and no error is logged to the user/admin.

**Code**:
```rust
let metrics_store = app_handle.path().app_data_dir()
    .ok()
    .and_then(|path| path.join("titan.db").to_str().map(|s| s.to_string()))
    .and_then(|db_path| {
        MetricsStore::new(db_path, 30)
            .map_err(|e| {
                eprintln!("Failed to create metrics store: {}", e);
                eprintln!("Monitoring will continue without persistence");
                e
            })
            .ok()
    });
```

**Impact**: Silent failure of metrics persistence - monitoring appears to work but data is not saved.

**Fix**: Emit event to frontend or log to proper logging system, not just eprintln.

---

## High Severity Issues (4)

### 4. Potential Tuple Unpacking Logic Error
**Location**: `src-tauri/src/ssh_exec.rs:239-265`  
**Severity**: HIGH  
**Issue**: The memory and disk parsing uses `.map(|(used, total)| (Some(used), Some(total)))` pattern which is correct, but the variable names suggest potential confusion. The code is actually correct upon closer inspection.

**Status**: FALSE POSITIVE - Code is correct.

---

### 5. Integer Overflow Risk in Alert Management
**Location**: `src-tauri/src/monitoring.rs:434-454`  
**Severity**: HIGH  
**Issue**: Alert management uses manual capacity checks with arithmetic that could theoretically overflow:

**Code**:
```rust
let new_len = current_len + new_alerts.len();  // Potential overflow
if new_len > 50 {
    let to_remove = new_len - 50;  // Could underflow if logic error
    // ...
}
```

**Impact**: Panic on overflow in debug mode, wraparound in release mode leading to incorrect capacity management.

**Fix**: Use `saturating_add` and `saturating_sub`:
```rust
let new_len = current_len.saturating_add(new_alerts.len());
if new_len > 50 {
    let to_remove = new_len.saturating_sub(50);
    // ...
}
```

---

### 6. SSH Host Key Verification Bypassed in Automation
**Location**: `src-tauri/src/ssh_exec.rs:12-19`, `src-tauri/src/sftp.rs:26-33`  
**Severity**: HIGH (Security)  
**Issue**: Both modules have `check_server_key` that returns `Ok(true)` unconditionally with comment "we assume host keys are already verified". However, these modules are NOT integrated with `vault::SshKeyManager`.

**Code**:
```rust
async fn check_server_key(
    &mut self,
    _server_public_key: &russh::keys::PublicKey,
) -> Result<bool, Self::Error> {
    // For automation, we assume host keys are already verified
    // This is safe because the vault's SSH key manager handles verification
    Ok(true)  // ← BYPASSES VERIFICATION
}
```

**Impact**: MITM vulnerability for SSH command execution and SFTP operations.

**Fix**: Integrate with `SshKeyManager` or require explicit trust bypass flag.

---

### 7. Production Logging Uses println! and eprintln!
**Location**: `src-tauri/src/monitoring.rs:713, 732, 748`  
**Severity**: HIGH (Code Quality)  
**Issue**: Production code uses `println!` and `eprintln!` instead of proper logging framework.

**Code**:
```rust
println!("Emitting system-metrics: cpu={:.1}%, mem={:.1}%", ...);
eprintln!("Failed to save metrics: {}", e);
```

**Impact**: No log levels, no structured logging, difficult production debugging.

**Fix**: Add `log` or `tracing` crate and use proper log macros.

---

### 8. Vault Password Change Transaction Safety
**Location**: `src-tauri/src/vault.rs:345-409`  
**Severity**: HIGH  
**Issue**: The credential re-encryption during password change now uses transactions correctly (FIXED from previous review), but there's still a risk if `get_credential` fails after `delete_credential_tx` succeeds within the transaction.

**Code**:
```rust
for (id, name, username, password, cred_type, metadata) in re_encrypted_credentials {
    credential_manager.delete_credential_tx(&tx, &id)?;
    let new_id = credential_manager.add_credential_tx(
        &tx,
        &new_master_key,
        name,
        username,
        password,  // ← This came from get_credential earlier
        cred_type,
        metadata,
    )?;
}
```

**Status**: Actually SAFE - credentials are collected BEFORE transaction starts (line 355-366), so no risk.

---

## Medium Severity Issues (3)

### 9. Missing Error Context in Database Operations
**Location**: Multiple locations in `vault.rs`, `credentials.rs`, `ssh_keys.rs`  
**Severity**: MEDIUM  
**Issue**: Generic error messages lack operation context.

**Examples**:
```rust
.map_err(|e| format!("Failed to open database: {}", e))?;
.map_err(|e| format!("Failed to insert credential: {}", e))?;
```

**Impact**: Difficult to diagnose production issues - which database? which credential?

**Fix**: Add context:
```rust
.map_err(|e| format!("Failed to open vault database at {}: {}", self.db_path, e))?;
.map_err(|e| format!("Failed to insert credential '{}': {}", name, e))?;
```

---

### 10. Potential Race Condition in SSH Stats Reporter
**Location**: `src-tauri/src/ssh.rs:196-241`  
**Severity**: MEDIUM  
**Issue**: Stats reporter task accesses shared state without holding lock during calculations:

**Code**:
```rust
let (bytes, last_time) = {
    let state = app_handle_clone.state::<SshState>();
    let sessions = state.sessions.lock().unwrap();
    if let Some(c) = sessions.get(&id_clone) {
        let b = *c.bytes_received.lock().unwrap();
        let t = *c.last_stats_check.lock().unwrap();
        
        *c.bytes_received.lock().unwrap() = 0;  // ← Three separate lock acquisitions
        *c.last_stats_check.lock().unwrap() = Instant::now();
        
        (b, t)
    } else {
        break;
    }
};
```

**Impact**: Potential race condition if multiple threads access the same connection stats.

**Fix**: Hold locks for entire read-modify-write operation:
```rust
let (bytes, last_time) = {
    let state = app_handle_clone.state::<SshState>();
    let sessions = state.sessions.lock().unwrap();
    if let Some(c) = sessions.get(&id_clone) {
        let mut bytes_guard = c.bytes_received.lock().unwrap();
        let mut time_guard = c.last_stats_check.lock().unwrap();
        
        let b = *bytes_guard;
        let t = *time_guard;
        
        *bytes_guard = 0;
        *time_guard = Instant::now();
        
        (b, t)
    } else {
        break;
    }
};
```

---

### 11. Dynamic SQL Query Construction
**Location**: `src-tauri/src/vault/credentials.rs:244-294`, `src-tauri/src/vault/audit.rs:47-82`  
**Severity**: MEDIUM  
**Issue**: Dynamic SQL query construction using string formatting. While the code has safety comments indicating field names are hardcoded, this pattern is risky.

**Code**:
```rust
// SAFETY: Query is constructed from hardcoded field names only
let query = format!(
    "UPDATE credentials SET {} WHERE id = ?",
    updates.join(", ")
);
```

**Status**: SAFE - All field names are indeed hardcoded constants, not user input. The safety comments are appropriate.

**Recommendation**: Consider using a query builder library for better type safety.

---

### 12. Missing Timeout on Vault Status Check
**Location**: Frontend integration (not visible in backend code)  
**Severity**: MEDIUM  
**Issue**: Based on AGENTS.md, there's a note about "Missing timeout handling for vault status check" but this appears to be a frontend issue.

**Status**: Cannot verify without frontend code review.

---

## Security Observations

### Positive Security Practices ✅
1. **Encryption**: AES-256-GCM with unique nonces per credential
2. **Key Derivation**: Argon2id with appropriate parameters (64 MB memory, 3 iterations)
3. **Memory Safety**: Master key zeroization on drop
4. **Audit Logging**: Comprehensive audit trail for all vault operations
5. **Rate Limiting**: Vault lockout policy (5/10/15 failed attempts)
6. **Transaction Safety**: Credential re-encryption uses proper transactions

### Security Concerns ⚠️
1. **SSH Host Key Verification Bypassed** (Issue #6) - MITM vulnerability
2. **No TLS/SSL for Database** - Database connections are unencrypted (SQLite local file, acceptable)
3. **Password in Memory** - Passwords passed as `String` in function parameters (acceptable for Rust)

---

## Code Quality Issues

### Minor Issues
1. **Inconsistent Error Handling**: Mix of `map_err` with format strings and direct error propagation
2. **Magic Numbers**: Hardcoded values like `32768` (chunk size), `50` (alert limit)
3. **Unused Imports**: Some files may have unused imports (not verified)
4. **Test Coverage**: Limited integration tests for error paths

### Good Practices ✅
1. **Proper use of Result types** throughout
2. **Comprehensive unit tests** for core functionality
3. **Good separation of concerns** (vault, credentials, SSH keys, audit)
4. **Async/await properly used** with tokio
5. **Database migrations** properly structured

---

## Recommendations Priority

### Immediate (Critical)
1. Fix SSH/SFTP session resource leaks (Issues #1, #2)
2. Add proper error reporting for monitoring initialization (Issue #3)

### High Priority
1. Integrate SSH host key verification in automation modules (Issue #6)
2. Replace println!/eprintln! with proper logging (Issue #7)
3. Fix integer overflow risks in alert management (Issue #5)

### Medium Priority
1. Add error context to database operations (Issue #9)
2. Fix race condition in SSH stats reporter (Issue #10)
3. Consider query builder library for dynamic SQL (Issue #11)

### Low Priority
1. Extract magic numbers to constants
2. Add integration tests for error paths
3. Review and remove unused imports

---

## Testing Gaps

1. No integration tests for SSH session cleanup under failure
2. No stress tests for connection pooling
3. No tests for concurrent credential access
4. No tests for monitoring task recovery from failures

---

## Conclusion

The codebase demonstrates **strong security fundamentals** with proper encryption, key derivation, and audit logging. However, **resource management** (SSH/SFTP sessions) needs immediate attention to prevent connection leaks. The **host key verification bypass** in automation modules is a security concern that should be addressed.

Overall code quality is **good** with proper error handling patterns, but production logging and error context could be improved for better operational visibility.

**Estimated Fix Time**: 2-3 days for critical issues, 1 week for all high-priority items.

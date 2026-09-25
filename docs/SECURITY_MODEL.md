# Security Model - Credential Vault

## Threat Model

### Assets to Protect

1. **Credentials** - Passwords, SSH keys, API tokens
2. **Master Password** - Key to decrypt all credentials
3. **SSH Host Keys** - Trust anchors for remote connections
4. **Audit Logs** - Evidence of credential access

### Threat Actors

1. **External Attacker** - Gains access to database file
2. **Malware** - Running on user's system
3. **Physical Access** - Unauthorized access to unlocked workstation
4. **Network Attacker** - MITM on SSH connections
5. **Insider Threat** - Authorized user attempting privilege escalation

### Attack Vectors

| Attack Vector | Threat Level | Mitigation |
|---------------|--------------|------------|
| Database theft | HIGH | AES-256-GCM encryption at rest |
| Memory dump (locked) | MEDIUM | Master key cleared on lock |
| Memory dump (unlocked) | HIGH | Cannot fully mitigate, minimize exposure time |
| Keylogger | HIGH | Cannot mitigate at application level |
| SSH MITM | HIGH | Host key verification (known_hosts) |
| Brute force master password | MEDIUM | Argon2id (slow), rate limiting, lockout |
| Weak master password | MEDIUM | Password strength requirements, meter |
| Credential reuse | LOW | Audit log tracks usage patterns |
| Unattended workstation | MEDIUM | Auto-lock timer |

## Security Boundaries

### Defense in Depth Layers

```
┌─────────────────────────────────────────────────────┐
│ Layer 1: Physical Security (User Responsibility)    │
│ - Secure workstation, lock screen                   │
└─────────────────────────────────────────────────────┘
                        ▼
┌─────────────────────────────────────────────────────┐
│ Layer 2: Operating System Security                  │
│ - OS-level encryption (BitLocker, FileVault)        │
│ - Process isolation, memory protection              │
└─────────────────────────────────────────────────────┘
                        ▼
┌─────────────────────────────────────────────────────┐
│ Layer 3: Application Security (Our Responsibility)  │
│ - Master password authentication                    │
│ - AES-256-GCM encryption                            │
│ - SSH host key verification                         │
│ - Auto-lock, rate limiting                          │
└─────────────────────────────────────────────────────┘
                        ▼
┌─────────────────────────────────────────────────────┐
│ Layer 4: Data Security                              │
│ - Encrypted database                                │
│ - Secure memory handling                            │
│ - Audit logging                                     │
└─────────────────────────────────────────────────────┘
```

## Cryptographic Design

### Encryption Algorithm: AES-256-GCM

**Why AES-256-GCM?**
- NIST approved, FIPS 140-2 compliant
- Authenticated encryption (confidentiality + integrity)
- Fast hardware acceleration (AES-NI)
- Resistant to timing attacks

**Parameters:**
- Key size: 256 bits (32 bytes)
- Nonce size: 96 bits (12 bytes) - unique per credential
- Tag size: 128 bits (16 bytes) - authentication tag

**Security Properties:**
- Confidentiality: Ciphertext reveals nothing about plaintext
- Integrity: Any tampering detected via authentication tag
- Authenticity: Only holder of key can encrypt/decrypt

### Key Derivation: Argon2id

**Why Argon2id?**
- Winner of Password Hashing Competition (2015)
- Resistant to GPU/ASIC attacks (memory-hard)
- Resistant to side-channel attacks (data-independent)
- Configurable time/memory trade-off

**Parameters (Tuned for ~500ms on modern hardware):**
- Memory: 64 MB (65536 KiB)
- Iterations: 3
- Parallelism: 4 threads
- Salt: 16 bytes (random, stored in vault_settings)
- Output: 32 bytes (256-bit key for AES)

**Security Properties:**
- Brute force resistance: ~500ms per attempt = 172,800 attempts/day
- GPU resistance: Memory-hard algorithm limits GPU advantage
- Rainbow table resistance: Unique salt per vault

### Nonce Management

**Critical Security Requirement:** Never reuse a nonce with the same key.

**Implementation:**
- Generate cryptographically random 12-byte nonce per credential
- Store nonce alongside ciphertext in database
- Use OS-provided CSPRNG (e.g., `getrandom()` on Linux)

**Collision Probability:**
- With 2^96 possible nonces, collision probability is negligible
- Even encrypting 1 billion credentials: P(collision) < 10^-15

### SSH Host Key Verification

**Algorithm:** SHA256 fingerprinting (OpenSSH format)

**Process:**
1. Extract server's public key during SSH handshake
2. Compute SHA256 hash of public key bytes
3. Encode as Base64 (OpenSSH format: `SHA256:...`)
4. Compare with stored fingerprint in known_hosts table

**Security Properties:**
- Prevents MITM attacks (attacker cannot forge key)
- Detects compromised servers (key change alert)
- Compatible with OpenSSH known_hosts format

## Access Control

### Vault States

```
┌──────────────┐
│ Uninitialized│ ──[Set Master Password]──> ┌────────┐
└──────────────┘                             │ Locked │
                                             └────┬───┘
                                                  │
                                   [Unlock]       │       [Lock/Timeout]
                                                  │
                                                  ▼
                                             ┌──────────┐
                                             │ Unlocked │
                                             └──────────┘
```

### State Transitions

| From State | To State | Trigger | Security Check |
|------------|----------|---------|----------------|
| Uninitialized | Locked | Set master password | Password strength validation |
| Locked | Unlocked | Unlock with password | Argon2id verification, rate limiting |
| Unlocked | Locked | Manual lock | Clear master key from memory |
| Unlocked | Locked | Auto-lock timeout | Clear master key from memory |
| Unlocked | Locked | System sleep/hibernate | Clear master key from memory |

### Rate Limiting

**Failed Unlock Attempts:**
- 5 failed attempts → 5 minute lockout
- 10 failed attempts → 15 minute lockout
- 15 failed attempts → 60 minute lockout

**Rationale:**
- Prevents brute force attacks
- With 500ms per attempt + lockouts, attacker can try ~1000 passwords/day
- Strong password (80 bits entropy) = 2^80 attempts = infeasible

## Memory Security

### Sensitive Data Lifecycle

```
1. Credential Requested
   ↓
2. Master Key Retrieved from VaultState (in memory)
   ↓
3. Credential Decrypted (plaintext in memory)
   ↓
4. Credential Used (copied to clipboard, passed to SSH)
   ↓
5. Credential Cleared (zeroize memory)
   ↓
6. On Vault Lock: Master Key Cleared (zeroize memory)
```

### Memory Clearing Strategy

**Rust `zeroize` Crate:**
- Overwrites memory with zeros
- Prevents compiler optimization from removing clearing code
- Used for: master key, decrypted passwords, encryption keys

**Limitations:**
- Cannot prevent OS from swapping memory to disk
- Cannot prevent debugger from reading memory
- Cannot prevent hardware attacks (cold boot, DMA)

**Best Practices:**
- Minimize time credentials are in plaintext
- Clear immediately after use
- Lock vault when not actively needed

## Audit Logging

### Events Logged

| Event Type | Logged Data | Purpose |
|------------|-------------|---------|
| `vault_unlock` | Timestamp, result (success/failure) | Track unauthorized access attempts |
| `vault_lock` | Timestamp | Track vault usage patterns |
| `credential_access` | Timestamp, credential_id, result | Track credential usage |
| `credential_create` | Timestamp, credential_id | Track credential lifecycle |
| `credential_update` | Timestamp, credential_id | Track credential modifications |
| `credential_delete` | Timestamp, credential_id | Track credential removal |
| `host_key_verify` | Timestamp, host, port, result | Track SSH connection security |
| `host_key_trust` | Timestamp, host, port, fingerprint | Track trusted host additions |
| `host_key_changed` | Timestamp, host, port, old/new fingerprint | Track potential MITM attacks |

### Audit Log Security

**Integrity:**
- Append-only (no deletion or modification)
- Stored in separate table from credentials
- Consider HMAC signing for tamper detection (future enhancement)

**Privacy:**
- Does NOT log plaintext credentials
- Does NOT log master password
- Logs only metadata (IDs, timestamps, results)

**Retention:**
- Keep last 10,000 entries (configurable)
- Older entries archived or deleted
- Export functionality for long-term storage

## Compliance Considerations

### NIST 800-53 Alignment

| Control | Implementation |
|---------|----------------|
| IA-5 (Authenticator Management) | Master password with strength requirements |
| SC-12 (Cryptographic Key Establishment) | Argon2id key derivation |
| SC-13 (Cryptographic Protection) | AES-256-GCM encryption |
| AU-2 (Audit Events) | Comprehensive audit logging |
| AU-3 (Content of Audit Records) | Timestamp, event type, result, resource ID |

### GDPR Considerations

- **Data Minimization:** Only store necessary credential metadata
- **Right to Erasure:** Delete credential command
- **Data Portability:** Export credentials in standard format
- **Security of Processing:** Encryption at rest and in transit

## Known Limitations

### What This System DOES Protect Against

✅ Database theft (encrypted at rest)  
✅ SSH MITM attacks (host key verification)  
✅ Brute force attacks (Argon2id + rate limiting)  
✅ Credential leakage from database  
✅ Unauthorized credential access (audit log)  

### What This System DOES NOT Protect Against

❌ Keyloggers (OS-level threat)  
❌ Screen capture malware (OS-level threat)  
❌ Compromised operating system  
❌ Physical access to unlocked workstation (auto-lock helps)  
❌ User choosing weak master password (strength meter helps)  
❌ Memory dumps while vault unlocked (minimize exposure time)  
❌ Hardware attacks (cold boot, DMA)  

### Recommendations for Users

1. **Use a strong master password** (12+ characters, mixed case, numbers, symbols)
2. **Enable auto-lock** (15 minutes or less)
3. **Lock workstation when away** (OS-level screen lock)
4. **Keep OS and antivirus updated** (prevent malware)
5. **Use full-disk encryption** (BitLocker, FileVault, LUKS)
6. **Verify SSH host keys** (don't blindly trust)
7. **Review audit logs periodically** (detect unauthorized access)

## Security Review Checklist

Before marking this track complete, verify:

- [ ] All credentials encrypted with AES-256-GCM
- [ ] Master key derived with Argon2id (64 MB, 3 iterations)
- [ ] Unique nonce per credential
- [ ] Master key cleared from memory on lock
- [ ] Decrypted credentials cleared after use
- [ ] SSH host keys verified before connection
- [ ] Host key changes trigger warning
- [ ] Rate limiting prevents brute force
- [ ] Auto-lock timer functions correctly
- [ ] Audit log captures all credential access
- [ ] No plaintext credentials in database (verified via inspection)
- [ ] No plaintext credentials in memory dumps (tested)
- [ ] Password strength requirements enforced
- [ ] Export/import maintains encryption
- [ ] All security tests passing

## References

- [NIST SP 800-38D](https://csrc.nist.gov/publications/detail/sp/800-38d/final) - GCM Mode
- [RFC 9106](https://www.rfc-editor.org/rfc/rfc9106.html) - Argon2 Specification
- [NIST SP 800-53](https://csrc.nist.gov/publications/detail/sp/800-53/rev-5/final) - Security Controls
- [OpenSSH known_hosts Format](https://man.openbsd.org/sshd.8#SSH_KNOWN_HOSTS_FILE_FORMAT)
- [OWASP Cryptographic Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Cryptographic_Storage_Cheat_Sheet.html)

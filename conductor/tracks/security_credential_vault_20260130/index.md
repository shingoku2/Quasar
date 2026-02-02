# Security & Credential Vault Track

**Status:** Not Started  
**Priority:** High  
**Created:** 2026-01-30

## Overview

Implement secure credential storage and management with encryption, SSH host key verification, and comprehensive security hardening. This track addresses critical security gaps and fulfills the v0.1 MVP requirement for a "secure credential vault with Argon2id key derivation and libsodium encryption."

## Objectives

1. Integrate existing crypto functions with credential storage
2. Implement SSH host key verification to prevent MITM attacks
3. Build credential vault UI for managing encrypted credentials
4. Harden security across the application with audit logging

## Key Deliverables

- Encrypted credential storage with master password
- SSH known_hosts management system
- Credential vault UI with search and organization
- Security audit logging and session management
- Security settings panel

## Success Metrics

- All credentials encrypted at rest with AES-256-GCM
- SSH host key verification prevents MITM attacks
- Master password unlock time < 500ms
- Zero plaintext credentials in database or memory dumps
- Audit log captures all credential access events

## Documentation

- [Specification](./spec.md) - Detailed technical specification
- [Implementation Plan](./plan.md) - Phase-by-phase implementation guide
- [Security Model](./security-model.md) - Threat model and security architecture

## Related Tracks

- Asset Discovery & Monitoring (credential usage)
- Remote Connection Manager (SSH/RDP authentication)

## Dependencies

- Existing `crypto.rs` module (Argon2id, AES-256-GCM)
- SQLite database for credential storage
- SSH module for host key management

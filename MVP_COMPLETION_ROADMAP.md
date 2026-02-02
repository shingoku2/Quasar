# ProjectTitan v0.1 MVP Completion Roadmap

**Status**: ~85% Complete (Updated: February 1, 2026 - 8:50 PM)

## Completed Core Features ✅

### Infrastructure & Security
- ✅ SSH terminal connections with xterm.js
- ✅ Secure credential vault (Argon2id + AES-256-GCM)
- ✅ SSH host key verification (MITM protection)
- ✅ Vault UI with auto-lock and audit logging
- ✅ Master password management and credential re-encryption

### Discovery & Monitoring
- ✅ Network scanning (ping + port scan)
- ✅ mDNS auto-discovery
- ✅ System monitoring (CPU/RAM/disk/network)
- ✅ Alert engine with rules
- ✅ Real-time metrics collection

### Automation & Workflows
- ✅ Workflow automation framework (definitions, execution)
- ✅ **Real SSH Command Execution** (NEW - Feb 1, 2026)
  - Location: `src-tauri/src/ssh_exec.rs` (280 lines)
  - Single command execution with timeout
  - Batch command execution on same session
  - System metrics collection via SSH
  - Integrated into automation engine
- ✅ **SSH Health Checks** (NEW - Feb 1, 2026)
  - Pre-flight ping validation
  - Full system metrics via SSH (CPU, RAM, disk, uptime, load)
  - Credential-optional design

## Remaining Critical Gaps (20%)

### 1. SFTP File Transfer ✅ COMPLETED
**Priority**: High  
**Completed**: February 1, 2026  
**Status**: Fully implemented and integrated

**Implementation Complete**:
- ✅ Upload files to remote hosts via SFTP
- ✅ Download files from remote hosts via SFTP
- ✅ List remote directory contents
- ✅ Check remote file existence
- ✅ Progress callback infrastructure (ready for UI)
- ✅ Integration with workflow FileTransfer action
- ✅ Comprehensive error handling

**Files Created/Modified**:
- `src-tauri/src/sftp.rs` (380 lines) - Complete SFTP client
- `src-tauri/Cargo.toml` - Added russh-sftp dependency
- `src-tauri/src/automation/engine.rs` - FileTransfer action implementation
- `src-tauri/src/lib.rs` - Added 4 Tauri commands for frontend

**Features**:
- 32KB chunk transfers for efficiency
- 10-second connection timeout
- Proper resource cleanup
- Host format: `username@host:port`
- Password from execution context

---

### 3. VNC Client Integration
**Priority**: Medium  
**Estimated Time**: 3-5 days  
**Status**: Not started

**Requirements**:
- VNC protocol support (RFB)
- Embedded VNC viewer in UI
- Mouse/keyboard input forwarding
- Screen updates and rendering
- Authentication (password, none)

**Implementation Options**:
1. **noVNC** (JavaScript VNC client)
   - Pros: Web-based, no native dependencies
   - Cons: Requires WebSocket proxy
   
2. **rust-vnc** crate
   - Pros: Native Rust, direct integration
   - Cons: May need custom UI rendering

**Recommended Approach**: noVNC + WebSocket proxy
```rust
// src-tauri/src/vnc.rs
- VncProxy with tokio WebSocket
- Forward VNC traffic to/from remote host
- Session management similar to SSH
```

---

### 4. Database Schema Migration ✅ COMPLETED
**Priority**: Low (Technical Debt)  
**Completed**: February 2, 2026  
**Status**: Fully implemented

**Changes**:
- ✅ Consolidated `credentials_new` into `credentials`
- ✅ Added migration `005_consolidate_credentials.sql`
- ✅ Updated all backend references to use the new table name
- ✅ Improved application startup error handling in `lib.rs`
- ✅ Added `tauri-plugin-dialog` for native file management

---

## Implementation Timeline

### Week 1 (Feb 2-8, 2026)
- **Days 1-3**: SFTP file transfer implementation
  - Core SFTP client
  - Upload/download functions
  - Integration with automation engine
  - Frontend file picker UI

- **Days 4-5**: Workflow scheduling daemon
  - Cron scheduler setup
  - Database schema and persistence
  - Background task execution

### Week 2 (Feb 9-15, 2026)
- **Days 1-2**: Workflow scheduling (continued)
  - Webhook listener
  - Frontend schedule management UI
  - Testing and debugging

- **Days 3-5**: VNC client integration
  - noVNC setup and WebSocket proxy
  - Session management
  - Frontend VNC viewer component

### Week 3 (Feb 16-22, 2026)
- **Days 1-2**: VNC (continued)
  - Authentication and security
  - Performance optimization
  - Error handling

- **Day 3**: Database migration
  - Schema consolidation
  - Data migration script
  - Frontend updates

- **Days 4-5**: Testing, bug fixes, documentation
  - End-to-end testing
  - Performance testing
  - User documentation

---

## Testing Strategy

### Unit Tests
- ✅ SSH command execution
- ✅ Health check metrics parsing
- 🔄 SFTP upload/download
- ⏳ Cron schedule parsing
- ⏳ VNC protocol handling

### Integration Tests
- ✅ Workflow execution with SSH commands
- 🔄 File transfer in workflows
- ⏳ Scheduled workflow execution
- ⏳ VNC session management

### E2E Tests
- ⏳ Complete workflow with SSH + SFTP
- ⏳ Scheduled automation execution
- ⏳ Multi-protocol remote management (SSH + VNC)

---

## Success Criteria for v0.1 MVP

- [x] SSH terminal connections work reliably
- [x] Credential vault is secure and functional
- [x] Network discovery finds hosts automatically
- [x] System monitoring displays real-time metrics
- [x] Workflows can execute SSH commands
- [x] Health checks provide pre-flight validation
- [ ] SFTP file transfers work in workflows
- [ ] Workflows can be scheduled with cron
- [ ] VNC connections allow remote desktop access
- [ ] Database schema is clean and consolidated
- [ ] All features have basic error handling
- [ ] Documentation covers core workflows

---

## Post-MVP Enhancements (v0.2+)

### Phase 8: SSH Feature Enhancements
- SSH tunneling (local/remote port forwarding)
- SOCKS proxy support
- SSH agent forwarding
- Custom terminal themes and fonts
- Session recording and playback

### Phase 9: Monitoring & Alerting
- Custom metric collectors
- Alert notification channels (email, Slack, etc.)
- Metric history and trending
- Dashboard customization
- Alert rule templates

### Phase 10: Automation Enhancements
- Visual workflow builder (drag-and-drop)
- Workflow templates library
- Conditional branching (if/else)
- Loop constructs (for-each, while)
- Error handling and retry policies
- Workflow versioning

### Phase 11: Multi-User & Permissions
- User authentication and authorization
- Role-based access control (RBAC)
- Credential sharing policies
- Audit log for all user actions
- Team collaboration features

---

## Known Issues & Technical Debt

### Minor Issues (Not Blocking MVP)
1. Unused variable warnings in automation engine
2. Unused imports in various modules
3. Terminal resize observer potential memory leak
4. Inconsistent error message formatting

### Future Improvements
1. Connection pooling for SSH sessions
2. Credential caching with TTL
3. Workflow execution queue management
4. Better progress feedback for long-running tasks
5. Comprehensive logging framework

---

## Dependencies & Requirements

### Rust Crates (Additional)
```toml
[dependencies]
# Existing...
tokio-cron-scheduler = "0.9"  # For cron scheduling
axum = "0.7"                  # For webhook listener (alternative: actix-web)
tower = "0.4"                 # Middleware for axum
tokio-tungstenite = "0.21"    # WebSocket for VNC proxy
```

### Frontend Dependencies (Additional)
```json
{
  "dependencies": {
    "@novnc/novnc": "^1.4.0",  // VNC client
    "cron-parser": "^4.9.0"     // Cron expression validation
  }
}
```

---

## Conclusion

With the completion of real SSH command execution and health checks, ProjectTitan is now **~80% complete** for the v0.1 MVP. The remaining work focuses on:

1. **SFTP** - Essential for file management workflows
2. **Scheduling** - Enables automation without manual triggers
3. **VNC** - Completes the "SSH, RDP, and VNC" product requirement
4. **Database cleanup** - Technical debt that should be addressed

**Estimated time to v0.1 MVP**: 2-3 weeks of focused development.

The foundation is solid, and all critical security and infrastructure components are in place. The remaining features are well-defined and can be implemented incrementally without major architectural changes.

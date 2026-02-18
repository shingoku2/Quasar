# Quasar v0.1 MVP Completion Roadmap

**Status**: ~90% Complete (Updated: February 18, 2026)

### Recent Progress (February 18, 2026)
- **Workflow scheduling**: Cron-based scheduled tasks (migration 010/011), `scheduler.rs` background loop (60s), CRUD + `run_scheduled_task_now`; Automation view with list/add/edit/delete, last run status (success/failure + error/output), and **Run now** button with result panel.
- **Scheduler tests**: In-memory DB tests for CRUD, set_run_result, load_enabled_tasks, load_task_by_id, output truncation; cron 6-field format in UI (`0 0 9 * * *`).
- **Documentation**: `docs/CORE_WORKFLOWS.md` — core workflows (vault, SSH, scheduled tasks, SFTP, monitoring, discovery; cron quick reference). Success criterion "Documentation covers core workflows" ✅.
- SSH terminal: refactored to use `Channel::wait()` for data; fixes hang on consecutive/simultaneous commands (flow control).
- Dashboard: System Health shows Hosts Online count and vault timeout; Real-time Metrics shows all attached disks.
- UX: password autocomplete on inputs; SSH packet size 32 KB; launcher/tauri-dev spawn fix on Windows.

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
- ✅ **Real SSH Command Execution** (Feb 1, 2026)
  - Location: `src-tauri/src/ssh_exec.rs`
  - Single command execution with timeout
  - Batch command execution on same session
  - System metrics collection via SSH
  - Integrated into automation engine
- ✅ **SSH Health Checks** (Feb 1, 2026)
  - Pre-flight ping validation
  - Full system metrics via SSH (CPU, RAM, disk, uptime, load)
  - Credential-optional design
- ✅ **Workflow scheduling (cron)** (Feb 18, 2026)
  - `scheduler.rs`: 60s loop, load enabled tasks, run due via cron expression, SSH with saved host + optional credential
  - Migrations 010 (`scheduled_tasks`), 011 (last_run_status, last_run_error, last_run_output)
  - Automation view: list/add/edit/delete tasks, last run status and output, **Run now** with result panel
  - Tauri commands: list/get/add/update/remove scheduled tasks, run_scheduled_task_now

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
- `src-tauri/src/scheduler.rs` + `sftp.rs` + `ssh_exec.rs` - Scheduled SFTP/SSH execution (no automation/engine.rs; workflow execution is scheduler + Tauri commands)
- `src-tauri/src/lib.rs` - Added 4 Tauri commands for frontend

**Features**:
- 32KB chunk transfers for efficiency
- 10-second connection timeout
- Proper resource cleanup
- Host format: `username@host:port`
- Password from execution context

---

### 3. Database Schema Migration ✅ COMPLETED
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

- **Days 3-5**: Database migration and testing
  - Schema consolidation
  - Data migration script
  - Frontend updates

### Week 3 (Feb 16-22, 2026)
- **Days 1-3**: Testing and polish
  - End-to-end testing
  - Performance testing
  - Error handling

- **Days 4-5**: Documentation and bug fixes
  - User documentation
  - Bug fixes and polish

---

## Testing Strategy

### Unit Tests
- ✅ SSH command execution
- ✅ Health check metrics parsing
- 🔄 SFTP upload/download
- ✅ Cron schedule parsing (scheduler `is_due` test)
- ✅ Scheduler CRUD and run result (add/list/get/update/set_run_result/remove, load_enabled_tasks, load_task_by_id, output truncation)

### Integration Tests
- ✅ Workflow execution with SSH commands
- 🔄 File transfer in workflows
- ✅ Scheduled task execution (scheduler loop + run_scheduled_task_now)

### E2E Tests
- ⏳ Complete workflow with SSH + SFTP
- ⏳ Scheduled automation execution

---

## Success Criteria for v0.1 MVP

- [x] SSH terminal connections work reliably
- [x] Credential vault is secure and functional
- [x] Network discovery finds hosts automatically
- [x] System monitoring displays real-time metrics
- [x] Workflows can execute SSH commands
- [x] Health checks provide pre-flight validation
- [x] SFTP file transfers work in workflows
- [x] Workflows can be scheduled with cron
- [x] Database schema is clean and consolidated
- [x] All features have basic error handling
- [x] Documentation covers core workflows

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
```

### Frontend Dependencies (Additional)
```json
{
  "dependencies": {
    "cron-parser": "^4.9.0"     // Cron expression validation
  }
}
```

---

## Conclusion

With the completion of SFTP in scheduled workflows, database schema documentation, and error-handling confirmation, Quasar meets all **v0.1 MVP success criteria**.

**Completed this phase**:
- **SFTP in workflows**: Scheduled tasks support task types SSH command, SFTP upload, and SFTP download; backend runs the appropriate action; UI includes task type selector and path fields.
- **Database schema**: Documented in `docs/SCHEMA.md`; single `credentials` table (consolidated); AGENTS.md updated.
- **Error handling**: Backend commands return `Result` and use `sanitize_error`; scheduled-task contexts added; documented in `docs/SCHEMA.md`.

**Optional polish**: E2E test for scheduled task execution; terminal resize observer if needed. *(Remote desktop is out of scope; use external tools such as RustDesk.)*

The foundation is solid, and all critical security and infrastructure components are in place. The remaining features are well-defined and can be implemented incrementally without major architectural changes.

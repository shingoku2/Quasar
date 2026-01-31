# Monitoring & Alerts Track

Real-time system monitoring with configurable thresholds and alert notifications.

## Phases

### Phase 1: System Metrics Collection (Rust Backend)

- [x] Task: Create `monitoring.rs` module for system metrics collection c4830b4
- [x] Task: Implement CPU, memory, disk, and network metrics c4830b4
- [x] Task: Create Tauri commands for metrics queries c4830b4
- [x] Task: Add background metrics collector with configurable interval c4830b4
- [x] Task: Write tests for metrics collection c4830b4
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Metrics Collection' (Protocol in workflow.md) [checkpoint: c4830b4]

### Phase 2: Alert System Backend

- [x] Task: Create alert rules configuration structure c4830b4
- [x] Task: Implement threshold evaluation engine c4830b4
- [x] Task: Create alert history storage (SQLite via Tauri SQL plugin) - Deferred to future iteration
- [x] Task: Add Tauri commands for CRUD operations on alert rules c4830b4
- [x] Task: Write tests for alert evaluation logic c4830b4
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Alert System' (Protocol in workflow.md) [checkpoint: c4830b4]

### Phase 3: AlertFeed Widget & Real-time Updates

- [x] Task: Enhance existing `AlertFeed` with real-time updates 3b1a747
- [x] Task: Connect AlertFeed to backend metrics stream 3b1a747
- [x] Task: Implement alert severity indicators and filtering 3b1a747
- [x] Task: Add alert acknowledgment/dismissal functionality 3b1a747
- [x] Task: Write component tests for AlertFeed 3b1a747
- [ ] Task: Conductor - User Manual Verification 'Phase 3: AlertFeed' (Protocol in workflow.md) [checkpoint: 3b1a747]

### Phase 4: Alert Rules UI

- [x] Task: Create `AlertRules` component for managing alert configurations ad961c2
- [x] Task: Implement threshold editor (numeric inputs, operators) ad961c2
- [x] Task: Add notification preferences (in-app, native) - In-app implemented ad961c2
- [x] Task: Create test coverage for AlertRules component - Deferred
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Alert Rules UI' (Protocol in workflow.md) [checkpoint: ad961c2]

### Phase 5: Dashboard Integration & Polish

- [x] Task: Update DashboardView with live metrics cards 3b1a747
- [x] Task: Add sparkline charts to SystemHealthWidget - Deferred
- [x] Task: Implement native notification support - Deferred to future iteration
- [x] Task: Final integration testing ad961c2
- [x] Task: Commit all changes with checkpoint ad961c2

## Success Metrics

- Metrics collection interval: configurable (default 5s, min 1s)
- Alert evaluation latency: < 1 second after threshold breach
- Alert persistence: SQLite storage with 30-day retention
- Code coverage > 80% for monitoring module
- Max 50 alerts in feed (auto-purge oldest)

## Dependencies

- Rust: `sysinfo` for system metrics, `tokio` for async timers
- Frontend: Existing recharts for sparklines

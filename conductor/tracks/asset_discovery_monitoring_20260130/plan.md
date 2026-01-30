# Implementation Plan: Asset Discovery & Monitoring

This plan outlines the steps to implement automated LAN discovery and pre-flight health monitoring for Project Titan.

## Phase 1: LAN Scanner Backend

Implement the core network scanning functionality in Rust with async/concurrent scanning.

- [x] Task: Add Rust dependencies (tokio, surge-ping, trust-dns-resolver)
- [x] Task: Implement ICMP ping scanner with concurrent execution
- [x] Task: Implement TCP port scanner for common service ports
- [x] Task: Create Tauri commands: `scan_network`, `stop_scan`, `get_scan_progress`
- [x] Task: Add scan results emitter for real-time UI updates
- [~] Task: Write unit tests for scanner modules
- [ ] Task: Conductor - User Manual Verification 'Phase 1: LAN Scanner' (Protocol in workflow.md)

## Phase 2: Discovery UI & Integration

Build the user interface for network scanning and discovered host management.

- [ ] Task: Create `NetworkScanner` component with start/stop controls
- [ ] Task: Implement `DiscoveryResults` list with add-to-inventory actions
- [ ] Task: Add scan progress indicator (progress bar, stats)
- [ ] Task: Update `Discovery.tsx` to use new embedded scanner (remove mDNS-only limitation)
- [ ] Task: Implement host suggestion dialog for newly discovered devices
- [ ] Task: Write component tests for Discovery UI
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Discovery UI' (Protocol in workflow.md)

## Phase 3: Pre-flight Health Checks (Sidecar)

Implement sidecar monitoring for pre-connection health diagnostics.

- [ ] Task: Create sidecar Rust module for health check queries
- [ ] Task: Implement SSH-based health check (CPU, RAM, disk, uptime)
- [ ] Task: Implement WMI/WinRM health check for Windows targets
- [ ] Task: Create `HealthCheckBadge` component for HostList items
- [ ] Task: Add pre-flight dialog showing health metrics before connect
- [ ] Task: Write tests for health check commands
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Health Checks' (Protocol in workflow.md)

## Phase 4: Dashboard Integration

Integrate discovery and monitoring into the main dashboard view.

- [ ] Task: Create `DiscoveryWidget` for dashboard showing recent discoveries
- [ ] Task: Create `NetworkMapWidget` simple visualization of discovered hosts
- [ ] Task: Update `DashboardView.tsx` to include new widgets
- [ ] Task: Add auto-refresh mechanism for health indicators
- [ ] Task: Final integration testing and performance verification (<10s scan target)
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Dashboard Integration' (Protocol in workflow.md)

## Success Metrics

- LAN scan (/24 subnet) completes in < 10 seconds
- UI remains responsive during scanning
- Health check queries complete in < 3 seconds
- Code coverage > 80% for new modules

# Specification: Asset Discovery & Monitoring

## Overview

<<<<<<< HEAD
This track implements automated network discovery and real-time system health monitoring for Quasar. It enables zero-config LAN scanning, automated asset inventory population, and pre-flight health checks before establishing remote connections.
=======
This track implements automated network discovery and real-time system health monitoring for Project Titan. It enables zero-config LAN scanning, automated asset inventory population, and pre-flight health checks before establishing remote connections.
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7

## Functional Requirements

### 1. LAN Network Scanner
- **Fast Subnet Scanning**: Scan a /24 network in < 10 seconds (per MVP requirements)
- **Multi-protocol Detection**: Detect hosts via ICMP ping, common ports (22, 3389, 80, 443, 445)
- **OS Fingerprinting**: Basic OS detection from TTL values and port responses
- **Progressive Discovery**: Background scanning that doesn't block the UI

### 2. Asset Discovery Integration
- **Auto-merge Discovered Hosts**: Match discovered IPs with existing inventory
- **New Host Prompt**: Prompt user to add newly discovered hosts to inventory
- **Protocol Suggestion**: Suggest likely protocol (SSH/RDP) based on open ports
- **Conflict Resolution**: Handle IP/hostname changes for existing assets

### 3. Pre-flight Health Monitoring (Sidecar)
- **CPU Usage Check**: Query target system CPU load before connection
- **Memory Usage**: Check available RAM on target
- **Disk Health**: Basic disk space/health indicators
- **Uptime**: System uptime metric
- **Network Latency**: Pre-connection ping/response time

### 4. Dashboard Widgets
- **Discovery Status**: Visual indicator of scan progress
- **Network Map**: Simple visualization of discovered hosts
- **Health Indicators**: Traffic light status for monitored systems
- **Recent Discoveries**: List of newly found devices

## Non-Functional Requirements

- **Performance**: LAN scan (/24) must complete in < 10 seconds
- **Non-blocking**: Discovery must not freeze the UI
- **Resource Efficiency**: Background scanning should use < 5% CPU
- **Cross-platform**: Works on Windows, macOS, and Linux

## Acceptance Criteria

- [ ] Users can trigger a network scan and see progress in real-time
- [ ] Discovered hosts can be added to inventory with one click
- [ ] Pre-flight health checks display before establishing connections
- [ ] Dashboard shows discovery status and recent findings
- [ ] Scanning works on all three target platforms (Windows, macOS, Linux)

## Out of Scope

- Continuous/real-time monitoring (alerting)
- SNMP-based monitoring
- Advanced network topology mapping
- Historical performance data storage
- External monitoring agent deployment

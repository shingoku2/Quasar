-- Migration 006: Discovered Hosts Tracking
-- Created: February 2, 2026
-- Purpose: Track discovered network hosts and their services for historical analysis

-- Discovered Hosts Table
CREATE TABLE IF NOT EXISTS discovered_hosts (
    id TEXT PRIMARY KEY,
    ip TEXT NOT NULL UNIQUE,
    hostname TEXT,
    mac_address TEXT,
    device_type TEXT NOT NULL DEFAULT 'unknown',
    vendor TEXT,
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    scan_count INTEGER NOT NULL DEFAULT 1,
    metadata TEXT,
    UNIQUE(ip)
);

CREATE INDEX IF NOT EXISTS idx_discovered_hosts_ip ON discovered_hosts(ip);
CREATE INDEX IF NOT EXISTS idx_discovered_hosts_last_seen ON discovered_hosts(last_seen);
CREATE INDEX IF NOT EXISTS idx_discovered_hosts_device_type ON discovered_hosts(device_type);

-- Host Services Table
CREATE TABLE IF NOT EXISTS host_services (
    id TEXT PRIMARY KEY,
    host_id TEXT NOT NULL,
    port INTEGER NOT NULL,
    protocol TEXT NOT NULL,
    service TEXT NOT NULL,
    version TEXT,
    first_detected INTEGER NOT NULL,
    last_detected INTEGER NOT NULL,
    FOREIGN KEY (host_id) REFERENCES discovered_hosts(id) ON DELETE CASCADE,
    UNIQUE(host_id, port, protocol)
);

CREATE INDEX IF NOT EXISTS idx_host_services_host ON host_services(host_id);
CREATE INDEX IF NOT EXISTS idx_host_services_port ON host_services(port);

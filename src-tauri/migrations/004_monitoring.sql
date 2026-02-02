-- Monitoring Enhancement - Phase 2: Data Persistence
-- Created: February 2, 2026

-- Metrics history table
CREATE TABLE IF NOT EXISTS metrics_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    host TEXT NOT NULL DEFAULT 'localhost',
    
    -- Core metrics
    cpu_usage REAL,
    memory_usage REAL,
    disk_usage REAL,
    
    -- Network metrics
    network_rx INTEGER,
    network_tx INTEGER,
    network_packets_rx INTEGER,
    network_packets_tx INTEGER,
    
    -- System info
    load_avg_1m REAL,
    load_avg_5m REAL,
    load_avg_15m REAL,
    process_count INTEGER,
    uptime INTEGER,
    
    -- Additional metadata stored as JSON
    metadata TEXT
);

CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON metrics_history(timestamp);
CREATE INDEX IF NOT EXISTS idx_metrics_host ON metrics_history(host);
CREATE INDEX IF NOT EXISTS idx_metrics_host_timestamp ON metrics_history(host, timestamp);

-- Alert history table
CREATE TABLE IF NOT EXISTS alert_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alert_id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    host TEXT NOT NULL DEFAULT 'localhost',
    message TEXT NOT NULL,
    severity TEXT NOT NULL,
    triggered_at INTEGER NOT NULL,
    acknowledged_at INTEGER,
    dismissed_at INTEGER,
    resolved_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_alert_history_triggered ON alert_history(triggered_at);
CREATE INDEX IF NOT EXISTS idx_alert_history_rule ON alert_history(rule_id);
CREATE INDEX IF NOT EXISTS idx_alert_history_host ON alert_history(host);

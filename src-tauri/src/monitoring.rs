use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, System};
use tauri::{Emitter, Manager};
use tokio::time::interval;
use log::{info, error, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    // Existing metrics
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_usage_percent: f32,
    pub disk_read_mb: u64,
    pub disk_write_mb: u64,
    pub network_rx_mb: u64,
    pub network_tx_mb: u64,
    pub timestamp: u64,
    
    // System info
    pub uptime_seconds: u64,
    pub load_average_1m: f32,
    pub load_average_5m: f32,
    pub load_average_15m: f32,
    pub process_count: usize,
    pub boot_time: u64,
    
    // CPU details
    pub cpu_count: usize,
    pub cpu_per_core: Vec<f32>,
    pub cpu_frequency_mhz: u64,
    
    // Disk details (aggregate + per-disk)
    pub disk_total_gb: u64,
    pub disk_used_gb: u64,
    pub disk_free_gb: u64,
    pub disk_usage_percent: f32,
    pub disks: Vec<DiskInfo>,
    
    // Network details
    pub network_packets_rx: u64,
    pub network_packets_tx: u64,
    pub network_errors_rx: u64,
    pub network_errors_tx: u64,
    
    // Top processes
    pub top_cpu_processes: Vec<ProcessInfo>,
    pub top_memory_processes: Vec<ProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_mb: u64,
}

/// Per-disk usage for display in the UI (all attached disks).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_gb: u64,
    pub used_gb: u64,
    pub free_gb: u64,
    pub usage_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: String,
    pub metric: MetricType,
    pub operator: ComparisonOperator,
    pub threshold: f64,
    pub severity: AlertSeverity,
    pub enabled: bool,
    #[serde(default = "default_cooldown")]
    pub cooldown_seconds: u64,
}

fn default_cooldown() -> u64 {
    300 // 5 minutes default cooldown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    CpuUsage,
    MemoryUsage,
    DiskUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    Equals,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub rule_id: String,
    pub message: String,
    pub severity: AlertSeverity,
    pub timestamp: u64,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRecovery {
    pub rule_id: String,
    pub message: String,
    pub recovered_at: u64,
}

pub struct MetricsCollector {
    system: System,
    last_disk_read: u64,
    last_disk_write: u64,
    last_network_rx: u64,
    last_network_tx: u64,
    last_update: Instant,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        let (disk_read, disk_write) = Self::get_disk_io(&disks);
        let (net_rx, net_tx) = Self::get_network_io(&networks);

        Self {
            system,
            last_disk_read: disk_read,
            last_disk_write: disk_write,
            last_network_rx: net_rx,
            last_network_tx: net_tx,
            last_update: Instant::now(),
        }
    }

    pub fn collect(&mut self) -> SystemMetrics {
        self.system.refresh_cpu_all();
        self.system.refresh_memory();

        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        // Existing metrics
        let cpu_usage = self.system.global_cpu_usage();
        let memory_used = self.system.used_memory();
        let memory_total = self.system.total_memory();
        let memory_usage_percent = if memory_total > 0 {
            (memory_used as f32 / memory_total as f32) * 100.0
        } else {
            0.0
        };

        let (disk_read, disk_write) = Self::get_disk_io(&disks);
        let (net_rx, net_tx) = Self::get_network_io(&networks);

        let now = Instant::now();
        let elapsed_secs = now.duration_since(self.last_update).as_secs_f64().max(1.0);

        let disk_read_rate = ((disk_read.saturating_sub(self.last_disk_read)) as f64 / elapsed_secs) as u64;
        let disk_write_rate = ((disk_write.saturating_sub(self.last_disk_write)) as f64 / elapsed_secs) as u64;
        let net_rx_rate = ((net_rx.saturating_sub(self.last_network_rx)) as f64 / elapsed_secs) as u64;
        let net_tx_rate = ((net_tx.saturating_sub(self.last_network_tx)) as f64 / elapsed_secs) as u64;

        self.last_disk_read = disk_read;
        self.last_disk_write = disk_write;
        self.last_network_rx = net_rx;
        self.last_network_tx = net_tx;
        self.last_update = now;

        // New metrics - System info
        let load_avg = System::load_average();
        let uptime = System::uptime();
        let boot_time = System::boot_time();
        let process_count = self.system.processes().len();

        // New metrics - CPU details
        let cpu_count = self.system.cpus().len();
        let cpu_per_core: Vec<f32> = self.system.cpus().iter().map(|cpu| cpu.cpu_usage()).collect();
        let cpu_frequency_mhz = self.system.cpus().first().map(|cpu| cpu.frequency()).unwrap_or(0);

        // New metrics - Disk details (aggregate + per-disk list)
        let (disk_total_gb, disk_used_gb, disk_free_gb, disk_usage_percent) = Self::calculate_disk_space(&disks);
        let disks_list = Self::get_disk_list(&disks);

        // New metrics - Network details
        let (network_packets_rx, network_packets_tx, network_errors_rx, network_errors_tx) = Self::get_network_packets(&networks);

        // New metrics - Top processes
        let top_cpu_processes = self.get_top_processes_by_cpu(5);
        let top_memory_processes = self.get_top_processes_by_memory(5);

        SystemMetrics {
            // Existing metrics
            cpu_usage_percent: cpu_usage,
            memory_used_mb: memory_used / 1024 / 1024,
            memory_total_mb: memory_total / 1024 / 1024,
            memory_usage_percent,
            disk_read_mb: disk_read_rate / 1024 / 1024,
            disk_write_mb: disk_write_rate / 1024 / 1024,
            network_rx_mb: net_rx_rate / 1024 / 1024,
            network_tx_mb: net_tx_rate / 1024 / 1024,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            
            // System info
            uptime_seconds: uptime,
            load_average_1m: load_avg.one as f32,
            load_average_5m: load_avg.five as f32,
            load_average_15m: load_avg.fifteen as f32,
            process_count,
            boot_time,
            
            // CPU details
            cpu_count,
            cpu_per_core,
            cpu_frequency_mhz,
            
            // Disk details
            disk_total_gb,
            disk_used_gb,
            disk_free_gb,
            disk_usage_percent,
            disks: disks_list,

            // Network details
            network_packets_rx,
            network_packets_tx,
            network_errors_rx,
            network_errors_tx,
            
            // Top processes
            top_cpu_processes,
            top_memory_processes,
        }
    }

    fn get_disk_io(disks: &Disks) -> (u64, u64) {
        let mut read = 0u64;
        let mut write = 0u64;
        for disk in disks.list() {
            read += disk.usage().read_bytes;
            write += disk.usage().written_bytes;
        }
        (read, write)
    }

    fn get_network_io(networks: &Networks) -> (u64, u64) {
        let mut rx = 0u64;
        let mut tx = 0u64;
        for (_, data) in networks.iter() {
            rx += data.total_received();
            tx += data.total_transmitted();
        }
        (rx, tx)
    }

    fn get_network_packets(networks: &Networks) -> (u64, u64, u64, u64) {
        let mut packets_rx = 0u64;
        let mut packets_tx = 0u64;
        let mut errors_rx = 0u64;
        let mut errors_tx = 0u64;
        for (_, data) in networks.iter() {
            packets_rx += data.total_packets_received();
            packets_tx += data.total_packets_transmitted();
            errors_rx += data.total_errors_on_received();
            errors_tx += data.total_errors_on_transmitted();
        }
        (packets_rx, packets_tx, errors_rx, errors_tx)
    }

    fn calculate_disk_space(disks: &Disks) -> (u64, u64, u64, f32) {
        let mut total = 0u64;
        let mut available = 0u64;
        for disk in disks.list() {
            total = total.saturating_add(disk.total_space());
            available = available.saturating_add(disk.available_space());
        }
        let used = total.saturating_sub(available);
        let usage_percent = if total > 0 {
            (used as f64 / total as f64 * 100.0) as f32
        } else {
            0.0
        };
        // Convert bytes to GB
        let total_gb = total / 1024 / 1024 / 1024;
        let used_gb = used / 1024 / 1024 / 1024;
        let free_gb = available / 1024 / 1024 / 1024;
        (total_gb, used_gb, free_gb, usage_percent)
    }

    fn get_disk_list(disks: &Disks) -> Vec<DiskInfo> {
        disks.list().iter().map(|disk| {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);
            let usage_percent = if total > 0 {
                (used as f64 / total as f64 * 100.0) as f32
            } else {
                0.0
            };
            let total_gb = total / 1024 / 1024 / 1024;
            let used_gb = used / 1024 / 1024 / 1024;
            let free_gb = available / 1024 / 1024 / 1024;
            let name = disk.name().to_string_lossy().to_string();
            let mount_point = disk.mount_point().to_string_lossy().to_string();
            DiskInfo {
                name,
                mount_point,
                total_gb,
                used_gb,
                free_gb,
                usage_percent,
            }
        }).collect()
    }

    fn get_top_processes_by_cpu(&self, limit: usize) -> Vec<ProcessInfo> {
        let mut processes: Vec<_> = self.system.processes()
            .iter()
            .map(|(pid, process)| ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                cpu_usage: process.cpu_usage(),
                memory_mb: process.memory() / 1024 / 1024,
            })
            .collect();
        
        processes.sort_by(|a, b| b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap_or(std::cmp::Ordering::Equal));
        processes.truncate(limit);
        processes
    }

    fn get_top_processes_by_memory(&self, limit: usize) -> Vec<ProcessInfo> {
        let mut processes: Vec<_> = self.system.processes()
            .iter()
            .map(|(pid, process)| ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                cpu_usage: process.cpu_usage(),
                memory_mb: process.memory() / 1024 / 1024,
            })
            .collect();
        
        processes.sort_by(|a, b| b.memory_mb.cmp(&a.memory_mb));
        processes.truncate(limit);
        processes
    }
}

use std::collections::HashMap;

pub struct AlertEngine {
    rules: Arc<Mutex<Vec<AlertRule>>>,
    active_alerts: Arc<Mutex<Vec<Alert>>>,
    alert_counter: Arc<Mutex<u64>>,
    cooldown_tracker: Arc<Mutex<HashMap<String, Instant>>>,
    last_alert_state: Arc<Mutex<HashMap<String, bool>>>,
}

impl AlertEngine {
    pub fn new() -> Self {
        Self {
            rules: Arc::new(Mutex::new(Vec::new())),
            active_alerts: Arc::new(Mutex::new(Vec::new())),
            alert_counter: Arc::new(Mutex::new(0)),
            cooldown_tracker: Arc::new(Mutex::new(HashMap::new())),
            last_alert_state: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_rule(&self, rule: AlertRule) {
        let mut rules = self.rules.lock().unwrap();
        rules.retain(|r| r.id != rule.id);
        rules.push(rule);
    }

    pub fn remove_rule(&self, rule_id: &str) {
        let mut rules = self.rules.lock().unwrap();
        rules.retain(|r| r.id != rule_id);
    }

    pub fn get_rules(&self) -> Vec<AlertRule> {
        self.rules.lock().unwrap().clone()
    }

    pub fn evaluate(&self, metrics: &SystemMetrics) -> (Vec<Alert>, Vec<AlertRecovery>) {
        let rules = self.rules.lock().unwrap();
        let mut new_alerts = Vec::new();
        let mut recoveries = Vec::new();
        let mut counter = self.alert_counter.lock().unwrap();
        let mut cooldowns = self.cooldown_tracker.lock().unwrap();
        let mut alert_states = self.last_alert_state.lock().unwrap();

        for rule in rules.iter().filter(|r| r.enabled) {
            let value = match rule.metric {
                MetricType::CpuUsage => metrics.cpu_usage_percent as f64,
                MetricType::MemoryUsage => metrics.memory_usage_percent as f64,
                MetricType::DiskUsage => metrics.disk_usage_percent as f64,
            };

            let triggered = match rule.operator {
                ComparisonOperator::GreaterThan => value > rule.threshold,
                ComparisonOperator::LessThan => value < rule.threshold,
                ComparisonOperator::Equals => (value - rule.threshold).abs() < 0.01,
                ComparisonOperator::GreaterThanOrEqual => value >= rule.threshold,
                ComparisonOperator::LessThanOrEqual => value <= rule.threshold,
            };

            let was_triggered = alert_states.get(&rule.id).copied().unwrap_or(false);

            // Check for recovery
            if was_triggered && !triggered {
                recoveries.push(AlertRecovery {
                    rule_id: rule.id.clone(),
                    message: format!("{:?} recovered: now {:.1}% (threshold: {:.1}%)", rule.metric, value, rule.threshold),
                    recovered_at: metrics.timestamp,
                });
                alert_states.insert(rule.id.clone(), false);
                cooldowns.remove(&rule.id);
            }

            // Check if alert should be triggered
            if triggered {
                alert_states.insert(rule.id.clone(), true);

                // Check cooldown
                if let Some(last_alert_time) = cooldowns.get(&rule.id) {
                    if last_alert_time.elapsed().as_secs() < rule.cooldown_seconds {
                        continue; // Still in cooldown, skip alert
                    }
                }

                // Trigger alert
                *counter += 1;
                let alert = Alert {
                    id: format!("alert-{}", *counter),
                    rule_id: rule.id.clone(),
                    message: format!("{:?} is {:.1}% (threshold: {:.1}%)", rule.metric, value, rule.threshold),
                    severity: rule.severity.clone(),
                    timestamp: metrics.timestamp,
                    acknowledged: false,
                };
                new_alerts.push(alert);
                cooldowns.insert(rule.id.clone(), Instant::now());
            }
        }

        if !new_alerts.is_empty() {
            let mut active = match self.active_alerts.lock() {
                Ok(a) => a,
                Err(_) => return (new_alerts, recoveries),
            };
            
            // Check capacity before adding to prevent unbounded growth
            let current_len = active.len();
            let new_len = current_len.saturating_add(new_alerts.len());
            
            if new_len > 50 {
                // Remove oldest alerts to make room
                let to_remove = new_len.saturating_sub(50);
                // Ensure we don't try to remove more than we have
                if to_remove > 0 && current_len > 0 {
                    let remove_count = to_remove.min(current_len);
                    active.drain(0..remove_count);
                }
            }
            
            active.extend(new_alerts.clone());
            
            // Final safety check: if still over limit, truncate to 50
            let final_len = active.len();
            if final_len > 50 {
                active.drain(0..(final_len.saturating_sub(50)));
            }
        }

        (new_alerts, recoveries)
    }

    #[allow(dead_code)]
    pub fn get_active_alerts(&self) -> Vec<Alert> {
        self.active_alerts.lock()
            .map(|a| a.clone())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn acknowledge_alert(&self, alert_id: &str) {
        if let Ok(mut alerts) = self.active_alerts.lock() {
            if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
                alert.acknowledged = true;
            }
        }
    }

    #[allow(dead_code)]
    pub fn dismiss_alert(&self, alert_id: &str) {
        if let Ok(mut alerts) = self.active_alerts.lock() {
            alerts.retain(|a| a.id != alert_id);
        }
    }
}

pub struct MetricsStore {
    db_path: String,
    retention_days: u32,
    conn: std::sync::Mutex<Option<rusqlite::Connection>>,
}

impl MetricsStore {
    pub fn new(db_path: String, retention_days: u32) -> Result<Self, String> {
        Ok(Self {
            db_path,
            retention_days,
            conn: std::sync::Mutex::new(None),
        })
    }
    
    fn get_connection(&self) -> Result<std::sync::MutexGuard<'_, Option<rusqlite::Connection>>, String> {
        let mut conn_guard = self.conn.lock()
            .map_err(|e| format!("Failed to acquire connection lock: {}", e))?;
        
        // Check if connection exists and is valid
        if conn_guard.is_none() {
            let new_conn = rusqlite::Connection::open(&self.db_path)
                .map_err(|e| format!("Failed to open metrics database at {}: {}", self.db_path, e))?;
            *conn_guard = Some(new_conn);
        }
        
        Ok(conn_guard)
    }

    pub fn save_metrics(&self, metrics: &SystemMetrics, host: &str) -> Result<(), String> {
        let mut conn_guard = self.get_connection()?;
        let conn = conn_guard.as_mut()
            .ok_or_else(|| "Database connection not available".to_string())?;

        // Store core metrics and additional data as JSON
        let metadata = serde_json::json!({
            "cpu_count": metrics.cpu_count,
            "cpu_per_core": metrics.cpu_per_core,
            "cpu_frequency_mhz": metrics.cpu_frequency_mhz,
            "disk_total_gb": metrics.disk_total_gb,
            "disk_used_gb": metrics.disk_used_gb,
            "disk_free_gb": metrics.disk_free_gb,
            "boot_time": metrics.boot_time,
            "network_errors_rx": metrics.network_errors_rx,
            "network_errors_tx": metrics.network_errors_tx,
            "top_cpu_processes": metrics.top_cpu_processes,
            "top_memory_processes": metrics.top_memory_processes,
            "disks": metrics.disks,
        });

        conn.execute(
            "INSERT INTO metrics_history (
                timestamp, host, cpu_usage, memory_usage, disk_usage,
                network_rx, network_tx, network_packets_rx, network_packets_tx,
                load_avg_1m, load_avg_5m, load_avg_15m, process_count, uptime, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                metrics.timestamp as i64,
                host,
                metrics.cpu_usage_percent,
                metrics.memory_usage_percent,
                metrics.disk_usage_percent,
                metrics.network_rx_mb as i64,
                metrics.network_tx_mb as i64,
                metrics.network_packets_rx as i64,
                metrics.network_packets_tx as i64,
                metrics.load_average_1m,
                metrics.load_average_5m,
                metrics.load_average_15m,
                metrics.process_count as i64,
                metrics.uptime_seconds as i64,
                metadata.to_string(),
            ],
        ).map_err(|e| format!("Failed to insert metrics for host '{}' at {}: {}", host, metrics.timestamp, e))?;

        Ok(())
    }

    pub fn get_metrics_range(&self, start: u64, end: u64, host: &str) -> Result<Vec<SystemMetrics>, String> {
        let mut conn_guard = self.get_connection()?;
        let conn = conn_guard.as_mut()
            .ok_or_else(|| "Database connection not available".to_string())?;

        let mut stmt = conn.prepare(
            "SELECT timestamp, cpu_usage, memory_usage, disk_usage,
                    network_rx, network_tx, network_packets_rx, network_packets_tx,
                    load_avg_1m, load_avg_5m, load_avg_15m, process_count, uptime, metadata
             FROM metrics_history
             WHERE host = ?1 AND timestamp >= ?2 AND timestamp <= ?3
             ORDER BY timestamp ASC"
        ).map_err(|e| format!("Failed to prepare metrics query: {}", e))?;

        let metrics_iter = stmt.query_map(
            rusqlite::params![host, start as i64, end as i64],
            |row| {
                let metadata_str: String = row.get(13)?;
                let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
                    .unwrap_or(serde_json::json!({}));

                Ok(SystemMetrics {
                    timestamp: row.get::<_, i64>(0)? as u64,
                    cpu_usage_percent: row.get(1)?,
                    memory_usage_percent: row.get(2)?,
                    disk_usage_percent: row.get(3)?,
                    network_rx_mb: row.get::<_, i64>(4)? as u64,
                    network_tx_mb: row.get::<_, i64>(5)? as u64,
                    network_packets_rx: row.get::<_, i64>(6)? as u64,
                    network_packets_tx: row.get::<_, i64>(7)? as u64,
                    load_average_1m: row.get(8)?,
                    load_average_5m: row.get(9)?,
                    load_average_15m: row.get(10)?,
                    process_count: row.get::<_, i64>(11)? as usize,
                    uptime_seconds: row.get::<_, i64>(12)? as u64,
                    // Defaults for fields not stored in main table
                    memory_used_mb: 0,
                    memory_total_mb: 0,
                    disk_read_mb: 0,
                    disk_write_mb: 0,
                    boot_time: metadata["boot_time"].as_u64().unwrap_or(0),
                    cpu_count: metadata["cpu_count"].as_u64().unwrap_or(0) as usize,
                    cpu_per_core: metadata["cpu_per_core"].as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
                        .unwrap_or_default(),
                    cpu_frequency_mhz: metadata["cpu_frequency_mhz"].as_u64().unwrap_or(0),
                    disk_total_gb: metadata["disk_total_gb"].as_u64().unwrap_or(0),
                    disk_used_gb: metadata["disk_used_gb"].as_u64().unwrap_or(0),
                    disk_free_gb: metadata["disk_free_gb"].as_u64().unwrap_or(0),
                    network_errors_rx: metadata["network_errors_rx"].as_u64().unwrap_or(0),
                    network_errors_tx: metadata["network_errors_tx"].as_u64().unwrap_or(0),
                    top_cpu_processes: serde_json::from_value(metadata["top_cpu_processes"].clone()).unwrap_or_default(),
                    top_memory_processes: serde_json::from_value(metadata["top_memory_processes"].clone()).unwrap_or_default(),
                    disks: serde_json::from_value(metadata["disks"].clone()).unwrap_or_default(),
                })
            }
        ).map_err(|e| format!("Failed to query metrics: {}", e))?;

        let mut results = Vec::new();
        for metric in metrics_iter {
            results.push(metric.map_err(|e| format!("Failed to parse metric: {}", e))?);
        }

        Ok(results)
    }

    pub fn cleanup_old_metrics(&self) -> Result<usize, String> {
        let mut conn_guard = self.get_connection()?;
        let conn = conn_guard.as_mut()
            .ok_or_else(|| "Database connection not available".to_string())?;

        let cutoff_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() - (self.retention_days as u64 * 86400);

        let deleted = conn.execute(
            "DELETE FROM metrics_history WHERE timestamp < ?1",
            rusqlite::params![cutoff_time as i64],
        ).map_err(|e| format!("Failed to cleanup old metrics (older than {}): {}", cutoff_time, e))?;

        Ok(deleted)
    }

    pub fn save_alert(&self, alert: &Alert, host: &str) -> Result<(), String> {
        let mut conn_guard = self.get_connection()?;
        let conn = conn_guard.as_mut()
            .ok_or_else(|| "Database connection not available".to_string())?;

        conn.execute(
            "INSERT INTO alert_history (
                alert_id, rule_id, host, message, severity, triggered_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                alert.id,
                alert.rule_id,
                host,
                alert.message,
                format!("{:?}", alert.severity),
                alert.timestamp as i64,
            ],
        ).map_err(|e| format!("Failed to insert alert '{}' for rule '{}': {}", alert.id, alert.rule_id, e))?;

        Ok(())
    }

    pub fn get_alert_history(&self, start: u64, end: u64) -> Result<Vec<Alert>, String> {
        let mut conn_guard = self.get_connection()?;
        let conn = conn_guard.as_mut()
            .ok_or_else(|| "Database connection not available".to_string())?;

        let mut stmt = conn.prepare(
            "SELECT alert_id, rule_id, message, severity, triggered_at, acknowledged_at
             FROM alert_history
             WHERE triggered_at >= ?1 AND triggered_at <= ?2
             ORDER BY triggered_at DESC"
        ).map_err(|e| format!("Failed to prepare alert history query: {}", e))?;

        let alerts_iter = stmt.query_map(
            rusqlite::params![start as i64, end as i64],
            |row| {
                let severity_str: String = row.get(3)?;
                let severity = match severity_str.as_str() {
                    "Critical" => AlertSeverity::Critical,
                    "Warning" => AlertSeverity::Warning,
                    _ => AlertSeverity::Info,
                };

                Ok(Alert {
                    id: row.get(0)?,
                    rule_id: row.get(1)?,
                    message: row.get(2)?,
                    severity,
                    timestamp: row.get::<_, i64>(4)? as u64,
                    acknowledged: row.get::<_, Option<i64>>(5)?.is_some(),
                })
            }
        ).map_err(|e| format!("Failed to query alerts: {}", e))?;

        let mut results = Vec::new();
        for alert in alerts_iter {
            results.push(alert.map_err(|e| format!("Failed to parse alert: {}", e))?);
        }

        Ok(results)
    }
}

pub async fn start_monitoring_task<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    interval_secs: u64,
) {
    let mut collector = MetricsCollector::new();
    let alert_engine = app_handle.state::<Arc<AlertEngine>>();
    let mut ticker = interval(Duration::from_secs(interval_secs));

    // Try to initialize metrics store, but continue without it if it fails
    let metrics_store = match app_handle.path().app_data_dir() {
        Ok(path) => {
            let db_path = path.join("quasar.db");
            if let Some(db_path_str) = db_path.to_str() {
                match MetricsStore::new(db_path_str.to_string(), 30) {
                    Ok(store) => Some(store),
                    Err(e) => {
                        error!("[MONITORING] CRITICAL: Failed to initialize metrics store: {}", e);
                        None
                    }
                }
            } else {
                error!("[MONITORING] ERROR: Invalid database path");
                None
            }
        },
        Err(e) => {
            error!("[MONITORING] ERROR: Failed to get app data directory: {}", e);
            None
        }
    };

    if metrics_store.is_none() {
        warn!("[MONITORING] WARNING: Persistence disabled. Metrics and alerts will not be saved.");
    }

    let mut save_counter = 0u32;
    let mut cleanup_counter = 0u32;
    let save_interval = 6; // Save every 30 seconds (6 * 5 seconds)
    let cleanup_interval = 17280; // Cleanup daily (86400 / 5 seconds)

    loop {
        ticker.tick().await;

        let metrics = collector.collect();
        let (alerts, recoveries) = alert_engine.evaluate(&metrics);

        // Emit real-time metrics
        let _ = app_handle.emit("system-metrics", metrics.clone());

        // Save metrics to database every 30 seconds (if store available)
        if let Some(ref store) = metrics_store {
            save_counter += 1;
            if save_counter >= save_interval {
                save_counter = 0;
                if let Err(e) = store.save_metrics(&metrics, "localhost") {
                    error!("[MONITORING] ERROR: Failed to save metrics: {}", e);
                }
            }

            // Run cleanup daily
            cleanup_counter += 1;
            if cleanup_counter >= cleanup_interval {
                cleanup_counter = 0;
                match store.cleanup_old_metrics() {
                    Ok(deleted) => info!("[MONITORING] INFO: Cleaned up {} old metric records", deleted),
                    Err(e) => error!("[MONITORING] ERROR: Failed to cleanup metrics: {}", e),
                }
            }
        }

        // Emit recovery notifications
        if !recoveries.is_empty() {
            let _ = app_handle.emit("alerts-recovered", recoveries);
        }

        // Save alerts to database (if store available)
        if !alerts.is_empty() {
            let _ = app_handle.emit("alerts-triggered", alerts.clone());
            if let Some(ref store) = metrics_store {
                for alert in &alerts {
                    if let Err(e) = store.save_alert(alert, "localhost") {
                        error!("[MONITORING] ERROR: Failed to save alert {}: {}", alert.id, e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        assert!(collector.last_update.elapsed().as_secs() < 1);
    }

    #[test]
    fn test_metrics_collection() {
        let mut collector = MetricsCollector::new();
        std::thread::sleep(Duration::from_millis(100));
        let metrics = collector.collect();

        assert!(metrics.cpu_usage_percent >= 0.0 && metrics.cpu_usage_percent <= 100.0);
        assert!(metrics.memory_total_mb > 0);
        assert!(metrics.memory_used_mb <= metrics.memory_total_mb);
        assert!(metrics.memory_usage_percent >= 0.0 && metrics.memory_usage_percent <= 100.0);
        assert!(metrics.timestamp > 0);
    }

    #[test]
    fn test_alert_engine_add_rule() {
        let engine = AlertEngine::new();
        let rule = AlertRule {
            id: "test-1".to_string(),
            metric: MetricType::CpuUsage,
            operator: ComparisonOperator::GreaterThan,
            threshold: 80.0,
            severity: AlertSeverity::Warning,
            enabled: true,
            cooldown_seconds: 300,
        };
        engine.add_rule(rule.clone());

        let rules = engine.get_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "test-1");
    }

    #[test]
    fn test_alert_engine_evaluation() {
        let engine = AlertEngine::new();
        let rule = AlertRule {
            id: "test-2".to_string(),
            metric: MetricType::CpuUsage,
            operator: ComparisonOperator::GreaterThan,
            threshold: 50.0,
            severity: AlertSeverity::Critical,
            enabled: true,
            cooldown_seconds: 300,
        };
        engine.add_rule(rule);

        let metrics = SystemMetrics {
            cpu_usage_percent: 75.0,
            memory_used_mb: 0,
            memory_total_mb: 0,
            memory_usage_percent: 0.0,
            disk_read_mb: 0,
            disk_write_mb: 0,
            network_rx_mb: 0,
            network_tx_mb: 0,
            timestamp: 1234567890,
            uptime_seconds: 0,
            load_average_1m: 0.0,
            load_average_5m: 0.0,
            load_average_15m: 0.0,
            process_count: 0,
            boot_time: 0,
            cpu_count: 0,
            cpu_per_core: vec![],
            cpu_frequency_mhz: 0,
            disk_total_gb: 0,
            disk_used_gb: 0,
            disk_free_gb: 0,
            disk_usage_percent: 0.0,
            network_packets_rx: 0,
            network_packets_tx: 0,
            network_errors_rx: 0,
            network_errors_tx: 0,
            top_cpu_processes: vec![],
            top_memory_processes: vec![],
            disks: vec![],
        };

        let (alerts, _recoveries) = engine.evaluate(&metrics);
        assert_eq!(alerts.len(), 1);
        assert!(matches!(alerts[0].severity, AlertSeverity::Critical));
    }

    #[test]
    fn test_alert_engine_no_trigger() {
        let engine = AlertEngine::new();
        let rule = AlertRule {
            id: "test-3".to_string(),
            metric: MetricType::CpuUsage,
            operator: ComparisonOperator::GreaterThan,
            threshold: 90.0,
            severity: AlertSeverity::Warning,
            enabled: true,
            cooldown_seconds: 300,
        };
        engine.add_rule(rule);

        let metrics = SystemMetrics {
            cpu_usage_percent: 50.0,
            memory_used_mb: 0,
            memory_total_mb: 0,
            memory_usage_percent: 0.0,
            disk_read_mb: 0,
            disk_write_mb: 0,
            network_rx_mb: 0,
            network_tx_mb: 0,
            timestamp: 1234567890,
            uptime_seconds: 0,
            load_average_1m: 0.0,
            load_average_5m: 0.0,
            load_average_15m: 0.0,
            process_count: 0,
            boot_time: 0,
            cpu_count: 0,
            cpu_per_core: vec![],
            cpu_frequency_mhz: 0,
            disk_total_gb: 0,
            disk_used_gb: 0,
            disk_free_gb: 0,
            disk_usage_percent: 0.0,
            network_packets_rx: 0,
            network_packets_tx: 0,
            network_errors_rx: 0,
            network_errors_tx: 0,
            top_cpu_processes: vec![],
            top_memory_processes: vec![],
            disks: vec![],
        };

        let (alerts, _recoveries) = engine.evaluate(&metrics);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_alert_acknowledge_and_dismiss() {
        let engine = AlertEngine::new();
        let rule = AlertRule {
            id: "test-4".to_string(),
            metric: MetricType::MemoryUsage,
            operator: ComparisonOperator::GreaterThan,
            threshold: 0.0,
            severity: AlertSeverity::Info,
            enabled: true,
            cooldown_seconds: 300,
        };
        engine.add_rule(rule);

        let metrics = SystemMetrics {
            cpu_usage_percent: 0.0,
            memory_used_mb: 100,
            memory_total_mb: 100,
            memory_usage_percent: 100.0,
            disk_read_mb: 0,
            disk_write_mb: 0,
            network_rx_mb: 0,
            network_tx_mb: 0,
            timestamp: 1234567890,
            uptime_seconds: 0,
            load_average_1m: 0.0,
            load_average_5m: 0.0,
            load_average_15m: 0.0,
            process_count: 0,
            boot_time: 0,
            cpu_count: 0,
            cpu_per_core: vec![],
            cpu_frequency_mhz: 0,
            disk_total_gb: 0,
            disk_used_gb: 0,
            disk_free_gb: 0,
            disk_usage_percent: 0.0,
            network_packets_rx: 0,
            network_packets_tx: 0,
            network_errors_rx: 0,
            network_errors_tx: 0,
            top_cpu_processes: vec![],
            top_memory_processes: vec![],
            disks: vec![],
        };

        let (_alerts, _recoveries) = engine.evaluate(&metrics);
        let alerts = engine.get_active_alerts();
        assert_eq!(alerts.len(), 1);

        engine.acknowledge_alert(&alerts[0].id);
        let alerts = engine.get_active_alerts();
        assert!(alerts[0].acknowledged);

        engine.dismiss_alert(&alerts[0].id);
        let alerts = engine.get_active_alerts();
        assert!(alerts.is_empty());
    }
}

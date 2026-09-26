//! Metrics and alert history in SQLite.

use super::{Alert, SystemMetrics};
use crate::db;

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

    fn get_connection(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<rusqlite::Connection>>, String> {
        let mut conn_guard = self
            .conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Check if connection exists and is valid
        if conn_guard.is_none() {
            let new_conn = db::open_connection(&self.db_path)
                .map_err(|e| format!("Failed to open metrics database at {}: {}", self.db_path, e))?;
            *conn_guard = Some(new_conn);
        }

        Ok(conn_guard)
    }

    pub fn save_metrics(&self, metrics: &SystemMetrics, host: &str) -> Result<(), String> {
        let mut conn_guard = self.get_connection()?;
        let conn = conn_guard
            .as_mut()
            .ok_or_else(|| "Database connection not available".to_string())?;

        // Store core metrics and additional data as JSON.
        //
        // The top-process lists are deliberately NOT persisted. They were roughly
        // half of every stored row while being the least useful field to keep
        // historically, and the UI sources its process tables from the live
        // `system-metrics` event rather than from history. `get_metrics_range`
        // defaults them to empty for rows that predate this change.
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
                metrics.network_rx_mb,
                metrics.network_tx_mb,
                metrics.network_packets_rx as i64,
                metrics.network_packets_tx as i64,
                metrics.load_average_1m,
                metrics.load_average_5m,
                metrics.load_average_15m,
                metrics.process_count as i64,
                metrics.uptime_seconds as i64,
                metadata.to_string(),
            ],
        )
        .map_err(|e| {
            format!(
                "Failed to insert metrics for host '{}' at {}: {}",
                host, metrics.timestamp, e
            )
        })?;

        Ok(())
    }

    #[cfg(test)]
    pub fn get_metrics_range(
        &self,
        start: u64,
        end: u64,
        host: &str,
    ) -> Result<Vec<SystemMetrics>, String> {
        let mut conn_guard = self.get_connection()?;
        let conn = conn_guard
            .as_mut()
            .ok_or_else(|| "Database connection not available".to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT timestamp, cpu_usage, memory_usage, disk_usage,
                    network_rx, network_tx, network_packets_rx, network_packets_tx,
                    load_avg_1m, load_avg_5m, load_avg_15m, process_count, uptime, metadata
             FROM metrics_history
             WHERE host = ?1 AND timestamp >= ?2 AND timestamp <= ?3
             ORDER BY timestamp ASC",
            )
            .map_err(|e| format!("Failed to prepare metrics query: {}", e))?;

        let metrics_iter = stmt
            .query_map(rusqlite::params![host, start as i64, end as i64], |row| {
                let metadata_str: String = row.get(13)?;
                let metadata: serde_json::Value =
                    serde_json::from_str(&metadata_str).unwrap_or(serde_json::json!({}));

                Ok(SystemMetrics {
                    timestamp: row.get::<_, i64>(0)? as u64,
                    cpu_usage_percent: row.get(1)?,
                    memory_usage_percent: row.get(2)?,
                    disk_usage_percent: row.get(3)?,
                    network_rx_mb: row.get::<_, f64>(4)?,
                    network_tx_mb: row.get::<_, f64>(5)?,
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
                    disk_read_mb: 0.0,
                    disk_write_mb: 0.0,
                    boot_time: metadata["boot_time"].as_u64().unwrap_or(0),
                    cpu_count: metadata["cpu_count"].as_u64().unwrap_or(0) as usize,
                    cpu_per_core: metadata["cpu_per_core"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_f64().map(|f| f as f32))
                                .collect()
                        })
                        .unwrap_or_default(),
                    cpu_frequency_mhz: metadata["cpu_frequency_mhz"].as_u64().unwrap_or(0),
                    disk_total_gb: metadata["disk_total_gb"].as_u64().unwrap_or(0),
                    disk_used_gb: metadata["disk_used_gb"].as_u64().unwrap_or(0),
                    disk_free_gb: metadata["disk_free_gb"].as_u64().unwrap_or(0),
                    network_errors_rx: metadata["network_errors_rx"].as_u64().unwrap_or(0),
                    network_errors_tx: metadata["network_errors_tx"].as_u64().unwrap_or(0),
                    top_cpu_processes: serde_json::from_value(
                        metadata["top_cpu_processes"].clone(),
                    )
                    .unwrap_or_default(),
                    top_memory_processes: serde_json::from_value(
                        metadata["top_memory_processes"].clone(),
                    )
                    .unwrap_or_default(),
                    disks: serde_json::from_value(metadata["disks"].clone()).unwrap_or_default(),
                })
            })
            .map_err(|e| format!("Failed to query metrics: {}", e))?;

        metrics_iter
            .map(|metric| metric.map_err(|e| format!("Failed to parse metric: {}", e)))
            .collect()
    }

    pub fn cleanup_old_metrics(&self) -> Result<usize, String> {
        let mut conn_guard = self.get_connection()?;
        let conn = conn_guard
            .as_mut()
            .ok_or_else(|| "Database connection not available".to_string())?;

        let cutoff_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(self.retention_days as u64 * 86400);

        let deleted = conn
            .execute(
                "DELETE FROM metrics_history WHERE timestamp < ?1",
                rusqlite::params![cutoff_time as i64],
            )
            .map_err(|e| {
                format!(
                    "Failed to cleanup old metrics (older than {}): {}",
                    cutoff_time, e
                )
            })?;

        Ok(deleted)
    }

    pub fn save_alert(&self, alert: &Alert, host: &str) -> Result<(), String> {
        let mut conn_guard = self.get_connection()?;
        let conn = conn_guard
            .as_mut()
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
        )
        .map_err(|e| {
            format!(
                "Failed to insert alert '{}' for rule '{}': {}",
                alert.id, alert.rule_id, e
            )
        })?;

        Ok(())
    }

}

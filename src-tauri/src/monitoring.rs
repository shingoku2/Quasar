use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, System};
use tauri::{Emitter, Manager};
use tokio::time::interval;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_usage_percent: f32,
    pub disk_read_mb: u64,
    pub disk_write_mb: u64,
    pub network_rx_mb: u64,
    pub network_tx_mb: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: String,
    pub metric: MetricType,
    pub operator: ComparisonOperator,
    pub threshold: f64,
    pub severity: AlertSeverity,
    pub enabled: bool,
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

        SystemMetrics {
            cpu_usage_percent: cpu_usage,
            memory_used_mb: memory_used / 1024 / 1024, // Convert bytes to MB
            memory_total_mb: memory_total / 1024 / 1024,
            memory_usage_percent,
            disk_read_mb: disk_read_rate / 1024 / 1024, // Convert bytes/s to MB/s
            disk_write_mb: disk_write_rate / 1024 / 1024,
            network_rx_mb: net_rx_rate / 1024 / 1024,
            network_tx_mb: net_tx_rate / 1024 / 1024,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
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
}

pub struct AlertEngine {
    rules: Arc<Mutex<Vec<AlertRule>>>,
    active_alerts: Arc<Mutex<Vec<Alert>>>,
    alert_counter: Arc<Mutex<u64>>,
}

impl AlertEngine {
    pub fn new() -> Self {
        Self {
            rules: Arc::new(Mutex::new(Vec::new())),
            active_alerts: Arc::new(Mutex::new(Vec::new())),
            alert_counter: Arc::new(Mutex::new(0)),
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

    pub fn evaluate(&self, metrics: &SystemMetrics) -> Vec<Alert> {
        let rules = self.rules.lock().unwrap();
        let mut new_alerts = Vec::new();
        let mut counter = self.alert_counter.lock().unwrap();

        for rule in rules.iter().filter(|r| r.enabled) {
            let value = match rule.metric {
                MetricType::CpuUsage => metrics.cpu_usage_percent as f64,
                MetricType::MemoryUsage => metrics.memory_usage_percent as f64,
                MetricType::DiskUsage => {
                    // Calculate disk usage as percentage (would need actual disk capacity info)
                    // For now, skip as we don't have disk capacity in metrics
                    continue;
                }
            };

            let triggered = match rule.operator {
                ComparisonOperator::GreaterThan => value > rule.threshold,
                ComparisonOperator::LessThan => value < rule.threshold,
                ComparisonOperator::Equals => (value - rule.threshold).abs() < 0.01,
                ComparisonOperator::GreaterThanOrEqual => value >= rule.threshold,
                ComparisonOperator::LessThanOrEqual => value <= rule.threshold,
            };

            if triggered {
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
            }
        }

        if !new_alerts.is_empty() {
            let mut active = self.active_alerts.lock().unwrap();
            active.extend(new_alerts.clone());
            let len = active.len();
            if len > 50 {
                active.drain(0..len - 50);
            }
        }

        new_alerts
    }

    pub fn get_active_alerts(&self) -> Vec<Alert> {
        self.active_alerts.lock().unwrap().clone()
    }

    pub fn acknowledge_alert(&self, alert_id: &str) {
        let mut alerts = self.active_alerts.lock().unwrap();
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
        }
    }

    pub fn dismiss_alert(&self, alert_id: &str) {
        let mut alerts = self.active_alerts.lock().unwrap();
        alerts.retain(|a| a.id != alert_id);
    }
}

pub async fn start_monitoring_task<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    interval_secs: u64,
) {
    let mut collector = MetricsCollector::new();
    let alert_engine = app_handle.state::<Arc<AlertEngine>>();
    let mut ticker = interval(Duration::from_secs(interval_secs));

    loop {
        ticker.tick().await;

        let metrics = collector.collect();
        let alerts = alert_engine.evaluate(&metrics);

        println!("Emitting system-metrics: cpu={:.1}%, mem={:.1}%", metrics.cpu_usage_percent, metrics.memory_usage_percent);
        let _ = app_handle.emit("system-metrics", metrics);

        if !alerts.is_empty() {
            let _ = app_handle.emit("alerts-triggered", alerts);
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
        };

        let alerts = engine.evaluate(&metrics);
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
        };

        let alerts = engine.evaluate(&metrics);
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
        };

        engine.evaluate(&metrics);
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

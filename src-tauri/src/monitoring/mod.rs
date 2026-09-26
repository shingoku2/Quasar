//! Monitoring: local metrics, alert rules, history storage and the background loop. Split
//! by concern; everything is re-exported here, so `monitoring::AlertRule` and friends keep
//! their paths.

mod alerts;
mod collector;
mod store;
mod task;

pub use alerts::*;
pub use collector::*;
pub use store::*;
pub use task::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    /// FE-017: sub-MB/s rates keep their fraction instead of truncating to 0.
    #[test]
    fn io_rates_keep_fractions() {
        assert_eq!(bytes_to_mb(512 * 1024), 0.5);
        assert!(bytes_to_mb(10_000) > 0.0);
        assert_eq!(bytes_to_mb(3 * 1024 * 1024), 3.0);
    }

    fn percent_rule(id: &str, threshold: f64) -> AlertRule {
        AlertRule {
            id: id.to_string(),
            metric: MetricType::CpuUsage,
            operator: ComparisonOperator::GreaterThan,
            threshold,
            severity: AlertSeverity::Warning,
            enabled: true,
            cooldown_seconds: 60,
        }
    }

    /// RUST-002: rules survive a restart (a fresh engine attached to the same database),
    /// and removals are persisted too.
    #[test]
    fn alert_rules_persist_across_restart() {
        let path = std::env::temp_dir().join(format!("quasar-alerts-{}.db", uuid::Uuid::new_v4()));
        let path_str = path.to_str().unwrap().to_string();
        let mut conn = crate::db::open_connection(&path_str).unwrap();
        crate::MIGRATIONS.to_latest(&mut conn).unwrap();
        drop(conn);

        let engine = AlertEngine::new();
        assert_eq!(engine.attach_store(path_str.clone()).unwrap(), 0);
        engine.add_rule(percent_rule("cpu-hot", 90.0)).unwrap();
        engine.add_rule(percent_rule("cpu-warm", 70.0)).unwrap();
        engine.add_rule(percent_rule("cpu-hot", 95.0)).unwrap(); // update in place
        engine.remove_rule("cpu-warm").unwrap();

        let restarted = AlertEngine::new();
        assert_eq!(restarted.attach_store(path_str).unwrap(), 1);
        let rules = restarted.get_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "cpu-hot");
        assert_eq!(rules[0].threshold, 95.0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_alert_rules_are_rejected() {
        let engine = AlertEngine::new();
        assert!(engine.add_rule(percent_rule("", 50.0)).is_err());
        assert!(engine.add_rule(percent_rule("x", f64::NAN)).is_err());
        assert!(engine.add_rule(percent_rule("x", 150.0)).is_err());
        let mut slow = percent_rule("x", 50.0);
        slow.cooldown_seconds = MAX_COOLDOWN_SECONDS + 1;
        assert!(engine.add_rule(slow).is_err());
        assert!(engine.get_rules().is_empty());
    }

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

    fn metrics_store_db() -> (MetricsStore, String) {
        use std::sync::atomic::{AtomicU64, Ordering};
        // A nanosecond timestamp alone isn't a reliable uniqueness guarantee
        // (clock resolution can be coarser than 1ns, and concurrent test threads
        // can race), so a per-process counter is appended to rule out collisions.
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_path = format!(
            "test_metrics_store_{}_{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            seq
        );
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(include_str!("../../migrations/004_monitoring.sql"))
            .unwrap();
        drop(conn);
        let store = MetricsStore::new(db_path.clone(), 30).unwrap();
        (store, db_path)
    }

    fn sample_metrics() -> SystemMetrics {
        SystemMetrics {
            cpu_usage_percent: 12.5,
            memory_used_mb: 3192,
            memory_total_mb: 15909,
            memory_usage_percent: 20.0,
            disk_read_mb: 1.0,
            disk_write_mb: 2.0,
            network_rx_mb: 3.0,
            network_tx_mb: 4.0,
            timestamp: 1_700_000_000,
            uptime_seconds: 944_918,
            load_average_1m: 0.87,
            load_average_5m: 0.84,
            load_average_15m: 0.76,
            process_count: 412,
            boot_time: 1_699_000_000,
            cpu_count: 8,
            cpu_per_core: vec![10.0, 20.0],
            cpu_frequency_mhz: 2400,
            disk_total_gb: 119,
            disk_used_gb: 45,
            disk_free_gb: 74,
            disk_usage_percent: 37.8,
            disks: vec![],
            network_packets_rx: 10,
            network_packets_tx: 11,
            network_errors_rx: 0,
            network_errors_tx: 0,
            top_cpu_processes: vec![ProcessInfo {
                pid: 1,
                name: "init".to_string(),
                cpu_usage: 1.0,
                memory_mb: 10,
            }],
            top_memory_processes: vec![ProcessInfo {
                pid: 2,
                name: "chrome".to_string(),
                cpu_usage: 2.0,
                memory_mb: 900,
            }],
        }
    }

    #[test]
    fn test_stored_metrics_round_trip_without_process_lists() {
        let (store, db_path) = metrics_store_db();
        let metrics = sample_metrics();
        store.save_metrics(&metrics, "localhost").unwrap();

        let rows = store
            .get_metrics_range(metrics.timestamp - 1, metrics.timestamp + 1, "localhost")
            .unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];

        // Fields that are still persisted must survive the round trip.
        assert_eq!(row.cpu_usage_percent, 12.5);
        assert_eq!(row.uptime_seconds, 944_918);
        assert_eq!(row.disk_total_gb, 119);
        assert_eq!(row.cpu_count, 8);
        assert_eq!(row.cpu_per_core, vec![10.0, 20.0]);

        // Process lists are no longer stored; reading must degrade to empty
        // rather than erroring, which is also how pre-existing rows behave.
        assert!(row.top_cpu_processes.is_empty());
        assert!(row.top_memory_processes.is_empty());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_stored_metadata_excludes_process_lists() {
        let (store, db_path) = metrics_store_db();
        store.save_metrics(&sample_metrics(), "localhost").unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let metadata: String = conn
            .query_row("SELECT metadata FROM metrics_history", [], |r| r.get(0))
            .unwrap();
        drop(conn);

        assert!(
            !metadata.contains("top_cpu_processes") && !metadata.contains("top_memory_processes"),
            "process lists dominated stored row size and are never read back: {metadata}"
        );
        assert!(metadata.contains("cpu_per_core"), "other fields are kept");

        let _ = std::fs::remove_file(&db_path);
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
        engine.add_rule(rule.clone()).unwrap();

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
        engine.add_rule(rule).unwrap();

        let metrics = SystemMetrics {
            cpu_usage_percent: 75.0,
            memory_used_mb: 0,
            memory_total_mb: 0,
            memory_usage_percent: 0.0,
            disk_read_mb: 0.0,
            disk_write_mb: 0.0,
            network_rx_mb: 0.0,
            network_tx_mb: 0.0,
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

    /// PR #68 review: a tick that evaluated before an import's suspend must not persist its
    /// alerts, and a tick can't persist at all while the import holds the swap.
    #[tokio::test]
    async fn alerts_from_before_a_suspend_are_not_persisted() {
        let engine = Arc::new(AlertEngine::new());
        let before = engine.generation();
        engine.suspend();
        let mut ran = false;
        assert!(!engine.persist_if_current(before, || ran = true).await);
        assert!(!ran, "stale results are dropped");

        let current = engine.generation();
        let swap = engine.hold_persistence().await;
        let waiting = {
            let engine = engine.clone();
            tokio::spawn(async move { engine.persist_if_current(current, || {}).await })
        };
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!waiting.is_finished(), "persisting waits for the swap");
        engine.suspend(); // the import's reload
        drop(swap);
        assert!(!waiting.await.unwrap(), "and then finds its results stale");
        assert!(engine.persist_if_current(engine.generation(), || {}).await);
    }

    /// PR #68 review: after a reload (an import), a rule id that was triggered in the old
    /// database must not produce a recovery for an alert the new database never had.
    #[test]
    fn reload_forgets_the_previous_triggered_state() {
        let path = std::env::temp_dir().join(format!("quasar_alert_reload_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE alert_rules (id TEXT PRIMARY KEY, rule TEXT NOT NULL, updated_at INTEGER NOT NULL);")
            .unwrap();
        let engine = AlertEngine::new();
        engine.attach_store(path.to_str().unwrap().to_string()).unwrap();
        engine
            .add_rule(AlertRule {
                id: "shared-id".to_string(),
                metric: MetricType::CpuUsage,
                operator: ComparisonOperator::GreaterThan,
                threshold: 50.0,
                severity: AlertSeverity::Warning,
                enabled: true,
                cooldown_seconds: 0,
            })
            .unwrap();
        let mut metrics = sample_metrics();
        metrics.cpu_usage_percent = 90.0;
        let (alerts, _) = engine.evaluate(&metrics);
        assert_eq!(alerts.len(), 1, "fixture: the rule triggers before the reload");

        engine.reload().unwrap();
        metrics.cpu_usage_percent = 10.0;
        let (_, recoveries) = engine.evaluate(&metrics);
        assert!(recoveries.is_empty(), "no recovery for an alert of the previous database");
        let _ = std::fs::remove_file(&path);
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
        engine.add_rule(rule).unwrap();

        let metrics = SystemMetrics {
            cpu_usage_percent: 50.0,
            memory_used_mb: 0,
            memory_total_mb: 0,
            memory_usage_percent: 0.0,
            disk_read_mb: 0.0,
            disk_write_mb: 0.0,
            network_rx_mb: 0.0,
            network_tx_mb: 0.0,
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
        engine.add_rule(rule).unwrap();

        let metrics = SystemMetrics {
            cpu_usage_percent: 0.0,
            memory_used_mb: 100,
            memory_total_mb: 100,
            memory_usage_percent: 100.0,
            disk_read_mb: 0.0,
            disk_write_mb: 0.0,
            network_rx_mb: 0.0,
            network_tx_mb: 0.0,
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

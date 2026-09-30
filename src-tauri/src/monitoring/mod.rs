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

    /// The alert state machine in both directions: during cooldown a triggered rule stays
    /// silent, dropping below the threshold emits exactly one recovery (which clears the
    /// cooldown tracker), and re-crossing the threshold alerts again at once.
    #[test]
    fn cooldown_suppresses_repeats_and_recovery_resets_the_rule() {
        let engine = AlertEngine::new();
        let mut rule = percent_rule("cpu-cycle", 50.0);
        rule.cooldown_seconds = 300;
        engine.add_rule(rule).unwrap();
        let mut metrics = sample_metrics();

        metrics.cpu_usage_percent = 90.0;
        let (alerts, recoveries) = engine.evaluate(&metrics);
        assert_eq!(alerts.len(), 1);
        assert!(recoveries.is_empty());
        assert_eq!(alerts[0].rule_id, "cpu-cycle");
        assert!(alerts[0].message.contains("90.0%"), "{}", alerts[0].message);

        // Still hot on the next tick: the cooldown suppresses the repeat.
        metrics.cpu_usage_percent = 95.0;
        metrics.timestamp += 5;
        let (alerts, recoveries) = engine.evaluate(&metrics);
        assert!(alerts.is_empty(), "a rule in cooldown must not re-alert");
        assert!(recoveries.is_empty());

        // Dropped below the threshold: exactly one recovery referring to this tick.
        metrics.cpu_usage_percent = 10.0;
        metrics.timestamp += 5;
        let (alerts, recoveries) = engine.evaluate(&metrics);
        assert!(alerts.is_empty());
        assert_eq!(recoveries.len(), 1);
        assert_eq!(recoveries[0].rule_id, "cpu-cycle");
        assert_eq!(recoveries[0].recovered_at, metrics.timestamp);
        assert!(recoveries[0].message.contains("recovered"));

        // Staying low does not repeat the recovery...
        metrics.timestamp += 5;
        assert!(engine.evaluate(&metrics).1.is_empty());

        // ...and re-crossing alerts immediately: the recovery cleared the cooldown.
        metrics.cpu_usage_percent = 90.0;
        metrics.timestamp += 5;
        assert_eq!(engine.evaluate(&metrics).0.len(), 1);
    }

    #[test]
    fn zero_cooldown_alerts_on_every_tick() {
        let engine = AlertEngine::new();
        let mut rule = percent_rule("cpu-spam", 50.0);
        rule.cooldown_seconds = 0;
        engine.add_rule(rule).unwrap();
        let mut metrics = sample_metrics();
        metrics.cpu_usage_percent = 90.0;

        assert_eq!(engine.evaluate(&metrics).0.len(), 1);
        metrics.timestamp += 5;
        assert_eq!(engine.evaluate(&metrics).0.len(), 1);
        assert_eq!(engine.get_active_alerts().len(), 2);
    }

    /// The whole comparison matrix at and around the boundary, the Equals epsilon, each
    /// metric reading its own field, and disabled rules never firing.
    #[test]
    fn evaluate_covers_every_operator_and_metric() {
        fn fires(metrics: &SystemMetrics, operator: ComparisonOperator, threshold: f64) -> bool {
            let engine = AlertEngine::new();
            engine
                .add_rule(AlertRule {
                    id: "m".to_string(),
                    metric: MetricType::CpuUsage,
                    operator,
                    threshold,
                    severity: AlertSeverity::Info,
                    enabled: true,
                    cooldown_seconds: 0,
                })
                .unwrap();
            !engine.evaluate(metrics).0.is_empty()
        }

        let mut metrics = sample_metrics();
        metrics.cpu_usage_percent = 50.0; // the value under test below
        let m = &metrics;
        use ComparisonOperator::*;

        assert!(fires(m, GreaterThan, 49.9));
        assert!(!fires(m, GreaterThan, 50.0));
        assert!(!fires(m, GreaterThan, 50.1));
        assert!(fires(m, LessThan, 50.1));
        assert!(!fires(m, LessThan, 50.0));
        assert!(!fires(m, LessThan, 49.9));
        assert!(fires(m, GreaterThanOrEqual, 50.0));
        assert!(!fires(m, GreaterThanOrEqual, 50.1));
        assert!(fires(m, LessThanOrEqual, 50.0));
        assert!(!fires(m, LessThanOrEqual, 49.9));
        // Equals matches within a 0.01 tolerance and not beyond it.
        assert!(fires(m, Equals, 50.0));
        assert!(fires(m, Equals, 50.005));
        assert!(fires(m, Equals, 49.995));
        assert!(!fires(m, Equals, 50.02));
        assert!(!fires(m, Equals, 49.98));

        // Each metric reads its own field: only the disk rule fires.
        metrics.cpu_usage_percent = 10.0;
        metrics.memory_usage_percent = 20.0;
        metrics.disk_usage_percent = 90.0;
        for (metric, expected) in [
            (MetricType::CpuUsage, false),
            (MetricType::MemoryUsage, false),
            (MetricType::DiskUsage, true),
        ] {
            let engine = AlertEngine::new();
            engine
                .add_rule(AlertRule {
                    id: "which".to_string(),
                    metric,
                    operator: ComparisonOperator::GreaterThan,
                    threshold: 80.0,
                    severity: AlertSeverity::Warning,
                    enabled: true,
                    cooldown_seconds: 0,
                })
                .unwrap();
            assert_eq!(engine.evaluate(&metrics).0.len(), usize::from(expected));
        }

        // A disabled rule never evaluates.
        let engine = AlertEngine::new();
        let mut disabled = percent_rule("off", 0.0);
        disabled.enabled = false;
        engine.add_rule(disabled).unwrap();
        let (alerts, recoveries) = engine.evaluate(&metrics);
        assert!(alerts.is_empty() && recoveries.is_empty());
    }

    /// The active-alerts list is capped at 50, evicting the oldest first, so a flapping
    /// rule cannot grow it unbounded.
    #[test]
    fn active_alerts_are_capped_at_fifty() {
        let engine = AlertEngine::new();
        let mut rule = percent_rule("cpu-cap", 50.0);
        rule.cooldown_seconds = 0;
        engine.add_rule(rule).unwrap();
        let mut metrics = sample_metrics();
        metrics.cpu_usage_percent = 90.0;

        for i in 0..60u64 {
            metrics.timestamp = 1_700_000_000 + i;
            let (alerts, _) = engine.evaluate(&metrics);
            assert_eq!(alerts.len(), 1);
        }
        let active = engine.get_active_alerts();
        assert_eq!(active.len(), 50);
        assert_eq!(active.first().unwrap().timestamp, 1_700_000_010, "the ten oldest were evicted");
        assert_eq!(active.last().unwrap().timestamp, 1_700_000_059);
    }

    /// Stored rules are JSON keyed on these exact variant names (alert_rules.rule); renaming
    /// one would orphan every stored rule on upgrade. Also pins the cooldown default for
    /// rows written before the field existed.
    #[test]
    fn alert_rule_wire_format_is_stable() {
        let rule = AlertRule {
            id: "r1".to_string(),
            metric: MetricType::CpuUsage,
            operator: ComparisonOperator::GreaterThan,
            threshold: 90.0,
            severity: AlertSeverity::Warning,
            enabled: true,
            cooldown_seconds: 300,
        };
        let json = serde_json::to_string(&rule).unwrap();
        assert!(json.contains(r#""metric":"CpuUsage""#), "{json}");
        assert!(json.contains(r#""operator":"GreaterThan""#), "{json}");
        assert!(json.contains(r#""severity":"Warning""#), "{json}");
        assert!(json.contains(r#""cooldown_seconds":300"#), "{json}");

        let pre_cooldown: AlertRule = serde_json::from_str(
            r#"{"id":"r1","metric":"CpuUsage","operator":"GreaterThan","threshold":90.0,"severity":"Warning","enabled":true}"#,
        )
        .expect("a rule stored before cooldown_seconds existed still loads");
        assert_eq!(pre_cooldown.cooldown_seconds, default_cooldown());

        // Every variant name that can appear in stored JSON.
        let names = [
            serde_json::to_string(&MetricType::CpuUsage).unwrap(),
            serde_json::to_string(&MetricType::MemoryUsage).unwrap(),
            serde_json::to_string(&MetricType::DiskUsage).unwrap(),
            serde_json::to_string(&ComparisonOperator::GreaterThan).unwrap(),
            serde_json::to_string(&ComparisonOperator::LessThan).unwrap(),
            serde_json::to_string(&ComparisonOperator::Equals).unwrap(),
            serde_json::to_string(&ComparisonOperator::GreaterThanOrEqual).unwrap(),
            serde_json::to_string(&ComparisonOperator::LessThanOrEqual).unwrap(),
            serde_json::to_string(&AlertSeverity::Info).unwrap(),
            serde_json::to_string(&AlertSeverity::Warning).unwrap(),
            serde_json::to_string(&AlertSeverity::Critical).unwrap(),
        ];
        assert_eq!(
            names,
            [
                r#""CpuUsage""#,
                r#""MemoryUsage""#,
                r#""DiskUsage""#,
                r#""GreaterThan""#,
                r#""LessThan""#,
                r#""Equals""#,
                r#""GreaterThanOrEqual""#,
                r#""LessThanOrEqual""#,
                r#""Info""#,
                r#""Warning""#,
                r#""Critical""#
            ]
        );
    }

    /// A stored rule that no longer parses is skipped without failing the load, and
    /// validate_rule holds the documented boundaries (id ≤ 64, threshold 0..=100 inclusive,
    /// cooldown ≤ one day inclusive).
    #[test]
    fn attach_store_skips_unparseable_rules_and_validation_holds_its_boundaries() {
        let path = std::env::temp_dir().join(format!(
            "quasar_alert_attach_{}_{}.db",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE alert_rules (id TEXT PRIMARY KEY, rule TEXT NOT NULL, updated_at INTEGER NOT NULL);",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO alert_rules (id, rule, updated_at) VALUES ('good', ?1, 0)",
                [serde_json::to_string(&percent_rule("good", 50.0)).unwrap()],
            )
            .unwrap();
            conn.execute("INSERT INTO alert_rules (id, rule, updated_at) VALUES ('bad-json', '{}', 0)", [])
                .unwrap();
            conn.execute(
                "INSERT INTO alert_rules (id, rule, updated_at) VALUES ('bad-variant', ?1, 0)",
                [r#"{"id":"x","metric":"CpuLoad","operator":"GreaterThan","threshold":1.0,"severity":"Warning","enabled":true,"cooldown_seconds":0}"#],
            )
            .unwrap();
        }

        let engine = AlertEngine::new();
        assert_eq!(engine.attach_store(path.to_str().unwrap().to_string()).unwrap(), 1);
        assert_eq!(engine.get_rules().len(), 1);
        assert_eq!(engine.get_rules()[0].id, "good");
        let _ = std::fs::remove_file(&path);

        assert!(validate_rule(&percent_rule("x", 0.0)).is_ok(), "0% is a valid threshold");
        assert!(validate_rule(&percent_rule("x", 100.0)).is_ok(), "100% is a valid threshold");
        assert!(validate_rule(&percent_rule("x", -0.1)).is_err());
        let mut maxed = percent_rule("x", 50.0);
        maxed.cooldown_seconds = MAX_COOLDOWN_SECONDS;
        assert!(validate_rule(&maxed).is_ok(), "exactly one day of cooldown is allowed");
        assert!(validate_rule(&percent_rule(&"i".repeat(64), 50.0)).is_ok());
        assert!(validate_rule(&percent_rule(&"i".repeat(65), 50.0)).is_err());
    }

    /// save_alert persists the full alert (severity as its Debug name is the stored
    /// contract), and cleanup_old_metrics deletes only rows past the retention window.
    #[test]
    fn alert_history_round_trip_and_metrics_cleanup() {
        let (store, db_path) = metrics_store_db();
        let alert = Alert {
            id: "a-1".to_string(),
            rule_id: "cpu-hot".to_string(),
            message: "CpuUsage is 90.0% (threshold: 50.0%)".to_string(),
            severity: AlertSeverity::Warning,
            timestamp: 1_700_000_000,
            acknowledged: false,
        };
        store.save_alert(&alert, "localhost").unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let old_ts = now - 31 * 86_400;
        let fresh_ts = now - 29 * 86_400;
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let (rule_id, host, severity, triggered_at, message): (String, String, String, i64, String) =
                conn.query_row(
                    "SELECT rule_id, host, severity, triggered_at, message FROM alert_history WHERE alert_id = 'a-1'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .unwrap();
            assert_eq!((rule_id.as_str(), host.as_str()), ("cpu-hot", "localhost"));
            assert_eq!(severity, "Warning", "severity is stored as its Debug name");
            assert_eq!(triggered_at, 1_700_000_000);
            assert!(message.contains("90.0%"));
            for ts in [old_ts, fresh_ts] {
                conn.execute(
                    "INSERT INTO metrics_history (timestamp, host) VALUES (?1, 'localhost')",
                    [ts],
                )
                .unwrap();
            }
        }

        let deleted = store.cleanup_old_metrics().unwrap();
        assert_eq!(deleted, 1, "only the 31-day-old row is past the 30-day retention");
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let remaining: Vec<i64> = conn
                .prepare("SELECT timestamp FROM metrics_history")
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();
            assert_eq!(remaining, vec![fresh_ts]);
        }
        let _ = std::fs::remove_file(&db_path);
    }
}

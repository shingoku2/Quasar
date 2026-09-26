//! The monitoring loop: collect, evaluate alerts, emit, persist, and housekeeping.

use super::{AlertEngine, MetricsCollector, MetricsStore};
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager};
use tokio::time::interval;

/// How long security audit entries are kept before the daily cleanup prunes them.
///
/// Deliberately longer than the 30-day metrics retention because these are
/// security records. Raise it if you need a longer compliance window.
pub(super) const AUDIT_RETENTION_DAYS: u32 = 90;

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
            if let Ok(db_path_str) = crate::db::db_path_in(&path) {
                match MetricsStore::new(db_path_str, 30) {
                    Ok(store) => Some(store),
                    Err(e) => {
                        error!(
                            "[MONITORING] CRITICAL: Failed to initialize metrics store: {}",
                            e
                        );
                        None
                    }
                }
            } else {
                error!("[MONITORING] ERROR: Invalid database path");
                None
            }
        }
        Err(e) => {
            error!(
                "[MONITORING] ERROR: Failed to get app data directory: {}",
                e
            );
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
        let generation = alert_engine.generation();
        let (alerts, recoveries) = alert_engine.evaluate(&metrics);

        // Close SSH sessions that have gone idle or aged out. Cheap map scan;
        // slots that are in use are skipped and caught by a later tick.
        if let Some(pool) = app_handle.try_state::<crate::ssh_pool::SshConnectionPool>() {
            let closed = pool.sweep_idle().await;
            if closed > 0 {
                info!("[MONITORING] INFO: Closed {} idle SSH session(s)", closed);
            }
        }

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
                    Ok(deleted) => info!(
                        "[MONITORING] INFO: Cleaned up {} old metric records",
                        deleted
                    ),
                    Err(e) => error!("[MONITORING] ERROR: Failed to cleanup metrics: {}", e),
                }

                // The audit log is otherwise unbounded; background credential
                // reads add rows on every host poll.
                if let Some(audit) = app_handle.try_state::<crate::vault::AuditLogManager>() {
                    match audit.cleanup_old_entries(AUDIT_RETENTION_DAYS) {
                        Ok(deleted) if deleted > 0 => info!(
                            "[MONITORING] INFO: Pruned {} audit entries older than {} days",
                            deleted, AUDIT_RETENTION_DAYS
                        ),
                        Ok(_) => {}
                        Err(e) => {
                            error!("[MONITORING] ERROR: Failed to prune audit log: {}", e)
                        }
                    }
                }
            }
        }

        // Emit recoveries and save alerts, unless an import replaced the rules since this
        // tick evaluated them.
        if !recoveries.is_empty() || !alerts.is_empty() {
            let persisted = alert_engine
                .persist_if_current(generation, || {
                    if !recoveries.is_empty() {
                        let _ = app_handle.emit("alerts-recovered", &recoveries);
                    }
                    if !alerts.is_empty() {
                        let _ = app_handle.emit("alerts-triggered", &alerts);
                        if let Some(ref store) = metrics_store {
                            for alert in &alerts {
                                if let Err(e) = store.save_alert(alert, "localhost") {
                                    error!(
                                        "[MONITORING] ERROR: Failed to save alert {}: {}",
                                        alert.id, e
                                    );
                                }
                            }
                        }
                    }
                })
                .await;
            if !persisted {
                info!("[MONITORING] INFO: Dropped alerts evaluated before a database import");
            }
        }
    }
}

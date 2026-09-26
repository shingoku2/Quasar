//! Alert rules and the engine that evaluates them, persists rules (RUST-002) and keeps
//! alert state across ticks (reload/suspend on import, invariant 13).

use super::SystemMetrics;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

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

pub(super) fn default_cooldown() -> u64 {
    300 // 5 minutes default cooldown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
// Variant names are serialized into the stored rule JSON (alert_rules.rule) — renaming would
// break stored rules.
#[allow(clippy::enum_variant_names)]
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

/// Longest cooldown a rule may have (one day).
pub(super) const MAX_COOLDOWN_SECONDS: u64 = 86_400;

/// Rejects rules the engine can't evaluate meaningfully. The metrics are percentages.
pub fn validate_rule(rule: &AlertRule) -> Result<(), String> {
    if rule.id.trim().is_empty() || rule.id.len() > 64 {
        return Err("Alert rule id must be 1-64 characters".to_string());
    }
    if !rule.threshold.is_finite() || !(0.0..=100.0).contains(&rule.threshold) {
        return Err("Alert threshold must be a percentage between 0 and 100".to_string());
    }
    if rule.cooldown_seconds > MAX_COOLDOWN_SECONDS {
        return Err("Alert cooldown can be at most one day".to_string());
    }
    Ok(())
}

pub struct AlertEngine {
    /// Database the rules are persisted to, once attached (RUST-002). Unset in unit tests.
    store: Mutex<Option<String>>,
    rules: Arc<Mutex<Vec<AlertRule>>>,
    active_alerts: Arc<Mutex<Vec<Alert>>>,
    cooldown_tracker: Arc<Mutex<HashMap<String, Instant>>>,
    last_alert_state: Arc<Mutex<HashMap<String, bool>>>,
    /// Bumped by every `suspend`. A monitoring tick records the generation before it
    /// evaluates and persists its alerts only if it hasn't changed (`persist_if_current`).
    generation: std::sync::atomic::AtomicU64,
    /// Held by an import for the whole database swap, and by a tick while it persists, so no
    /// alert from before the swap is written into the imported database (PR #68 review).
    persistence: Arc<tokio::sync::Mutex<()>>,
}

impl AlertEngine {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(None),
            rules: Arc::new(Mutex::new(Vec::new())),
            active_alerts: Arc::new(Mutex::new(Vec::new())),
            cooldown_tracker: Arc::new(Mutex::new(HashMap::new())),
            last_alert_state: Arc::new(Mutex::new(HashMap::new())),
            generation: std::sync::atomic::AtomicU64::new(0),
            persistence: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// The current rule generation; read it before `evaluate`.
    pub fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Blocks `persist_if_current` until the guard is dropped. An import holds it from
    /// before `suspend` until after `reload`.
    pub async fn hold_persistence(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.persistence.clone().lock_owned().await
    }

    /// Runs `persist` (emit and save a tick's alerts) only if no `suspend` happened since
    /// `generation` was read. Results of an evaluation from before an import are dropped,
    /// even if the tick got that far before the import started.
    pub async fn persist_if_current(&self, generation: u64, persist: impl FnOnce()) -> bool {
        let _guard = self.persistence.lock().await;
        if self.generation() != generation {
            return false;
        }
        persist();
        true
    }

    /// Loads the persisted rules from `db_path` and persists every later change there.
    /// Rules used to be in memory only, so every restart silently dropped them (RUST-002).
    /// A stored rule that no longer parses is skipped (and logged), not fatal.
    pub fn attach_store(&self, db_path: String) -> Result<usize, String> {
        let conn = crate::db::open_connection(&db_path)?;
        let mut stmt = conn
            .prepare("SELECT id, rule FROM alert_rules ORDER BY rowid")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        let mut loaded = Vec::new();
        for row in rows {
            let (id, json) = row.map_err(|e| e.to_string())?;
            match serde_json::from_str::<AlertRule>(&json) {
                Ok(rule) => loaded.push(rule),
                Err(e) => log::error!("Skipping unreadable alert rule {}: {}", id, e),
            }
        }
        let count = loaded.len();
        *self.rules.lock().unwrap_or_else(|p| p.into_inner()) = loaded;
        *self.store.lock().unwrap_or_else(|p| p.into_inner()) = Some(db_path);
        Ok(count)
    }

    /// Drops every rule and every piece of alert state (active alerts, cooldowns, last
    /// triggered state) until the next `reload`. An import calls this before it replaces the
    /// database, so the monitoring loop can't evaluate the old rules and record their alerts
    /// into the imported database in between (PR #68 review).
    pub fn suspend(&self) {
        self.generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.rules.lock().unwrap_or_else(|p| p.into_inner()).clear();
        self.active_alerts.lock().unwrap_or_else(|p| p.into_inner()).clear();
        self.cooldown_tracker.lock().unwrap_or_else(|p| p.into_inner()).clear();
        self.last_alert_state.lock().unwrap_or_else(|p| p.into_inner()).clear();
    }

    /// Forgets all alert state (`suspend`) and re-reads the rules from the attached database.
    /// Called after an import: the rules were loaded once at startup, so without this
    /// monitoring kept evaluating (and recording alerts from) the pre-import rules until
    /// restart, and a stale triggered state produced a recovery for an alert of the old
    /// database (PR #68 review). If the rules can't be read the engine has none, never the
    /// pre-import set. A no-op before `attach_store`.
    pub fn reload(&self) -> Result<usize, String> {
        let Some(path) = self.store_path() else {
            return Ok(0);
        };
        self.suspend();
        self.attach_store(path)
    }

    fn store_path(&self) -> Option<String> {
        self.store.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Adds or replaces a rule. It is persisted before the in-memory set changes, so a
    /// failed write never leaves a rule that would vanish on restart.
    pub fn add_rule(&self, rule: AlertRule) -> Result<(), String> {
        validate_rule(&rule)?;
        if let Some(path) = self.store_path() {
            let json = serde_json::to_string(&rule).map_err(|e| e.to_string())?;
            let conn = crate::db::open_connection(&path)?;
            conn.execute(
                "INSERT INTO alert_rules (id, rule, updated_at) VALUES (?1, ?2, strftime('%s','now'))
                 ON CONFLICT(id) DO UPDATE SET rule = excluded.rule, updated_at = excluded.updated_at",
                rusqlite::params![rule.id, json],
            )
            .map_err(|e| format!("Failed to save alert rule: {}", e))?;
        }
        // Recover from poison so a single panicked holder doesn't cascade.
        let mut rules = self
            .rules
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        rules.retain(|r| r.id != rule.id);
        rules.push(rule);
        Ok(())
    }

    pub fn remove_rule(&self, rule_id: &str) -> Result<(), String> {
        if let Some(path) = self.store_path() {
            let conn = crate::db::open_connection(&path)?;
            conn.execute("DELETE FROM alert_rules WHERE id = ?1", [rule_id])
                .map_err(|e| format!("Failed to delete alert rule: {}", e))?;
        }
        let mut rules = self
            .rules
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        rules.retain(|r| r.id != rule_id);
        Ok(())
    }

    pub fn get_rules(&self) -> Vec<AlertRule> {
        self.rules
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn evaluate(&self, metrics: &SystemMetrics) -> (Vec<Alert>, Vec<AlertRecovery>) {
        let rules = self
            .rules
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut new_alerts = Vec::new();
        let mut recoveries = Vec::new();
        let mut cooldowns = self
            .cooldown_tracker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut alert_states = self
            .last_alert_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

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
                    message: format!(
                        "{:?} recovered: now {:.1}% (threshold: {:.1}%)",
                        rule.metric, value, rule.threshold
                    ),
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
                let alert = Alert {
                    id: Uuid::new_v4().to_string(),
                    rule_id: rule.id.clone(),
                    message: format!(
                        "{:?} is {:.1}% (threshold: {:.1}%)",
                        rule.metric, value, rule.threshold
                    ),
                    severity: rule.severity.clone(),
                    timestamp: metrics.timestamp,
                    acknowledged: false,
                };
                new_alerts.push(alert);
                cooldowns.insert(rule.id.clone(), Instant::now());
            }
        }

        if !new_alerts.is_empty() {
            let mut active = self
                .active_alerts
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

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
        self.active_alerts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    #[allow(dead_code)]
    pub fn acknowledge_alert(&self, alert_id: &str) {
        let mut alerts = self
            .active_alerts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
        }
    }

    #[allow(dead_code)]
    pub fn dismiss_alert(&self, alert_id: &str) {
        let mut alerts = self
            .active_alerts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        alerts.retain(|a| a.id != alert_id);
    }
}

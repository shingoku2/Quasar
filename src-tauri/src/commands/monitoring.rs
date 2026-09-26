//! Local system metrics and alert rules.

use crate::*;

#[tauri::command]
pub(crate) fn get_system_metrics(
    collector: State<'_, Arc<std::sync::Mutex<monitoring::MetricsCollector>>>,
) -> Result<monitoring::SystemMetrics, String> {
    let mut collector = collector.lock().map_err(|e| {
        sanitize_error(
            format!("Failed to acquire metrics collector lock: {}", e),
            "monitoring",
        )
    })?;
    Ok(collector.collect())
}

#[tauri::command]
pub(crate) fn add_alert_rule(
    state: State<'_, Arc<monitoring::AlertEngine>>,
    rule: monitoring::AlertRule,
) -> Result<(), String> {
    state.add_rule(rule)
}

#[tauri::command]
pub(crate) fn remove_alert_rule(
    state: State<'_, Arc<monitoring::AlertEngine>>,
    rule_id: String,
) -> Result<(), String> {
    state.remove_rule(&rule_id)
}

#[tauri::command]
pub(crate) fn get_alert_rules(state: State<'_, Arc<monitoring::AlertEngine>>) -> Vec<monitoring::AlertRule> {
    state.get_rules()
}

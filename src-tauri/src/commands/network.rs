//! Network scanning, mDNS discovery, Tailscale status and one-off host checks.

use crate::*;

/// Snapshot of the local Tailscale node and its peers (via `tailscale status --json`).
/// A missing CLI is reported as `installed: false`, not as an error.
#[tauri::command]
pub(crate) async fn get_tailscale_status() -> Result<tailscale::TailscaleStatus, String> {
    tailscale::fetch_status()
        .await
        .map_err(|e| sanitize_error(e, "tailscale"))
}

#[tauri::command]
pub(crate) fn start_discovery(app: AppHandle, state: State<'_, discovery::DiscoveryState>) {
    discovery::start_mdns_discovery(app, state.running.clone(), state.stop_requested.clone());
}

#[tauri::command]
pub(crate) async fn scan_network(
    app: AppHandle,
    state: State<'_, Arc<scanner::ScannerState>>,
    tracker_state: State<'_, host_tracker::HostTracker>,
    cidr: String,
) -> Result<(), String> {
    validate_cidr(&cidr)?;
    let scanner_state = Arc::clone(state.inner());
    let app_for_progress = app.clone();
    let app_for_result = app.clone();
    let app_for_events = app.clone();

    // Clone the managed HostTracker to use in the spawned task
    let tracker = tracker_state.inner().clone();

    // Claim the scan synchronously, before spawning, so there is no window
    // in which a stop_scan() call could land between "task spawned" and
    // "task actually starts" and have its stop signal silently discarded by
    // the spawned task's own claim. See scanner::claim_scan for details.
    scanner::claim_scan(&scanner_state).map_err(|e| sanitize_error(e, "scanner"))?;

    tokio::spawn(async move {
        let on_progress = move |progress: scanner::ScanProgress| {
            let _ = app_for_progress.emit("scan_progress", progress);
        };

        let on_result = move |result: scanner::ScanResult| {
            // Save host to database if alive
            if result.is_alive {
                if let Err(e) = tracker.save_host(&result) {
                    error!("Failed to save discovered host {}: {}", result.ip, e);
                    let _ = app_for_result
                        .emit("scan_error", sanitize_error(e.to_string(), "database"));
                }
            }
            let _ = app_for_result.emit("scan_result", result);
        };

        match scanner::scan_network(scanner_state, cidr, on_progress, on_result).await {
            Ok(()) => {
                let _ = app_for_events.emit("scan_complete", ());
            }
            Err(e) => {
                let _ = app_for_events.emit("scan_error", sanitize_error(e, "scanner"));
                let _ = app_for_events.emit("scan_complete", ());
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub(crate) fn stop_scan(state: State<'_, Arc<scanner::ScannerState>>) {
    scanner::stop_scan(&state);
}

#[tauri::command]
pub(crate) fn get_scan_progress(state: State<'_, Arc<scanner::ScannerState>>) -> scanner::ScanProgress {
    scanner::get_scan_progress(&state)
}

#[tauri::command]
pub(crate) fn is_scanning(state: State<'_, Arc<scanner::ScannerState>>) -> bool {
    scanner::is_scanning(&state)
}

#[tauri::command]
pub(crate) fn get_discovered_hosts(
    host_tracker: State<'_, host_tracker::HostTracker>,
    limit: Option<usize>,
) -> Result<Vec<host_tracker::DiscoveredHost>, String> {
    host_tracker
        .list_hosts(limit)
        .map_err(|e| sanitize_error(e, "database"))
}

#[tauri::command]
pub(crate) fn delete_discovered_host(
    host_tracker: State<'_, host_tracker::HostTracker>,
    ip: String,
) -> Result<(), String> {
    validate_ip(&ip)?;
    host_tracker
        .delete_host(&ip)
        .map_err(|e| sanitize_error(e, "database"))
}

#[tauri::command]
pub(crate) async fn preflight_check(host: String) -> Result<health::HealthCheckResult, String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    Ok(health::preflight_check(&host).await)
}

#[tauri::command]
pub(crate) async fn check_host_health(
    app: AppHandle,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
) -> Result<health::HealthCheckResult, String> {
    if validate_ip(&host).is_err() && validate_hostname(&host).is_err() {
        return Err("Invalid host format".to_string());
    }
    validate_port(port)?;
    validate_username(&username)?;
    Ok(health::check_ssh_health(
        app,
        &host,
        port,
        &username,
        password.as_deref(),
        false,
        None,
        None,
        None,
    )
    .await)
}

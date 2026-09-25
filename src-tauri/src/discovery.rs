use log::{error, info};
use mdns_sd::{RecvTimeoutError, ServiceDaemon, ServiceEvent};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredHost {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub service_type: String,
}

pub struct DiscoveryState {
    pub running: Arc<AtomicBool>,
    /// Set to true to request the discovery thread to stop.
    pub stop_requested: Arc<AtomicBool>,
}

impl DiscoveryState {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            stop_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Request the background discovery thread to stop, if running.
    #[allow(dead_code)]
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
    }
}

pub fn start_mdns_discovery(
    app: AppHandle,
    running: Arc<AtomicBool>,
    stop_requested: Arc<AtomicBool>,
) {
    if running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        info!("[discovery] mDNS discovery already running, skipping duplicate spawn");
        return;
    }

    // Clear any previous stop request before starting.
    stop_requested.store(false, Ordering::SeqCst);

    let running_flag = running.clone();
    thread::spawn(move || {
        let _guard = DropGuard(running_flag);
        // Create a daemon; exit gracefully if creation fails (e.g. no multicast support)
        let mdns = match ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => {
                error!("[discovery] Failed to create mDNS daemon: {}", e);
                return;
            }
        };

        // Browse for SSH services and generic workstation services
        let service_types = vec!["_ssh._tcp.local.", "_workstation._tcp.local."];

        let mut receivers = Vec::new();
        for service_type in &service_types {
            match mdns.browse(service_type) {
                Ok(receiver) => receivers.push(receiver),
                Err(e) => error!("Failed to browse {}: {}", service_type, e),
            }
        }

        // Poll all receivers with timeout; exit when stop is requested.
        loop {
            if stop_requested.load(Ordering::SeqCst) {
                info!("[discovery] Stop requested, shutting down mDNS thread");
                break;
            }

            // A browse whose daemon has gone away reports Disconnected forever; drop it.
            // When none are left, end the thread (the DropGuard frees the singleton so
            // discovery can be started again) instead of spinning on dead channels.
            poll_receivers(&mut receivers, Duration::from_millis(100), |host| {
                let _ = app.emit("host-discovered", host);
            });
            if receivers.is_empty() {
                error!("[discovery] mDNS daemon stopped; ending discovery");
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    });
}

/// Waits up to `wait` on each receiver, passes resolved hosts to `on_host`, and drops
/// receivers whose sender (the mDNS daemon) is gone.
fn poll_receivers(
    receivers: &mut Vec<mdns_sd::Receiver<ServiceEvent>>,
    wait: Duration,
    mut on_host: impl FnMut(DiscoveredHost),
) {
    receivers.retain(|receiver| match receiver.recv_timeout(wait) {
        Ok(event) => {
            if let Some(host) = discovered_host(event) {
                on_host(host);
            }
            true
        }
        Err(RecvTimeoutError::Timeout) => true,
        Err(RecvTimeoutError::Disconnected) => false,
    });
}

/// The host a resolved service event announces, if it has an address.
fn discovered_host(event: ServiceEvent) -> Option<DiscoveredHost> {
    let ServiceEvent::ServiceResolved(info) = event else {
        return None;
    };
    let addr = info.get_addresses().iter().next()?.to_string();
    Some(DiscoveredHost {
        name: info.get_fullname().to_string(),
        address: addr,
        port: info.get_port(),
        service_type: "discovered".to_string(),
    })
}

struct DropGuard(Arc<AtomicBool>);

impl Drop for DropGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod poll_tests {
    use super::*;

    /// D3 / DEP-001 follow-up: a dead daemon's receivers are dropped, so the discovery
    /// loop can end instead of spinning on Disconnected forever.
    #[test]
    fn disconnected_receivers_are_dropped_and_live_ones_kept() {
        let (live_tx, live_rx) = flume::bounded::<ServiceEvent>(1);
        let (dead_tx, dead_rx) = flume::bounded::<ServiceEvent>(1);
        drop(dead_tx);
        let mut receivers = vec![live_rx, dead_rx];
        let mut hosts = 0;
        poll_receivers(&mut receivers, Duration::from_millis(10), |_| hosts += 1);
        assert_eq!(receivers.len(), 1, "only the live receiver remains");
        assert_eq!(hosts, 0);
        drop(live_tx);
        poll_receivers(&mut receivers, Duration::from_millis(10), |_| hosts += 1);
        assert!(receivers.is_empty());
    }
}

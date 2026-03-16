use tauri::{AppHandle, Emitter};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::Serialize;
use log::{error, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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

pub fn start_mdns_discovery(app: AppHandle, running: Arc<AtomicBool>, stop_requested: Arc<AtomicBool>) {
    if running.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
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

            for receiver in &receivers {
                if let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) {
                    match event {
                        ServiceEvent::ServiceResolved(info) => {
                            let addresses = info.get_addresses();
                            let port = info.get_port();
                            let name = info.get_fullname();

                            if let Some(addr) = addresses.iter().next() {
                                let host = DiscoveredHost {
                                    name: name.to_string(),
                                    address: addr.to_string(),
                                    port,
                                    service_type: "discovered".to_string(),
                                };

                                // Emit event to frontend
                                let _ = app.emit("host-discovered", host);
                            }
                        }
                        _ => {}
                    }
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    });
}

struct DropGuard(Arc<AtomicBool>);

impl Drop for DropGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

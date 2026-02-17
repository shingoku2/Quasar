use tauri::{AppHandle, Emitter};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::Serialize;
use log::error;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredHost {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub service_type: String,
}

pub fn start_mdns_discovery(app: AppHandle) {
    thread::spawn(move || {
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

        // Poll all receivers with timeout
        loop {
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

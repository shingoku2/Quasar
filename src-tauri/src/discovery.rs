use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Clone, serde::Serialize)]
pub struct DiscoveredHost {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub service_type: String,
}

pub fn start_mdns_discovery(app: AppHandle) {
    thread::spawn(move || {
        // Create a daemon
        let mdns = ServiceDaemon::new().expect("Failed to create daemon");

        // Browse for SSH services and generic workstation services
        let service_types = vec!["_ssh._tcp.local.", "_workstation._tcp.local."];

        for service_type in service_types {
            let receiver = mdns.browse(service_type).expect("Failed to browse");

            while let Ok(event) = receiver.recv() {
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
                                service_type: service_type.to_string(),
                            };
                            
                            // Emit event to frontend
                            let _ = app.emit("host-discovered", host);
                        }
                    }
                    _ => {}
                }
            }
        }
    });
}

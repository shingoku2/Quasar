use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::scanner::{ScanResult, ServiceInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredHost {
    pub id: String,
    pub ip: String,
    pub hostname: Option<String>,
    pub mac_address: Option<String>,
    pub device_type: String,
    pub vendor: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub scan_count: i32,
    pub services: Vec<ServiceInfo>,
}

pub struct HostTracker {
    db_path: String,
}

impl HostTracker {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

     fn open_connection(&self) -> Result<Connection, String> {
         let conn = Connection::open(&self.db_path)
             .map_err(|e| format!("Failed to open database: {}", e))?;
         conn.execute_batch("PRAGMA foreign_keys = ON;")
             .map_err(|e| format!("Failed to enable foreign keys: {}", e))?;
         Ok(conn)
     }

    pub fn save_host(&self, scan_result: &ScanResult) -> Result<String, String> {
        let conn = self.open_connection()?;

        // Check if host already exists
        let existing: Result<(String, i64, i32), rusqlite::Error> = conn.query_row(
            "SELECT id, first_seen, scan_count FROM discovered_hosts WHERE ip = ?1",
            [&scan_result.ip],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );

        let host_id = match existing {
            Ok((id, _first_seen, scan_count)) => {
                // Update existing host
                conn.execute(
                    "UPDATE discovered_hosts SET 
                        hostname = ?1, 
                        mac_address = ?2, 
                        device_type = ?3, 
                        vendor = ?4, 
                        last_seen = ?5, 
                        scan_count = ?6
                    WHERE id = ?7",
                    rusqlite::params![
                        scan_result.hostname,
                        scan_result.mac_address,
                        scan_result.device_type,
                        scan_result.vendor,
                        scan_result.last_seen,
                        scan_count + 1,
                        id,
                    ],
                ).map_err(|e| format!("Failed to update host: {}", e))?;
                id
            }
            Err(_) => {
                // Insert new host
                let id = Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO discovered_hosts (id, ip, hostname, mac_address, device_type, vendor, first_seen, last_seen, scan_count)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    rusqlite::params![
                        id,
                        scan_result.ip,
                        scan_result.hostname,
                        scan_result.mac_address,
                        scan_result.device_type,
                        scan_result.vendor,
                        scan_result.last_seen,
                        scan_result.last_seen,
                        1,
                    ],
                ).map_err(|e| format!("Failed to insert host: {}", e))?;
                id
            }
        };

        // Update services
        for service in &scan_result.services {
            self.save_service(&conn, &host_id, service, scan_result.last_seen)?;
        }

        Ok(host_id)
    }

    fn save_service(
        &self,
        conn: &Connection,
        host_id: &str,
        service: &ServiceInfo,
        timestamp: i64,
    ) -> Result<(), String> {
        // Check if service already exists
        let existing: Result<(String, i64), rusqlite::Error> = conn.query_row(
            "SELECT id, first_detected FROM host_services WHERE host_id = ?1 AND port = ?2 AND protocol = ?3",
            rusqlite::params![host_id, service.port, service.protocol],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        match existing {
            Ok((id, _first_detected)) => {
                // Update existing service
                conn.execute(
                    "UPDATE host_services SET service = ?1, version = ?2, last_detected = ?3 WHERE id = ?4",
                    rusqlite::params![service.service, service.version, timestamp, id],
                ).map_err(|e| format!("Failed to update service: {}", e))?;
            }
            Err(_) => {
                // Insert new service
                let id = Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO host_services (id, host_id, port, protocol, service, version, first_detected, last_detected)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        id,
                        host_id,
                        service.port,
                        service.protocol,
                        service.service,
                        service.version,
                        timestamp,
                        timestamp,
                    ],
                ).map_err(|e| format!("Failed to insert service: {}", e))?;
            }
        }

        Ok(())
    }

    pub fn get_host(&self, ip: &str) -> Result<Option<DiscoveredHost>, String> {
        let conn = self.open_connection()?;

        let host_result: Result<DiscoveredHost, rusqlite::Error> = conn.query_row(
            "SELECT id, ip, hostname, mac_address, device_type, vendor, first_seen, last_seen, scan_count
             FROM discovered_hosts WHERE ip = ?1",
            [ip],
            |row| {
                Ok(DiscoveredHost {
                    id: row.get(0)?,
                    ip: row.get(1)?,
                    hostname: row.get(2)?,
                    mac_address: row.get(3)?,
                    device_type: row.get(4)?,
                    vendor: row.get(5)?,
                    first_seen: row.get(6)?,
                    last_seen: row.get(7)?,
                    scan_count: row.get(8)?,
                    services: Vec::new(), // Will be populated below
                })
            },
        );

        match host_result {
            Ok(mut host) => {
                // Load services for this host
                host.services = self.get_host_services(&conn, &host.id)?;
                Ok(Some(host))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to get host: {}", e)),
        }
    }

    fn get_host_services(&self, conn: &Connection, host_id: &str) -> Result<Vec<ServiceInfo>, String> {
        let mut stmt = conn.prepare(
            "SELECT port, protocol, service, version FROM host_services WHERE host_id = ?1"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let services = stmt.query_map([host_id], |row| {
            Ok(ServiceInfo {
                port: row.get(0)?,
                protocol: row.get(1)?,
                service: row.get(2)?,
                version: row.get(3)?,
            })
        }).map_err(|e| format!("Failed to query services: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect services: {}", e))?;

        Ok(services)
    }

    pub fn list_hosts(&self, limit: Option<usize>) -> Result<Vec<DiscoveredHost>, String> {
        let conn = self.open_connection()?;

        let query = if let Some(limit) = limit {
            format!(
                "SELECT id, ip, hostname, mac_address, device_type, vendor, first_seen, last_seen, scan_count
                 FROM discovered_hosts ORDER BY last_seen DESC LIMIT {}",
                limit
            )
        } else {
            "SELECT id, ip, hostname, mac_address, device_type, vendor, first_seen, last_seen, scan_count
             FROM discovered_hosts ORDER BY last_seen DESC".to_string()
        };

        let mut stmt = conn.prepare(&query)
            .map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let hosts = stmt.query_map([], |row| {
            Ok(DiscoveredHost {
                id: row.get(0)?,
                ip: row.get(1)?,
                hostname: row.get(2)?,
                mac_address: row.get(3)?,
                device_type: row.get(4)?,
                vendor: row.get(5)?,
                first_seen: row.get(6)?,
                last_seen: row.get(7)?,
                scan_count: row.get(8)?,
                services: Vec::new(),
            })
        }).map_err(|e| format!("Failed to query hosts: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect hosts: {}", e))?;

        // Load services for each host
        let mut hosts_with_services = Vec::new();
        for mut host in hosts {
            host.services = self.get_host_services(&conn, &host.id)?;
            hosts_with_services.push(host);
        }

        Ok(hosts_with_services)
    }

    pub fn search_hosts(&self, query: &str) -> Result<Vec<DiscoveredHost>, String> {
        let conn = self.open_connection()?;

        let search_pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            "SELECT id, ip, hostname, mac_address, device_type, vendor, first_seen, last_seen, scan_count
             FROM discovered_hosts 
             WHERE ip LIKE ?1 OR hostname LIKE ?1 OR device_type LIKE ?1
             ORDER BY last_seen DESC"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let hosts = stmt.query_map([&search_pattern], |row| {
            Ok(DiscoveredHost {
                id: row.get(0)?,
                ip: row.get(1)?,
                hostname: row.get(2)?,
                mac_address: row.get(3)?,
                device_type: row.get(4)?,
                vendor: row.get(5)?,
                first_seen: row.get(6)?,
                last_seen: row.get(7)?,
                scan_count: row.get(8)?,
                services: Vec::new(),
            })
        }).map_err(|e| format!("Failed to query hosts: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect hosts: {}", e))?;

        // Load services for each host
        let mut hosts_with_services = Vec::new();
        for mut host in hosts {
            host.services = self.get_host_services(&conn, &host.id)?;
            hosts_with_services.push(host);
        }

        Ok(hosts_with_services)
    }

    pub fn delete_host(&self, ip: &str) -> Result<(), String> {
        let conn = self.open_connection()?;

        conn.execute(
            "DELETE FROM discovered_hosts WHERE ip = ?1",
            [ip],
        ).map_err(|e| format!("Failed to delete host: {}", e))?;

        Ok(())
    }
}

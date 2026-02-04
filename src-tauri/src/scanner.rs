use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use cidr_utils::cidr::Ipv4Cidr;
use futures::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use surge_ping::{Client, Config, PingIdentifier, PingSequence};
use tokio::time::timeout;

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ServiceInfo {
    pub port: u16,
    pub protocol: String,
    pub service: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub ip: String,
    pub is_alive: bool,
    pub latency_ms: Option<u32>,
    pub open_ports: Vec<u16>,
    pub hostname: Option<String>,
    pub mac_address: Option<String>,
    pub device_type: String,
    pub services: Vec<ServiceInfo>,
    pub vendor: Option<String>,
    pub last_seen: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    pub total: usize,
    pub completed: usize,
    pub current_ip: Option<String>,
}

pub struct ScannerState {
    pub is_scanning: Arc<Mutex<bool>>,
    pub results: Arc<Mutex<Vec<ScanResult>>>,
    pub progress: Arc<Mutex<ScanProgress>>,
    pub stop_signal: Arc<Mutex<bool>>,
}

impl ScannerState {
    pub fn new() -> Self {
        Self {
            is_scanning: Arc::new(Mutex::new(false)),
            results: Arc::new(Mutex::new(Vec::new())),
            progress: Arc::new(Mutex::new(ScanProgress {
                total: 0,
                completed: 0,
                current_ip: None,
            })),
            stop_signal: Arc::new(Mutex::new(false)),
        }
    }
}

 struct ScanRunningGuard {
     is_scanning: Arc<Mutex<bool>>,
 }

 impl Drop for ScanRunningGuard {
     fn drop(&mut self) {
         if let Ok(mut is_scanning) = self.is_scanning.lock() {
             *is_scanning = false;
         }
     }
 }

fn parse_cidr(cidr_str: &str) -> Result<Vec<IpAddr>, String> {
    let cidr: Ipv4Cidr = cidr_str.parse().map_err(|e| format!("Invalid CIDR: {}", e))?;
    Ok(cidr.iter().map(|ip_u32| IpAddr::from(std::net::Ipv4Addr::from(ip_u32))).collect())
}

async fn ping_host(client: &Client, ip: IpAddr) -> Result<u32, String> {
    let payload = vec![0; 56];
    let ident = PingIdentifier(rand::random::<u16>());
    let seq = PingSequence(0);
    
    let mut pinger = client.pinger(ip, ident).await;
    
    match timeout(Duration::from_secs(2), pinger.ping(seq, &payload)).await {
        Ok(Ok((_, dur))) => Ok(dur.as_millis() as u32),
        Ok(Err(e)) => Err(format!("Ping error: {}", e)),
        Err(_) => Err("Ping timeout".to_string()),
    }
}

async fn scan_port(ip: IpAddr, port: u16) -> bool {
    use tokio::net::TcpStream;
    
    let addr = format!("{}:{}", ip, port);
    match timeout(Duration::from_secs(1), TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => true,
        _ => false,
    }
}

async fn resolve_hostname(ip: IpAddr) -> Option<String> {
    use tokio::net::lookup_host;
    
    match timeout(Duration::from_secs(2), lookup_host((ip, 0))).await {
        Ok(Ok(mut addrs)) => {
            if let Some(addr) = addrs.next() {
                // Try reverse DNS lookup
                match timeout(Duration::from_secs(2), tokio::task::spawn_blocking(move || {
                    use std::net::ToSocketAddrs;
                    addr.to_string().to_socket_addrs().ok()
                })).await {
                    Ok(Ok(_)) => {
                        // Simple approach: return IP as hostname for now
                        // Full DNS reverse lookup would require additional dependencies
                        None
                    },
                    _ => None,
                }
            } else {
                None
            }
        },
        _ => None,
    }
}

fn detect_device_type(open_ports: &[u16]) -> String {
    // Router detection: common management ports
    if open_ports.contains(&80) && open_ports.contains(&443) && open_ports.len() <= 3 {
        return "router".to_string();
    }
    
    // Printer detection
    if open_ports.contains(&9100) || open_ports.contains(&515) || open_ports.contains(&631) {
        return "printer".to_string();
    }
    
    // Server detection: multiple services
    if open_ports.len() >= 3 || open_ports.contains(&3389) || open_ports.contains(&445) {
        return "server".to_string();
    }
    
    // Workstation: SSH or RDP
    if open_ports.contains(&22) || open_ports.contains(&3389) {
        return "workstation".to_string();
    }
    
    "unknown".to_string()
}

fn identify_service(port: u16) -> ServiceInfo {
    let (service, protocol) = match port {
        22 => ("SSH", "tcp"),
        23 => ("Telnet", "tcp"),
        80 => ("HTTP", "tcp"),
        443 => ("HTTPS", "tcp"),
        445 => ("SMB", "tcp"),
        3306 => ("MySQL", "tcp"),
        3389 => ("RDP", "tcp"),
        5432 => ("PostgreSQL", "tcp"),
        6379 => ("Redis", "tcp"),
        8080 => ("HTTP-Alt", "tcp"),
        9100 => ("Printer", "tcp"),
        515 => ("LPD", "tcp"),
        631 => ("IPP", "tcp"),
        _ => ("Unknown", "tcp"),
    };
    
    ServiceInfo {
        port,
        protocol: protocol.to_string(),
        service: service.to_string(),
        version: None,
    }
}

pub async fn scan_network(
    state: Arc<ScannerState>,
    cidr: String,
    on_progress: impl Fn(ScanProgress) + Send + 'static,
    on_result: impl Fn(ScanResult) + Send + 'static,
) -> Result<(), String> {
    // FIX: Check-and-set atomically to prevent TOCTOU race condition
    // Keep lock held during entire check-and-set operation
    {
        let mut is_scanning = state.is_scanning.lock().unwrap();
        if *is_scanning {
            return Err("Scan already in progress".to_string());
        }
        *is_scanning = true;
        
        // Reset stop signal while we have exclusive access
        let mut stop = state.stop_signal.lock().unwrap();
        *stop = false;
    }

     let _guard = ScanRunningGuard {
         is_scanning: Arc::clone(&state.is_scanning),
     };
    
    // Clear previous results
    {
        let mut results = state.results.lock().unwrap();
        results.clear();
    }
    
    let ips = parse_cidr(&cidr)?;
    let total = ips.len();
    
    // Update total in progress
    {
        let mut progress = state.progress.lock().unwrap();
        progress.total = total;
        progress.completed = 0;
    }
    
    let client = Client::new(&Config::default()).map_err(|e| e.to_string())?;
    let client = Arc::new(client);
    let state_arc = Arc::clone(&state);
    
    // Process IPs concurrently with a limit of 50 concurrent pings
    let mut futures = FuturesUnordered::new();
    let mut completed = 0;
    
    for (idx, ip) in ips.iter().enumerate() {
        // Check stop signal
        {
            let stop = state.stop_signal.lock().unwrap();
            if *stop {
                break;
            }
        }
        
        let ip = *ip;
        let client = Arc::clone(&client);
        let state_for_task = Arc::clone(&state_arc);
        
        futures.push(tokio::spawn(async move {
            // Update current IP in progress
            {
                let mut progress = state_for_task.progress.lock().unwrap();
                progress.current_ip = Some(ip.to_string());
            }
            
            let ping_result = ping_host(&client, ip).await;
            let is_alive = ping_result.is_ok();
            let latency_ms = ping_result.ok();
            
            let mut open_ports = Vec::new();
            
            let mut services = Vec::new();
            
            if is_alive {
                // Scan expanded port list for alive hosts
                let common_ports = [22, 23, 80, 443, 445, 3306, 3389, 5432, 6379, 8080, 9100, 515, 631];
                for port in common_ports {
                    if scan_port(ip, port).await {
                        open_ports.push(port);
                        services.push(identify_service(port));
                    }
                }
            }
            
            // Resolve hostname (only for alive hosts to save time)
            let hostname = if is_alive {
                resolve_hostname(ip).await
            } else {
                None
            };
            
            // Detect device type based on open ports
            let device_type = detect_device_type(&open_ports);
            
            // Get current timestamp
            let last_seen = chrono::Utc::now().timestamp();
            
            ScanResult {
                ip: ip.to_string(),
                is_alive,
                latency_ms,
                open_ports,
                hostname,
                mac_address: None, // MAC address detection requires raw sockets/ARP
                device_type,
                services,
                vendor: None, // Vendor lookup would require MAC address
                last_seen,
            }
        }));
        
        // Process completed futures when we hit concurrency limit
        if futures.len() >= 50 || idx == ips.len() - 1 {
            while let Some(result) = futures.next().await {
                if let Ok(scan_result) = result {
                    // Store result
                    {
                        let mut results = state.results.lock().unwrap();
                        results.push(scan_result.clone());
                    }
                    // Emit result
                    on_result(scan_result);
                    
                    completed += 1;
                    {
                        let mut progress = state.progress.lock().unwrap();
                        progress.completed = completed;
                    }
                    on_progress(state.progress.lock().unwrap().clone());
                }
            }
        }
    }
    
    Ok(())
}

pub fn stop_scan(state: &ScannerState) {
    let mut stop = state.stop_signal.lock().unwrap();
    *stop = true;
    
    let mut is_scanning = state.is_scanning.lock().unwrap();
    *is_scanning = false;
}

pub fn get_scan_progress(state: &ScannerState) -> ScanProgress {
    state.progress.lock().unwrap().clone()
}

pub fn get_scan_results(state: &ScannerState) -> Vec<ScanResult> {
    state.results.lock().unwrap().clone()
}

pub fn is_scanning(state: &ScannerState) -> bool {
    *state.is_scanning.lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cidr() {
        let ips = parse_cidr("192.168.1.0/30").unwrap();
        assert_eq!(ips.len(), 4);
        
        // Verify the IPs are correct
        assert_eq!(ips[0].to_string(), "192.168.1.0");
        assert_eq!(ips[1].to_string(), "192.168.1.1");
        assert_eq!(ips[2].to_string(), "192.168.1.2");
        assert_eq!(ips[3].to_string(), "192.168.1.3");
    }

    #[test]
    fn test_parse_cidr_invalid() {
        let result = parse_cidr("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_cidr_larger_subnet() {
        let ips = parse_cidr("10.0.0.0/24").unwrap();
        assert_eq!(ips.len(), 256);
    }

    #[test]
    fn test_scan_result_serialization() {
        let result = ScanResult {
            ip: "192.168.1.1".to_string(),
            is_alive: true,
            latency_ms: Some(10),
            open_ports: vec![22, 80],
            hostname: Some("test-host".to_string()),
            mac_address: None,
            device_type: "workstation".to_string(),
            services: vec![],
            vendor: None,
            last_seen: 1234567890,
        };
        
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("192.168.1.1"));
        assert!(json.contains("is_alive"));
        assert!(json.contains("latency_ms"));
        assert!(json.contains("open_ports"));
    }

    #[test]
    fn test_scan_progress_serialization() {
        let progress = ScanProgress {
            total: 256,
            completed: 100,
            current_ip: Some("192.168.1.100".to_string()),
        };
        
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("256"));
        assert!(json.contains("100"));
        assert!(json.contains("192.168.1.100"));
    }

    #[test]
    fn test_scanner_state_new() {
        let state = ScannerState::new();
        assert!(!is_scanning(&state));
        
        let results = get_scan_results(&state);
        assert!(results.is_empty());
        
        let progress = get_scan_progress(&state);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_stop_scan() {
        let state = ScannerState::new();
        
        // Set scanning state
        *state.is_scanning.lock().unwrap() = true;
        assert!(is_scanning(&state));
        
        // Stop scan
        stop_scan(&state);
        assert!(!is_scanning(&state));
        
        // Verify stop signal is set
        assert!(*state.stop_signal.lock().unwrap());
    }

    #[test]
    fn test_scan_result_fields() {
        let result = ScanResult {
            ip: "10.0.0.1".to_string(),
            is_alive: true,
            latency_ms: Some(5),
            open_ports: vec![22, 443],
            hostname: None,
            mac_address: None,
            device_type: "workstation".to_string(),
            services: vec![],
            vendor: None,
            last_seen: 1234567890,
        };
        
        assert_eq!(result.ip, "10.0.0.1");
        assert!(result.is_alive);
        assert_eq!(result.latency_ms, Some(5));
        assert_eq!(result.open_ports.len(), 2);
    }

    #[test]
    fn test_scan_result_dead_host() {
        let result = ScanResult {
            ip: "192.168.255.255".to_string(),
            is_alive: false,
            latency_ms: None,
            open_ports: vec![],
            hostname: None,
            mac_address: None,
            device_type: "unknown".to_string(),
            services: vec![],
            vendor: None,
            last_seen: 1234567890,
        };
        
        assert!(!result.is_alive);
        assert!(result.open_ports.is_empty());
        assert_eq!(result.latency_ms, None);
    }
}

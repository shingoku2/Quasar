use crate::validation;
use cidr_utils::cidr::Ipv4Cidr;
use futures::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
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

/// Scanner shared state. Mutex locks use unwrap() (no poison recovery) so that concurrency bugs surface.
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
    // Parse as Ipv4Inet first: unlike Ipv4Cidr, it accepts host bits set
    // (e.g. "192.168.1.5/24"), matching the old cidr-utils 0.5 behavior.
    let inet: cidr_utils::cidr::Ipv4Inet = cidr_str
        .parse()
        .map_err(|e| format!("Invalid CIDR: {}", e))?;
    let cidr: Ipv4Cidr = inet.network();
    if cidr.network_length() < 16 {
        return Err(format!(
            "CIDR range too large (/{} covers {} hosts). Maximum supported range is /16.",
            cidr.network_length(),
            1u64 << (32 - cidr.network_length())
        ));
    }
    Ok(cidr
        .iter()
        .map(|inet| IpAddr::from(inet.address()))
        .collect())
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
    matches!(
        timeout(Duration::from_secs(1), TcpStream::connect(&addr)).await,
        Ok(Ok(_))
    )
}

async fn resolve_hostname(ip: IpAddr) -> Option<String> {
    // Perform reverse DNS lookup using blocking DNS resolution
    let ip_clone = ip;
    match timeout(
        Duration::from_secs(2),
        tokio::task::spawn_blocking(move || {
            use std::net::ToSocketAddrs;
            // Create a socket address for reverse lookup
            let socket_addr = format!("{}:0", ip_clone);

            // Try to resolve the hostname
            // Note: This uses the system's DNS resolver
            match socket_addr.to_socket_addrs() {
                Ok(mut addrs) => {
                    if let Some(addr) = addrs.next() {
                        // Try reverse lookup using DNS
                        dns_lookup::lookup_addr(&addr.ip()).ok()
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        }),
    )
    .await
    {
        Ok(Ok(result)) => result,
        _ => None,
    }
}

fn detect_device_type(open_ports: &[u16]) -> String {
    // Router detection: common management ports only
    if open_ports.contains(&80) && open_ports.contains(&443) && open_ports.len() <= 3 {
        return "router".to_string();
    }

    // Printer detection
    if open_ports.contains(&9100) || open_ports.contains(&515) || open_ports.contains(&631) {
        return "printer".to_string();
    }

    // Server detection: multiple services or SMB file sharing
    // Note: RDP alone does NOT indicate a server (could be a workstation)
    if open_ports.len() >= 3 || open_ports.contains(&445) {
        return "server".to_string();
    }

    // Workstation: SSH or RDP (with fewer than 3 open ports)
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

/// Number of hosts probed concurrently.
const MAX_CONCURRENT_SCANS: usize = 50;

/// Store, emit and count a single finished host probe.
fn record_scan_result<P, R>(
    scan_result: ScanResult,
    state: &Arc<ScannerState>,
    completed: &mut usize,
    on_progress: &P,
    on_result: &R,
) where
    P: Fn(ScanProgress) + Send + 'static,
    R: Fn(ScanResult) + Send + 'static,
{
    // Store result
    {
        if let Ok(mut results) = state.results.lock() {
            results.push(scan_result.clone());
        }
    }

    // Emit result
    on_result(scan_result);

    *completed += 1;
    {
        if let Ok(mut progress) = state.progress.lock() {
            progress.completed = *completed;
        }
    }
    if let Ok(progress) = state.progress.lock() {
        on_progress(progress.clone());
    }
}

/// Retire exactly one finished probe, freeing a slot in the concurrency window.
///
/// Draining the whole batch at the limit made every window wait on its slowest
/// host before any new probe could start.
async fn drain_one_scan_future<P, R>(
    futures: &mut FuturesUnordered<tokio::task::JoinHandle<ScanResult>>,
    state: &Arc<ScannerState>,
    completed: &mut usize,
    on_progress: &P,
    on_result: &R,
) where
    P: Fn(ScanProgress) + Send + 'static,
    R: Fn(ScanResult) + Send + 'static,
{
    if let Some(Ok(scan_result)) = futures.next().await {
        record_scan_result(scan_result, state, completed, on_progress, on_result);
    }
}

async fn drain_scan_futures<P, R>(
    futures: &mut FuturesUnordered<tokio::task::JoinHandle<ScanResult>>,
    state: &Arc<ScannerState>,
    completed: &mut usize,
    on_progress: &P,
    on_result: &R,
) where
    P: Fn(ScanProgress) + Send + 'static,
    R: Fn(ScanResult) + Send + 'static,
{
    while let Some(result) = futures.next().await {
        if let Ok(scan_result) = result {
            record_scan_result(scan_result, state, completed, on_progress, on_result);
        }
    }
}

pub async fn scan_network(
    state: Arc<ScannerState>,
    cidr: String,
    on_progress: impl Fn(ScanProgress) + Send + 'static,
    on_result: impl Fn(ScanResult) + Send + 'static,
) -> Result<(), String> {
    // Input validation
    validation::validate_cidr(&cidr)?;

    // FIX: Check-and-set atomically to prevent TOCTOU race condition
    // Keep lock held during entire check-and-set operation
    {
        let mut is_scanning = state
            .is_scanning
            .lock()
            .map_err(|e| format!("Failed to acquire scanning lock: {}", e))?;
        if *is_scanning {
            return Err("Scan already in progress".to_string());
        }
        *is_scanning = true;

        // Reset stop signal while we have exclusive access
        let mut stop = state
            .stop_signal
            .lock()
            .map_err(|e| format!("Failed to acquire stop signal lock: {}", e))?;
        *stop = false;
    }

    let _guard = ScanRunningGuard {
        is_scanning: Arc::clone(&state.is_scanning),
    };

    // Clear previous results
    {
        let mut results = state
            .results
            .lock()
            .map_err(|e| format!("Failed to acquire results lock: {}", e))?;
        results.clear();
    }

    let ips = parse_cidr(&cidr)?;
    let total = ips.len();

    // Update total in progress
    {
        let mut progress = state
            .progress
            .lock()
            .map_err(|e| format!("Failed to acquire progress lock: {}", e))?;
        progress.total = total;
        progress.completed = 0;
    }

    let client = Client::new(&Config::default()).map_err(|e| e.to_string())?;
    let client = Arc::new(client);
    let state_arc = Arc::clone(&state);

    // Process IPs concurrently with a limit of 50 concurrent pings
    let mut futures = FuturesUnordered::new();
    let mut completed = 0;

    for ip in ips.iter() {
        // Check stop signal
        {
            let stop = state
                .stop_signal
                .lock()
                .map_err(|e| format!("Failed to acquire stop signal lock: {}", e))?;
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
                if let Ok(mut progress) = state_for_task.progress.lock() {
                    progress.current_ip = Some(ip.to_string());
                }
            }

            let ping_result = ping_host(&client, ip).await;
            let is_alive = ping_result.is_ok();
            let latency_ms = ping_result.ok();

            let mut open_ports = Vec::new();

            let mut services = Vec::new();

            if is_alive {
                // Scan expanded port list for alive hosts. The probes run
                // concurrently — sequentially this cost up to one connect timeout
                // per port (13 seconds a host in the worst case). `join_all`
                // preserves order, so open_ports stays deterministic.
                let common_ports = [
                    22, 23, 80, 443, 445, 3306, 3389, 5432, 6379, 8080, 9100, 515, 631,
                ];
                let probes = common_ports.map(|port| async move { (port, scan_port(ip, port).await) });
                for (port, is_open) in futures::future::join_all(probes).await {
                    if is_open {
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

        // Keep the window full: retire one finished probe rather than the whole batch.
        if futures.len() >= MAX_CONCURRENT_SCANS {
            drain_one_scan_future(
                &mut futures,
                &state_arc,
                &mut completed,
                &on_progress,
                &on_result,
            )
            .await;
        }
    }

    // Always drain any already spawned tasks before returning.
    // This guarantees stop requests do not leave detached scan work running in background.
    if !futures.is_empty() {
        drain_scan_futures(
            &mut futures,
            &state_arc,
            &mut completed,
            &on_progress,
            &on_result,
        )
        .await;
    }

    Ok(())
}

pub fn stop_scan(state: &ScannerState) {
    // Only set the stop signal. The ScanRunningGuard (RAII) is the sole owner
    // of the is_scanning flag and will reset it when the scan task exits.
    // Manually resetting is_scanning here would race with a new scan's guard.
    if let Ok(mut stop) = state.stop_signal.lock() {
        *stop = true;
    }
}

pub fn get_scan_progress(state: &ScannerState) -> ScanProgress {
    state
        .progress
        .lock()
        .map(|p| p.clone())
        .unwrap_or_else(|_| ScanProgress {
            total: 0,
            completed: 0,
            current_ip: None,
        })
}

#[allow(dead_code)]
pub fn get_scan_results(state: &ScannerState) -> Vec<ScanResult> {
    state.results.lock().map(|r| r.clone()).unwrap_or_default()
}

pub fn is_scanning(state: &ScannerState) -> bool {
    state.is_scanning.lock().map(|s| *s).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration as TokioDuration};

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
    fn test_stop_scan_only_sets_signal() {
        let state = ScannerState::new();

        // Set scanning state manually (simulating an active scan)
        *state.is_scanning.lock().unwrap() = true;
        assert!(is_scanning(&state));

        // stop_scan should only set the stop signal, NOT reset is_scanning.
        // The ScanRunningGuard (RAII) is the sole owner of is_scanning.
        stop_scan(&state);
        assert!(
            is_scanning(&state),
            "stop_scan must not reset is_scanning; only the guard should"
        );
        assert!(
            *state.stop_signal.lock().unwrap(),
            "stop signal must be set"
        );
    }

    #[test]
    fn test_scan_running_guard_resets_on_drop() {
        let state = ScannerState::new();
        *state.is_scanning.lock().unwrap() = true;

        {
            let _guard = ScanRunningGuard {
                is_scanning: Arc::clone(&state.is_scanning),
            };
            assert!(is_scanning(&state));
        } // guard drops here

        assert!(
            !is_scanning(&state),
            "Guard drop must reset is_scanning to false"
        );
    }

    #[test]
    fn test_guard_no_race_on_stop_start() {
        // Simulate: scan1 running → stop_scan → scan2 starts → scan1 guard drops
        // scan2's is_scanning must remain true after scan1's guard drops.
        let state = ScannerState::new();

        // Scan 1 starts
        *state.is_scanning.lock().unwrap() = true;
        let guard1 = ScanRunningGuard {
            is_scanning: Arc::clone(&state.is_scanning),
        };

        // User calls stop_scan (only sets signal, does NOT touch is_scanning)
        stop_scan(&state);
        assert!(is_scanning(&state), "stop_scan must not reset is_scanning");

        // Scan 1 detects stop signal and exits — guard1 drops, resetting is_scanning
        drop(guard1);
        assert!(!is_scanning(&state));

        // Scan 2 starts with its own guard
        *state.is_scanning.lock().unwrap() = true;
        let _guard2 = ScanRunningGuard {
            is_scanning: Arc::clone(&state.is_scanning),
        };

        // is_scanning must still be true — no stale guard can interfere
        assert!(
            is_scanning(&state),
            "Scan 2 must remain active after scan 1's guard dropped"
        );
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

    #[test]
    fn test_detect_device_type_rdp_only_is_workstation() {
        assert_eq!(detect_device_type(&[3389]), "workstation");
    }

    #[test]
    fn test_detect_device_type_ssh_only_is_workstation() {
        assert_eq!(detect_device_type(&[22]), "workstation");
    }

    #[test]
    fn test_detect_device_type_many_ports_is_server() {
        // 3+ ports (without matching router pattern) → server
        assert_eq!(detect_device_type(&[22, 80, 3306]), "server");
        // RDP + multiple other ports → server (not workstation)
        assert_eq!(detect_device_type(&[22, 3389, 80]), "server");
    }

    #[test]
    fn test_detect_device_type_smb_is_server() {
        assert_eq!(detect_device_type(&[445]), "server");
    }

    #[test]
    fn test_detect_device_type_router() {
        assert_eq!(detect_device_type(&[80, 443]), "router");
    }

    #[test]
    fn test_detect_device_type_printer() {
        assert_eq!(detect_device_type(&[9100]), "printer");
    }

    #[test]
    fn test_detect_device_type_empty_is_unknown() {
        assert_eq!(detect_device_type(&[]), "unknown");
    }

    fn build_test_scan_result(ip: &str) -> ScanResult {
        ScanResult {
            ip: ip.to_string(),
            is_alive: true,
            latency_ms: Some(1),
            open_ports: vec![22],
            hostname: None,
            mac_address: None,
            device_type: "workstation".to_string(),
            services: vec![ServiceInfo {
                port: 22,
                protocol: "tcp".to_string(),
                service: "SSH".to_string(),
                version: None,
            }],
            vendor: None,
            last_seen: 1,
        }
    }

    #[tokio::test]
    async fn test_drain_scan_futures_drains_pending_tasks() {
        let state = Arc::new(ScannerState::new());
        let mut futures = FuturesUnordered::new();

        futures.push(tokio::spawn(async {
            sleep(TokioDuration::from_millis(15)).await;
            build_test_scan_result("10.0.0.1")
        }));
        futures.push(tokio::spawn(async {
            sleep(TokioDuration::from_millis(5)).await;
            build_test_scan_result("10.0.0.2")
        }));
        futures.push(tokio::spawn(async {
            sleep(TokioDuration::from_millis(1)).await;
            build_test_scan_result("10.0.0.3")
        }));

        let mut completed = 0;
        let on_progress = |_p: ScanProgress| {};
        let on_result = |_r: ScanResult| {};

        drain_scan_futures(
            &mut futures,
            &state,
            &mut completed,
            &on_progress,
            &on_result,
        )
        .await;

        assert!(
            futures.is_empty(),
            "All spawned futures must be drained before returning"
        );
        assert_eq!(
            completed, 3,
            "Completed count must include all pending tasks"
        );

        let stored = state.results.lock().unwrap();
        assert_eq!(
            stored.len(),
            3,
            "All drained task results must be persisted"
        );

        let progress = state.progress.lock().unwrap();
        assert_eq!(progress.completed, 3, "Progress must reflect drained tasks");
    }

    #[tokio::test]
    async fn test_drain_one_scan_future_retires_exactly_one_task() {
        let state = Arc::new(ScannerState::new());
        let mut futures = FuturesUnordered::new();

        for ip in ["10.0.0.1", "10.0.0.2", "10.0.0.3"] {
            futures.push(tokio::spawn(async move {
                sleep(TokioDuration::from_millis(1)).await;
                build_test_scan_result(ip)
            }));
        }

        let mut completed = 0;
        let on_progress = |_p: ScanProgress| {};
        let on_result = |_r: ScanResult| {};

        drain_one_scan_future(
            &mut futures,
            &state,
            &mut completed,
            &on_progress,
            &on_result,
        )
        .await;

        // A sliding window frees a single slot; the rest must stay in flight so the
        // scan is not gated on the slowest host in the batch.
        assert_eq!(completed, 1, "exactly one task should be retired");
        assert_eq!(futures.len(), 2, "remaining tasks must stay pending");
        assert_eq!(state.results.lock().unwrap().len(), 1);
        assert_eq!(state.progress.lock().unwrap().completed, 1);
    }
}

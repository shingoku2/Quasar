//! Local system metrics: the types the UI receives and the sysinfo-based collector.

use serde::{Deserialize, Serialize};
use std::time::Instant;
use sysinfo::{Disks, Networks, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    // Existing metrics
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_usage_percent: f32,
    /// Rates in MB/s, fractional: integer MB/s read as 0 for anything under 1 MB/s, so the
    /// disk and network charts were flat on a typical desktop (FE-017).
    pub disk_read_mb: f64,
    pub disk_write_mb: f64,
    pub network_rx_mb: f64,
    pub network_tx_mb: f64,
    pub timestamp: u64,

    // System info
    pub uptime_seconds: u64,
    pub load_average_1m: f32,
    pub load_average_5m: f32,
    pub load_average_15m: f32,
    pub process_count: usize,
    pub boot_time: u64,

    // CPU details
    pub cpu_count: usize,
    pub cpu_per_core: Vec<f32>,
    pub cpu_frequency_mhz: u64,

    // Disk details (aggregate + per-disk)
    pub disk_total_gb: u64,
    pub disk_used_gb: u64,
    pub disk_free_gb: u64,
    pub disk_usage_percent: f32,
    pub disks: Vec<DiskInfo>,

    // Network details
    pub network_packets_rx: u64,
    pub network_packets_tx: u64,
    pub network_errors_rx: u64,
    pub network_errors_tx: u64,

    // Top processes
    pub top_cpu_processes: Vec<ProcessInfo>,
    pub top_memory_processes: Vec<ProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_mb: u64,
}

/// Per-disk usage for display in the UI (all attached disks).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_gb: u64,
    pub used_gb: u64,
    pub free_gb: u64,
    pub usage_percent: f32,
}

/// Bytes per second to MB/s, keeping the fraction.
pub(super) fn bytes_to_mb(bytes_per_sec: u64) -> f64 {
    bytes_per_sec as f64 / (1024.0 * 1024.0)
}

/// Only cpu and memory are read off each process (see `ProcessInfo`), so the
/// refresh is narrowed to those. The default `ProcessRefreshKind` also resolves
/// per-process tasks and executable paths, which is markedly more expensive and
/// runs on every collection tick.
pub(super) fn process_refresh_kind() -> sysinfo::ProcessRefreshKind {
    sysinfo::ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
}

pub struct MetricsCollector {
    system: System,
    /// Held across collections and refreshed in place; rebuilding these lists
    /// re-enumerates every disk and interface from scratch on each tick.
    disks: Disks,
    networks: Networks,
    last_disk_read: u64,
    last_disk_write: u64,
    last_network_rx: u64,
    last_network_tx: u64,
    pub(super) last_update: Instant,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        let (disk_read, disk_write) = Self::get_disk_io(&disks);
        let (net_rx, net_tx) = Self::get_network_io(&networks);

        Self {
            system,
            disks,
            networks,
            last_disk_read: disk_read,
            last_disk_write: disk_write,
            last_network_rx: net_rx,
            last_network_tx: net_tx,
            last_update: Instant::now(),
        }
    }

    pub fn collect(&mut self) -> SystemMetrics {
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        // Without this, processes() keeps returning the snapshot taken in new(), so
        // process_count and the top-process lists stay frozen at their startup values.
        self.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            process_refresh_kind(),
        );

        self.disks.refresh(true);
        self.networks.refresh(true);
        let disks = &self.disks;
        let networks = &self.networks;

        // Existing metrics
        let cpu_usage = self.system.global_cpu_usage();
        let memory_used = self.system.used_memory();
        let memory_total = self.system.total_memory();
        let memory_usage_percent = if memory_total > 0 {
            (memory_used as f32 / memory_total as f32) * 100.0
        } else {
            0.0
        };

        let (disk_read, disk_write) = Self::get_disk_io(disks);
        let (net_rx, net_tx) = Self::get_network_io(networks);

        let now = Instant::now();
        let elapsed_secs = now.duration_since(self.last_update).as_secs_f64().max(1.0);

        let disk_read_rate =
            ((disk_read.saturating_sub(self.last_disk_read)) as f64 / elapsed_secs) as u64;
        let disk_write_rate =
            ((disk_write.saturating_sub(self.last_disk_write)) as f64 / elapsed_secs) as u64;
        let net_rx_rate =
            ((net_rx.saturating_sub(self.last_network_rx)) as f64 / elapsed_secs) as u64;
        let net_tx_rate =
            ((net_tx.saturating_sub(self.last_network_tx)) as f64 / elapsed_secs) as u64;

        self.last_disk_read = disk_read;
        self.last_disk_write = disk_write;
        self.last_network_rx = net_rx;
        self.last_network_tx = net_tx;
        self.last_update = now;

        // New metrics - System info
        let load_avg = System::load_average();
        let uptime = System::uptime();
        let boot_time = System::boot_time();
        let process_count = self.system.processes().len();

        // New metrics - CPU details
        let cpu_count = self.system.cpus().len();
        let cpu_per_core: Vec<f32> = self
            .system
            .cpus()
            .iter()
            .map(|cpu| cpu.cpu_usage())
            .collect();
        let cpu_frequency_mhz = self
            .system
            .cpus()
            .first()
            .map(|cpu| cpu.frequency())
            .unwrap_or(0);

        // New metrics - Disk details (aggregate + per-disk list)
        let (disk_total_gb, disk_used_gb, disk_free_gb, disk_usage_percent) =
            Self::calculate_disk_space(disks);
        let disks_list = Self::get_disk_list(disks);

        // New metrics - Network details
        let (network_packets_rx, network_packets_tx, network_errors_rx, network_errors_tx) =
            Self::get_network_packets(networks);

        // New metrics - Top processes
        let top_cpu_processes = self.get_top_processes_by_cpu(5);
        let top_memory_processes = self.get_top_processes_by_memory(5);

        SystemMetrics {
            // Existing metrics
            cpu_usage_percent: cpu_usage,
            memory_used_mb: memory_used / 1024 / 1024,
            memory_total_mb: memory_total / 1024 / 1024,
            memory_usage_percent,
            disk_read_mb: bytes_to_mb(disk_read_rate),
            disk_write_mb: bytes_to_mb(disk_write_rate),
            network_rx_mb: bytes_to_mb(net_rx_rate),
            network_tx_mb: bytes_to_mb(net_tx_rate),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),

            // System info
            uptime_seconds: uptime,
            load_average_1m: load_avg.one as f32,
            load_average_5m: load_avg.five as f32,
            load_average_15m: load_avg.fifteen as f32,
            process_count,
            boot_time,

            // CPU details
            cpu_count,
            cpu_per_core,
            cpu_frequency_mhz,

            // Disk details
            disk_total_gb,
            disk_used_gb,
            disk_free_gb,
            disk_usage_percent,
            disks: disks_list,

            // Network details
            network_packets_rx,
            network_packets_tx,
            network_errors_rx,
            network_errors_tx,

            // Top processes
            top_cpu_processes,
            top_memory_processes,
        }
    }

    fn get_disk_io(disks: &Disks) -> (u64, u64) {
        let mut read = 0u64;
        let mut write = 0u64;
        for disk in disks.list() {
            read += disk.usage().read_bytes;
            write += disk.usage().written_bytes;
        }
        (read, write)
    }

    fn get_network_io(networks: &Networks) -> (u64, u64) {
        let mut rx = 0u64;
        let mut tx = 0u64;
        for data in networks.values() {
            rx += data.total_received();
            tx += data.total_transmitted();
        }
        (rx, tx)
    }

    fn get_network_packets(networks: &Networks) -> (u64, u64, u64, u64) {
        let mut packets_rx = 0u64;
        let mut packets_tx = 0u64;
        let mut errors_rx = 0u64;
        let mut errors_tx = 0u64;
        for data in networks.values() {
            packets_rx += data.total_packets_received();
            packets_tx += data.total_packets_transmitted();
            errors_rx += data.total_errors_on_received();
            errors_tx += data.total_errors_on_transmitted();
        }
        (packets_rx, packets_tx, errors_rx, errors_tx)
    }

    fn calculate_disk_space(disks: &Disks) -> (u64, u64, u64, f32) {
        let mut total = 0u64;
        let mut available = 0u64;
        for disk in disks.list() {
            total = total.saturating_add(disk.total_space());
            available = available.saturating_add(disk.available_space());
        }
        let used = total.saturating_sub(available);
        let usage_percent = if total > 0 {
            (used as f64 / total as f64 * 100.0) as f32
        } else {
            0.0
        };
        // Convert bytes to GB
        let total_gb = total / 1024 / 1024 / 1024;
        let used_gb = used / 1024 / 1024 / 1024;
        let free_gb = available / 1024 / 1024 / 1024;
        (total_gb, used_gb, free_gb, usage_percent)
    }

    fn get_disk_list(disks: &Disks) -> Vec<DiskInfo> {
        disks
            .list()
            .iter()
            .map(|disk| {
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total.saturating_sub(available);
                let usage_percent = if total > 0 {
                    (used as f64 / total as f64 * 100.0) as f32
                } else {
                    0.0
                };
                let total_gb = total / 1024 / 1024 / 1024;
                let used_gb = used / 1024 / 1024 / 1024;
                let free_gb = available / 1024 / 1024 / 1024;
                let name = disk.name().to_string_lossy().to_string();
                let mount_point = disk.mount_point().to_string_lossy().to_string();
                DiskInfo {
                    name,
                    mount_point,
                    total_gb,
                    used_gb,
                    free_gb,
                    usage_percent,
                }
            })
            .collect()
    }

    fn get_top_processes_by_cpu(&self, limit: usize) -> Vec<ProcessInfo> {
        let mut processes: Vec<_> = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                cpu_usage: process.cpu_usage(),
                memory_mb: process.memory() / 1024 / 1024,
            })
            .collect();

        processes.sort_by(|a, b| {
            b.cpu_usage
                .partial_cmp(&a.cpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        processes.truncate(limit);
        processes
    }

    fn get_top_processes_by_memory(&self, limit: usize) -> Vec<ProcessInfo> {
        let mut processes: Vec<_> = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                cpu_usage: process.cpu_usage(),
                memory_mb: process.memory() / 1024 / 1024,
            })
            .collect();

        processes.sort_by_key(|p| std::cmp::Reverse(p.memory_mb));
        processes.truncate(limit);
        processes
    }
}

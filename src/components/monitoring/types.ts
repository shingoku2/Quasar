export interface ProcessInfo {
  pid: number;
  name: string;
  cpu_usage: number;
  memory_mb: number;
}

export interface SystemMetrics {
  // Existing metrics
  cpu_usage_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  memory_usage_percent: number;
  disk_read_mb: number;
  disk_write_mb: number;
  network_rx_mb: number;
  network_tx_mb: number;
  timestamp: number;
  
  // System info
  uptime_seconds: number;
  load_average_1m: number;
  load_average_5m: number;
  load_average_15m: number;
  process_count: number;
  boot_time: number;
  
  // CPU details
  cpu_count: number;
  cpu_per_core: number[];
  cpu_frequency_mhz: number;
  
  // Disk details
  disk_total_gb: number;
  disk_used_gb: number;
  disk_free_gb: number;
  disk_usage_percent: number;
  
  // Network details
  network_packets_rx: number;
  network_packets_tx: number;
  network_errors_rx: number;
  network_errors_tx: number;
  
  // Top processes
  top_cpu_processes: ProcessInfo[];
  top_memory_processes: ProcessInfo[];
}

export interface HealthMetrics {
  cpu_percent?: number | null;
  memory_used_mb?: number | null;
  memory_total_mb?: number | null;
  disk_used_gb?: number | null;
  disk_total_gb?: number | null;
  uptime_seconds?: number | null;
  load_average?: number[] | null;
}

export interface RemoteHostMetric {
  id: string;
  name: string;
  address: string;
  port: number;
  reachable: boolean;
  latency_ms: number | null;
  error: string | null;
  metrics?: HealthMetrics | null;
}

export interface MonitoredHost {
  id: string;
  name: string;
  address: string;
  port: number;
  username?: string | null;
  protocol: string;
  credential_id?: string | null;
}

export interface CredentialSummary {
  id: string;
  name: string;
  username: string;
  credential_type: string;
}

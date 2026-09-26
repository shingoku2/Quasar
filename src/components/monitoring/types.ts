export type { ProcessInfo, SystemMetrics } from '../../hooks/useSystemMetrics';

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

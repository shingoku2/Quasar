import React, { useState, useEffect } from 'react';
import { Server, AlertTriangle, Cpu, HardDrive, Shield } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useVisiblePolling } from '../../hooks/useViewVisibility';
import { cn } from '../../lib/utils';

interface DiskInfo {
  name: string;
  mount_point: string;
  total_gb: number;
  used_gb: number;
  free_gb: number;
  usage_percent: number;
}

interface SystemMetrics {
  cpu_usage_percent: number;
  memory_usage_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  disk_usage_percent: number;
  disk_used_gb: number;
  disk_total_gb: number;
  uptime_seconds: number;
  disks?: DiskInfo[];
}

interface Alert {
  id: string;
  rule_id: string;
  severity: string;
  acknowledged: boolean;
}

/** Alert payload as emitted by the Rust backend (monitoring.rs `Alert`). */
interface BackendAlert {
  id: string;
  rule_id: string;
  message: string;
  severity: string;
  timestamp: number;
  acknowledged: boolean;
}

interface RemoteHostMetric {
  id: string;
  name: string;
  address: string;
  port: number;
  reachable: boolean;
  latency_ms: number | null;
  error: string | null;
}

interface VaultSettings {
  auto_lock_timeout_minutes: number;
  require_password_on_credential_use: boolean;
  vault_initialized: boolean;
}

interface SystemHealthWidgetProps {
  variant?: 'metrics' | 'summary';
}

const SystemHealthWidget: React.FC<SystemHealthWidgetProps> = ({ variant = 'metrics' }) => {
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
  const [alerts, setAlerts] = useState<Alert[]>([]);
  const [onlineHostsCount, setOnlineHostsCount] = useState<number | null>(null);
  const [vaultTimeout, setVaultTimeout] = useState<number | null>(null);

  useEffect(() => {
    const unlistenMetrics = listen<SystemMetrics>('system-metrics', (event) => {
      setMetrics(event.payload);
    });

    const unlistenAlerts = listen<BackendAlert[]>('alerts-triggered', (event) => {
      const newAlerts: Alert[] = event.payload.map(a => ({
        id: a.id,
        rule_id: a.rule_id,
        severity: a.severity,
        acknowledged: false,
      }));
      setAlerts(prev => [...newAlerts, ...prev].slice(0, 50));
    });

    // A recovered rule clears the "active" status of any alert(s) still counted
    // against it, so the summary count actually goes back down.
    const unlistenRecovered = listen<Array<{ rule_id: string }>>('alerts-recovered', (event) => {
      const recoveredRuleIds = new Set(event.payload.map(r => r.rule_id));
      setAlerts(prev => prev.map(a =>
        recoveredRuleIds.has(a.rule_id) ? { ...a, acknowledged: true } : a
      ));
    });

    invoke<SystemMetrics>('get_system_metrics').then(setMetrics).catch(console.error);

    return () => {
      unlistenMetrics.then(fn => fn());
      unlistenAlerts.then(fn => fn());
      unlistenRecovered.then(fn => fn());
    };
  }, [variant]);

  // Online hosts count (summary variant only). get_remote_hosts_health pings and
  // SSHes into every saved host, so it is paused while the Dashboard is hidden.
  const fetchHostsHealth = async () => {
    if (variant !== 'summary') return;
    try {
      const hosts = await invoke<RemoteHostMetric[]>('get_remote_hosts_health');
      const onlineCount = Array.isArray(hosts) ? hosts.filter(h => h.reachable).length : 0;
      setOnlineHostsCount(onlineCount);
    } catch {
      setOnlineHostsCount(0);
    }
  };
  useVisiblePolling(fetchHostsHealth, 30000);

  // Fetch vault settings for auto-lock timeout (only for summary variant)
  useEffect(() => {
    if (variant === 'summary') {
      invoke<VaultSettings>('get_vault_settings')
        .then(settings => setVaultTimeout(settings.auto_lock_timeout_minutes))
        .catch(() => setVaultTimeout(null));
    }
  }, [variant]);

  const activeAlertCount = alerts.filter(a => !a.acknowledged).length;

  if (variant === 'summary') {
    return (
      <div className="bg-bg-card border border-border rounded-xl p-4 shadow-sm">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest">System Health</h3>
        </div>
        <div className="space-y-2">
          <div className="flex items-center space-x-3 bg-bg-root rounded-lg px-3 py-2">
            <Server className="h-4 w-4 text-accent" />
            <span className="text-xs text-gray-300 flex-1">Hosts Online</span>
            <span className="text-xs font-bold text-white">
              {onlineHostsCount !== null ? onlineHostsCount : '--'}
            </span>
          </div>
          <div className="flex items-center space-x-3 bg-bg-root rounded-lg px-3 py-2">
            <AlertTriangle className={cn("h-4 w-4", activeAlertCount > 0 ? "text-warning" : "text-gray-500")} />
            <span className="text-xs text-gray-300 flex-1">Alerts</span>
            <span className={cn("text-xs font-bold", activeAlertCount > 0 ? "text-warning" : "text-white")}>{activeAlertCount}</span>
          </div>
          <div className="flex items-center space-x-3 bg-bg-root rounded-lg px-3 py-2">
            <Shield className="h-4 w-4 text-success" />
            <span className="text-xs text-gray-300 flex-1">Vault Auto-lock</span>
            <span className="text-xs font-bold text-white">
              {vaultTimeout !== null ? `${vaultTimeout} min` : '--'}
            </span>
          </div>
        </div>
      </div>
    );
  }

  const cpuPercent = metrics?.cpu_usage_percent ?? 0;
  const memUsed = metrics?.memory_used_mb ? (metrics.memory_used_mb / 1024).toFixed(1) : '0';
  const memTotal = metrics?.memory_total_mb ? (metrics.memory_total_mb / 1024).toFixed(0) : '0';
  const diskList = (metrics?.disks && metrics.disks.length > 0)
    ? metrics.disks
    : (metrics?.disk_used_gb != null && metrics?.disk_total_gb != null
      ? [{ name: 'Disk', mount_point: '', total_gb: metrics.disk_total_gb, used_gb: metrics.disk_used_gb, free_gb: 0, usage_percent: metrics?.disk_usage_percent ?? 0 }]
      : []);

  return (
    <div className="bg-bg-card border border-border rounded-xl p-4 shadow-sm">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest">Real-time Metrics</h3>
      </div>
      <div className="space-y-3">
        {/* CPU */}
        <div>
          <div className="flex items-center justify-between mb-1">
            <div className="flex items-center space-x-2">
              <Cpu className="h-3.5 w-3.5 text-accent" />
              <span className="text-xs text-gray-300">CPU</span>
            </div>
            <span className={cn(
              "text-xs font-bold",
              cpuPercent > 80 ? "text-alert" : cpuPercent > 60 ? "text-warning" : "text-white"
            )}>{cpuPercent.toFixed(0)}%</span>
          </div>
          <div className="h-1.5 bg-bg-root rounded-full overflow-hidden">
            <div 
              className={cn(
                "h-full rounded-full transition-all duration-500",
                cpuPercent > 80 ? "bg-alert" : cpuPercent > 60 ? "bg-warning" : "bg-accent"
              )}
              style={{ width: `${Math.min(cpuPercent, 100)}%` }}
            />
          </div>
        </div>
        {/* Memory */}
        <div>
          <div className="flex items-center justify-between mb-1">
            <div className="flex items-center space-x-2">
              <Server className="h-3.5 w-3.5 text-accent" />
              <span className="text-xs text-gray-300">Memory</span>
            </div>
            <span className="text-xs font-bold text-white">{memUsed}GB/{memTotal}GB</span>
          </div>
          <div className="h-1.5 bg-bg-root rounded-full overflow-hidden">
            <div 
              className="h-full rounded-full bg-accent transition-all duration-500"
              style={{ width: `${Math.min(metrics?.memory_usage_percent ?? 0, 100)}%` }}
            />
          </div>
        </div>
        {/* Disks: all attached disks */}
        {diskList.length > 0 && diskList.map((disk, idx) => (
          <div key={disk.mount_point || disk.name || idx}>
            <div className="flex items-center justify-between mb-1">
              <div className="flex items-center space-x-2 min-w-0">
                <HardDrive className="h-3.5 w-3.5 text-accent shrink-0" />
                <span className="text-xs text-gray-300 truncate" title={disk.mount_point || disk.name}>
                  {disk.mount_point || disk.name || `Disk ${idx + 1}`}
                </span>
              </div>
              <span className="text-xs font-bold text-white shrink-0 ml-1">
                {disk.used_gb}GB/{disk.total_gb}GB
              </span>
            </div>
            <div className="h-1.5 bg-bg-root rounded-full overflow-hidden">
              <div 
                className={cn(
                  "h-full rounded-full transition-all duration-500",
                  disk.usage_percent > 80 ? "bg-alert" : disk.usage_percent > 60 ? "bg-warning" : "bg-accent"
                )}
                style={{ width: `${Math.min(disk.usage_percent, 100)}%` }}
              />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default SystemHealthWidget;

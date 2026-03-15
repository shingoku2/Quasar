import React, { useEffect, useState } from 'react';
import { AlertCircle, Clock, Check, X, Activity, Cpu, HardDrive } from 'lucide-react';
import { cn } from '../../lib/utils';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

export interface Alert {
  id: string;
  source: string;
  message: string;
  severity: 'critical' | 'warning' | 'info';
  timestamp: string;
  acknowledged?: boolean;
}

interface ProcessInfo {
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

interface AlertFeedProps {
  alerts?: Alert[];
}

const AlertFeed: React.FC<AlertFeedProps> = ({ alerts: initialAlerts = [] }) => {
  const [alerts, setAlerts] = useState<Alert[]>(initialAlerts);
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);

  const mapSeverity = (severity: unknown): 'critical' | 'warning' | 'info' => {
    const severityStr = String(severity).toLowerCase();
    if (severityStr.includes('critical')) return 'critical';
    if (severityStr.includes('warning')) return 'warning';
    return 'info';
  };

  useEffect(() => {
    // Listen for real-time system metrics
    const unlistenMetrics = listen<SystemMetrics>('system-metrics', (event) => {
      setMetrics(event.payload);
    });

    // Listen for triggered alerts
    const unlistenAlerts = listen<Alert[]>('alerts-triggered', (event) => {
      const newAlerts: Alert[] = event.payload.map((a: Alert) => ({
        id: a.id,
        source: a.rule_id || 'System',
        message: a.message,
        severity: mapSeverity(a.severity),
        timestamp: new Date(a.timestamp * 1000).toLocaleTimeString(),
        acknowledged: false
      }));
      setAlerts(prev => [...newAlerts, ...prev].slice(0, 50));
    });

    // Listen for recovered alerts (metric returned below threshold)
    const unlistenRecovered = listen<Array<{rule_id: string; message: string; recovered_at: number}>>('alerts-recovered', (event) => {
      const recoveryItems: Alert[] = event.payload.map((r) => ({
        id: `recovery-${r.rule_id}-${r.recovered_at}`,
        source: r.rule_id,
        message: r.message,
        severity: 'info' as const,
        timestamp: new Date(r.recovered_at * 1000).toLocaleTimeString(),
        acknowledged: true,
      }));
      setAlerts(prev => [...recoveryItems, ...prev].slice(0, 50));
    });

    // Initial metrics fetch
    invoke<SystemMetrics>('get_system_metrics').then(setMetrics).catch(console.error);

    return () => {
      unlistenMetrics.then(fn => fn());
      unlistenAlerts.then(fn => fn());
      unlistenRecovered.then(fn => fn());
    };
  }, []);

  const dismissAlert = (id: string) => {
    setAlerts(prev => prev.filter(a => a.id !== id));
  };

  const acknowledgeAlert = (id: string) => {
    setAlerts(prev => prev.map(a => 
      a.id === id ? { ...a, acknowledged: true } : a
    ));
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B/s';
    const k = 1024;
    const sizes = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  };

  return (
    <div className="bg-bg-card border border-border rounded-xl flex flex-col overflow-hidden shadow-sm max-h-56">
      <div className="p-4 border-b border-border flex justify-between items-center">
        <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest">Recent Activity</h3>
        <span className="text-[10px] bg-bg-root text-accent px-2 py-0.5 rounded-full font-mono animate-pulse">LIVE</span>
      </div>

      {/* Metrics Summary */}
      {metrics && typeof metrics.cpu_usage_percent === 'number' && (
        <div className="grid grid-cols-3 gap-2 p-3 border-b border-border/50 bg-black/20">
          <div className="flex items-center space-x-2">
            <Cpu className="h-3 w-3 text-accent" />
            <div>
              <p className="text-[10px] text-gray-500 uppercase">CPU</p>
              <p className={cn(
                "text-xs font-mono font-bold",
                metrics.cpu_usage_percent > 80 ? "text-alert" : metrics.cpu_usage_percent > 60 ? "text-warning" : "text-gray-300"
              )}>
                {metrics.cpu_usage_percent.toFixed(1)}%
              </p>
            </div>
          </div>
          <div className="flex items-center space-x-2">
            <Activity className="h-3 w-3 text-accent" />
            <div>
              <p className="text-[10px] text-gray-500 uppercase">RAM</p>
              <p className={cn(
                "text-xs font-mono font-bold",
                metrics.memory_usage_percent > 80 ? "text-alert" : metrics.memory_usage_percent > 60 ? "text-warning" : "text-gray-300"
              )}>
                {metrics.memory_usage_percent.toFixed(0)}%
              </p>
            </div>
          </div>
          <div className="flex items-center space-x-2">
            <HardDrive className="h-3 w-3 text-accent" />
            <div>
              <p className="text-[10px] text-gray-500 uppercase">Disk I/O</p>
              <p className="text-xs font-mono font-bold text-gray-300">
                {formatBytes((metrics.disk_read_mb + metrics.disk_write_mb) * 1024 * 1024)}
              </p>
            </div>
          </div>
        </div>
      )}
      
      <div className="flex-1 overflow-y-auto no-scrollbar">
        {alerts.length === 0 ? (
          <div className="p-8 text-center">
            <div className="w-12 h-12 rounded-full bg-green-500/10 flex items-center justify-center mx-auto mb-3">
              <Activity className="h-5 w-5 text-green-500" />
            </div>
            <p className="text-gray-500 text-sm">All systems nominal</p>
            <p className="text-[10px] text-gray-600 mt-1">No active alerts</p>
          </div>
        ) : (
          <div className="divide-y divide-border/50">
            {alerts.map((alert) => (
              <div 
                key={alert.id} 
                className={cn(
                  "p-3 flex items-start space-x-3 transition-all border-l-4 group",
                  alert.acknowledged ? "opacity-50 bg-white/[0.02]" : "hover:bg-white/5",
                  alert.severity === 'critical' ? "border-alert bg-alert/5" : 
                  alert.severity === 'warning' ? "border-warning bg-warning/5" : "border-accent"
                )}
              >
                <AlertCircle className={cn(
                  "h-4 w-4 mt-0.5 shrink-0",
                  alert.severity === 'critical' ? "text-alert" : 
                  alert.severity === 'warning' ? "text-warning" : "text-accent"
                )} />
                <div className="flex-1 min-w-0">
                  <div className="flex justify-between items-start">
                    <p className="text-xs font-bold text-gray-300 truncate">{alert.source}</p>
                    <div className="flex items-center space-x-1">
                      <div className="flex items-center text-[10px] text-gray-500 font-mono shrink-0">
                        <Clock className="h-3 w-3 mr-1" />
                        {alert.timestamp}
                      </div>
                      {!alert.acknowledged && (
                        <button
                          onClick={() => acknowledgeAlert(alert.id)}
                          className="p-1 hover:bg-green-500/10 rounded text-green-500 opacity-0 group-hover:opacity-100 transition-opacity"
                          title="Acknowledge"
                        >
                          <Check className="h-3 w-3" />
                        </button>
                      )}
                      <button
                        onClick={() => dismissAlert(alert.id)}
                        className="p-1 hover:bg-red-500/10 rounded text-gray-500 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                        title="Dismiss"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </div>
                  </div>
                  <p className={cn(
                    "text-xs mt-1 line-clamp-2 leading-relaxed",
                    alert.acknowledged ? "text-gray-600" : "text-gray-400"
                  )}>{alert.message}</p>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
      
      <div className="p-2 border-t border-border bg-bg-card/30">
        <div className="flex justify-between items-center">
          <span className="text-[10px] text-gray-600">
            {alerts.filter(a => !a.acknowledged).length} unacknowledged
          </span>
          <button 
            onClick={() => setAlerts([])}
            className="text-[10px] font-bold text-gray-500 hover:text-gray-300 uppercase tracking-widest transition-colors"
          >
            Clear All
          </button>
        </div>
      </div>
    </div>
  );
};

export default AlertFeed;

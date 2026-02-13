import React, { useState, useEffect } from 'react';
import { Server, AlertTriangle, Cpu, HardDrive, Shield, MoreHorizontal } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { cn } from '../../lib/utils';

interface SystemMetrics {
  cpu_usage_percent: number;
  memory_usage_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  disk_usage_percent: number;
  disk_used_gb: number;
  disk_total_gb: number;
  uptime_seconds: number;
}

interface Alert {
  id: string;
  severity: string;
}

interface SystemHealthWidgetProps {
  variant?: 'metrics' | 'summary';
}

const SystemHealthWidget: React.FC<SystemHealthWidgetProps> = ({ variant = 'metrics' }) => {
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
  const [alerts, setAlerts] = useState<Alert[]>([]);

  useEffect(() => {
    const unlistenMetrics = listen<SystemMetrics>('system-metrics', (event) => {
      setMetrics(event.payload);
    });

    const unlistenAlerts = listen<Alert[]>('alerts-triggered', (event) => {
      setAlerts(prev => [...event.payload, ...prev].slice(0, 50));
    });

    invoke<SystemMetrics>('get_system_metrics').then(setMetrics).catch(console.error);

    return () => {
      unlistenMetrics.then(fn => fn());
      unlistenAlerts.then(fn => fn());
    };
  }, []);

  const activeAlertCount = alerts.filter(a => !a.id.includes('acknowledged')).length;

  if (variant === 'summary') {
    return (
      <div className="bg-bg-card border border-border rounded-xl p-4 shadow-sm">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest">System Health</h3>
          <button className="p-1 text-gray-500 hover:text-gray-300 transition-colors">
            <MoreHorizontal className="h-4 w-4" />
          </button>
        </div>
        <div className="space-y-2">
          <div className="flex items-center space-x-3 bg-bg-root rounded-lg px-3 py-2">
            <Server className="h-4 w-4 text-accent" />
            <span className="text-xs text-gray-300 flex-1">Hosts Online</span>
            <span className="text-xs font-bold text-white">--</span>
          </div>
          <div className="flex items-center space-x-3 bg-bg-root rounded-lg px-3 py-2">
            <AlertTriangle className={cn("h-4 w-4", activeAlertCount > 0 ? "text-warning" : "text-gray-500")} />
            <span className="text-xs text-gray-300 flex-1">Alerts</span>
            <span className={cn("text-xs font-bold", activeAlertCount > 0 ? "text-warning" : "text-white")}>{activeAlertCount}</span>
          </div>
          <div className="flex items-center space-x-3 bg-bg-root rounded-lg px-3 py-2">
            <Shield className="h-4 w-4 text-success" />
            <span className="text-xs text-gray-300 flex-1">Vault Auto-lock</span>
            <span className="text-xs font-bold text-white">15:00</span>
          </div>
        </div>
      </div>
    );
  }

  const cpuPercent = metrics?.cpu_usage_percent ?? 0;
  const memUsed = metrics?.memory_used_mb ? (metrics.memory_used_mb / 1024).toFixed(1) : '0';
  const memTotal = metrics?.memory_total_mb ? (metrics.memory_total_mb / 1024).toFixed(0) : '0';
  const diskUsed = metrics?.disk_used_gb?.toFixed(0) ?? '0';
  const diskTotal = metrics?.disk_total_gb?.toFixed(0) ?? '0';

  return (
    <div className="bg-bg-card border border-border rounded-xl p-4 shadow-sm">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest">Real-time Metrics</h3>
        <button className="p-1 text-gray-500 hover:text-gray-300 transition-colors">
          <MoreHorizontal className="h-4 w-4" />
        </button>
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
        {/* Disk */}
        <div>
          <div className="flex items-center justify-between mb-1">
            <div className="flex items-center space-x-2">
              <HardDrive className="h-3.5 w-3.5 text-accent" />
              <span className="text-xs text-gray-300">Disk</span>
            </div>
            <span className="text-xs font-bold text-white">{diskUsed}GB/{diskTotal}GB</span>
          </div>
          <div className="h-1.5 bg-bg-root rounded-full overflow-hidden">
            <div 
              className="h-full rounded-full bg-accent transition-all duration-500"
              style={{ width: `${Math.min(metrics?.disk_usage_percent ?? 0, 100)}%` }}
            />
          </div>
        </div>
      </div>
    </div>
  );
};

export default SystemHealthWidget;

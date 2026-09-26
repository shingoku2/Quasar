import React, { useState, useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import MetricChartCard from './dashboard/MetricChartCard';
import AlertRules from './dashboard/AlertRules';
import DiskSpaceCard from './monitoring/DiskSpaceCard';
import RemoteHostsList from './monitoring/RemoteHostsList';
import SystemInfoBar from './monitoring/SystemInfoBar';
import TopProcessesCard from './monitoring/TopProcessesCard';
import { formatMetricTime, formatRateMb } from './monitoring/format';
import type { SystemMetrics } from './monitoring/types';
import { useRemoteHostsHealth } from './monitoring/useRemoteHostsHealth';

const MonitoringView: React.FC = () => {
  const [cpuData, setCpuData] = useState<{ time: string; value: number }[]>([]);
  const [memData, setMemData] = useState<{ time: string; value: number }[]>([]);
  const [diskData, setDiskData] = useState<{ time: string; value: number }[]>([]);
  const [netData, setNetData] = useState<{ time: string; value: number }[]>([]);
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
  const { remoteHosts, savedHosts, credentials, setHostCredential } = useRemoteHostsHealth();
  const unlistenRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    let cancelled = false;

    const setupListener = async () => {
      try {
        const unlisten = await listen('system-metrics', (event) => {
          const data = event.payload as SystemMetrics;
          const timeLabel = formatMetricTime(new Date());
          setMetrics(data);
          setCpuData(prev => [...prev.slice(-19), { time: timeLabel, value: data.cpu_usage_percent }]);
          setMemData(prev => [...prev.slice(-19), { time: timeLabel, value: data.memory_usage_percent }]);
          setDiskData(prev => [...prev.slice(-19), { time: timeLabel, value: data.disk_read_mb + data.disk_write_mb }]);
          setNetData(prev => [...prev.slice(-19), { time: timeLabel, value: data.network_rx_mb + data.network_tx_mb }]);
        });
        if (cancelled) {
          unlisten();
        } else {
          unlistenRef.current = unlisten;
        }
      } catch (error) {
        console.error('Error setting up listener:', error);
      }
    };

    setupListener();

    invoke<SystemMetrics>('get_system_metrics').then((data) => {
      const initialTimeLabel = formatMetricTime(new Date());
      setMetrics(data);
      setCpuData([{ time: initialTimeLabel, value: data.cpu_usage_percent }]);
      setMemData([{ time: initialTimeLabel, value: data.memory_usage_percent }]);
      setDiskData([{ time: initialTimeLabel, value: data.disk_read_mb + data.disk_write_mb }]);
      setNetData([{ time: initialTimeLabel, value: data.network_rx_mb + data.network_tx_mb }]);
    }).catch((err) => {
      console.warn('Failed to get system metrics:', err);
    });

    return () => {
      cancelled = true;
      unlistenRef.current?.();
      unlistenRef.current = null;
    };
  }, []);

  return (
    <div className="p-6 space-y-6 h-full overflow-y-auto no-scrollbar bg-bg-root animate-in fade-in duration-500">
      <div className="mb-6">
        <h2 className="text-2xl font-bold uppercase tracking-widest text-gray-100">System Monitoring</h2>
        <p className="text-sm text-gray-400 mt-1">Real-time system metrics and performance monitoring</p>
      </div>

      {/* System Info Bar */}
      {metrics && <SystemInfoBar metrics={metrics} />}

      {/* Main Metrics Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <MetricChartCard 
          title="CPU Usage" 
          value={metrics?.cpu_usage_percent !== undefined ? `${metrics.cpu_usage_percent.toFixed(1)}%` : '--%'} 
          data={cpuData} 
          color="#3b82f6" 
        />
        <MetricChartCard 
          title="Memory Utilization" 
          value={metrics?.memory_used_mb !== undefined && metrics?.memory_total_mb !== undefined ? `${(metrics.memory_used_mb / 1024).toFixed(1)} GB / ${(metrics.memory_total_mb / 1024).toFixed(1)} GB` : '-- GB'} 
          data={memData} 
          color="#10b981" 
        />
        <MetricChartCard 
          title="Disk I/O Traffic" 
          value={metrics?.disk_read_mb !== undefined && metrics?.disk_write_mb !== undefined ? `${formatRateMb(metrics.disk_read_mb + metrics.disk_write_mb)} MB/s` : '-- MB/s'} 
          data={diskData} 
          color="#f59e0b" 
        />
        <MetricChartCard 
          title="Network Traffic" 
          value={metrics?.network_rx_mb !== undefined && metrics?.network_tx_mb !== undefined ? `${formatRateMb(metrics.network_rx_mb + metrics.network_tx_mb)} MB/s` : '-- MB/s'} 
          data={netData} 
          color="#8b5cf6" 
        />
      </div>

      {/* Remote saved hosts */}
      <RemoteHostsList
        remoteHosts={remoteHosts}
        savedHosts={savedHosts}
        credentials={credentials}
        setHostCredential={setHostCredential}
      />

      {/* Disk Space Card */}
      {metrics && metrics.disk_total_gb > 0 && <DiskSpaceCard metrics={metrics} />}

      {/* Alert rules (FE-003: the editor existed but was never mounted, so no rule could
          be created). Rules evaluate this machine's CPU/memory/disk metrics. */}
      <AlertRules />

      {/* Top Processes */}
      {metrics && (metrics.top_cpu_processes.length > 0 || metrics.top_memory_processes.length > 0) && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {metrics.top_cpu_processes.length > 0 && (
            <TopProcessesCard kind="cpu" processes={metrics.top_cpu_processes} />
          )}
          {metrics.top_memory_processes.length > 0 && (
            <TopProcessesCard kind="memory" processes={metrics.top_memory_processes} />
          )}
        </div>
      )}
    </div>
  );
};

export default MonitoringView;

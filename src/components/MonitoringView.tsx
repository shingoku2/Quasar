import React, { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import MetricChartCard from './dashboard/MetricChartCard';

interface ProcessInfo {
  pid: number;
  name: string;
  cpu_usage: number;
  memory_mb: number;
}

interface SystemMetrics {
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

const MonitoringView: React.FC = () => {
  const [cpuData, setCpuData] = useState<{ time: string; value: number }[]>([]);
  const [memData, setMemData] = useState<{ time: string; value: number }[]>([]);
  const [diskData, setDiskData] = useState<{ time: string; value: number }[]>([]);
  const [netData, setNetData] = useState<{ time: string; value: number }[]>([]);
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);

  useEffect(() => {
    let unlisten: Promise<() => void> | null = null;
    
    const setupListener = async () => {
      try {
        unlisten = listen('system-metrics', (event) => {
          const data = event.payload as SystemMetrics;
          const timeLabel = new Date().toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
          
          setMetrics(data);
          
          setCpuData(prev => [...prev.slice(-19), { time: timeLabel, value: data.cpu_usage_percent }]);
          setMemData(prev => [...prev.slice(-19), { time: timeLabel, value: data.memory_usage_percent }]);
          setDiskData(prev => [...prev.slice(-19), { time: timeLabel, value: data.disk_read_mb + data.disk_write_mb }]);
          setNetData(prev => [...prev.slice(-19), { time: timeLabel, value: data.network_rx_mb + data.network_tx_mb }]);
        });
        
        await unlisten;
      } catch (error) {
        console.error('Error setting up listener:', error);
      }
    };

    setupListener();

    invoke<SystemMetrics>('get_system_metrics').then((data) => {
      const initialTimeLabel = new Date().toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
      setMetrics(data);
      setCpuData([{ time: initialTimeLabel, value: data.cpu_usage_percent }]);
      setMemData([{ time: initialTimeLabel, value: data.memory_usage_percent }]);
      setDiskData([{ time: initialTimeLabel, value: data.disk_read_mb + data.disk_write_mb }]);
      setNetData([{ time: initialTimeLabel, value: data.network_rx_mb + data.network_tx_mb }]);
    }).catch((err) => {
      console.warn('Failed to get system metrics:', err);
    });

    return () => {
      if (unlisten) {
        unlisten.then(fn => fn()).catch(console.error);
      }
    };
  }, []);

  const formatUptime = (seconds: number): string => {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${mins}m`;
    return `${mins}m`;
  };

  return (
    <div className="p-6 space-y-6 h-full overflow-y-auto no-scrollbar bg-bg-root animate-in fade-in duration-500">
      <div className="mb-6">
        <h2 className="text-2xl font-bold uppercase tracking-widest text-gray-100">System Monitoring</h2>
        <p className="text-sm text-gray-400 mt-1">Real-time system metrics and performance monitoring</p>
      </div>

      {/* System Info Bar */}
      {metrics && (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
          <div className="bg-bg-card border border-gray-800 rounded-lg p-4">
            <p className="text-xs text-gray-500 uppercase tracking-wider">Uptime</p>
            <p className="text-xl font-bold text-white mt-1">{formatUptime(metrics.uptime_seconds)}</p>
          </div>
          <div className="bg-bg-card border border-gray-800 rounded-lg p-4">
            <p className="text-xs text-gray-500 uppercase tracking-wider">Load Avg</p>
            <p className="text-xl font-bold text-white mt-1">
              {metrics.load_average_1m.toFixed(2)}
            </p>
            <p className="text-xs text-gray-500 mt-1">
              {metrics.load_average_5m.toFixed(2)} / {metrics.load_average_15m.toFixed(2)}
            </p>
          </div>
          <div className="bg-bg-card border border-gray-800 rounded-lg p-4">
            <p className="text-xs text-gray-500 uppercase tracking-wider">Processes</p>
            <p className="text-xl font-bold text-white mt-1">{metrics.process_count}</p>
          </div>
          <div className="bg-bg-card border border-gray-800 rounded-lg p-4">
            <p className="text-xs text-gray-500 uppercase tracking-wider">CPU Cores</p>
            <p className="text-xl font-bold text-white mt-1">{metrics.cpu_count}</p>
            <p className="text-xs text-gray-500 mt-1">{metrics.cpu_frequency_mhz} MHz</p>
          </div>
        </div>
      )}

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
          value={metrics?.disk_read_mb !== undefined && metrics?.disk_write_mb !== undefined ? `${(metrics.disk_read_mb + metrics.disk_write_mb).toFixed(0)} MB/s` : '-- MB/s'} 
          data={diskData} 
          color="#f59e0b" 
        />
        <MetricChartCard 
          title="Network Traffic" 
          value={metrics?.network_rx_mb !== undefined && metrics?.network_tx_mb !== undefined ? `${((metrics.network_rx_mb + metrics.network_tx_mb) / 1024).toFixed(2)} MB/s` : '-- MB/s'} 
          data={netData} 
          color="#8b5cf6" 
        />
      </div>

      {/* Disk Space Card */}
      {metrics && metrics.disk_total_gb > 0 && (
        <div className="bg-bg-card border border-gray-800 rounded-xl p-6">
          <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest mb-4">Disk Space</h3>
          <div className="flex items-center justify-between mb-2">
            <span className="text-2xl font-black text-white">
              {metrics.disk_used_gb} GB / {metrics.disk_total_gb} GB
            </span>
            <span className="text-lg font-bold text-gray-400">
              {metrics.disk_usage_percent.toFixed(1)}%
            </span>
          </div>
          <div className="w-full bg-gray-800 rounded-full h-3 overflow-hidden">
            <div 
              className="h-full rounded-full transition-all duration-500"
              style={{ 
                width: `${metrics.disk_usage_percent}%`,
                backgroundColor: metrics.disk_usage_percent > 90 ? '#ef4444' : 
                                metrics.disk_usage_percent > 75 ? '#f59e0b' : '#10b981'
              }}
            />
          </div>
          <p className="text-xs text-gray-500 mt-2">
            {metrics.disk_free_gb} GB free
          </p>
        </div>
      )}

      {/* Top Processes */}
      {metrics && (metrics.top_cpu_processes.length > 0 || metrics.top_memory_processes.length > 0) && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {/* Top CPU Processes */}
          {metrics.top_cpu_processes.length > 0 && (
            <div className="bg-bg-card border border-gray-800 rounded-xl p-6">
              <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest mb-4">
                Top CPU Processes
              </h3>
              <div className="space-y-3">
                {metrics.top_cpu_processes.map((proc, idx) => (
                  <div key={`cpu-${proc.pid}`} className="flex items-center justify-between">
                    <div className="flex items-center space-x-3 flex-1 min-w-0">
                      <span className="text-xs font-mono text-gray-500 w-6">#{idx + 1}</span>
                      <span className="text-sm text-gray-300 truncate">{proc.name}</span>
                    </div>
                    <div className="flex items-center space-x-3">
                      <span className="text-xs text-gray-500">PID {proc.pid}</span>
                      <span className="text-sm font-bold text-blue-400 w-16 text-right">
                        {proc.cpu_usage.toFixed(1)}%
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Top Memory Processes */}
          {metrics.top_memory_processes.length > 0 && (
            <div className="bg-bg-card border border-gray-800 rounded-xl p-6">
              <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest mb-4">
                Top Memory Processes
              </h3>
              <div className="space-y-3">
                {metrics.top_memory_processes.map((proc, idx) => (
                  <div key={`mem-${proc.pid}`} className="flex items-center justify-between">
                    <div className="flex items-center space-x-3 flex-1 min-w-0">
                      <span className="text-xs font-mono text-gray-500 w-6">#{idx + 1}</span>
                      <span className="text-sm text-gray-300 truncate">{proc.name}</span>
                    </div>
                    <div className="flex items-center space-x-3">
                      <span className="text-xs text-gray-500">PID {proc.pid}</span>
                      <span className="text-sm font-bold text-green-400 w-16 text-right">
                        {proc.memory_mb} MB
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default MonitoringView;

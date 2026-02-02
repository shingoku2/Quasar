import React, { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import MetricChartCard from './dashboard/MetricChartCard';

interface SystemMetrics {
  cpu_usage_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  memory_usage_percent: number;
  disk_read_mb: number;
  disk_write_mb: number;
  network_rx_mb: number;
  network_tx_mb: number;
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

  return (
    <div className="p-6 space-y-6 h-full overflow-y-auto no-scrollbar bg-bg-root animate-in fade-in duration-500">
      <div className="mb-6">
        <h2 className="text-2xl font-bold uppercase tracking-widest text-gray-100">System Monitoring</h2>
        <p className="text-sm text-gray-400 mt-1">Real-time system metrics and performance monitoring</p>
      </div>

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
    </div>
  );
};

export default MonitoringView;

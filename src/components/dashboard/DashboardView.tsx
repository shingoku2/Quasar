import React, { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import SystemHealthWidget from './SystemHealthWidget';
import MetricChartCard from './MetricChartCard';
import AlertFeed from './AlertFeed';
import AlertRules from './AlertRules';
import DiscoveryWidget from './DiscoveryWidget';
import NetworkMapWidget from './NetworkMapWidget';
import NetworkScanner, { ScanResult } from '../NetworkScanner';
import { Search } from 'lucide-react';

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

const DashboardView: React.FC = () => {
  const [cpuData, setCpuData] = useState<{ time: string; value: number }[]>([]);
  const [memData, setMemData] = useState<{ time: string; value: number }[]>([]);
  const [diskData, setDiskData] = useState<{ time: string; value: number }[]>([]);
  const [netData, setNetData] = useState<{ time: string; value: number }[]>([]);
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
  const [discoveredHosts, setDiscoveredHosts] = useState<ScanResult[]>([]);

  // Listen for real-time metrics from backend
  useEffect(() => {
    const timeLabel = new Date().toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
    
    const unlisten = listen<SystemMetrics>('system-metrics', (event) => {
      const data = event.payload;
      setMetrics(data);
      
      setCpuData(prev => [...prev.slice(-19), { time: timeLabel, value: data.cpu_usage_percent }]);
      setMemData(prev => [...prev.slice(-19), { time: timeLabel, value: data.memory_usage_percent }]);
      setDiskData(prev => [...prev.slice(-19), { time: timeLabel, value: data.disk_read_mb + data.disk_write_mb }]);
      setNetData(prev => [...prev.slice(-19), { time: timeLabel, value: data.network_rx_mb + data.network_tx_mb }]);
    });

    // Get initial metrics
    invoke<SystemMetrics>('get_system_metrics').then((data) => {
      setMetrics(data);
      setCpuData([{ time: timeLabel, value: data.cpu_usage_percent }]);
      setMemData([{ time: timeLabel, value: data.memory_usage_percent }]);
      setDiskData([{ time: timeLabel, value: data.disk_read_mb + data.disk_write_mb }]);
      setNetData([{ time: timeLabel, value: data.network_rx_mb + data.network_tx_mb }]);
    }).catch(console.error);

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  return (
    <div className="p-6 space-y-6 h-full overflow-y-auto no-scrollbar bg-bg-root animate-in fade-in duration-500">
      {/* Top Banner */}
      <SystemHealthWidget 
        healthScore={98} 
        statusText="SYSTEMS NOMINAL" 
        nodes={[
          { id: 'DB-01', status: 'online' },
          { id: 'WEB-01', status: 'online' },
          { id: 'WEB-02', status: 'warning' },
          { id: 'LB-01', status: 'online' },
        ]}
      />

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Main Charts Column */}
        <div className="lg:col-span-2 space-y-6">
          {/* Network Scanner */}
          <NetworkScanner 
            onHostFound={(host) => {
              setDiscoveredHosts(prev => {
                if (prev.some(h => h.ip === host.ip)) return prev;
                return [...prev, host];
              });
            }}
          />

          {/* Discovery and Network Map Widgets */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <DiscoveryWidget 
              hosts={discoveredHosts.map(h => ({
                name: `Host ${h.ip}`,
                address: h.ip,
                port: h.open_ports[0] || 22,
                protocol: h.open_ports.includes(3389) ? 'rdp' : 'ssh',
                discoveredAt: new Date()
              }))}
            />
            <NetworkMapWidget 
              hosts={discoveredHosts.map(h => ({
                id: h.ip,
                ip: h.ip,
                type: h.open_ports.includes(3389) ? 'server' : 'workstation',
                status: h.is_alive ? 'online' : 'offline'
              }))}
            />
          </div>
          
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <MetricChartCard 
              title="CPU Usage" 
              value={metrics ? `${metrics.cpu_usage_percent.toFixed(1)}%` : '--%'} 
              data={cpuData} 
              color="#3b82f6" 
            />
            <MetricChartCard 
              title="Memory Utilization" 
              value={metrics ? `${(metrics.memory_used_mb / 1024).toFixed(1)} GB / ${(metrics.memory_total_mb / 1024).toFixed(1)} GB` : '-- GB'} 
              data={memData} 
              color="#10b981" 
            />
            <MetricChartCard 
              title="Disk I/O Traffic" 
              value={metrics ? `${(metrics.disk_read_mb + metrics.disk_write_mb).toFixed(0)} MB/s` : '-- MB/s'} 
              data={diskData} 
              color="#f59e0b" 
            />
            <MetricChartCard 
              title="Network Traffic" 
              value={metrics ? `${((metrics.network_rx_mb + metrics.network_tx_mb) / 1024).toFixed(2)} MB/s` : '-- MB/s'} 
              data={netData} 
              color="#8b5cf6" 
            />
          </div>
        </div>

        {/* Right Sidebar Column */}
        <div className="space-y-6 flex flex-col h-full">
          {/* Quick Connect Widget (Placeholder from PDF) */}
          <div className="bg-bg-card border border-gray-800 rounded-xl p-4 shadow-sm shrink-0">
            <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest mb-3">Quick Connect</h3>
            <div className="relative mb-3">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-gray-500" />
              <input 
                type="text" 
                placeholder="Search inventory..." 
                className="w-full bg-zinc-900 border border-gray-800 rounded-lg py-1.5 pl-8 pr-3 text-xs focus:outline-none focus:border-accent/50"
              />
            </div>
            <div className="space-y-1">
              {['Database-01', 'Database-02', 'Web-01', 'Web-02'].map(name => (
                <button key={name} className="w-full text-left px-2 py-1.5 rounded hover:bg-white/5 text-[11px] text-gray-400 hover:text-gray-200 transition-colors flex items-center">
                  <div className="h-1.5 w-1.5 rounded-full bg-green-500 mr-2" />
                  {name}
                </button>
              ))}
            </div>
          </div>

          <div className="flex-1 min-h-0">
            <AlertFeed />
          </div>
          
          <div className="shrink-0">
            <AlertRules />
          </div>
        </div>
      </div>
    </div>
  );
};

export default DashboardView;

import React, { useState, useEffect } from 'react';
import SystemHealthWidget from './SystemHealthWidget';
import MetricChartCard from './MetricChartCard';
import AlertFeed, { Alert } from './AlertFeed';
import DiscoveryWidget from './DiscoveryWidget';
import NetworkMapWidget from './NetworkMapWidget';
import NetworkScanner, { ScanResult } from '../NetworkScanner';
import { Search } from 'lucide-react';

const DashboardView: React.FC = () => {
  const [cpuData, setCpuData] = useState<{ time: string; value: number }[]>([]);
  const [memData, setMemData] = useState<{ time: string; value: number }[]>([]);
  const [discoveredHosts, setDiscoveredHosts] = useState<ScanResult[]>([]);

  // Generate initial mock data
  useEffect(() => {
    const generateData = () => {
      const data = [];
      for (let i = 0; i < 20; i++) {
        data.push({
          time: `${i}:00`,
          value: Math.floor(Math.random() * 40) + 30
        });
      }
      return data;
    };
    setCpuData(generateData());
    setMemData(generateData());

    // Simulated real-time update
    const interval = setInterval(() => {
      setCpuData(prev => [...prev.slice(1), { time: 'Now', value: Math.floor(Math.random() * 40) + 30 }]);
      setMemData(prev => [...prev.slice(1), { time: 'Now', value: Math.floor(Math.random() * 40) + 40 }]);
    }, 3000);

    return () => clearInterval(interval);
  }, []);

  const alerts: Alert[] = [
    { id: '1', source: 'Database-01', message: 'High CPU Usage (over 90% for 5 mins)', severity: 'critical', timestamp: '3 mins ago' },
    { id: '2', source: 'Database-01', message: 'High I/O Wait', severity: 'warning', timestamp: '5 mins ago' },
    { id: '3', source: 'Web-02', message: 'Disk Space Low (< 10%)', severity: 'warning', timestamp: '12 mins ago' },
    { id: '4', source: 'Web-03', message: 'Disk Space Low (< 15%)', severity: 'info', timestamp: '15 mins ago' },
  ];

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
            <MetricChartCard title="CPU Usage" value="42.5%" data={cpuData} color="#3b82f6" />
            <MetricChartCard title="Memory Utilization" value="12.8 GB" data={memData} color="#10b981" />
            <MetricChartCard title="Disk I/O Traffic" value="240 MB/s" data={cpuData} color="#f59e0b" />
            <MetricChartCard title="Network Traffic" value="1.2 Gbps" data={memData} color="#8b5cf6" />
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

          <div className="flex-1">
            <AlertFeed alerts={alerts} />
          </div>
        </div>
      </div>
    </div>
  );
};

export default DashboardView;

import React, { useState } from 'react';
import SystemHealthWidget from './SystemHealthWidget';
import AlertFeed from './AlertFeed';
import AlertRules from './AlertRules';
import DiscoveryWidget from './DiscoveryWidget';
import NetworkMapWidget from './NetworkMapWidget';
import NetworkScanner, { ScanResult } from '../NetworkScanner';
import QuickConnectWidget from './QuickConnectWidget';
import { ViewId } from '../Sidebar';

interface SavedHost {
  id: number;
  name: string;
  address: string;
  port: number | null;
  username: string | null;
  protocol: string;
}

interface DashboardViewProps {
  onNavigate: (view: ViewId) => void;
}

const DashboardView: React.FC<DashboardViewProps> = ({ onNavigate }) => {
  const [discoveredHosts, setDiscoveredHosts] = useState<ScanResult[]>([]);

  const handleQuickConnect = async (host: SavedHost) => {
    // Switch to Remote view
    onNavigate('remote');
    
    // Store the selected host in sessionStorage so RemoteManager can pick it up
    sessionStorage.setItem('quickConnectHost', JSON.stringify(host));
    
    console.log(`Quick connect: Navigating to Remote view for ${host.name}`);
  };


  return (
    <div className="p-6 space-y-6 h-full overflow-y-auto no-scrollbar bg-bg-root animate-in fade-in duration-500">
      {/* Top Banner */}
      <SystemHealthWidget />

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
        </div>

        {/* Right Sidebar Column */}
        <div className="space-y-6 flex flex-col h-full">
          {/* Quick Connect Widget */}
          <QuickConnectWidget onConnect={handleQuickConnect} />

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

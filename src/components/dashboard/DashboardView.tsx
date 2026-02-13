import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import SystemHealthWidget from './SystemHealthWidget';
import AlertFeed from './AlertFeed';
import AlertRules from './AlertRules';
import DiscoveryWidget from './DiscoveryWidget';
import NetworkMapWidget from './NetworkMapWidget';
import NetworkScanner, { ScanResult } from '../NetworkScanner';
import NetworkTopologyView from '../NetworkTopologyView';
import HostDetailDialog from '../HostDetailDialog';
import QuickConnectWidget from './QuickConnectWidget';
import { ViewId } from '../Sidebar';
import { List, Network as NetworkIcon } from 'lucide-react';

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
  const [selectedHost, setSelectedHost] = useState<ScanResult | null>(null);
  const [viewMode, setViewMode] = useState<'list' | 'topology'>('list');

  const handleQuickConnect = async (host: SavedHost) => {
    // Switch to Remote view
    onNavigate('remote');
    
    // Store the selected host in sessionStorage so RemoteManager can pick it up
    sessionStorage.setItem('quickConnectHost', JSON.stringify(host));
    // Notify same-window listeners (storage event only fires cross-window)
    window.dispatchEvent(new Event('quickConnectTriggered'));
    
    console.log(`Quick connect: Navigating to Remote view for ${host.name}`);
  };

  const handleHostClick = (host: ScanResult) => {
    setSelectedHost(host);
  };

  const handleHostConnect = (host: ScanResult) => {
    // Convert ScanResult to SavedHost format for quick connect
    const savedHost: SavedHost = {
      id: Date.now(), // Temporary ID
      name: host.hostname || host.ip,
      address: host.ip,
      port: host.open_ports?.[0] ?? 22,
      username: null,
      protocol: host.open_ports.includes(3389) ? 'rdp' : 'ssh',
    };
    setSelectedHost(null);
    handleQuickConnect(savedHost);
  };

  const handleHostSave = async (host: ScanResult) => {
    // TODO: Implement save to hosts database
    console.log('Save host:', host);
    alert('Save to hosts feature coming soon!');
  };

  const handleHostDelete = async (ip: string) => {
    try {
      await invoke('delete_discovered_host', { ip });
      setDiscoveredHosts(prev => prev.filter(h => h.ip !== ip));
    } catch (err) {
      console.error('Failed to delete host:', err);
      alert('Failed to delete host');
    }
  };


  return (
    <div className="p-6 space-y-6 h-full overflow-y-auto no-scrollbar bg-bg-root animate-in fade-in duration-500">
      {/* Top Banner */}
      <SystemHealthWidget />

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Main Charts Column */}
        <div className="lg:col-span-2 space-y-6">
          {/* Network Scanner with View Toggle */}
          <div className="space-y-4">
            {/* View Mode Toggle */}
            <div className="flex justify-end space-x-2">
              <button
                onClick={() => setViewMode('list')}
                className={`px-4 py-2 rounded-lg text-sm font-medium transition-all flex items-center space-x-2 ${
                  viewMode === 'list'
                    ? 'bg-accent text-white'
                    : 'bg-bg-sidebar text-gray-400 hover:text-white border border-gray-700'
                }`}
              >
                <List className="h-4 w-4" />
                <span>List View</span>
              </button>
              <button
                onClick={() => setViewMode('topology')}
                className={`px-4 py-2 rounded-lg text-sm font-medium transition-all flex items-center space-x-2 ${
                  viewMode === 'topology'
                    ? 'bg-accent text-white'
                    : 'bg-bg-sidebar text-gray-400 hover:text-white border border-gray-700'
                }`}
              >
                <NetworkIcon className="h-4 w-4" />
                <span>Topology View</span>
              </button>
            </div>

            {/* Conditional View Rendering */}
            {viewMode === 'list' ? (
              <NetworkScanner 
                onHostFound={(host) => {
                  setDiscoveredHosts(prev => {
                    if (prev.some(h => h.ip === host.ip)) return prev;
                    return [...prev, host];
                  });
                }}
                onHostClick={handleHostClick}
              />
            ) : (
              <NetworkTopologyView
                hosts={discoveredHosts}
                onHostClick={handleHostClick}
                onHostConnect={handleHostConnect}
              />
            )}
          </div>

          {/* Discovery and Network Map Widgets */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <DiscoveryWidget 
              hosts={discoveredHosts.map(h => ({
                name: `Host ${h.ip}`,
                address: h.ip,
                port: h.open_ports?.[0] ?? 22,
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

      {/* Host Detail Dialog */}
      <HostDetailDialog
        host={selectedHost}
        onClose={() => setSelectedHost(null)}
        onConnect={handleHostConnect}
        onSave={handleHostSave}
        onDelete={handleHostDelete}
      />
    </div>
  );
};

export default DashboardView;

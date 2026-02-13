import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import SystemHealthWidget from './SystemHealthWidget';
import AlertFeed from './AlertFeed';
import NetworkScanner, { ScanResult } from '../NetworkScanner';
import NetworkTopologyView from '../NetworkTopologyView';
import HostDetailDialog from '../HostDetailDialog';
import QuickConnectWidget from './QuickConnectWidget';
import { ViewId } from '../Sidebar';
import { List, Network as NetworkIcon, MoreHorizontal } from 'lucide-react';

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
  const [viewMode, setViewMode] = useState<'list' | 'topology'>('topology');

  const handleQuickConnect = async (host: SavedHost) => {
    onNavigate('remote');
    sessionStorage.setItem('quickConnectHost', JSON.stringify(host));
    window.dispatchEvent(new Event('quickConnectTriggered'));
    console.log(`Quick connect: Navigating to Remote view for ${host.name}`);
  };

  const handleHostClick = (host: ScanResult) => {
    setSelectedHost(host);
  };

  const handleHostConnect = (host: ScanResult) => {
    const savedHost: SavedHost = {
      id: Date.now(),
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
    <div className="flex flex-col h-full bg-bg-root animate-in fade-in duration-500">
      {/* Hero: Network Topology / Scanner */}
      <div className="flex-1 min-h-0 p-4 pb-2">
        <div className="bg-bg-card border border-border rounded-xl flex flex-col h-full overflow-hidden">
          {/* Card Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-border">
            <h2 className="text-sm font-bold text-white">Network Topology</h2>
            <div className="flex items-center space-x-2">
              {/* Inline View Toggle */}
              <div className="flex bg-bg-root rounded-lg p-0.5">
                <button
                  onClick={() => setViewMode('list')}
                  className={`px-2.5 py-1 rounded-md text-xs font-medium transition-all flex items-center space-x-1.5 ${
                    viewMode === 'list'
                      ? 'bg-accent text-white shadow-sm'
                      : 'text-gray-400 hover:text-gray-200'
                  }`}
                >
                  <List className="h-3.5 w-3.5" />
                  <span>List</span>
                </button>
                <button
                  onClick={() => setViewMode('topology')}
                  className={`px-2.5 py-1 rounded-md text-xs font-medium transition-all flex items-center space-x-1.5 ${
                    viewMode === 'topology'
                      ? 'bg-accent text-white shadow-sm'
                      : 'text-gray-400 hover:text-gray-200'
                  }`}
                >
                  <NetworkIcon className="h-3.5 w-3.5" />
                  <span>Topology</span>
                </button>
              </div>
              <button className="p-1.5 text-gray-500 hover:text-gray-300 transition-colors">
                <MoreHorizontal className="h-4 w-4" />
              </button>
            </div>
          </div>

          {/* Card Body */}
          <div className="flex-1 min-h-0 overflow-hidden">
            {viewMode === 'list' ? (
              <div className="h-full overflow-y-auto no-scrollbar">
                <NetworkScanner 
                  onHostFound={(host) => {
                    setDiscoveredHosts(prev => {
                      if (prev.some(h => h.ip === host.ip)) return prev;
                      return [...prev, host];
                    });
                  }}
                  onHostClick={handleHostClick}
                />
              </div>
            ) : (
              <NetworkTopologyView
                hosts={discoveredHosts}
                onHostClick={handleHostClick}
                onHostConnect={handleHostConnect}
              />
            )}
          </div>
        </div>
      </div>

      {/* Bottom: 4-Card Row */}
      <div className="shrink-0 px-4 pb-4 pt-2">
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3">
          {/* Card 1: Real-time Metrics */}
          <SystemHealthWidget />

          {/* Card 2: Quick Connect / Active Sessions */}
          <QuickConnectWidget onConnect={handleQuickConnect} />

          {/* Card 3: Alert Feed / Recent Activity */}
          <AlertFeed />

          {/* Card 4: System Health Summary */}
          <SystemHealthWidget variant="summary" />
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

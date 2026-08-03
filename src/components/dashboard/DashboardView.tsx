import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import SystemHealthWidget from './SystemHealthWidget';
import AlertFeed from './AlertFeed';
import NetworkScanner, { ScanResult } from '../NetworkScanner';
import NetworkTopologyView from '../NetworkTopologyView';
import HostDetailDialog from '../HostDetailDialog';
import QuickConnectWidget from './QuickConnectWidget';
import AddHostDialog, { AddHostInitialValues } from '../AddHostDialog';
import { ViewId } from '../Sidebar';
import { List, Network as NetworkIcon } from 'lucide-react';

/** Backend discovered host shape (get_discovered_hosts). */
interface PersistedDiscoveredHost {
  id: string;
  ip: string;
  hostname?: string | null;
  mac_address?: string | null;
  device_type: string;
  vendor?: string | null;
  first_seen: number;
  last_seen: number;
  scan_count: number;
  services: Array<{ port: number; protocol: string; service: string; version?: string | null }>;
}

function persistedToScanResult(h: PersistedDiscoveredHost): ScanResult {
  const open_ports = h.services.map((s) => s.port);
  return {
    ip: h.ip,
    is_alive: true,
    open_ports,
    hostname: h.hostname ?? undefined,
    mac_address: h.mac_address ?? undefined,
    device_type: h.device_type,
    services: h.services.map((s) => ({ port: s.port, protocol: s.protocol, service: s.service, version: s.version ?? undefined })),
    vendor: h.vendor ?? undefined,
    last_seen: h.last_seen,
  };
}

interface SavedHost {
  id: string;
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
  const [hostToSave, setHostToSave] = useState<AddHostInitialValues | null>(null);

  // Load last scan from DB so the user sees persisted results without running a new scan
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const list = await invoke<PersistedDiscoveredHost[] | undefined>('get_discovered_hosts', { limit: 500 });
        const arr = Array.isArray(list) ? list : [];
        if (!cancelled) setDiscoveredHosts(arr.map(persistedToScanResult));
      } catch {
        if (!cancelled) setDiscoveredHosts([]);
      }
    })();
    return () => { cancelled = true; };
  }, []);

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
      id: String(Date.now()),
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
    const protocol: 'ssh' | 'rdp' = host.open_ports.includes(3389) ? 'rdp' : 'ssh';
    const defaultPort = protocol === 'rdp'
      ? (host.open_ports.includes(3389) ? 3389 : null)
      : (host.open_ports.includes(22) ? 22 : host.open_ports[0] ?? 22);

    setHostToSave({
      name: host.hostname || host.ip,
      address: host.ip,
      protocol,
      port: defaultPort,
      username: '',
    });
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
          </div>

          {/* Card Body */}
          <div className="flex-1 min-h-0 overflow-hidden">
            {viewMode === 'list' ? (
              <div className="h-full overflow-y-auto no-scrollbar">
                <NetworkScanner
                  initialResults={discoveredHosts}
                  onResults={setDiscoveredHosts}
                  onHostFound={(host) => {
                    setDiscoveredHosts(prev => {
                      const idx = prev.findIndex(h => h.ip === host.ip);
                      if (idx === -1) return [...prev, host];
                      const next = [...prev];
                      next[idx] = host;
                      return next;
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

      {hostToSave && (
        <AddHostDialog
          initialValues={hostToSave}
          onClose={() => setHostToSave(null)}
          onAdded={() => {
            setHostToSave(null);
            setSelectedHost(null);
          }}
        />
      )}
    </div>
  );
};

export default DashboardView;

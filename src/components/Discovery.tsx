<<<<<<< HEAD
import React, { useState, useEffect, useRef } from 'react';
=======
import React, { useState, useEffect } from 'react';
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface DiscoveredHost {
  name: string;
  address: string;
  port: number;
  service_type: string;
}

interface TrackedDiscoveredHost {
  ip: string;
  hostname?: string;
  services: Array<{ port: number }>;
}

interface DiscoveryProps {
  onAddHost?: (host: DiscoveredHost) => void;
}

const hostKey = (host: DiscoveredHost) => `${host.address}:${host.port}`;

const Discovery: React.FC<DiscoveryProps> = ({ onAddHost }) => {
  const [mdnsHosts, setMdnsHosts] = useState<DiscoveredHost[]>([]);
  const [scanHosts, setScanHosts] = useState<DiscoveredHost[]>([]);
  const [scanning, setScanning] = useState(false);
<<<<<<< HEAD
  const cancelledRef = useRef(false);
  const unlistenRef = useRef<{ mdns?: () => void; scanComplete?: () => void }>({});

  useEffect(() => {
    cancelledRef.current = false;
    unlistenRef.current = {};
=======

  useEffect(() => {
    let unlistenMdns: (() => void) | undefined;
    let unlistenScanComplete: (() => void) | undefined;
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7

    const mergeHosts = (hosts: DiscoveredHost[]) => {
      const deduped = new Map<string, DiscoveredHost>();
      hosts.forEach((host) => {
        deduped.set(hostKey(host), host);
      });
      return Array.from(deduped.values());
    };

    const refreshScanHosts = async () => {
      try {
        const discovered = await invoke<TrackedDiscoveredHost[] | undefined>('get_discovered_hosts', { limit: 100 });
        const safeDiscovered = Array.isArray(discovered) ? discovered : [];
        const mapped = safeDiscovered.map<DiscoveredHost>((host) => {
          const fallbackPort = host.services[0]?.port ?? 22;
          const preferredPort = host.services.find((service) => service.port === 22)?.port ?? fallbackPort;
          return {
            name: host.hostname || host.ip,
            address: host.ip,
            port: preferredPort,
            service_type: 'network-scan',
          };
        });
        setScanHosts(mergeHosts(mapped));
      } catch (error) {
        console.warn('Failed to load discovered_hosts from scanner persistence:', error);
      }
    };

    const startScan = async () => {
      await refreshScanHosts();
<<<<<<< HEAD
      if (cancelledRef.current) return;

      setScanning(true);
      await invoke('start_discovery');
      if (cancelledRef.current) return;

      const unlistenMdns = await listen<DiscoveredHost>('host-discovered', (event) => {
        setMdnsHosts(prev => {
=======

      setScanning(true);
      await invoke('start_discovery');
      
      unlistenMdns = await listen<DiscoveredHost>('host-discovered', (event) => {
        setMdnsHosts(prev => {
          // Avoid duplicates
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
          if (prev.some(h => h.address === event.payload.address)) return prev;
          return mergeHosts([...prev, event.payload]);
        });
      });
<<<<<<< HEAD
      if (cancelledRef.current) {
        unlistenMdns();
        return;
      }
      unlistenRef.current.mdns = unlistenMdns;

      const unlistenScanComplete = await listen('scan_complete', async () => {
        await refreshScanHosts();
      });
      if (cancelledRef.current) {
        unlistenScanComplete();
        return;
      }
      unlistenRef.current.scanComplete = unlistenScanComplete;
=======

      unlistenScanComplete = await listen('scan_complete', async () => {
        await refreshScanHosts();
      });
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
    };

    startScan().catch((error) => {
      setScanning(false);
      console.warn('Failed to start LAN discovery:', error);
    });

    return () => {
<<<<<<< HEAD
      cancelledRef.current = true;
      unlistenRef.current.mdns?.();
      unlistenRef.current.scanComplete?.();
      unlistenRef.current = {};
=======
      if (unlistenMdns) unlistenMdns();
      if (unlistenScanComplete) unlistenScanComplete();
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
      setScanning(false);
    };
  }, []);

  const discoveredHosts = (() => {
    const merged = new Map<string, DiscoveredHost>();
    scanHosts.forEach((host) => merged.set(hostKey(host), host));
    mdnsHosts.forEach((host) => {
      const key = hostKey(host);
      if (!merged.has(key)) {
        merged.set(key, host);
      }
    });
    return Array.from(merged.values());
  })();

  return (
    <div className="p-4 bg-gray-800 rounded mt-4">
      <div className="flex justify-between items-center mb-2">
        <h3 className="text-sm font-bold text-gray-400 uppercase">Discovered Devices (LAN)</h3>
        {scanning && <span className="text-xs text-blue-400 animate-pulse">Scanning...</span>}
      </div>
      
      {discoveredHosts.length === 0 ? (
        <p className="text-xs text-gray-500 italic">No devices found yet.</p>
      ) : (
        <ul className="space-y-2 max-h-56 overflow-y-auto pr-1">
          {discoveredHosts.map((host) => (
            <li key={hostKey(host)} className="flex justify-between items-center bg-gray-900 p-2 rounded">
              <div>
                <div className="text-sm font-medium text-white">{host.name}</div>
                <div className="text-xs text-gray-400">{host.address}:{host.port}</div>
              </div>
              <button
                type="button"
                onClick={() => onAddHost?.(host)}
                disabled={!onAddHost}
                className="text-xs bg-blue-900 text-blue-300 px-2 py-1 rounded hover:bg-blue-800 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Add
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

export default Discovery;

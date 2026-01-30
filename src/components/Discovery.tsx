import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface DiscoveredHost {
  name: string;
  address: string;
  port: number;
  service_type: string;
}

const Discovery: React.FC = () => {
  const [discoveredHosts, setDiscoveredHosts] = useState<DiscoveredHost[]>([]);
  const [scanning, setScanning] = useState(false);

  useEffect(() => {
    let unlisten: () => void;

    const startScan = async () => {
      setScanning(true);
      await invoke('start_discovery');
      
      unlisten = await listen<DiscoveredHost>('host-discovered', (event) => {
        setDiscoveredHosts(prev => {
          // Avoid duplicates
          if (prev.some(h => h.address === event.payload.address)) return prev;
          return [...prev, event.payload];
        });
      });
    };

    startScan();

    return () => {
      if (unlisten) unlisten();
      setScanning(false);
    };
  }, []);

  return (
    <div className="p-4 bg-gray-800 rounded mt-4">
      <div className="flex justify-between items-center mb-2">
        <h3 className="text-sm font-bold text-gray-400 uppercase">Discovered Devices (LAN)</h3>
        {scanning && <span className="text-xs text-blue-400 animate-pulse">Scanning...</span>}
      </div>
      
      {discoveredHosts.length === 0 ? (
        <p className="text-xs text-gray-500 italic">No devices found yet.</p>
      ) : (
        <ul className="space-y-2">
          {discoveredHosts.map((host, idx) => (
            <li key={idx} className="flex justify-between items-center bg-gray-900 p-2 rounded">
              <div>
                <div className="text-sm font-medium text-white">{host.name}</div>
                <div className="text-xs text-gray-400">{host.address}:{host.port}</div>
              </div>
              <button className="text-xs bg-blue-900 text-blue-300 px-2 py-1 rounded hover:bg-blue-800">
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

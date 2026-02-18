import React, { useState, useEffect } from 'react';
import Database from "@tauri-apps/plugin-sql";
import Discovery, { DiscoveredHost } from './Discovery';
import HealthCheckBadge from './HealthCheckBadge';
import { AddHostInitialValues } from './AddHostDialog';

export interface Host {
  id: string;
  name: string;
  address: string;
  protocol: string;
  port?: number;
  username?: string;
}

const HostList: React.FC<{ 
  onConnect: (host: Host) => void;
  onSftp: (host: Host) => void;
  onAddHost?: (values: AddHostInitialValues) => void;
}> = ({ onConnect, onSftp, onAddHost }) => {
  const [hosts, setHosts] = useState<Host[]>([]);
  const [filter, setFilter] = useState('');
  const [loading, setLoading] = useState(true);

  const fetchHosts = async () => {
    try {
      const db = await Database.load("sqlite:quasar.db");
      const result = await db.select<Host[]>("SELECT * FROM hosts ORDER BY name ASC");
      setHosts(result);
    } catch (err) {
      console.error("Failed to fetch hosts:", err);
    } finally {
      setLoading(false);
    }
  };

  const getDuplicateHostIds = (items: Host[]) => {
    const firstByKey = new Map<string, string>();
    const duplicateIds: string[] = [];

    items.forEach((host) => {
      const key = `${host.address}|${host.protocol}|${host.port ?? ''}`;
      if (firstByKey.has(key)) {
        duplicateIds.push(host.id);
      } else {
        firstByKey.set(key, host.id);
      }
    });

    return duplicateIds;
  };

  const duplicateHostIds = getDuplicateHostIds(hosts);

  const handleRemoveDuplicates = async () => {
    if (duplicateHostIds.length === 0) {
      return;
    }

    if (!confirm(`Remove ${duplicateHostIds.length} duplicate saved host(s)?`)) {
      return;
    }

    try {
      const db = await Database.load("sqlite:quasar.db");
      for (const id of duplicateHostIds) {
        await db.execute("DELETE FROM hosts WHERE id = ?", [id]);
      }
      await fetchHosts();
      window.dispatchEvent(new Event('hostsUpdated'));
    } catch (err) {
      console.error('Failed to remove duplicate hosts:', err);
      alert(`Failed to remove duplicate hosts: ${err}`);
    }
  };

  useEffect(() => {
    fetchHosts();

    const handleHostsUpdated = () => {
      fetchHosts();
    };

    window.addEventListener('hostsUpdated', handleHostsUpdated);

    return () => {
      window.removeEventListener('hostsUpdated', handleHostsUpdated);
    };
  }, []);

  const handleAddDiscoveredHost = (host: DiscoveredHost) => {
    if (!onAddHost) return;

    const isRdp = host.port === 3389 || host.service_type.toLowerCase().includes('rdp');
    onAddHost({
      name: host.name,
      address: host.address,
      protocol: isRdp ? 'rdp' : 'ssh',
      port: host.port,
      username: '',
    });
  };

  const handleRemoveHost = async (host: Host) => {
    if (!confirm(`Remove ${host.name} from saved hosts?`)) {
      return;
    }

    try {
      const db = await Database.load("sqlite:quasar.db");
      await db.execute("DELETE FROM hosts WHERE id = ?", [host.id]);
      await fetchHosts();
      window.dispatchEvent(new Event('hostsUpdated'));
    } catch (err) {
      console.error('Failed to remove host:', err);
      alert(`Failed to remove host: ${err}`);
    }
  };

  const filteredHosts = hosts.filter(h => 
    h.name.toLowerCase().includes(filter.toLowerCase()) || 
    h.address.toLowerCase().includes(filter.toLowerCase())
  );

  if (loading) return <div className="p-4 text-gray-400">Loading hosts...</div>;

  return (
    <div className="flex flex-col h-full">
      <div className="p-4 border-b border-gray-700 space-y-3">
        <input 
          type="text" 
          placeholder="Filter hosts..." 
          className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <div className="flex justify-end items-center gap-2">
          {duplicateHostIds.length > 0 && (
            <span className="text-xs text-amber-300/90">
              {duplicateHostIds.length} duplicate{duplicateHostIds.length === 1 ? '' : 's'} detected
            </span>
          )}
          <button
            type="button"
            onClick={handleRemoveDuplicates}
            disabled={duplicateHostIds.length === 0}
            className="text-xs px-3 py-1.5 rounded border border-gray-700 text-gray-300 hover:border-red-400 hover:text-red-300 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Remove duplicates{duplicateHostIds.length > 0 ? ` (${duplicateHostIds.length})` : ''}
          </button>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto">
        {filteredHosts.length === 0 ? (
          <div className="p-8 text-center text-gray-500">
            {filter ? 'No hosts match your filter.' : 'No hosts added yet.'}
          </div>
        ) : (
          <table className="w-full text-left text-sm">
            <thead className="text-gray-400 border-b border-gray-700">
              <tr>
                <th className="px-4 py-2 font-medium">Name</th>
                <th className="px-4 py-2 font-medium">Protocol</th>
                <th className="px-4 py-2 font-medium">Address</th>
                <th className="px-4 py-2 font-medium text-right">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-800">
              {filteredHosts.map(host => (
                <tr key={host.id} className="hover:bg-gray-850 group">
                  <td className="px-4 py-3 font-medium text-gray-200">{host.name}</td>
                  <td className="px-4 py-3">
                    <span className={`px-2 py-0.5 rounded text-xs font-bold uppercase ${
                      host.protocol === 'ssh' ? 'bg-blue-900/50 text-blue-300' : 'bg-purple-900/50 text-purple-300'
                    }`}>
                      {host.protocol}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-gray-400">
                    <div className="flex items-center space-x-2">
                      <span>{host.address}{host.port ? `:${host.port}` : ''}</span>
                      <HealthCheckBadge host={host.address} />
                    </div>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex justify-end space-x-3">
                      <button
                        onClick={() => handleRemoveHost(host)}
                        className="text-gray-400 hover:text-red-400 font-medium transition-colors"
                      >
                        Remove
                      </button>
                      {host.protocol === 'ssh' && (
                        <button 
                          onClick={() => onSftp(host)}
                          className="text-gray-400 hover:text-accent font-medium transition-colors"
                        >
                          SFTP
                        </button>
                      )}
                      <button 
                        onClick={() => onConnect(host)}
                        className="text-accent hover:text-accent/80 font-medium transition-colors"
                      >
                        Connect
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      
      {/* LAN Discovery Section */}
      <div className="border-t border-gray-700 bg-gray-850 p-4">
        <Discovery onAddHost={handleAddDiscoveredHost} />
      </div>
    </div>
  );
};

export default HostList;

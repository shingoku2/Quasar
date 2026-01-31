import React, { useState, useEffect } from 'react';
import Database from "@tauri-apps/plugin-sql";
import Discovery from './Discovery';
import HealthCheckBadge from './HealthCheckBadge';

export interface Host {
  id: number;
  name: string;
  address: string;
  protocol: string;
  port?: number;
  username?: string;
}

const HostList: React.FC<{ onConnect: (host: Host) => void }> = ({ onConnect }) => {
  const [hosts, setHosts] = useState<Host[]>([]);
  const [filter, setFilter] = useState('');
  const [loading, setLoading] = useState(true);

  const fetchHosts = async () => {
    try {
      const db = await Database.load("sqlite:titan.db");
      const result = await db.select<Host[]>("SELECT * FROM hosts ORDER BY name ASC");
      setHosts(result);
    } catch (err) {
      console.error("Failed to fetch hosts:", err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchHosts();
  }, []);

  const filteredHosts = hosts.filter(h => 
    h.name.toLowerCase().includes(filter.toLowerCase()) || 
    h.address.toLowerCase().includes(filter.toLowerCase())
  );

  if (loading) return <div className="p-4 text-gray-400">Loading hosts...</div>;

  return (
    <div className="flex flex-col h-full">
      <div className="p-4 border-b border-gray-700">
        <input 
          type="text" 
          placeholder="Filter hosts..." 
          className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
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
                    <button 
                      onClick={() => onConnect(host)}
                      className="text-blue-400 hover:text-blue-300 font-medium"
                    >
                      Connect
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      
      {/* LAN Discovery Section */}
      <div className="border-t border-gray-700 bg-gray-850 p-4">
        <Discovery />
      </div>
    </div>
  );
};

export default HostList;

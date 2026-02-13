import React, { useState, useEffect } from 'react';
import { Search, Server, Database as DatabaseIcon, Globe, Terminal } from 'lucide-react';
import Database from "@tauri-apps/plugin-sql";

interface SavedHost {
  id: number;
  name: string;
  address: string;
  port: number | null;
  username: string | null;
  protocol: string;
}

interface QuickConnectWidgetProps {
  onConnect: (host: SavedHost) => void;
}

const QuickConnectWidget: React.FC<QuickConnectWidgetProps> = ({ onConnect }) => {
  const [hosts, setHosts] = useState<SavedHost[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    loadHosts();
  }, []);

  const loadHosts = async () => {
    setIsLoading(true);
    try {
      const db = await Database.load("sqlite:titan.db");
      const result = await db.select<SavedHost[]>("SELECT * FROM hosts ORDER BY name ASC LIMIT 10");
      setHosts(result);
    } catch (err) {
      console.error('Failed to load saved hosts:', err);
    } finally {
      setIsLoading(false);
    }
  };

  const filteredHosts = hosts.filter(host => 
    host.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    host.address.toLowerCase().includes(searchQuery.toLowerCase()) ||
    (host.username && host.username.toLowerCase().includes(searchQuery.toLowerCase()))
  ).slice(0, 6); // Show max 6 hosts

  const getHostIcon = (protocol: string) => {
    switch (protocol.toLowerCase()) {
      case 'ssh':
        return Terminal;
      case 'rdp':
        return Server;
      case 'database':
        return DatabaseIcon;
      default:
        return Globe;
    }
  };

  const getStatusColor = (host: SavedHost): string => {
    // For now, assume all hosts are available
    // In the future, this could ping the host to check status
    return 'bg-green-500';
  };

  return (
    <div className="bg-bg-card border border-border rounded-xl p-4 shadow-sm shrink-0">
      <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest mb-3">Active Sessions</h3>
      
      <div className="relative mb-3">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-gray-500" />
        <input 
          type="text" 
          placeholder="Search hosts..." 
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="w-full bg-bg-root border border-border rounded-lg py-1.5 pl-8 pr-3 text-xs focus:outline-none focus:border-accent/50 text-gray-300 placeholder-gray-500"
        />
      </div>

      <div className="space-y-1 max-h-48 overflow-y-auto no-scrollbar">
        {isLoading ? (
          <div className="text-center py-4 text-gray-500 text-xs">Loading hosts...</div>
        ) : filteredHosts.length === 0 ? (
          <div className="text-center py-4 text-gray-500 text-xs">
            {searchQuery ? 'No hosts found' : 'No saved hosts'}
          </div>
        ) : (
          filteredHosts.map(host => {
            const Icon = getHostIcon(host.protocol);
            return (
              <button 
                key={host.id} 
                onClick={() => onConnect(host)}
                className="w-full text-left px-2 py-1.5 rounded-lg hover:bg-white/5 text-[11px] text-gray-400 hover:text-gray-200 transition-colors flex items-center group"
              >
                <div className={`h-1.5 w-1.5 rounded-full ${getStatusColor(host)} mr-2 shrink-0`} />
                <Icon className="h-3 w-3 mr-1.5 text-gray-500 group-hover:text-accent transition-colors shrink-0" />
                <div className="flex-1 min-w-0">
                  <div className="truncate font-medium">{host.name}</div>
                  <div className="text-[9px] text-gray-500 truncate">
                    {host.username ? `${host.username}@` : ''}{host.address}
                  </div>
                </div>
              </button>
            );
          })
        )}
      </div>

      {hosts.length > 6 && !searchQuery && (
        <div className="mt-2 pt-2 border-t border-border text-center">
          <span className="text-[9px] text-gray-500">+{hosts.length - 6} more hosts</span>
        </div>
      )}
    </div>
  );
};

export default QuickConnectWidget;

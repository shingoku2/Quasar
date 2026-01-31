import React from 'react';
import { Network, Radio, Plus } from 'lucide-react';

interface DiscoveredHost {
  name: string;
  address: string;
  port: number;
  protocol: string;
  discoveredAt: Date;
}

interface DiscoveryWidgetProps {
  hosts?: DiscoveredHost[];
  onAddHost?: (host: DiscoveredHost) => void;
  className?: string;
}

const DiscoveryWidget: React.FC<DiscoveryWidgetProps> = ({
  hosts = [],
  onAddHost,
  className = ''
}) => {
  const recentHosts = hosts.slice(0, 5); // Show last 5 discovered

  return (
    <div className={`bg-bg-sidebar border border-gray-800 rounded-lg p-4 ${className}`}>
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center space-x-2">
          <Network className="h-5 w-5 text-accent" />
          <h3 className="text-lg font-bold text-white">Recent Discoveries</h3>
        </div>
        {hosts.length > 0 && (
          <span className="text-xs bg-accent/20 text-accent px-2 py-1 rounded-full">
            {hosts.length} total
          </span>
        )}
      </div>

      {recentHosts.length === 0 ? (
        <div className="text-center py-8">
          <Radio className="h-12 w-12 text-gray-700 mx-auto mb-3" />
          <p className="text-sm text-gray-500">
            No hosts discovered yet.
          </p>
          <p className="text-xs text-gray-600 mt-1">
            Run a network scan to discover devices.
          </p>
        </div>
      ) : (
        <ul className="space-y-2">
          {recentHosts.map((host, idx) => (
            <li 
              key={idx}
              className="flex items-center justify-between bg-bg-root rounded-lg p-3"
            >
              <div>
                <div className="text-sm font-medium text-white">{host.name}</div>
                <div className="text-xs text-gray-400">
                  {host.address}:{host.port} 
                  <span className={`ml-2 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase ${
                    host.protocol === 'ssh' 
                      ? 'bg-blue-900/50 text-blue-300' 
                      : 'bg-purple-900/50 text-purple-300'
                  }`}>
                    {host.protocol}
                  </span>
                </div>
              </div>
              {onAddHost && (
                <button
                  onClick={() => onAddHost(host)}
                  className="p-1.5 text-gray-500 hover:text-accent transition-colors"
                  title="Add to inventory"
                >
                  <Plus className="h-4 w-4" />
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

export default DiscoveryWidget;

import React from 'react';
import { Map, Server, Laptop, Router } from 'lucide-react';

interface NetworkHost {
  id: string;
  ip: string;
  name?: string;
  type: 'server' | 'workstation' | 'router' | 'other';
  status: 'online' | 'offline';
}

interface NetworkMapWidgetProps {
  hosts?: NetworkHost[];
  localIp?: string;
  className?: string;
}

const NetworkMapWidget: React.FC<NetworkMapWidgetProps> = ({
  hosts = [],
  localIp = '192.168.1.1',
  className = ''
}) => {
  const onlineHosts = hosts.filter(h => h.status === 'online');
  const offlineHosts = hosts.filter(h => h.status === 'offline');

  const getHostIcon = (type: string) => {
    switch (type) {
      case 'server':
        return <Server className="h-5 w-5" />;
      case 'router':
        return <Router className="h-5 w-5" />;
      default:
        return <Laptop className="h-5 w-5" />;
    }
  };

  return (
    <div className={`bg-bg-sidebar border border-gray-800 rounded-lg p-4 ${className}`}>
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center space-x-2">
          <Map className="h-5 w-5 text-accent" />
          <h3 className="text-lg font-bold text-white">Network Map</h3>
        </div>
        <div className="flex items-center space-x-4 text-xs">
          <span className="flex items-center space-x-1">
            <span className="w-2 h-2 bg-green-500 rounded-full" />
            <span className="text-gray-400">{onlineHosts.length} Online</span>
          </span>
          <span className="flex items-center space-x-1">
            <span className="w-2 h-2 bg-red-500 rounded-full" />
            <span className="text-gray-400">{offlineHosts.length} Offline</span>
          </span>
        </div>
      </div>

      {hosts.length === 0 ? (
        <div className="text-center py-8">
          <Map className="h-12 w-12 text-gray-700 mx-auto mb-3" />
          <p className="text-sm text-gray-500">No network data available.</p>
          <p className="text-xs text-gray-600 mt-1">
            Run a scan to visualize your network.
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          {/* Local Network Hub */}
          <div className="flex items-center justify-center">
            <div className="bg-accent/20 border border-accent rounded-lg px-4 py-2 text-center">
              <Router className="h-6 w-6 text-accent mx-auto mb-1" />
              <span className="text-xs text-gray-300">Local Network</span>
              <p className="text-xs text-gray-500">{localIp}/24</p>
            </div>
          </div>

          {/* Connected Hosts */}
          <div className="border-t border-gray-800 pt-4">
            <p className="text-xs text-gray-500 uppercase tracking-wider mb-3">Discovered Hosts</p>
            <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
              {hosts.slice(0, 6).map((host) => (
                <div
                  key={host.id}
                  className={`p-3 rounded-lg border ${
                    host.status === 'online'
                      ? 'bg-green-900/20 border-green-800'
                      : 'bg-gray-800 border-gray-700'
                  }`}
                >
                  <div className={`flex items-center justify-between mb-2 ${
                    host.status === 'online' ? 'text-green-400' : 'text-gray-500'
                  }`}>
                    {getHostIcon(host.type)}
                    <span className={`w-2 h-2 rounded-full ${
                      host.status === 'online' ? 'bg-green-500' : 'bg-red-500'
                    }`} />
                  </div>
                  <p className="text-sm font-medium text-white truncate">
                    {host.name || host.ip}
                  </p>
                  <p className="text-xs text-gray-500 truncate">{host.ip}</p>
                </div>
              ))}
            </div>
            {hosts.length > 6 && (
              <p className="text-xs text-gray-500 text-center mt-3">
                +{hosts.length - 6} more hosts
              </p>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export default NetworkMapWidget;

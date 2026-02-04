import React, { useEffect, useRef, useState } from 'react';
import { Network } from 'vis-network';
import { DataSet } from 'vis-data';
import { ScanResult } from './NetworkScanner';
import { ZoomIn, ZoomOut, Maximize2, Pause, Play, Search } from 'lucide-react';

interface NetworkTopologyViewProps {
  hosts: ScanResult[];
  gatewayIp?: string;
  onHostClick?: (host: ScanResult) => void;
  onHostConnect?: (host: ScanResult) => void;
  className?: string;
}

const NetworkTopologyView: React.FC<NetworkTopologyViewProps> = ({
  hosts,
  gatewayIp = '192.168.1.1',
  onHostClick,
  onHostConnect,
  className = ''
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const networkRef = useRef<Network | null>(null);
  const [physicsEnabled, setPhysicsEnabled] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');

  useEffect(() => {
    if (!containerRef.current) return;

    // Create nodes
    const nodes = new DataSet<any>();
    const edges = new DataSet<any>();

    // Add gateway node
    nodes.add({
      id: 'gateway',
      label: `Gateway\n${gatewayIp}`,
      shape: 'box',
      color: {
        background: '#00d4ff',
        border: '#0099cc',
        highlight: {
          background: '#00e5ff',
          border: '#00b8e6'
        }
      },
      font: { color: '#ffffff', size: 14, face: 'monospace' },
      size: 30,
      mass: 5,
    });

    // Add host nodes
    hosts.forEach(host => {
      const deviceColors = {
        server: { bg: '#3b82f6', border: '#2563eb' },
        router: { bg: '#8b5cf6', border: '#7c3aed' },
        printer: { bg: '#ec4899', border: '#db2777' },
        workstation: { bg: '#10b981', border: '#059669' },
        unknown: { bg: '#6b7280', border: '#4b5563' }
      };

      const colors = deviceColors[host.device_type as keyof typeof deviceColors] || deviceColors.unknown;

      nodes.add({
        id: host.ip,
        label: `${host.hostname || host.ip}\n${host.device_type}`,
        shape: 'dot',
        color: {
          background: colors.bg,
          border: colors.border,
          highlight: {
            background: colors.bg,
            border: '#ffffff'
          }
        },
        font: { color: '#ffffff', size: 12 },
        size: 20 + (host.services.length * 2),
        title: `${host.ip}\n${host.hostname || 'No hostname'}\nDevice: ${host.device_type}\nLatency: ${host.latency_ms || 'N/A'}ms\nServices: ${host.services.length}`,
        mass: 2,
      });

      // Connect to gateway
      edges.add({
        id: `${host.ip}-gateway`,
        from: host.ip,
        to: 'gateway',
        color: {
          color: '#4b5563',
          highlight: '#00d4ff',
          opacity: 0.5
        },
        width: Math.max(1, 5 - (host.latency_ms || 100) / 20),
        smooth: {
          type: 'continuous',
          roundness: 0.5
        }
      });
    });

    // Network options
    const options = {
      nodes: {
        borderWidth: 2,
        shadow: {
          enabled: true,
          color: 'rgba(0,0,0,0.3)',
          size: 10,
          x: 2,
          y: 2
        }
      },
      edges: {
        smooth: {
          enabled: true,
          type: 'continuous',
          roundness: 0.5
        }
      },
      physics: {
        enabled: true,
        stabilization: {
          enabled: true,
          iterations: 100,
          updateInterval: 25
        },
        barnesHut: {
          gravitationalConstant: -2000,
          centralGravity: 0.3,
          springLength: 200,
          springConstant: 0.04,
          damping: 0.09,
          avoidOverlap: 0.5
        }
      },
      interaction: {
        hover: true,
        tooltipDelay: 100,
        zoomView: true,
        dragView: true,
        navigationButtons: false,
        keyboard: {
          enabled: false
        }
      },
      layout: {
        improvedLayout: true,
        hierarchical: false
      }
    };

    // Create network
    const network = new Network(containerRef.current, { nodes, edges }, options);
    networkRef.current = network;

    // Event handlers
    network.on('click', (params) => {
      if (params.nodes.length > 0) {
        const nodeId = params.nodes[0];
        if (nodeId !== 'gateway') {
          const host = hosts.find(h => h.ip === nodeId);
          if (host && onHostClick) {
            onHostClick(host);
          }
        }
      }
    });

    network.on('doubleClick', (params) => {
      if (params.nodes.length > 0) {
        const nodeId = params.nodes[0];
        if (nodeId !== 'gateway') {
          const host = hosts.find(h => h.ip === nodeId);
          if (host && onHostConnect) {
            onHostConnect(host);
          }
        }
      }
    });

    // Cleanup
    return () => {
      if (networkRef.current) {
        networkRef.current.destroy();
        networkRef.current = null;
      }
    };
  }, [hosts, gatewayIp, onHostClick, onHostConnect]);

  // Update physics when toggled
  useEffect(() => {
    if (networkRef.current) {
      networkRef.current.setOptions({ physics: { enabled: physicsEnabled } });
    }
  }, [physicsEnabled]);

  // Search functionality
  useEffect(() => {
    if (!networkRef.current || !searchQuery) return;

    const matchingHosts = hosts.filter(h => 
      h.ip.includes(searchQuery) || 
      h.hostname?.toLowerCase().includes(searchQuery.toLowerCase()) ||
      h.device_type.toLowerCase().includes(searchQuery.toLowerCase())
    );

    if (matchingHosts.length > 0) {
      networkRef.current.selectNodes(matchingHosts.map(h => h.ip));
      networkRef.current.focus(matchingHosts[0].ip, {
        scale: 1.5,
        animation: {
          duration: 500,
          easingFunction: 'easeInOutQuad'
        }
      });
    }
  }, [searchQuery, hosts]);

  const handleZoomIn = () => {
    if (networkRef.current) {
      const scale = networkRef.current.getScale();
      networkRef.current.moveTo({ scale: scale * 1.2 });
    }
  };

  const handleZoomOut = () => {
    if (networkRef.current) {
      const scale = networkRef.current.getScale();
      networkRef.current.moveTo({ scale: scale * 0.8 });
    }
  };

  const handleFit = () => {
    if (networkRef.current) {
      networkRef.current.fit({
        animation: {
          duration: 500,
          easingFunction: 'easeInOutQuad'
        }
      });
    }
  };

  return (
    <div className={`relative bg-bg-root border border-gray-800 rounded-lg overflow-hidden ${className}`}>
      {/* Controls */}
      <div className="absolute top-4 left-4 z-10 flex flex-col space-y-2">
        <button
          onClick={handleZoomIn}
          className="bg-bg-sidebar border border-gray-700 hover:border-accent text-white p-2 rounded-lg transition-colors"
          title="Zoom In"
        >
          <ZoomIn className="h-4 w-4" />
        </button>
        <button
          onClick={handleZoomOut}
          className="bg-bg-sidebar border border-gray-700 hover:border-accent text-white p-2 rounded-lg transition-colors"
          title="Zoom Out"
        >
          <ZoomOut className="h-4 w-4" />
        </button>
        <button
          onClick={handleFit}
          className="bg-bg-sidebar border border-gray-700 hover:border-accent text-white p-2 rounded-lg transition-colors"
          title="Fit to Screen"
        >
          <Maximize2 className="h-4 w-4" />
        </button>
        <button
          onClick={() => setPhysicsEnabled(!physicsEnabled)}
          className="bg-bg-sidebar border border-gray-700 hover:border-accent text-white p-2 rounded-lg transition-colors"
          title={physicsEnabled ? 'Freeze Layout' : 'Enable Physics'}
        >
          {physicsEnabled ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
        </button>
      </div>

      {/* Search */}
      <div className="absolute top-4 right-4 z-10">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-gray-500" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search hosts..."
            className="bg-bg-sidebar border border-gray-700 focus:border-accent text-white pl-10 pr-4 py-2 rounded-lg text-sm outline-none transition-colors w-64"
          />
        </div>
      </div>

      {/* Legend */}
      <div className="absolute bottom-4 left-4 z-10 bg-bg-sidebar border border-gray-700 rounded-lg p-3">
        <p className="text-xs text-gray-400 uppercase tracking-wider mb-2">Device Types</p>
        <div className="space-y-1">
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-blue-500" />
            <span className="text-xs text-white">Server</span>
          </div>
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-purple-500" />
            <span className="text-xs text-white">Router</span>
          </div>
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-pink-500" />
            <span className="text-xs text-white">Printer</span>
          </div>
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-green-500" />
            <span className="text-xs text-white">Workstation</span>
          </div>
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-gray-500" />
            <span className="text-xs text-white">Unknown</span>
          </div>
        </div>
      </div>

      {/* Info */}
      <div className="absolute bottom-4 right-4 z-10 bg-bg-sidebar border border-gray-700 rounded-lg p-3">
        <p className="text-xs text-gray-400">
          <span className="font-medium text-white">{hosts.length}</span> hosts discovered
        </p>
        <p className="text-xs text-gray-500 mt-1">
          Click: Details • Double-click: Connect
        </p>
      </div>

      {/* Network Container */}
      <div ref={containerRef} className="w-full h-[600px]" />

      {/* Empty State */}
      {hosts.length === 0 && (
        <div className="absolute inset-0 flex items-center justify-center bg-bg-root">
          <div className="text-center">
            <p className="text-gray-500 mb-2">No hosts discovered yet</p>
            <p className="text-xs text-gray-600">Run a network scan to visualize your network</p>
          </div>
        </div>
      )}
    </div>
  );
};

export default NetworkTopologyView;

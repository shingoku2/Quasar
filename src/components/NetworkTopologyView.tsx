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
  // Tracks the latest physicsEnabled value for the host-rebuild effect below, without
  // making that effect depend on it (which would tear down and rebuild the network,
  // resetting the layout, on every play/pause toggle).
  const physicsEnabledRef = useRef(physicsEnabled);
  useEffect(() => {
    physicsEnabledRef.current = physicsEnabled;
  }, [physicsEnabled]);

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
        server: { bg: '#0ea5e9', border: '#0284c7' },
        router: { bg: '#8b5cf6', border: '#7c3aed' },
        printer: { bg: '#f472b6', border: '#ec4899' },
        workstation: { bg: '#2dd4bf', border: '#14b8a6' },
        unknown: { bg: '#64748b', border: '#475569' }
      };

      const colors = deviceColors[host.device_type as keyof typeof deviceColors] || deviceColors.unknown;

      nodes.add({
        id: host.ip,
        label: host.hostname || host.ip,
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
          color: '#1e3a5f',
          highlight: '#00d4ff',
          opacity: 0.6
        },
        width: Math.max(1, 5 - (host.latency_ms || 100) / 20),
        smooth: {
          type: 'continuous',
          roundness: 0.5
        }
      });
    });

    // Scale spring length so nodes spread out more on large networks
    const springLength = Math.max(120, 80 + hosts.length * 6);

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
        // Rebuilding the network (e.g. when a scan discovers a new host) must not
        // silently re-enable physics if the user had frozen the layout.
        enabled: physicsEnabledRef.current,
        stabilization: {
          enabled: true,
          iterations: 500,
          updateInterval: 50,
          fit: true,
        },
        barnesHut: {
          gravitationalConstant: -8000,
          centralGravity: 0.05,
          springLength,
          springConstant: 0.05,
          damping: 0.9,
          avoidOverlap: 1.0
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

    // Freeze layout once physics has settled — prevents perpetual bouncing
    network.on('stabilizationIterationsDone', () => {
      network.setOptions({ physics: { enabled: false } });
      setPhysicsEnabled(false);
    });

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
    <div className={`relative bg-bg-root rounded-lg overflow-hidden h-full ${className}`}>
      {/* Controls */}
      <div className="absolute top-4 left-4 z-10 flex flex-col space-y-2">
        <button
          onClick={handleZoomIn}
          className="bg-bg-card border border-border hover:border-accent text-white p-2 rounded-lg transition-colors"
          title="Zoom In"
        >
          <ZoomIn className="h-4 w-4" />
        </button>
        <button
          onClick={handleZoomOut}
          className="bg-bg-card border border-border hover:border-accent text-white p-2 rounded-lg transition-colors"
          title="Zoom Out"
        >
          <ZoomOut className="h-4 w-4" />
        </button>
        <button
          onClick={handleFit}
          className="bg-bg-card border border-border hover:border-accent text-white p-2 rounded-lg transition-colors"
          title="Fit to Screen"
        >
          <Maximize2 className="h-4 w-4" />
        </button>
        <button
          onClick={() => setPhysicsEnabled(!physicsEnabled)}
          className="bg-bg-card border border-border hover:border-accent text-white p-2 rounded-lg transition-colors"
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
            className="bg-bg-card border border-border focus:border-accent text-white pl-10 pr-4 py-2 rounded-lg text-sm outline-none transition-colors w-64"
          />
        </div>
      </div>

      {/* Legend */}
      <div className="absolute bottom-4 left-4 z-10 bg-bg-card/90 backdrop-blur-sm border border-border rounded-lg p-3">
        <p className="text-xs text-gray-400 uppercase tracking-wider mb-2">Device Types</p>
        <div className="space-y-1">
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-sky-500" />
            <span className="text-xs text-white">Server</span>
          </div>
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-violet-500" />
            <span className="text-xs text-white">Router</span>
          </div>
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-pink-400" />
            <span className="text-xs text-white">Printer</span>
          </div>
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-teal-400" />
            <span className="text-xs text-white">Workstation</span>
          </div>
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-slate-500" />
            <span className="text-xs text-white">Unknown</span>
          </div>
        </div>
      </div>

      {/* Info */}
      <div className="absolute bottom-4 right-4 z-10 bg-bg-card/90 backdrop-blur-sm border border-border rounded-lg p-3">
        <p className="text-xs text-gray-400">
          <span className="font-medium text-white">{hosts.length}</span> hosts discovered
        </p>
        <p className="text-xs text-gray-500 mt-1">
          Click: Details • Double-click: Connect
        </p>
      </div>

      {/* Network Container */}
      <div ref={containerRef} className="w-full h-full" />

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

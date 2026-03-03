import React, { useState } from 'react';
import { X, Server, Laptop, Router, Printer, HelpCircle, Copy, Check, ExternalLink, Trash2 } from 'lucide-react';
import { ScanResult } from './NetworkScanner';

interface HostDetailDialogProps {
  host: ScanResult | null;
  onClose: () => void;
  onConnect?: (host: ScanResult) => void;
  onSave?: (host: ScanResult) => void;
  onDelete?: (ip: string) => void;
}

const HostDetailDialog: React.FC<HostDetailDialogProps> = ({
  host,
  onClose,
  onConnect,
  onSave,
  onDelete,
}) => {
  const [activeTab, setActiveTab] = useState<'overview' | 'services' | 'actions'>('overview');
  const [copiedField, setCopiedField] = useState<string | null>(null);

  if (!host) return null;

  const getDeviceIcon = (deviceType: string) => {
    const iconClass = "h-8 w-8";
    switch (deviceType) {
      case 'server':
        return <Server className={iconClass} />;
      case 'router':
        return <Router className={iconClass} />;
      case 'printer':
        return <Printer className={iconClass} />;
      case 'workstation':
        return <Laptop className={iconClass} />;
      default:
        return <HelpCircle className={iconClass} />;
    }
  };

  const copyToClipboard = async (text: string, field: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedField(field);
      setTimeout(() => setCopiedField(null), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  const formatTimestamp = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const getServiceColor = (service: string) => {
    const riskServices = ['telnet', 'ftp', 'http'];
    if (riskServices.includes(service.toLowerCase())) {
      return 'text-yellow-500';
    }
    return 'text-green-500';
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm" role="dialog" aria-modal="true" aria-label={`Host details: ${host.ip}`}>
      <div className="bg-bg-sidebar border border-gray-700 rounded-xl shadow-2xl w-full max-w-3xl max-h-[90vh] overflow-hidden animate-in fade-in zoom-in duration-200">
        {/* Header */}
        <div className="px-6 py-4 border-b border-gray-800 flex justify-between items-center bg-bg-root/50">
          <div className="flex items-center space-x-4">
            <div className="text-accent">
              {getDeviceIcon(host.device_type)}
            </div>
            <div>
              <h2 className="text-lg font-bold text-white">{host.ip}</h2>
              {host.hostname && (
                <p className="text-sm text-gray-400">{host.hostname}</p>
              )}
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-white transition-colors"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-gray-800 bg-bg-root/30">
          <button
            onClick={() => setActiveTab('overview')}
            className={`px-6 py-3 text-sm font-medium transition-colors ${
              activeTab === 'overview'
                ? 'text-accent border-b-2 border-accent'
                : 'text-gray-400 hover:text-white'
            }`}
          >
            Overview
          </button>
          <button
            onClick={() => setActiveTab('services')}
            className={`px-6 py-3 text-sm font-medium transition-colors ${
              activeTab === 'services'
                ? 'text-accent border-b-2 border-accent'
                : 'text-gray-400 hover:text-white'
            }`}
          >
            Services ({host.services.length})
          </button>
          <button
            onClick={() => setActiveTab('actions')}
            className={`px-6 py-3 text-sm font-medium transition-colors ${
              activeTab === 'actions'
                ? 'text-accent border-b-2 border-accent'
                : 'text-gray-400 hover:text-white'
            }`}
          >
            Actions
          </button>
        </div>

        {/* Content */}
        <div className="p-6 overflow-y-auto max-h-[calc(90vh-200px)]">
          {activeTab === 'overview' && (
            <div className="space-y-6">
              {/* Basic Info */}
              <div className="grid grid-cols-2 gap-4">
                <div className="bg-bg-root rounded-lg p-4">
                  <div className="flex items-center justify-between mb-2">
                    <p className="text-xs text-gray-500 uppercase tracking-wider">IP Address</p>
                    <button
                      onClick={() => copyToClipboard(host.ip, 'ip')}
                      className="text-gray-500 hover:text-accent transition-colors"
                    >
                      {copiedField === 'ip' ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
                    </button>
                  </div>
                  <p className="text-white font-medium">{host.ip}</p>
                </div>

                <div className="bg-bg-root rounded-lg p-4">
                  <p className="text-xs text-gray-500 uppercase tracking-wider mb-2">Device Type</p>
                  <p className="text-white font-medium capitalize">{host.device_type}</p>
                </div>

                {host.hostname && (
                  <div className="bg-bg-root rounded-lg p-4">
                    <div className="flex items-center justify-between mb-2">
                      <p className="text-xs text-gray-500 uppercase tracking-wider">Hostname</p>
                      <button
                        onClick={() => copyToClipboard(host.hostname!, 'hostname')}
                        className="text-gray-500 hover:text-accent transition-colors"
                      >
                        {copiedField === 'hostname' ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
                      </button>
                    </div>
                    <p className="text-white font-medium">{host.hostname}</p>
                  </div>
                )}

                {host.mac_address && (
                  <div className="bg-bg-root rounded-lg p-4">
                    <div className="flex items-center justify-between mb-2">
                      <p className="text-xs text-gray-500 uppercase tracking-wider">MAC Address</p>
                      <button
                        onClick={() => copyToClipboard(host.mac_address!, 'mac')}
                        className="text-gray-500 hover:text-accent transition-colors"
                      >
                        {copiedField === 'mac' ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
                      </button>
                    </div>
                    <p className="text-white font-medium">{host.mac_address}</p>
                  </div>
                )}

                {host.vendor && (
                  <div className="bg-bg-root rounded-lg p-4">
                    <p className="text-xs text-gray-500 uppercase tracking-wider mb-2">Vendor</p>
                    <p className="text-white font-medium">{host.vendor}</p>
                  </div>
                )}

                {host.latency_ms && (
                  <div className="bg-bg-root rounded-lg p-4">
                    <p className="text-xs text-gray-500 uppercase tracking-wider mb-2">Latency</p>
                    <p className="text-white font-medium">{host.latency_ms}ms</p>
                  </div>
                )}
              </div>

              {/* Status */}
              <div className="bg-bg-root rounded-lg p-4">
                <p className="text-xs text-gray-500 uppercase tracking-wider mb-3">Status</p>
                <div className="flex items-center space-x-2">
                  <div className="w-3 h-3 rounded-full bg-green-500" />
                  <span className="text-white font-medium">Online</span>
                  {host.latency_ms && (
                    <span className="text-gray-500 text-sm">({host.latency_ms}ms latency)</span>
                  )}
                </div>
              </div>

              {/* Timestamps */}
              <div className="bg-bg-root rounded-lg p-4">
                <p className="text-xs text-gray-500 uppercase tracking-wider mb-3">Last Seen</p>
                <p className="text-white">{formatTimestamp(host.last_seen)}</p>
              </div>

              {/* Open Ports */}
              {host.open_ports.length > 0 && (
                <div className="bg-bg-root rounded-lg p-4">
                  <p className="text-xs text-gray-500 uppercase tracking-wider mb-3">
                    Open Ports ({host.open_ports.length})
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {host.open_ports.map(port => (
                      <span
                        key={port}
                        className="text-sm bg-accent/20 text-accent px-3 py-1 rounded"
                      >
                        {port}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

          {activeTab === 'services' && (
            <div className="space-y-3">
              {host.services.length === 0 ? (
                <div className="text-center py-8 text-gray-500">
                  No services detected
                </div>
              ) : (
                host.services.map((service, index) => (
                  <div key={index} className="bg-bg-root rounded-lg p-4 flex items-center justify-between">
                    <div className="flex-1">
                      <div className="flex items-center space-x-3">
                        <span className={`text-lg font-bold ${getServiceColor(service.service)}`}>
                          {service.port}
                        </span>
                        <div>
                          <p className="text-white font-medium">{service.service}</p>
                          <p className="text-xs text-gray-500">{service.protocol.toUpperCase()}</p>
                        </div>
                      </div>
                    </div>
                    {service.version && (
                      <span className="text-sm text-gray-400">{service.version}</span>
                    )}
                  </div>
                ))
              )}
            </div>
          )}

          {activeTab === 'actions' && (
            <div className="space-y-4">
              {onConnect && (
                <button
                  onClick={() => onConnect(host)}
                  className="w-full bg-accent hover:bg-accent/80 text-white px-6 py-3 rounded-lg font-medium transition-all flex items-center justify-center space-x-2"
                >
                  <ExternalLink className="h-4 w-4" />
                  <span>Connect to Host</span>
                </button>
              )}

              {onSave && (
                <button
                  onClick={() => onSave(host)}
                  className="w-full bg-blue-600 hover:bg-blue-700 text-white px-6 py-3 rounded-lg font-medium transition-all"
                >
                  Save to Hosts
                </button>
              )}

              {onDelete && (
                <button
                  onClick={() => {
                    if (confirm(`Remove ${host.ip} from discovered hosts?`)) {
                      onDelete(host.ip);
                      onClose();
                    }
                  }}
                  className="w-full bg-red-600 hover:bg-red-700 text-white px-6 py-3 rounded-lg font-medium transition-all flex items-center justify-center space-x-2"
                >
                  <Trash2 className="h-4 w-4" />
                  <span>Remove from Discovered Hosts</span>
                </button>
              )}

              <div className="bg-bg-root rounded-lg p-4 mt-6">
                <p className="text-xs text-gray-500 uppercase tracking-wider mb-3">Quick Actions</p>
                <div className="space-y-2">
                  <button
                    onClick={() => copyToClipboard(host.ip, 'quick-ip')}
                    className="w-full text-left px-4 py-2 rounded bg-bg-sidebar hover:bg-bg-sidebar/80 text-white text-sm transition-colors flex items-center justify-between"
                  >
                    <span>Copy IP Address</span>
                    {copiedField === 'quick-ip' ? <Check className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4 text-gray-500" />}
                  </button>
                  {host.hostname && (
                    <button
                      onClick={() => copyToClipboard(host.hostname!, 'quick-hostname')}
                      className="w-full text-left px-4 py-2 rounded bg-bg-sidebar hover:bg-bg-sidebar/80 text-white text-sm transition-colors flex items-center justify-between"
                    >
                      <span>Copy Hostname</span>
                      {copiedField === 'quick-hostname' ? <Check className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4 text-gray-500" />}
                    </button>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default HostDetailDialog;

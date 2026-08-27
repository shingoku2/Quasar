import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Shield, Trash2, Search, AlertTriangle, CheckCircle, XCircle, HelpCircle } from 'lucide-react';
import { getErrorMessage } from '../../lib/utils';

interface KnownHost {
  id: number;
  host: string;
  port: number;
  key_type: string;
  fingerprint: string;
  trust_status: string;
  first_seen: string;
  last_seen?: string;
}

const KnownHostsManager: React.FC = () => {
  const [hosts, setHosts] = useState<KnownHost[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState('');

  const loadHosts = async () => {
    setIsLoading(true);
    setError('');
    try {
      const knownHosts = await invoke<KnownHost[]>('get_known_ssh_hosts');
      setHosts(knownHosts);
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to load known hosts'));
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadHosts();
  }, []);

  const handleRemove = async (host: string, port: number) => {
    if (!confirm(`Remove host key for ${host}:${port}?\n\nYou will be prompted to trust this host again on the next connection.`)) {
      return;
    }

    try {
      await invoke('remove_ssh_host_key', { host, port });
      await loadHosts();
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to remove host key'));
    }
  };

  const handleUpdateTrust = async (host: string, port: number, trustStatus: string) => {
    try {
      await invoke('update_ssh_host_trust', { host, port, trustStatus });
      await loadHosts();
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to update trust status'));
    }
  };

  const filteredHosts = hosts.filter(h =>
    h.host.toLowerCase().includes(searchQuery.toLowerCase()) ||
    h.fingerprint.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const getTrustIcon = (status: string) => {
    switch (status.toLowerCase()) {
      case 'trusted':
        return <CheckCircle className="h-4 w-4 text-success" />;
      case 'rejected':
        return <XCircle className="h-4 w-4 text-alert" />;
      case 'changed':
        return <AlertTriangle className="h-4 w-4 text-warning" />;
      default:
        return <HelpCircle className="h-4 w-4 text-gray-500" />;
    }
  };

  const getTrustColor = (status: string) => {
    switch (status.toLowerCase()) {
      case 'trusted':
        return 'text-success';
      case 'rejected':
        return 'text-alert';
      case 'changed':
        return 'text-warning';
      default:
        return 'text-gray-500';
    }
  };

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-gray-800">
        <div className="flex justify-between items-center mb-4">
          <div className="flex items-center space-x-2">
            <Shield className="h-5 w-5 text-accent" />
            <h1 className="text-xl font-bold text-white">Known SSH Hosts</h1>
          </div>
          <div className="text-sm text-gray-500">
            {hosts.length} {hosts.length === 1 ? 'host' : 'hosts'}
          </div>
        </div>

        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500" />
          <input
            type="text"
            placeholder="Search by host or fingerprint..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full bg-bg-sidebar border border-gray-700 rounded-lg pl-10 pr-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all placeholder:text-gray-600"
          />
        </div>
      </div>

      {error && (
        <div className="mx-6 mt-4 bg-alert/10 border border-alert/30 rounded-lg px-4 py-3 text-alert text-sm">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-auto p-6">
        {isLoading ? (
          <div className="text-center text-gray-500 py-12">Loading known hosts...</div>
        ) : filteredHosts.length === 0 ? (
          <div className="text-center text-gray-500 py-12">
            <Shield className="h-12 w-12 mx-auto mb-4 opacity-50" />
            <p>{searchQuery ? 'No matching hosts found' : 'No known hosts yet'}</p>
            <p className="text-sm mt-2">Host keys will be saved when you connect to SSH servers</p>
          </div>
        ) : (
          <div className="space-y-3">
            {filteredHosts.map((host) => (
              <div
                key={host.id}
                className="bg-bg-sidebar border border-gray-700 rounded-lg p-4 hover:border-accent/50 transition-all"
              >
                <div className="flex justify-between items-start mb-3">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center space-x-2 mb-1">
                      <h3 className="text-white font-bold font-mono truncate">
                        {host.host}:{host.port}
                      </h3>
                      <span className="text-xs font-mono uppercase px-2 py-0.5 rounded bg-accent/10 text-accent">
                        {host.key_type}
                      </span>
                    </div>
                    <div className="flex items-center space-x-2">
                      {getTrustIcon(host.trust_status)}
                      <span className={`text-xs font-medium uppercase ${getTrustColor(host.trust_status)}`}>
                        {host.trust_status}
                      </span>
                    </div>
                  </div>
                  <button
                    onClick={() => handleRemove(host.host, host.port)}
                    className="text-gray-400 hover:text-alert transition-colors p-2"
                    title="Remove host key"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </div>

                <div className="space-y-2">
                  <div>
                    <label className="block text-xs font-medium text-gray-500 mb-1">Fingerprint</label>
                    <p className="text-gray-300 font-mono text-xs bg-bg-root border border-gray-700 rounded px-3 py-2 break-all">
                      {host.fingerprint}
                    </p>
                  </div>

                  <div className="grid grid-cols-2 gap-4 text-xs">
                    <div>
                      <label className="block text-gray-500 mb-1">First Seen</label>
                      <p className="text-gray-300">{new Date(host.first_seen).toLocaleString()}</p>
                    </div>
                    {host.last_seen && (
                      <div>
                        <label className="block text-gray-500 mb-1">Last Seen</label>
                        <p className="text-gray-300">{new Date(host.last_seen).toLocaleString()}</p>
                      </div>
                    )}
                  </div>

                  {host.trust_status.toLowerCase() !== 'trusted' && (
                    <div className="pt-2 border-t border-gray-800 flex space-x-2">
                      <button
                        onClick={() => handleUpdateTrust(host.host, host.port, 'trusted')}
                        className="flex-1 bg-success/10 border border-success/30 hover:bg-success/20 text-success py-2 rounded-lg text-xs font-medium transition-all"
                      >
                        Mark as Trusted
                      </button>
                      {host.trust_status.toLowerCase() !== 'rejected' && (
                        <button
                          onClick={() => handleUpdateTrust(host.host, host.port, 'rejected')}
                          className="flex-1 bg-alert/10 border border-alert/30 hover:bg-alert/20 text-alert py-2 rounded-lg text-xs font-medium transition-all"
                        >
                          Mark as Rejected
                        </button>
                      )}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

export default KnownHostsManager;

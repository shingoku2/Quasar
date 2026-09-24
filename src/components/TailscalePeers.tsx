import React, { useMemo } from 'react';
import { RefreshCw } from 'lucide-react';
import { useTailscaleStatus, TailscalePeer } from '../hooks/useTailscaleStatus';
import { Host } from './HostList';
import { AddHostInitialValues } from './AddHostDialog';

interface TailscalePeersProps {
  hosts: Host[];
  onAddHost?: (values: AddHostInitialValues) => void;
}

const TailscalePeers: React.FC<TailscalePeersProps> = ({ hosts, onAddHost }) => {
  const { status, loading, error, refresh } = useTailscaleStatus();

  const savedAddresses = useMemo(
    () => new Set(hosts.map((h) => h.address.trim().toLowerCase())),
    [hosts],
  );

  // O(1) lookups instead of O(N) array iteration per peer.
  // We check if the preferred_address, dns_name, ipv4, or hostname are in the savedAddresses set.
  const isSaved = (peer: TailscalePeer) =>
    savedAddresses.has(peer.preferred_address.trim().toLowerCase()) ||
    (!!peer.dns_name && savedAddresses.has(peer.dns_name.toLowerCase())) ||
    (!!peer.ipv4 && savedAddresses.has(peer.ipv4.toLowerCase())) ||
    savedAddresses.has(peer.hostname.toLowerCase());

  const handleAdd = (peer: TailscalePeer) => {
    onAddHost?.({
      name: peer.hostname,
      address: peer.preferred_address,
      protocol: 'ssh',
      port: 22,
      username: '',
    });
  };

  let body: React.ReactNode;
  if (!status && loading) {
    body = <p className="text-xs text-gray-500 italic">Checking Tailscale…</p>;
  } else if (error) {
    body = <p className="text-xs text-red-400">{error}</p>;
  } else if (status && !status.installed) {
    body = (
      <p className="text-xs text-gray-500 italic">
        Tailscale CLI not found. Install Tailscale to see tailnet peers here.
      </p>
    );
  } else if (status && status.backend_state !== 'Running') {
    body = (
      <p className="text-xs text-gray-500 italic">
        Tailscale needs attention (status: {status.backend_state || 'unknown'}). Log in or start the
        Tailscale daemon to see peers.
      </p>
    );
  } else if (status && status.peers.length === 0) {
    body = <p className="text-xs text-gray-500 italic">No other devices on your tailnet yet.</p>;
  } else if (status) {
    body = (
      <ul className="space-y-2 max-h-56 overflow-y-auto pr-1">
        {status.peers.map((peer) => (
          <li
            key={peer.id}
            className="flex justify-between items-center bg-gray-900 p-2 rounded"
          >
            <div className="flex items-center space-x-2 min-w-0">
              <span
                className={`h-2 w-2 rounded-full shrink-0 ${peer.online ? 'bg-green-500' : 'bg-gray-600'}`}
                title={peer.online ? 'Online' : 'Offline'}
              />
              <div className="min-w-0">
                <div className="text-sm font-medium text-white flex items-center gap-1.5 truncate">
                  <span className="truncate">{peer.hostname}</span>
                  {peer.tailscale_ssh && (
                    <span className="text-[10px] font-bold uppercase bg-emerald-900/60 text-emerald-300 px-1.5 py-0.5 rounded shrink-0">
                      SSH
                    </span>
                  )}
                </div>
                <div className="text-xs text-gray-400 truncate">
                  {peer.os ? `${peer.os} · ` : ''}
                  {peer.preferred_address}
                </div>
              </div>
            </div>
            {isSaved(peer) ? (
              <span className="text-xs text-gray-500 px-2 py-1 shrink-0">Saved</span>
            ) : (
              <button
                type="button"
                onClick={() => handleAdd(peer)}
                disabled={!onAddHost}
                className="text-xs bg-blue-900 text-blue-300 px-2 py-1 rounded hover:bg-blue-800 disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
              >
                Add
              </button>
            )}
          </li>
        ))}
      </ul>
    );
  }

  return (
    <div className="p-4 bg-gray-800 rounded mt-4">
      <div className="flex justify-between items-center mb-2">
        <h3 className="text-sm font-bold text-gray-400 uppercase">Tailscale</h3>
        <button
          type="button"
          onClick={() => refresh()}
          className="text-gray-500 hover:text-gray-300 transition-colors"
          aria-label="Refresh Tailscale peers"
        >
          <RefreshCw className="h-3.5 w-3.5" />
        </button>
      </div>
      {body}
    </div>
  );
};

export default TailscalePeers;

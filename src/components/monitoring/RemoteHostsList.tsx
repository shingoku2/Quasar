import React from 'react';
import { SSH_CREDENTIAL_TYPES } from '../../lib/utils';
import type { CredentialSummary, MonitoredHost, RemoteHostMetric } from './types';

const RemoteHostsList = React.memo(function RemoteHostsList({ remoteHosts, savedHosts, credentials, setHostCredential }: {
  remoteHosts: RemoteHostMetric[];
  savedHosts: MonitoredHost[];
  credentials: CredentialSummary[];
  setHostCredential: (hostId: string, credentialId: string | null) => void;
}) {
  return (
    <div className="bg-bg-card border border-border rounded-xl p-6">
      <h3 className="text-xs font-bold text-gray-400 uppercase tracking-widest mb-4">Remote hosts</h3>
      <p className="text-sm text-gray-500 mb-4">Saved hosts are pinged every 30s. Link a vault credential to a host to fetch SSH metrics (CPU, memory, disk). Vault must be unlocked.</p>
      {(remoteHosts ?? []).length === 0 ? (
        <p className="text-gray-500 text-sm">No saved hosts. Add hosts in Remote to see them here.</p>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {(remoteHosts ?? []).map((host) => {
            const currentCredId = (savedHosts ?? []).find((s) => s.id === host.id)?.credential_id ?? '';
            return (
              <div
                key={host.id}
                className={`rounded-lg border p-4 ${
                  host.reachable
                    ? 'bg-success/5 border-success/30'
                    : 'bg-alert/5 border-alert/30'
                }`}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <p className="font-medium text-white truncate">{host.name}</p>
                    <p className="text-xs text-gray-500 truncate">{host.address}:{host.port}</p>
                  </div>
                  <span
                    className={`shrink-0 text-xs font-medium px-2 py-0.5 rounded ${
                      host.reachable ? 'bg-success/20 text-success' : 'bg-alert/20 text-alert'
                    }`}
                  >
                    {host.reachable ? 'Online' : 'Offline'}
                  </span>
                </div>
                <div className="mt-2 text-xs text-gray-400">
                  {host.reachable && host.latency_ms != null
                    ? `${host.latency_ms} ms`
                    : host.error ?? '—'}
                </div>
                {host.metrics && (
                  <div className="mt-3 pt-3 border-t border-border/50 grid grid-cols-3 gap-2 text-xs">
                    {host.metrics.cpu_percent != null && (
                      <span className="text-gray-400">CPU <span className="text-white font-medium">{host.metrics.cpu_percent.toFixed(0)}%</span></span>
                    )}
                    {host.metrics.memory_used_mb != null && host.metrics.memory_total_mb != null && host.metrics.memory_total_mb > 0 && (
                      <span className="text-gray-400">Mem <span className="text-white font-medium">{((host.metrics.memory_used_mb / host.metrics.memory_total_mb) * 100).toFixed(0)}%</span></span>
                    )}
                    {host.metrics.disk_used_gb != null && host.metrics.disk_total_gb != null && host.metrics.disk_total_gb > 0 && (
                      <span className="text-gray-400">Disk <span className="text-white font-medium">{((host.metrics.disk_used_gb / host.metrics.disk_total_gb) * 100).toFixed(0)}%</span></span>
                    )}
                  </div>
                )}
                <div className="mt-3">
                  <label htmlFor="monitoring-ssh-credential" className="block text-xs text-gray-500 mb-1">SSH metrics credential</label>
                  <select id="monitoring-ssh-credential"
                    value={currentCredId}
                    onChange={(e) => setHostCredential(host.id, e.target.value || null)}
                    className="w-full bg-bg-root border border-border rounded px-2 py-1.5 text-sm text-white focus:outline-none focus:border-accent"
                  >
                    <option value="">None</option>
                    {(credentials ?? []).filter((c) => SSH_CREDENTIAL_TYPES.includes(c.credential_type)).map((c) => (
                      <option key={c.id} value={c.id}>{c.name}</option>
                    ))}
                  </select>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
});

export default RemoteHostsList;

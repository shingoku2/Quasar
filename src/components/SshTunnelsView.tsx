import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Trash2, Plus, Network } from 'lucide-react';
import { useVisiblePolling } from '../hooks/useViewVisibility';
import { getErrorMessage } from '../lib/utils';

interface CredentialSummary {
  id: string;
  name: string;
  username: string;
  credential_type: string;
}

interface TunnelInfo {
  id: string;
  ssh_host: string;
  ssh_port: number;
  local_port: number;
  remote_host: string;
  remote_port: number;
}

const SshTunnelsView: React.FC = () => {
  const [tunnels, setTunnels] = useState<TunnelInfo[]>([]);
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState({
    ssh_host: '',
    ssh_port: 22,
    ssh_user: '',
    credential_id: '',
    local_port: 1080,
    remote_host: 'localhost',
    remote_port: 8080,
  });

  const loadTunnels = async () => {
    try {
      const list = await invoke<TunnelInfo[]>('list_ssh_tunnels');
      setTunnels(Array.isArray(list) ? list : []);
    } catch (e) {
      setError(getErrorMessage(e));
    }
  };

  useVisiblePolling(loadTunnels, 10000);

  useEffect(() => {
    invoke<CredentialSummary[]>('list_credentials')
      .then((creds) => setCredentials(creds.filter((c) => c.credential_type === 'ssh' || c.credential_type === 'ssh_key')))
      .catch((err) => setError(getErrorMessage(err)));
  }, []);

  const handleStart = async () => {
    if (!form.ssh_host.trim()) {
      setError('SSH host is required.');
      return;
    }
    if (!form.credential_id && !form.ssh_user.trim()) {
      setError('Either select a credential or enter SSH username.');
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const tunnelId = `tunnel-${Date.now()}`;
      await invoke('start_ssh_tunnel', {
        tunnelId,
        sshHost: form.ssh_host,
        sshPort: form.ssh_port,
        sshUser: form.ssh_user,
        password: undefined,
        credentialId: form.credential_id || undefined,
        localPort: form.local_port,
        remoteHost: form.remote_host,
        remotePort: form.remote_port,
      });
      await loadTunnels();
      setForm((f) => ({ ...f, credential_id: '' }));
    } catch (e) {
      setError(getErrorMessage(e));
    } finally {
      setLoading(false);
    }
  };

  const handleClose = async (id: string) => {
    try {
      await invoke('close_ssh_tunnel', { tunnelId: id });
      await loadTunnels();
    } catch (e) {
      setError(getErrorMessage(e));
    }
  };

  return (
    <div className="flex flex-col h-full bg-bg-root p-6">
      <h2 className="text-xl font-semibold text-white mb-4 flex items-center gap-2">
        <Network className="h-5 w-5 text-accent" />
        SSH Tunnels (Local Port Forward)
      </h2>
      {error && (
        <div className="mb-4 p-3 rounded-lg bg-red-900/30 border border-red-700 text-red-200 text-sm" role="alert" aria-live="assertive">
          {error}
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="bg-bg-card border border-border rounded-lg p-4">
          <h3 className="text-sm font-medium text-gray-300 mb-3">New tunnel</h3>
          <div className="space-y-3">
            <div className="grid grid-cols-2 gap-2">
              <input
                type="text"
                aria-label="SSH host"
                placeholder="SSH host"
                value={form.ssh_host}
                onChange={(e) => setForm((f) => ({ ...f, ssh_host: e.target.value }))}
                className="bg-bg-sidebar border border-border rounded px-3 py-2 text-sm text-white placeholder-gray-500"
              />
              <input
                type="number"
                aria-label="SSH port (1-65535)"
                placeholder="SSH port"
                min={1}
                max={65535}
                value={form.ssh_port}
                onChange={(e) => { const v = parseInt(e.target.value, 10); setForm((f) => ({ ...f, ssh_port: Number.isFinite(v) ? Math.max(1, Math.min(65535, v)) : f.ssh_port })); }}
                className="bg-bg-sidebar border border-border rounded px-3 py-2 text-sm text-white"
              />
            </div>
            <div className="flex gap-2">
              <input
                type="text"
                aria-label="SSH username"
                placeholder="SSH user"
                value={form.ssh_user}
                onChange={(e) => setForm((f) => ({ ...f, ssh_user: e.target.value }))}
                className="flex-1 bg-bg-sidebar border border-border rounded px-3 py-2 text-sm text-white placeholder-gray-500"
              />
              <select
                aria-label="Credential (optional)"
                value={form.credential_id}
                onChange={(e) => setForm((f) => ({ ...f, credential_id: e.target.value }))}
                className="w-40 bg-bg-sidebar border border-border rounded px-3 py-2 text-sm text-white"
              >
                <option value="">No credential</option>
                {credentials.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            </div>
            <div className="text-xs text-gray-500">Forward local port → remote host:port</div>
            <div className="grid grid-cols-3 gap-2">
              <input
                type="number"
                aria-label="Local port (1-65535)"
                placeholder="Local port"
                min={1}
                max={65535}
                value={form.local_port}
                onChange={(e) => { const v = parseInt(e.target.value, 10); setForm((f) => ({ ...f, local_port: Number.isFinite(v) ? Math.max(1, Math.min(65535, v)) : f.local_port })); }}
                className="bg-bg-sidebar border border-border rounded px-3 py-2 text-sm text-white"
              />
              <input
                type="text"
                aria-label="Remote host"
                placeholder="Remote host"
                value={form.remote_host}
                onChange={(e) => setForm((f) => ({ ...f, remote_host: e.target.value }))}
                className="bg-bg-sidebar border border-border rounded px-3 py-2 text-sm text-white placeholder-gray-500"
              />
              <input
                type="number"
                aria-label="Remote port (1-65535)"
                placeholder="Remote port"
                min={1}
                max={65535}
                value={form.remote_port}
                onChange={(e) => { const v = parseInt(e.target.value, 10); setForm((f) => ({ ...f, remote_port: Number.isFinite(v) ? Math.max(1, Math.min(65535, v)) : f.remote_port })); }}
                className="bg-bg-sidebar border border-border rounded px-3 py-2 text-sm text-white"
              />
            </div>
            <button
              type="button"
              onClick={handleStart}
              disabled={loading}
              className="flex items-center gap-2 px-4 py-2 bg-accent text-white rounded-lg hover:bg-accent/90 disabled:opacity-50 text-sm font-medium"
            >
              <Plus className="h-4 w-4" />
              {loading ? 'Starting…' : 'Start tunnel'}
            </button>
          </div>
        </div>

        <div className="bg-bg-card border border-border rounded-lg p-4">
          <h3 className="text-sm font-medium text-gray-300 mb-3">Active tunnels</h3>
          {(tunnels ?? []).length === 0 ? (
            <p className="text-gray-500 text-sm">No active tunnels. Start one to forward a local port through SSH.</p>
          ) : (
            <ul className="space-y-2">
              {(tunnels ?? []).map((t) => (
                <li
                  key={t.id}
                  className="flex items-center justify-between py-2 px-3 rounded bg-bg-sidebar border border-border text-sm"
                >
                  <span className="text-gray-300 font-mono">
                    {t.ssh_host}:{t.ssh_port} → 127.0.0.1:{t.local_port} → {t.remote_host}:{t.remote_port}
                  </span>
                  <button
                    type="button"
                    onClick={() => handleClose(t.id)}
                    className="p-1.5 rounded text-gray-400 hover:text-red-400 hover:bg-red-900/20"
                    aria-label="Close tunnel"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
};

export default SshTunnelsView;

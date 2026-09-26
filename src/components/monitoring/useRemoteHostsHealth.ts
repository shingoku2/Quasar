import { useCallback, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useOnViewShown, useVisiblePolling } from '../../hooks/useViewVisibility';
import type { CredentialSummary, MonitoredHost, RemoteHostMetric } from './types';

/**
 * Saved hosts' health (polled every 30 s while the view is visible), plus the saved
 * hosts and credentials the per-host metrics-credential picker needs.
 */
export function useRemoteHostsHealth() {
  const [remoteHosts, setRemoteHosts] = useState<RemoteHostMetric[]>([]);
  const [savedHosts, setSavedHosts] = useState<MonitoredHost[]>([]);
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);

  const fetchRemoteHealth = useCallback(async () => {
    try {
      const data = await invoke<RemoteHostMetric[]>('get_remote_hosts_health');
      setRemoteHosts(Array.isArray(data) ? data : []);
    } catch {
      setRemoteHosts([]);
    }
  }, []);
  // get_remote_hosts_health pings and SSHes into every saved host, so it must not
  // run while the Monitoring view is hidden.
  useVisiblePolling(fetchRemoteHealth, 30000);
  useOnViewShown(() => {
    invoke<MonitoredHost[]>('get_saved_hosts').then((data) => setSavedHosts(Array.isArray(data) ? data : [])).catch(() => setSavedHosts([]));
    invoke<CredentialSummary[]>('list_credentials').then((data) => setCredentials(Array.isArray(data) ? data : [])).catch(() => setCredentials([]));
  });

  const setHostCredential = useCallback(async (hostId: string, credentialId: string | null) => {
    try {
      await invoke('set_host_monitoring_credential', { hostId, credentialId });
      const hosts = await invoke<MonitoredHost[]>('get_saved_hosts');
      setSavedHosts(Array.isArray(hosts) ? hosts : []);
      await fetchRemoteHealth();
    } catch (e) {
      console.error('Failed to set monitoring credential:', e);
    }
  }, [fetchRemoteHealth]);

  return { remoteHosts, savedHosts, credentials, setHostCredential };
}

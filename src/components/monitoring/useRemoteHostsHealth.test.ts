import { renderHook, act, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { useRemoteHostsHealth } from './useRemoteHostsHealth';
import { invoke } from '@tauri-apps/api/core';
import type { CredentialSummary, MonitoredHost, RemoteHostMetric } from './types';

const mockInvoke = vi.mocked(invoke);

const sampleRemoteHosts: RemoteHostMetric[] = [
  {
    id: 'h1',
    name: 'prod-box',
    address: '10.0.0.1',
    port: 22,
    reachable: true,
    latency_ms: 15,
    error: null,
  },
];

const sampleSavedHosts: MonitoredHost[] = [
  {
    id: 'h1',
    name: 'prod-box',
    address: '10.0.0.1',
    port: 22,
    protocol: 'ssh',
    credential_id: 'c1',
  },
];

const sampleCredentials: CredentialSummary[] = [
  {
    id: 'c1',
    name: 'prod-ssh',
    username: 'admin',
    credential_type: 'ssh',
  },
];

describe('useRemoteHostsHealth', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('polls health and loads saved hosts and credentials when mounted', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_remote_hosts_health') return sampleRemoteHosts;
      if (cmd === 'get_saved_hosts') return sampleSavedHosts;
      if (cmd === 'list_credentials') return sampleCredentials;
      return [];
    });

    const { result } = renderHook(() => useRemoteHostsHealth());

    await waitFor(() => {
      expect(result.current.remoteHosts).toEqual(sampleRemoteHosts);
    });
    expect(result.current.savedHosts).toEqual(sampleSavedHosts);
    expect(result.current.credentials).toEqual(sampleCredentials);
    expect(mockInvoke).toHaveBeenCalledWith('get_remote_hosts_health');
    expect(mockInvoke).toHaveBeenCalledWith('get_saved_hosts');
    expect(mockInvoke).toHaveBeenCalledWith('list_credentials');
  });

  it('handles non-array or failing responses gracefully by defaulting to empty arrays', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_remote_hosts_health') throw new Error('ping failed');
      if (cmd === 'get_saved_hosts') return null;
      if (cmd === 'list_credentials') throw new Error('vault locked');
      return undefined;
    });

    const { result } = renderHook(() => useRemoteHostsHealth());

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_remote_hosts_health');
    });
    expect(result.current.remoteHosts).toEqual([]);
    expect(result.current.savedHosts).toEqual([]);
    expect(result.current.credentials).toEqual([]);
  });

  it('setHostCredential invokes backend command with lowerCamelCase payload and refreshes state', async () => {
    let savedHostsCallCount = 0;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_remote_hosts_health') return sampleRemoteHosts;
      if (cmd === 'get_saved_hosts') {
        savedHostsCallCount++;
        return savedHostsCallCount > 1
          ? [{ ...sampleSavedHosts[0], credential_id: 'c2' }]
          : sampleSavedHosts;
      }
      if (cmd === 'list_credentials') return sampleCredentials;
      if (cmd === 'set_host_monitoring_credential') return undefined;
      return [];
    });

    const { result } = renderHook(() => useRemoteHostsHealth());

    await waitFor(() => {
      expect(result.current.savedHosts).toEqual(sampleSavedHosts);
    });

    await act(async () => {
      await result.current.setHostCredential('h1', 'c2');
    });

    expect(mockInvoke).toHaveBeenCalledWith('set_host_monitoring_credential', {
      hostId: 'h1',
      credentialId: 'c2',
    });
    // Payload keys must be lowerCamelCase, never snake_case
    const callArgs = mockInvoke.mock.calls.find(
      ([cmd]) => cmd === 'set_host_monitoring_credential',
    )?.[1];
    expect(callArgs).toBeDefined();
    for (const key of Object.keys(callArgs as object)) {
      expect(key).not.toContain('_');
    }

    expect(result.current.savedHosts[0].credential_id).toBe('c2');
  });

  it('catches and logs errors when setHostCredential fails', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'set_host_monitoring_credential') {
        throw new Error('Host not found');
      }
      return [];
    });

    const { result } = renderHook(() => useRemoteHostsHealth());

    await act(async () => {
      await result.current.setHostCredential('missing-host', null);
    });

    expect(errorSpy).toHaveBeenCalledWith(
      'Failed to set monitoring credential:',
      expect.any(Error),
    );
  });
});

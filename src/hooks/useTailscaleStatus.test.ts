import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useTailscaleStatus, findTailscalePeer, TailscaleStatus } from './useTailscaleStatus';

const { mockInvoke } = vi.hoisted(() => ({ mockInvoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));

const baseStatus: TailscaleStatus = {
  installed: true,
  backend_state: 'Running',
  magic_dns_enabled: true,
  magic_dns_suffix: 'tail1234.ts.net',
  self_node: null,
  peers: [
    {
      id: 'nVPS',
      hostname: 'vps-a8fa83ff',
      dns_name: 'vps-a8fa83ff.tail1234.ts.net',
      ipv4: '100.64.0.2',
      os: 'linux',
      online: true,
      tailscale_ssh: true,
      exit_node: false,
      preferred_address: 'vps-a8fa83ff.tail1234.ts.net',
    },
  ],
};

describe('useTailscaleStatus', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('fetches status on mount', async () => {
    mockInvoke.mockResolvedValue(baseStatus);

    const { result } = renderHook(() => useTailscaleStatus());

    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.status).toEqual(baseStatus);
    expect(result.current.error).toBeNull();
    expect(mockInvoke).toHaveBeenCalledWith('get_tailscale_status');
  });

  it('surfaces an error and keeps status null on failure', async () => {
    mockInvoke.mockRejectedValue(new Error('tailscale status timed out'));

    const { result } = renderHook(() => useTailscaleStatus());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.status).toBeNull();
    expect(result.current.error).toMatch(/timed out/);
  });

  it('refresh() re-fetches and updates status', async () => {
    mockInvoke.mockResolvedValueOnce(baseStatus);
    const { result } = renderHook(() => useTailscaleStatus());
    await waitFor(() => expect(result.current.loading).toBe(false));

    const updated: TailscaleStatus = { ...baseStatus, backend_state: 'NeedsLogin', peers: [] };
    mockInvoke.mockResolvedValueOnce(updated);

    await act(async () => {
      await result.current.refresh();
    });

    expect(result.current.status).toEqual(updated);
    expect(mockInvoke).toHaveBeenCalledTimes(2);
  });
});

describe('findTailscalePeer', () => {
  it('matches by MagicDNS name, IPv4, or hostname, case-insensitively', () => {
    expect(findTailscalePeer(baseStatus, 'VPS-A8FA83FF.TAIL1234.TS.NET')?.id).toBe('nVPS');
    expect(findTailscalePeer(baseStatus, '100.64.0.2')?.id).toBe('nVPS');
    expect(findTailscalePeer(baseStatus, 'vps-a8fa83ff')?.id).toBe('nVPS');
  });

  it('returns undefined when there is no match, no status, or no address', () => {
    expect(findTailscalePeer(baseStatus, '10.0.0.99')).toBeUndefined();
    expect(findTailscalePeer(null, '100.64.0.2')).toBeUndefined();
    expect(findTailscalePeer(baseStatus, undefined)).toBeUndefined();
    expect(findTailscalePeer(baseStatus, '')).toBeUndefined();
  });
});
